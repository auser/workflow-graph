//! Workflow operations that can run without a scheduler.
//!
//! These functions enqueue jobs into the queue directly.
//! The scheduler (running in the same process or externally) handles
//! the DAG cascade when jobs complete.

use std::collections::HashMap;

use workflow_graph_queue::error::SchedulerError;
use workflow_graph_queue::scheduler::SharedState;
use workflow_graph_queue::traits::*;
use workflow_graph_shared::JobStatus;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Start a workflow: reset all jobs to Queued, then enqueue root jobs.
///
/// This is a stateless operation — it writes to the queue and returns.
/// The DagScheduler (wherever it runs) handles the cascade.
pub async fn start_workflow(
    state: &SharedState,
    queue: &(impl JobQueue + ?Sized),
    workflow_id: &str,
) -> Result<(), SchedulerError> {
    let root_jobs = {
        let mut s = state.write().await;
        let wf = s
            .workflows
            .get_mut(workflow_id)
            .ok_or_else(|| SchedulerError::WorkflowNotFound(workflow_id.to_string()))?;

        // Don't restart if workflow is already running
        let has_active = wf
            .jobs
            .iter()
            .any(|j| j.status == JobStatus::Running || j.status == JobStatus::Queued);
        if has_active {
            return Ok(());
        }

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
            .map(|j| {
                (
                    j.id.clone(),
                    j.command.clone(),
                    j.required_labels.clone(),
                    j.max_retries,
                )
            })
            .filter(|(id, _, _, _)| {
                wf.jobs
                    .iter()
                    .find(|j| j.id == *id)
                    .is_some_and(|j| j.depends_on.is_empty())
            })
            .collect::<Vec<_>>()
    };

    for (job_id, command, labels, max_retries) in root_jobs {
        let queued = QueuedJob {
            job_id,
            workflow_id: workflow_id.to_string(),
            command,
            required_labels: labels,
            retry_policy: RetryPolicy {
                max_retries,
                backoff: BackoffStrategy::None,
            },
            attempt: 0,
            upstream_outputs: HashMap::new(),
            enqueued_at_ms: now_ms(),
            delayed_until_ms: 0,
        };
        queue.enqueue(queued).await?;
    }

    Ok(())
}

/// Cancel a workflow: cancel all pending/active jobs in the queue and update state.
pub async fn cancel_workflow(
    state: &SharedState,
    queue: &(impl JobQueue + ?Sized),
    workflow_id: &str,
) -> Result<(), SchedulerError> {
    queue.cancel_workflow(workflow_id).await?;

    let mut s = state.write().await;
    if let Some(wf) = s.workflows.get_mut(workflow_id) {
        for job in &mut wf.jobs {
            if job.status == JobStatus::Queued || job.status == JobStatus::Running {
                job.status = JobStatus::Cancelled;
            }
        }
    }
    Ok(())
}
