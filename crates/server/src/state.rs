use std::sync::Arc;

use workflow_graph_queue::memory::*;
use workflow_graph_queue::scheduler::SharedState;

/// Application state shared across all request handlers.
///
/// Fully stateless — no scheduler reference. The scheduler runs
/// as a separate service (or embedded in all-in-one mode).
#[derive(Clone)]
pub struct AppState {
    pub workflow_state: SharedState,
    pub queue: Arc<InMemoryJobQueue>,
    pub artifacts: Arc<InMemoryArtifactStore>,
    pub logs: Arc<InMemoryLogSink>,
    pub workers: Arc<InMemoryWorkerRegistry>,
}
