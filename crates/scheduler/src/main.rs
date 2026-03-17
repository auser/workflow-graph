//! Standalone DAG scheduler service.
//!
//! Runs the DagScheduler event loop and lease reaper independently of the
//! HTTP API server. In production, the API server runs in API-only mode
//! (edge/serverless) while this service handles DAG orchestration.
//!
//! Configuration via environment variables:
//!   REAP_INTERVAL_SECS  — lease reaper interval (default: 5)

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use workflow_graph_queue::memory::*;
use workflow_graph_queue::scheduler::WorkflowState;
use workflow_graph_queue::{DagScheduler, JobQueue};

#[tokio::main]
async fn main() {
    let reap_interval: u64 = std::env::var("REAP_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    let workflow_state = Arc::new(RwLock::new(WorkflowState::new()));
    let queue = Arc::new(InMemoryJobQueue::new());
    let artifacts = Arc::new(InMemoryArtifactStore::new());

    let scheduler = Arc::new(DagScheduler::new(
        queue.clone(),
        artifacts.clone(),
        workflow_state.clone(),
    ));

    // Spawn lease reaper
    let reaper_queue = queue.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(reap_interval)).await;
            if let Err(e) = reaper_queue.reap_expired_leases().await {
                eprintln!("Lease reaper error: {e}");
            }
        }
    });

    println!("Scheduler service started (reap interval: {reap_interval}s)");
    scheduler.run().await;
}
