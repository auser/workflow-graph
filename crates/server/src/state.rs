use std::sync::Arc;

use github_graph_queue::memory::*;
use github_graph_queue::scheduler::SharedState;
use github_graph_queue::DagScheduler;

/// Application state shared across all request handlers.
///
/// For now uses concrete in-memory types. When adding Postgres/Redis backends,
/// either make this generic or use a trait-object wrapper crate.
#[derive(Clone)]
pub struct AppState {
    pub workflow_state: SharedState,
    pub queue: Arc<InMemoryJobQueue>,
    pub artifacts: Arc<InMemoryArtifactStore>,
    pub logs: Arc<InMemoryLogSink>,
    pub workers: Arc<InMemoryWorkerRegistry>,
    pub scheduler: Arc<DagScheduler<InMemoryJobQueue, InMemoryArtifactStore>>,
}
