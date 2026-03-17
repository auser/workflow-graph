---
title: Embedding in Your Server
description: Use workflow-graph as a library in your own Axum application
---

The server crate exposes `create_router()` so you can embed the workflow engine in your own Axum application.

## Setup

```toml
[dependencies]
workflow-graph-server = { path = "crates/server" }
workflow-graph-queue = { path = "crates/queue" }
tokio = { version = "1", features = ["full"] }
axum = "0.8"
```

## Basic Example

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use workflow_graph_queue::memory::*;
use workflow_graph_queue::{DagScheduler, WorkflowState};
use workflow_graph_server::state::AppState;

#[tokio::main]
async fn main() {
    let state = Arc::new(RwLock::new(WorkflowState::new()));
    let queue = Arc::new(InMemoryJobQueue::new());
    let artifacts = Arc::new(InMemoryArtifactStore::new());
    let logs = Arc::new(InMemoryLogSink::new());
    let workers = Arc::new(InMemoryWorkerRegistry::new());

    // Spawn scheduler (optional — omit for API-only / edge mode)
    let scheduler = Arc::new(DagScheduler::new(
        queue.clone(), artifacts.clone(), state.clone(),
    ));
    tokio::spawn(scheduler.clone().run());

    // Build router with the workflow API
    let app = workflow_graph_server::create_router(AppState {
        workflow_state: state,
        queue, artifacts, logs, workers,
    });

    // Merge with your own routes
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

## Merging with Your Own Routes

```rust
use axum::Router;

let workflow_router = workflow_graph_server::create_router(app_state);

let app = Router::new()
    .nest("/workflows", workflow_router)
    .route("/health", axum::routing::get(|| async { "ok" }));
```

## API-Only Mode (No Scheduler)

For edge deployments, skip the scheduler and just embed the stateless API:

```rust
let app = workflow_graph_server::create_router(AppState {
    workflow_state: state,
    queue, artifacts, logs, workers,
});

// No scheduler spawned — run it separately via:
// cargo run -p workflow-graph-scheduler
```

See [Deployment Modes](/workflow-graph/architecture/deployment-modes/) for details on the split architecture.

## Custom Queue Backends

Replace the `InMemory*` types with your own implementations:

```rust
let backend = Arc::new(MyPostgresBackend::new(pool));

let app = workflow_graph_server::create_router(AppState {
    workflow_state: state,
    queue: backend.clone(),
    artifacts: backend.clone(),
    logs: backend.clone(),
    workers: backend.clone(),
});
```

See [Custom Queue Backend](/workflow-graph/guides/custom-queue/) for trait implementation details.
