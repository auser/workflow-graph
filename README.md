# github-graph

A GitHub Actions-style workflow DAG visualizer and job execution engine, built with Rust + WebAssembly.

Render interactive workflow graphs in the browser with pixel-perfect GitHub Octicon icons, drag-and-drop nodes, pan & zoom, path highlighting, and real-time job status updates. Execute workflows via a pluggable queue system with external workers.

![Workflow Graph](https://img.shields.io/badge/status-alpha-orange)

## Features

### Visualization (WASM)
- **GitHub-accurate DAG rendering** — Octicon SVG icons via Canvas Path2D
- **Interactive nodes** — drag to reposition, click to select, shift+click for multi-select
- **Pan & zoom** — mouse wheel zoom (0.25x–4x), click+drag empty space to pan
- **Path highlighting** — hover a node to highlight its full upstream/downstream path in blue
- **Animated status icons** — spinning ring for running jobs, live elapsed timer
- **Status icons** — queued (hollow circle), running (animated ring), success (green check), failure (red X), skipped (gray slash)

### Job Execution (Server + Workers)
- **Pluggable queue** — trait-based: in-memory (dev), Postgres/pg-boss, Redis
- **DAG scheduler** — event-driven cascade: downstream jobs auto-start when deps succeed
- **External workers** — poll for jobs via HTTP, execute shell commands, stream logs
- **Atomic job claiming** — lease-based with TTL, prevents double-claiming
- **Heartbeats** — workers send periodic heartbeats; expired leases trigger re-queue
- **Retry policy** — configurable per-job retries with backoff
- **Cancellation** — cancel workflows/jobs; workers detect and abort gracefully
- **Worker labels** — jobs require labels, workers register capabilities
- **Log streaming** — workers push log chunks, server stores and serves them
- **Artifact outputs** — jobs publish key-value outputs, downstream jobs read them

### Library Design
- **5 crates** — shared types, queue engine, WASM frontend, worker SDK, reference server
- **Trait-based backends** — `JobQueue`, `ArtifactStore`, `LogSink`, `WorkerRegistry`
- **YAML/JSON workflows** — GitHub Actions-inspired definition format
- **Embeddable** — use `create_router()` to embed the API in your own Axum server

## Architecture

```
┌──────────┐  poll   ┌──────────┐  events  ┌─────────────┐
│  Worker   │◄───────►│  Server  │◄────────►│ DagScheduler│
│  (SDK)    │  HTTP   │  (Axum)  │          │             │
└──────────┘         └──────────┘          └──────┬──────┘
                          │                        │
                     ┌────┴────┐              ┌────┴────┐
                     │ LogSink │              │JobQueue │
                     │Artifacts│              │(trait)  │
                     └─────────┘              └─────────┘
```

## Crate Structure

| Crate | Purpose |
|-------|---------|
| `github-graph-shared` | Core types: `Job`, `Workflow`, `JobStatus`, YAML parser |
| `github-graph-queue` | Queue traits + in-memory implementations, DagScheduler |
| `github-graph-web` | WASM Canvas renderer with interactive graph |
| `github-graph-worker-sdk` | Worker binary + embeddable library |
| `github-graph-server` | Reference Axum server wiring everything together |

## Quick Start

### Prerequisites

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack just
```

### Build & Run

```bash
# Build WASM frontend + start server
just dev

# Or separately:
just build-wasm      # Build WASM
just serve           # Start server (auto-finds port if 3000 is taken)

# Development with auto-reload:
just watch            # cargo-watch restarts server on changes
```

Open `http://localhost:3000/index.html` and click **Run workflow**.

### Run a Worker (separate terminal)

```bash
cargo run -p github-graph-worker-sdk
# Or with custom server URL and labels:
SERVER_URL=http://localhost:3000 WORKER_LABELS=docker,linux cargo run -p github-graph-worker-sdk
```

## Workflow Definition (YAML)

```yaml
name: CI Pipeline
on: push

jobs:
  test:
    name: Unit Tests
    run: cargo test
    retries: 2

  lint:
    name: Lint
    run: cargo clippy

  build:
    name: Build
    needs: [test, lint]
    run: cargo build --release
    labels: [linux]

  deploy:
    name: Deploy
    needs: [build]
    labels: [linux, aws]
    steps:
      - name: Migrate DB
        run: ./scripts/migrate.sh
      - name: Deploy App
        run: ./scripts/deploy.sh
```

Place workflow files in `workflows/` directory (`.yml`, `.yaml`, or `.json`).

## WASM API

```javascript
import init, {
  render_workflow,
  update_workflow_data,
  select_node,
  deselect_all,
  reset_layout,
  zoom_to_fit,
  set_zoom,
  get_node_positions,
  set_node_positions,
  destroy,
} from 'github-graph-web';

await init();

// Render with callbacks
render_workflow(
  'canvas-id',
  workflowJson,
  (jobId) => console.log('clicked', jobId),       // on_node_click
  (jobId) => console.log('hover', jobId),          // on_node_hover
  () => console.log('canvas clicked'),             // on_canvas_click
  (ids) => console.log('selected', ids),           // on_selection_change
  (id, x, y) => console.log('dragged', id, x, y), // on_node_drag_end
);

// Update data (preserves positions, zoom, selection)
update_workflow_data('canvas-id', newWorkflowJson);

// Programmatic control
select_node('canvas-id', 'build');
reset_layout('canvas-id');
zoom_to_fit('canvas-id');
```

## REST API

### Workflow Management

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/workflows` | List all workflows |
| `POST` | `/api/workflows` | Create workflow |
| `GET` | `/api/workflows/{id}/status` | Get workflow status |
| `POST` | `/api/workflows/{id}/run` | Run workflow |
| `POST` | `/api/workflows/{id}/cancel` | Cancel workflow |

### Worker Protocol

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/workers/register` | Register worker with labels |
| `POST` | `/api/jobs/claim` | Claim next available job |
| `POST` | `/api/jobs/{lease_id}/heartbeat` | Renew job lease |
| `POST` | `/api/jobs/{lease_id}/complete` | Report job success |
| `POST` | `/api/jobs/{lease_id}/fail` | Report job failure |
| `POST` | `/api/jobs/{lease_id}/logs` | Push log chunks |
| `GET` | `/api/jobs/{wf_id}/{job_id}/cancelled` | Check cancellation |
| `GET` | `/api/workflows/{wf_id}/jobs/{job_id}/logs` | Get job logs |

## Implementing a Custom Queue Backend

All backends are trait-based. Implement these traits for your preferred storage:

```rust
use github_graph_queue::traits::*;

struct MyRedisQueue { /* ... */ }

impl JobQueue for MyRedisQueue {
    async fn enqueue(&self, job: QueuedJob) -> Result<(), QueueError> { /* ... */ }
    async fn claim(&self, worker_id: &str, labels: &[String], ttl: Duration)
        -> Result<Option<(QueuedJob, Lease)>, QueueError> { /* ... */ }
    // ... etc
}
```

### Trait → pg-boss Mapping

| Trait Method | pg-boss Equivalent |
|-------------|-------------------|
| `enqueue()` | `boss.send(queue, data, options)` |
| `claim()` | `boss.fetch(queue)` — `SELECT FOR UPDATE SKIP LOCKED` |
| `complete()` | `boss.complete(jobId)` |
| `fail()` | `boss.fail(jobId)` |
| `cancel()` | `boss.cancel(jobId)` |
| `reap_expired()` | pg-boss `maintain()` (automatic) |

## Embedding in Your Server

```rust
use github_graph_queue::memory::*;
use github_graph_queue::{DagScheduler, SharedState, WorkflowState};

#[tokio::main]
async fn main() {
    let state = SharedState::new(WorkflowState::new());
    let queue = Arc::new(InMemoryJobQueue::new());
    let artifacts = Arc::new(InMemoryArtifactStore::new());
    let logs = Arc::new(InMemoryLogSink::new());
    let workers = Arc::new(InMemoryWorkerRegistry::new());
    let scheduler = Arc::new(DagScheduler::new(queue.clone(), artifacts.clone(), state.clone()));

    // Spawn scheduler
    tokio::spawn(scheduler.clone().run());

    // Build your router with the API
    let app = github_graph_server::create_router(AppState {
        workflow_state: state,
        queue, artifacts, logs, workers, scheduler,
    });

    axum::serve(listener, app).await.unwrap();
}
```

## Testing

```bash
just test              # Run all tests (22 tests)
just check             # Type-check workspace
cargo test -p github-graph-queue   # Queue + scheduler tests only
```

## License

MIT
