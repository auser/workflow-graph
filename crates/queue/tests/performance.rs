//! Performance benchmarks for scheduler operations.
//!
//! These tests measure throughput at various scales and serve as
//! regression guards. They are not micro-benchmarks — they test
//! end-to-end scheduler performance including queue operations.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use workflow_graph_queue::memory::*;
use workflow_graph_queue::*;
use workflow_graph_shared::{Job, JobStatus, Workflow};

/// Generate a synthetic workflow with N jobs in a diamond pattern.
/// Layer 0: 1 root, Layer 1: N/3 jobs, Layer 2: N/3 jobs, Layer 3: 1 sink.
fn generate_diamond_workflow(total_jobs: usize) -> Workflow {
    let mut jobs = Vec::with_capacity(total_jobs);

    // Root job
    jobs.push(Job {
        id: "root".into(),
        name: "Root".into(),
        status: JobStatus::Queued,
        command: "echo root".into(),
        duration_secs: None,
        started_at: None,
        depends_on: vec![],
        output: None,
        required_labels: vec![],
        max_retries: 0,
        attempt: 0,
    });

    if total_jobs <= 2 {
        return Workflow {
            id: "perf-test".into(),
            name: "Perf Test".into(),
            trigger: "manual".into(),
            jobs,
        };
    }

    // Middle layers
    let middle = total_jobs.saturating_sub(2);
    let half = middle / 2;

    // Layer 1: depends on root
    for i in 0..half {
        jobs.push(Job {
            id: format!("mid1-{i}"),
            name: format!("Middle1-{i}"),
            status: JobStatus::Queued,
            command: format!("echo mid1-{i}"),
            duration_secs: None,
            started_at: None,
            depends_on: vec!["root".into()],
            output: None,
            required_labels: vec![],
            max_retries: 0,
            attempt: 0,
        });
    }

    // Layer 2: depends on all layer 1
    let layer1_ids: Vec<String> = (0..half).map(|i| format!("mid1-{i}")).collect();
    for i in 0..(middle - half) {
        jobs.push(Job {
            id: format!("mid2-{i}"),
            name: format!("Middle2-{i}"),
            status: JobStatus::Queued,
            command: format!("echo mid2-{i}"),
            duration_secs: None,
            started_at: None,
            depends_on: layer1_ids.clone(),
            output: None,
            required_labels: vec![],
            max_retries: 0,
            attempt: 0,
        });
    }

    // Sink job
    let layer2_ids: Vec<String> = (0..(middle - half)).map(|i| format!("mid2-{i}")).collect();
    jobs.push(Job {
        id: "sink".into(),
        name: "Sink".into(),
        status: JobStatus::Queued,
        command: "echo sink".into(),
        duration_secs: None,
        started_at: None,
        depends_on: layer2_ids,
        output: None,
        required_labels: vec![],
        max_retries: 0,
        attempt: 0,
    });

    Workflow {
        id: "perf-test".into(),
        name: "Perf Test".into(),
        trigger: "manual".into(),
        jobs,
    }
}

/// Benchmark: enqueue N jobs and measure throughput.
#[tokio::test]
async fn bench_enqueue_throughput() {
    for &n in &[10, 50, 100, 500] {
        let queue = InMemoryJobQueue::new();
        let start = Instant::now();

        for i in 0..n {
            let job = workflow_graph_queue::traits::QueuedJob {
                job_id: format!("job-{i}"),
                workflow_id: "bench".into(),
                command: "echo test".into(),
                required_labels: vec![],
                retry_policy: workflow_graph_queue::traits::RetryPolicy::default(),
                attempt: 0,
                upstream_outputs: std::collections::HashMap::new(),
                enqueued_at_ms: 0,
                delayed_until_ms: 0,
            };
            queue.enqueue(job).await.unwrap();
        }

        let elapsed = start.elapsed();
        let per_job = elapsed / n as u32;
        eprintln!("Enqueue {n} jobs: {elapsed:?} total, {per_job:?}/job");

        // Sanity: should be sub-millisecond per job for in-memory
        assert!(
            per_job < Duration::from_millis(1),
            "Enqueue too slow: {per_job:?}/job for {n} jobs"
        );
    }
}

/// Benchmark: claim N jobs and measure throughput.
#[tokio::test]
async fn bench_claim_throughput() {
    for &n in &[10, 50, 100, 500] {
        let queue = InMemoryJobQueue::new();

        // Pre-enqueue
        for i in 0..n {
            let job = workflow_graph_queue::traits::QueuedJob {
                job_id: format!("job-{i}"),
                workflow_id: "bench".into(),
                command: "echo test".into(),
                required_labels: vec![],
                retry_policy: workflow_graph_queue::traits::RetryPolicy::default(),
                attempt: 0,
                upstream_outputs: std::collections::HashMap::new(),
                enqueued_at_ms: 0,
                delayed_until_ms: 0,
            };
            queue.enqueue(job).await.unwrap();
        }

        let start = Instant::now();
        let mut claimed = 0u32;
        for _ in 0..n {
            if queue
                .claim("worker", &[], Duration::from_secs(30))
                .await
                .unwrap()
                .is_some()
            {
                claimed += 1;
            }
        }
        let elapsed = start.elapsed();
        let per_claim = elapsed / n as u32;
        eprintln!("Claim {claimed}/{n} jobs: {elapsed:?} total, {per_claim:?}/claim");

        assert_eq!(claimed, n as u32);
        assert!(
            per_claim < Duration::from_millis(1),
            "Claim too slow: {per_claim:?}/claim for {n} jobs"
        );
    }
}

/// Benchmark: full scheduler cascade for a diamond workflow at various scales.
#[tokio::test]
async fn bench_scheduler_cascade() {
    for &n in &[10, 50, 100] {
        let queue = Arc::new(InMemoryJobQueue::new());
        let artifacts = Arc::new(InMemoryArtifactStore::new());
        let state: SharedState = Arc::new(RwLock::new(WorkflowState::new()));

        let workflow = generate_diamond_workflow(n);
        let total_jobs = workflow.jobs.len();
        let wf_id = workflow.id.clone();
        state
            .write()
            .await
            .workflows
            .insert(wf_id.clone(), workflow);

        let scheduler = Arc::new(DagScheduler::new(
            queue.clone(),
            artifacts.clone(),
            state.clone(),
        ));
        let scheduler_handle = tokio::spawn(scheduler.clone().run());

        let start = Instant::now();

        scheduler.start_workflow(&wf_id).await.unwrap();

        let mut completed = 0u32;
        for _ in 0..total_jobs * 3 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            if let Some((_job, lease)) = queue
                .claim("worker", &[], Duration::from_secs(30))
                .await
                .unwrap()
            {
                queue
                    .complete(&lease.lease_id, std::collections::HashMap::new())
                    .await
                    .unwrap();
                completed += 1;
                tokio::time::sleep(Duration::from_millis(30)).await;
                if completed == total_jobs as u32 {
                    break;
                }
            }
        }

        let elapsed = start.elapsed();
        eprintln!(
            "Scheduler cascade {n}-node diamond ({total_jobs} jobs): {elapsed:?}, completed {completed}/{total_jobs}"
        );

        scheduler_handle.abort();

        assert_eq!(
            completed, total_jobs as u32,
            "All {total_jobs} jobs should complete for {n}-node diamond"
        );
    }
}
