use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use github_graph_shared::{JobStatus, Workflow};

use crate::executor;

pub type SharedState = Arc<RwLock<OrchestratorState>>;

pub struct OrchestratorState {
    pub workflows: HashMap<String, Workflow>,
}

impl OrchestratorState {
    pub fn new() -> Self {
        Self {
            workflows: HashMap::new(),
        }
    }
}

/// Start executing a workflow run. Spawns tasks for root jobs and cascades.
pub fn run_workflow(state: SharedState, workflow_id: String) {
    tokio::spawn(async move {
        // Mark all jobs as queued
        {
            let mut s = state.write().await;
            if let Some(wf) = s.workflows.get_mut(&workflow_id) {
                for job in &mut wf.jobs {
                    job.status = JobStatus::Queued;
                    job.duration_secs = None;
                    job.output = None;
                }
            }
        }

        // Find root jobs (no dependencies) and start them
        let root_ids: Vec<String> = {
            let s = state.read().await;
            match s.workflows.get(&workflow_id) {
                Some(wf) => wf
                    .jobs
                    .iter()
                    .filter(|j| j.depends_on.is_empty())
                    .map(|j| j.id.clone())
                    .collect(),
                None => return,
            }
        };

        // Execute root jobs concurrently
        let mut handles = Vec::new();
        for job_id in root_ids {
            let state = state.clone();
            let wf_id = workflow_id.clone();
            handles.push(tokio::spawn(async move {
                execute_and_cascade(state, wf_id, job_id).await;
            }));
        }

        for handle in handles {
            handle.await.ok();
        }
    });
}

/// Execute a single job, then cascade to downstream jobs whose deps are all satisfied.
fn execute_and_cascade(
    state: SharedState,
    workflow_id: String,
    job_id: String,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
    Box::pin(async move {
        execute_and_cascade_inner(state, workflow_id, job_id).await;
    })
}

async fn execute_and_cascade_inner(state: SharedState, workflow_id: String, job_id: String) {
    // Get the command for this job
    let command = {
        let mut s = state.write().await;
        let wf = match s.workflows.get_mut(&workflow_id) {
            Some(wf) => wf,
            None => return,
        };
        let job = match wf.jobs.iter_mut().find(|j| j.id == job_id) {
            Some(j) => j,
            None => return,
        };
        job.status = JobStatus::Running;
        job.started_at = Some(now_millis());
        job.command.clone()
    };

    // Execute the command
    let result = executor::execute_job(&command).await;

    // Update job status
    let (success, downstream_ids) = {
        let mut s = state.write().await;
        let wf = match s.workflows.get_mut(&workflow_id) {
            Some(wf) => wf,
            None => return,
        };
        let job = match wf.jobs.iter_mut().find(|j| j.id == job_id) {
            Some(j) => j,
            None => return,
        };
        job.status = if result.success {
            JobStatus::Success
        } else {
            JobStatus::Failure
        };
        job.duration_secs = Some(result.duration_secs);
        job.output = Some(result.output);

        let success = result.success;

        if !success {
            // Mark all transitive downstream jobs as skipped
            let skip_ids = find_transitive_downstream(wf, &job_id);
            for skip_id in &skip_ids {
                if let Some(j) = wf.jobs.iter_mut().find(|j| j.id == *skip_id) {
                    j.status = JobStatus::Skipped;
                }
            }
            (false, vec![])
        } else {
            // Find downstream jobs whose deps are now all satisfied
            let ready: Vec<String> = wf
                .jobs
                .iter()
                .filter(|j| {
                    j.depends_on.contains(&job_id)
                        && j.status == JobStatus::Queued
                        && j.depends_on.iter().all(|dep| {
                            wf.jobs
                                .iter()
                                .find(|dj| dj.id == *dep)
                                .is_some_and(|dj| dj.status == JobStatus::Success)
                        })
                })
                .map(|j| j.id.clone())
                .collect();
            (true, ready)
        }
    };

    if success {
        // Cascade: execute ready downstream jobs concurrently
        let mut handles = Vec::new();
        for next_id in downstream_ids {
            let state = state.clone();
            let wf_id = workflow_id.clone();
            handles.push(tokio::spawn(
                execute_and_cascade(state, wf_id, next_id),
            ));
        }
        for handle in handles {
            handle.await.ok();
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

fn now_millis() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64
}
