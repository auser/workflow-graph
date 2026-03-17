//! Integration tests for the full queue + scheduler + worker flow.
//!
//! These tests exercise the complete lifecycle of a workflow:
//! starting → scheduling → claiming → completing → cascading downstream.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use workflow_graph_queue::memory::*;
use workflow_graph_queue::*;
use workflow_graph_shared::{JobStatus, Workflow};

fn sample_workflow() -> Workflow {
    Workflow::sample()
}

/// Create a full set of in-memory backends.
fn create_backends() -> (
    Arc<InMemoryJobQueue>,
    Arc<InMemoryArtifactStore>,
    Arc<InMemoryLogSink>,
    Arc<InMemoryWorkerRegistry>,
    SharedState,
) {
    (
        Arc::new(InMemoryJobQueue::new()),
        Arc::new(InMemoryArtifactStore::new()),
        Arc::new(InMemoryLogSink::new()),
        Arc::new(InMemoryWorkerRegistry::new()),
        Arc::new(RwLock::new(WorkflowState::new())),
    )
}

/// Full lifecycle: start workflow → scheduler enqueues roots → worker claims and completes →
/// scheduler cascades downstream → all jobs complete.
#[tokio::test]
async fn test_full_workflow_lifecycle() {
    let (queue, artifacts, _logs, _workers, state) = create_backends();

    let workflow = sample_workflow();
    let wf_id = workflow.id.clone();

    // Store workflow in shared state
    state
        .write()
        .await
        .workflows
        .insert(wf_id.clone(), workflow.clone());

    let scheduler = Arc::new(DagScheduler::new(
        queue.clone(),
        artifacts.clone(),
        state.clone(),
    ));

    // Start the scheduler event loop
    let scheduler_handle = tokio::spawn(scheduler.clone().run());

    // Start workflow — should enqueue root jobs (unit-tests, lint, typecheck)
    scheduler
        .start_workflow(&wf_id)
        .await
        .expect("start_workflow should succeed");

    // Worker loop: claim and complete jobs until none remain
    let mut completed_count = 0u32;
    let max_iterations = 80;

    for _ in 0..max_iterations {
        // Give the scheduler time to process events and enqueue downstream jobs
        tokio::time::sleep(Duration::from_millis(50)).await;

        let claimed = queue
            .claim("test-worker", &[], Duration::from_secs(30))
            .await
            .expect("claim should not error");

        if let Some((_job, lease)) = claimed {
            queue
                .complete(&lease.lease_id, std::collections::HashMap::new())
                .await
                .expect("complete should succeed");
            completed_count += 1;

            // Give the scheduler time to process the completion event
            tokio::time::sleep(Duration::from_millis(100)).await;

            if completed_count == 8 {
                break;
            }
        }
    }

    // Wait for scheduler to finish processing all events
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(completed_count, 8, "Should have completed all 8 jobs");

    // Verify all 8 jobs completed successfully
    let final_state = state.read().await;
    let final_workflow = final_state.workflows.get(&wf_id).unwrap();
    for job in &final_workflow.jobs {
        assert_eq!(
            job.status,
            JobStatus::Success,
            "Job {} should be Success, was {:?}",
            job.id,
            job.status
        );
    }

    scheduler_handle.abort();
}

/// Failure cascading: when a root job fails, its downstream dependents should be skipped.
#[tokio::test]
async fn test_failure_cascading_integration() {
    let (queue, artifacts, _logs, _workers, state) = create_backends();

    let workflow = sample_workflow();
    let wf_id = workflow.id.clone();

    state
        .write()
        .await
        .workflows
        .insert(wf_id.clone(), workflow.clone());

    let scheduler = Arc::new(DagScheduler::new(
        queue.clone(),
        artifacts.clone(),
        state.clone(),
    ));
    let scheduler_handle = tokio::spawn(scheduler.clone().run());

    scheduler
        .start_workflow(&wf_id)
        .await
        .expect("start_workflow should succeed");

    // Claim all 3 root jobs, fail "unit-tests", complete the others
    let mut claimed_ids = Vec::new();
    for _ in 0..3 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let claimed = queue
            .claim("test-worker", &[], Duration::from_secs(30))
            .await
            .expect("claim should not error");

        if let Some((job, lease)) = claimed {
            if job.job_id == "unit-tests" {
                queue
                    .fail(&lease.lease_id, "test failure".into(), false)
                    .await
                    .expect("fail should succeed");
            } else {
                queue
                    .complete(&lease.lease_id, std::collections::HashMap::new())
                    .await
                    .expect("complete should succeed");
            }
            claimed_ids.push(job.job_id.clone());
        }
    }

    // Wait for scheduler to cascade
    tokio::time::sleep(Duration::from_millis(200)).await;

    // "build" depends on unit-tests, so it and all downstream should be Skipped
    let final_state = state.read().await;
    let wf = final_state.workflows.get(&wf_id).unwrap();

    let status_of = |id: &str| wf.jobs.iter().find(|j| j.id == id).unwrap().status.clone();

    assert_eq!(status_of("unit-tests"), JobStatus::Failure);
    assert_eq!(status_of("build"), JobStatus::Skipped);
    assert_eq!(status_of("deploy-db"), JobStatus::Skipped);
    assert_eq!(status_of("deploy-web"), JobStatus::Skipped);

    scheduler_handle.abort();
}

