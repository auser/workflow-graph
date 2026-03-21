use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use workflow_graph_shared::{JobStatus, Workflow};

use crate::error::SchedulerError;
use crate::traits::*;

/// Shared workflow state, readable by the frontend polling API.
pub type SharedState = Arc<RwLock<WorkflowState>>;

/// In-memory workflow state that the frontend reads via polling.
pub struct WorkflowState {
    pub workflows: HashMap<String, Workflow>,
}

impl WorkflowState {
    pub fn new() -> Self {
        Self {
            workflows: HashMap::new(),
        }
    }
}

impl Default for WorkflowState {
    fn default() -> Self {
        Self::new()
    }
}

/// Event-driven DAG scheduler.
///
/// Listens for `JobEvent`s from the queue and enqueues downstream jobs
/// when their dependencies are satisfied. Replaces the inline orchestrator.
pub struct DagScheduler<Q: JobQueue, A: ArtifactStore> {
    queue: Arc<Q>,
    artifacts: Arc<A>,
    state: SharedState,
}

impl<Q: JobQueue, A: ArtifactStore> DagScheduler<Q, A> {
    pub fn new(queue: Arc<Q>, artifacts: Arc<A>, state: SharedState) -> Self {
        Self {
            queue,
            artifacts,
            state,
        }
    }

    /// Initiate a workflow run: reset all jobs to Queued, then enqueue root jobs.
    pub async fn start_workflow(&self, workflow_id: &str) -> Result<(), SchedulerError> {
        let root_jobs = {
            let mut state = self.state.write().await;
            let wf = state
                .workflows
                .get_mut(workflow_id)
                .ok_or_else(|| SchedulerError::WorkflowNotFound(workflow_id.to_string()))?;

            // Reset all jobs
            for job in &mut wf.jobs {
                job.status = JobStatus::Queued;
                job.duration_secs = None;
                job.started_at = None;
                job.output = None;
            }

            // Find root jobs (no dependencies)
            wf.jobs
                .iter()
                .filter(|j| j.depends_on.is_empty())
                .map(|j| (j.id.clone(), j.command.clone()))
                .collect::<Vec<_>>()
        };

        // Enqueue root jobs
        for (job_id, command) in root_jobs {
            let queued = QueuedJob {
                job_id,
                workflow_id: workflow_id.to_string(),
                command,
                required_labels: vec![],
                retry_policy: RetryPolicy::default(),
                attempt: 0,
                upstream_outputs: HashMap::new(),
                enqueued_at_ms: now_ms(),
                delayed_until_ms: 0,
            };
            self.queue.enqueue(queued).await?;
        }

        Ok(())
    }

    /// Cancel a running workflow: cancel all pending/active jobs.
    pub async fn cancel_workflow(&self, workflow_id: &str) -> Result<(), SchedulerError> {
        self.queue.cancel_workflow(workflow_id).await?;

        let mut state = self.state.write().await;
        if let Some(wf) = state.workflows.get_mut(workflow_id) {
            for job in &mut wf.jobs {
                if job.status == JobStatus::Queued || job.status == JobStatus::Running {
                    job.status = JobStatus::Cancelled;
                }
            }
        }
        Ok(())
    }