/// Cancellation: cancelling a running workflow should mark remaining jobs as cancelled.
#[tokio::test]
async fn test_cancel_workflow_integration() {
    let (queue, artifacts, _logs, _workers, state) = create_backends();

    let workflow = sample_workflow();
    let wf_id = workflow.id.clone();

    state
        .write()
        .await
        .workflows
        .insert(wf_id.clone(), workflow.clone());

    let scheduler = Arc::new(DagScheduler::new(
        queue.clone(),
        artifacts.clone(),
        state.clone(),
    ));
    let scheduler_handle = tokio::spawn(scheduler.clone().run());

    scheduler
        .start_workflow(&wf_id)
        .await
        .expect("start_workflow should succeed");

    // Claim one job but don't complete it
    tokio::time::sleep(Duration::from_millis(50)).await;
    let claimed = queue
        .claim("test-worker", &[], Duration::from_secs(30))
        .await
        .expect("claim should not error");
    assert!(claimed.is_some(), "Should be able to claim a root job");

    // Cancel the entire workflow
    scheduler
        .cancel_workflow(&wf_id)
        .await
        .expect("cancel_workflow should succeed");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // All non-completed jobs should be Cancelled
    let final_state = state.read().await;
    let wf = final_state.workflows.get(&wf_id).unwrap();
    let cancelled_count = wf
        .jobs
        .iter()
        .filter(|j| j.status == JobStatus::Cancelled)
        .count();
    // At least the downstream jobs should be cancelled
    assert!(
        cancelled_count >= 5,
        "Most jobs should be cancelled, got {cancelled_count}"
    );

    scheduler_handle.abort();
}

/// Concurrent workers: multiple workers claiming jobs concurrently should not cause double-claims.
#[tokio::test]
async fn test_concurrent_workers_no_double_claim() {
    let (queue, artifacts, _logs, _workers, state) = create_backends();

    let workflow = sample_workflow();
    let wf_id = workflow.id.clone();

    state
        .write()
        .await
        .workflows
        .insert(wf_id.clone(), workflow.clone());

    let scheduler = Arc::new(DagScheduler::new(
        queue.clone(),
        artifacts.clone(),
        state.clone(),
    ));
    let scheduler_handle = tokio::spawn(scheduler.clone().run());

    scheduler
        .start_workflow(&wf_id)
        .await
        .expect("start_workflow should succeed");

    tokio::time::sleep(Duration::from_millis(50)).await;

    // 5 workers try to claim simultaneously — only 3 root jobs exist
    let mut handles = Vec::new();
    for i in 0..5 {
        let queue = queue.clone();
        handles.push(tokio::spawn(async move {
            queue
                .claim(&format!("worker-{i}"), &[], Duration::from_secs(30))
                .await
                .expect("claim should not error")
        }));
    }

    let mut claimed_count = 0u32;
    let mut claimed_ids: Vec<String> = Vec::new();
    for handle in handles {
        let result: Option<(
            workflow_graph_queue::traits::QueuedJob,
            workflow_graph_queue::traits::Lease,
        )> = handle.await.unwrap();
        if let Some((job, _lease)) = result {
            claimed_count += 1;
            claimed_ids.push(job.job_id.clone());
        }
    }

    assert_eq!(
        claimed_count, 3,
        "Exactly 3 root jobs should be claimed by 5 workers, got {claimed_count}"
    );

    // Verify no duplicate job IDs
    let unique: std::collections::HashSet<_> = claimed_ids.iter().collect();
    assert_eq!(unique.len(), claimed_ids.len(), "No duplicate claims");

    scheduler_handle.abort();
}