    /// Run the scheduler event loop. Listens for queue events and drives the DAG.
    /// This should be spawned as a background task.
    pub async fn run(self: Arc<Self>) {
        let mut rx = self.queue.subscribe();

        loop {
            let event = match rx.recv().await {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("Scheduler lagged by {n} events, some jobs may need manual recovery");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    eprintln!("Queue event channel closed, scheduler shutting down");
                    break;
                }
            };

            if let Err(e) = self.handle_event(event).await {
                eprintln!("Scheduler error: {e}");
            }
        }
    }

    async fn handle_event(&self, event: JobEvent) -> Result<(), SchedulerError> {
        match event {
            JobEvent::Started {
                workflow_id,
                job_id,
                ..
            } => {
                self.on_job_started(&workflow_id, &job_id).await;
            }
            JobEvent::Completed {
                workflow_id,
                job_id,
                outputs,
            } => {
                self.on_job_completed(&workflow_id, &job_id, outputs)
                    .await?;
            }
            JobEvent::Failed {
                workflow_id,
                job_id,
                error,
                retryable,
            } => {
                self.on_job_failed(&workflow_id, &job_id, &error, retryable)
                    .await;
            }
            JobEvent::LeaseExpired {
                workflow_id,
                job_id,
                ..
            } => {
                self.on_lease_expired(&workflow_id, &job_id).await;
            }
            JobEvent::Cancelled {
                workflow_id,
                job_id,
            } => {
                self.on_job_cancelled(&workflow_id, &job_id).await;
            }
            JobEvent::Ready { .. } => {
                // No action needed — job is in queue waiting for a worker
            }
        }
        Ok(())
    }

    async fn on_job_started(&self, workflow_id: &str, job_id: &str) {
        let mut state = self.state.write().await;
        if let Some(wf) = state.workflows.get_mut(workflow_id)
            && let Some(job) = wf.jobs.iter_mut().find(|j| j.id == job_id)
        {
            job.status = JobStatus::Running;
            job.started_at = Some(now_ms() as f64);
        }
    }

    async fn on_job_completed(
        &self,
        workflow_id: &str,
        job_id: &str,
        outputs: HashMap<String, String>,
    ) -> Result<(), SchedulerError> {
        // Store outputs
        self.artifacts
            .put_outputs(workflow_id, job_id, outputs)
            .await?;

        // Update state
        let ready_jobs = {
            let mut state = self.state.write().await;
            let wf = match state.workflows.get_mut(workflow_id) {
                Some(wf) => wf,
                None => return Ok(()),
            };

            // Mark this job as success
            if let Some(job) = wf.jobs.iter_mut().find(|j| j.id == job_id) {
                job.status = JobStatus::Success;
                if let Some(started) = job.started_at {
                    job.duration_secs =
                        Some(((now_ms() as f64 - started) / 1000.0).max(0.0) as u64);
                }
            }

            // Find downstream jobs whose deps are ALL succeeded
            let ready: Vec<(String, String, Vec<String>)> = wf
                .jobs
                .iter()
                .filter(|j| {
                    j.status == JobStatus::Queued
                        && j.depends_on.contains(&job_id.to_string())
                        && j.depends_on.iter().all(|dep| {
                            wf.jobs
                                .iter()
                                .find(|dj| dj.id == *dep)
                                .is_some_and(|dj| dj.status == JobStatus::Success)
                        })
                })
                .map(|j| (j.id.clone(), j.command.clone(), j.depends_on.clone()))
                .collect();

            ready
        };

        // Enqueue ready downstream jobs with upstream outputs
        for (next_id, command, deps) in ready_jobs {
            let upstream_outputs = self
                .artifacts
                .get_upstream_outputs(workflow_id, &deps)
                .await?;

            let queued = QueuedJob {
                job_id: next_id,
                workflow_id: workflow_id.to_string(),
                command,
                required_labels: vec![],
                retry_policy: RetryPolicy::default(),
                attempt: 0,
                upstream_outputs,
                enqueued_at_ms: now_ms(),
                delayed_until_ms: 0,
            };
            self.queue.enqueue(queued).await?;
        }

        Ok(())
    }

    async fn on_job_failed(&self, workflow_id: &str, job_id: &str, error: &str, retryable: bool) {
        let mut state = self.state.write().await;
        let Some(wf) = state.workflows.get_mut(workflow_id) else {
            return;
        };

        if retryable {
            // Job was re-enqueued by the queue — mark as queued again
            if let Some(job) = wf.jobs.iter_mut().find(|j| j.id == job_id) {
                job.status = JobStatus::Queued;
                job.started_at = None;
            }
        } else {
            // Permanent failure — mark job and skip all downstream
            if let Some(job) = wf.jobs.iter_mut().find(|j| j.id == job_id) {
                job.status = JobStatus::Failure;
                job.output = Some(error.to_string());
                if let Some(started) = job.started_at {
                    job.duration_secs =
                        Some(((now_ms() as f64 - started) / 1000.0).max(0.0) as u64);
                }
            }

            // Skip transitive downstream
            let skip_ids = find_transitive_downstream(wf, job_id);
            for skip_id in &skip_ids {
                if let Some(j) = wf.jobs.iter_mut().find(|j| j.id == *skip_id) {
                    j.status = JobStatus::Skipped;
                }
            }
        }
    }

    async fn on_lease_expired(&self, workflow_id: &str, job_id: &str) {
        // The queue already handled re-enqueueing if retries remain.
        // We just need to update the state if the job was permanently failed.
        let mut state = self.state.write().await;
        if let Some(wf) = state.workflows.get_mut(workflow_id)
            && let Some(job) = wf.jobs.iter_mut().find(|j| j.id == job_id)
        {
            // If the queue re-enqueued it, mark as queued; otherwise mark as failure
            // We can't easily know here, so mark as Queued — the next Started event
            // will update it correctly.
            job.status = JobStatus::Queued;
            job.started_at = None;
        }
    }

    async fn on_job_cancelled(&self, workflow_id: &str, job_id: &str) {
        let mut state = self.state.write().await;
        if let Some(wf) = state.workflows.get_mut(workflow_id)
            && let Some(job) = wf.jobs.iter_mut().find(|j| j.id == job_id)
        {
            job.status = JobStatus::Cancelled;
        }
    }
}

/// Find all jobs transitively downstream of the given job.
fn find_transitive_downstream(wf: &Workflow, job_id: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut stack = vec![job_id.to_string()];

    while let Some(current) = stack.pop() {
        for job in &wf.jobs {
            if job.depends_on.contains(&current) && !result.contains(&job.id) {
                result.push(job.id.clone());
                stack.push(job.id.clone());
            }
        }
    }

    result
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{InMemoryArtifactStore, InMemoryJobQueue};

    fn sample_workflow() -> Workflow {
        Workflow {
            id: "wf1".into(),
            name: "test".into(),
            trigger: "manual".into(),
            jobs: vec![
                workflow_graph_shared::Job {
                    id: "a".into(),
                    name: "Job A".into(),
                    status: JobStatus::Queued,
                    command: "echo a".into(),
                    duration_secs: None,
                    started_at: None,
                    required_labels: vec![],
                    max_retries: 0,
                    attempt: 0,
                    depends_on: vec![],
                    output: None,
                    metadata: std::collections::HashMap::new(),
                    ports: vec![],
                },
                workflow_graph_shared::Job {
                    id: "b".into(),
                    name: "Job B".into(),
                    status: JobStatus::Queued,
                    command: "echo b".into(),
                    duration_secs: None,
                    started_at: None,
                    required_labels: vec![],
                    max_retries: 0,
                    attempt: 0,
                    depends_on: vec!["a".into()],
                    output: None,
                    metadata: std::collections::HashMap::new(),
                    ports: vec![],
                },
                workflow_graph_shared::Job {
                    id: "c".into(),
                    name: "Job C".into(),
                    status: JobStatus::Queued,
                    command: "echo c".into(),
                    duration_secs: None,
                    started_at: None,
                    required_labels: vec![],
                    max_retries: 0,
                    attempt: 0,
                    depends_on: vec!["a".into()],
                    output: None,
                    metadata: std::collections::HashMap::new(),
                    ports: vec![],
                },
            ],
        }
    }

    async fn setup() -> (
        Arc<DagScheduler<InMemoryJobQueue, InMemoryArtifactStore>>,
        Arc<InMemoryJobQueue>,
        SharedState,
    ) {
        let queue = Arc::new(InMemoryJobQueue::new());
        let artifacts = Arc::new(InMemoryArtifactStore::new());
        let state = Arc::new(RwLock::new(WorkflowState::new()));

        state
            .write()
            .await
            .workflows
            .insert("wf1".into(), sample_workflow());

        let scheduler = Arc::new(DagScheduler::new(
            queue.clone(),
            artifacts.clone(),
            state.clone(),
        ));

        (scheduler, queue, state)
    }

    #[tokio::test]
    async fn test_start_workflow_enqueues_roots() {
        let (scheduler, queue, _state) = setup().await;

        scheduler.start_workflow("wf1").await.unwrap();

        // Only job "a" (root) should be enqueued
        let (job, _lease) = queue
            .claim("w1", &[], std::time::Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(job.job_id, "a");

        // No more jobs available
        assert!(
            queue
                .claim("w1", &[], std::time::Duration::from_secs(30))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_completed_enqueues_downstream() {
        let (scheduler, queue, state) = setup().await;

        scheduler.start_workflow("wf1").await.unwrap();

        // Claim and complete job A
        let (_, lease) = queue
            .claim("w1", &[], std::time::Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();

        // Process the Started event
        scheduler
            .handle_event(JobEvent::Started {
                workflow_id: "wf1".into(),
                job_id: "a".into(),
                worker_id: "w1".into(),
            })
            .await
            .unwrap();

        // Complete job A
        queue
            .complete(&lease.lease_id, HashMap::new())
            .await
            .unwrap();

        // Process the Completed event — should enqueue B and C
        scheduler
            .handle_event(JobEvent::Completed {
                workflow_id: "wf1".into(),
                job_id: "a".into(),
                outputs: HashMap::new(),
            })
            .await
            .unwrap();

        // Both B and C should now be claimable
        let (job1, _) = queue
            .claim("w1", &[], std::time::Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        let (job2, _) = queue
            .claim("w1", &[], std::time::Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();

        let mut ids = vec![job1.job_id, job2.job_id];
        ids.sort();
        assert_eq!(ids, vec!["b", "c"]);

        // Check state
        let s = state.read().await;
        let wf = &s.workflows["wf1"];
        assert_eq!(
            wf.jobs.iter().find(|j| j.id == "a").unwrap().status,
            JobStatus::Success
        );
    }

    #[tokio::test]
    async fn test_failure_skips_downstream() {
        let (scheduler, _queue, state) = setup().await;

        scheduler.start_workflow("wf1").await.unwrap();

        // Simulate job A failing permanently
        scheduler
            .handle_event(JobEvent::Failed {
                workflow_id: "wf1".into(),
                job_id: "a".into(),
                error: "boom".into(),
                retryable: false,
            })
            .await
            .unwrap();

        let s = state.read().await;
        let wf = &s.workflows["wf1"];
        assert_eq!(
            wf.jobs.iter().find(|j| j.id == "a").unwrap().status,
            JobStatus::Failure
        );
        assert_eq!(
            wf.jobs.iter().find(|j| j.id == "b").unwrap().status,
            JobStatus::Skipped
        );
        assert_eq!(
            wf.jobs.iter().find(|j| j.id == "c").unwrap().status,
            JobStatus::Skipped
        );
    }

    #[tokio::test]
    async fn test_cancel_workflow() {
        let (scheduler, _queue, state) = setup().await;

        scheduler.start_workflow("wf1").await.unwrap();
        scheduler.cancel_workflow("wf1").await.unwrap();

        let s = state.read().await;
        let wf = &s.workflows["wf1"];
        for job in &wf.jobs {
            assert!(
                job.status == JobStatus::Cancelled,
                "job {} should be cancelled, got {:?}",
                job.id,
                job.status
            );
        }
    }
}
