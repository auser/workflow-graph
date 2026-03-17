# Queue/Worker Architecture — Pluggable Job Execution System

## Context
The current system runs jobs inline on the server via `tokio::process::Command`. This doesn't scale — no external workers, no persistence, no retry, no log streaming. We need a production-grade job execution system inspired by GitHub Actions with pluggable queues, external workers, and full observability.

The initial queue backend is **pg-boss style (Postgres)**, with an in-memory implementation for dev/testing.

## Architecture Overview

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
                                                  │
                                          ┌───────┴───────┐
                                          │  InMemory  │ Postgres │
                                          └───────────────┘
```

## New Crate Structure

```
Cargo.toml                          # add: queue, worker-sdk
crates/
  queue/                            # NEW: workflow-graph-queue
    src/
      lib.rs                        # re-exports
      traits.rs                     # JobQueue, ArtifactStore, LogSink, WorkerRegistry
      scheduler.rs                  # DagScheduler (event-driven DAG cascade)
      error.rs                      # Error types
      memory/
        mod.rs
        queue.rs                    # InMemoryJobQueue
        artifacts.rs                # InMemoryArtifactStore
        logs.rs                     # InMemoryLogSink
        workers.rs                  # InMemoryWorkerRegistry
  worker-sdk/                       # NEW: workflow-graph-worker-sdk
    src/
      lib.rs                        # Worker struct, config, poll loop
      executor.rs                   # Shell command executor (moved from server)
      main.rs                       # Standalone worker binary
```

## Trait Definitions

### JobQueue (pluggable: in-memory, Postgres/pg-boss, Redis)
```rust
trait JobQueue: Send + Sync + 'static {
    async fn enqueue(&self, job: QueuedJob) -> Result<()>;
    async fn claim(&self, worker_id: &str, labels: &[String], lease_ttl: Duration) -> Result<Option<(QueuedJob, Lease)>>;
    async fn renew_lease(&self, lease_id: &str, extend_by: Duration) -> Result<()>;
    async fn complete(&self, lease_id: &str, outputs: HashMap<String, String>) -> Result<()>;
    async fn fail(&self, lease_id: &str, error: String, retryable: bool) -> Result<()>;
    async fn cancel(&self, workflow_id: &str, job_id: &str) -> Result<()>;
    async fn cancel_workflow(&self, workflow_id: &str) -> Result<()>;
    async fn is_cancelled(&self, workflow_id: &str, job_id: &str) -> Result<bool>;
    async fn reap_expired_leases(&self) -> Result<Vec<JobEvent>>;
    fn subscribe(&self) -> broadcast::Receiver<JobEvent>;
}
```

### ArtifactStore (pluggable: in-memory, S3, filesystem)
```rust
trait ArtifactStore: Send + Sync + 'static {
    async fn put_outputs(&self, workflow_id: &str, job_id: &str, outputs: HashMap<String, String>) -> Result<()>;
    async fn get_outputs(&self, workflow_id: &str, job_id: &str) -> Result<HashMap<String, String>>;
    async fn get_upstream_outputs(&self, workflow_id: &str, job_ids: &[String]) -> Result<HashMap<String, HashMap<String, String>>>;
}
```

### LogSink (pluggable: in-memory, file, S3)
```rust
trait LogSink: Send + Sync + 'static {
    async fn append(&self, chunk: LogChunk) -> Result<()>;
    async fn get_all(&self, workflow_id: &str, job_id: &str) -> Result<Vec<LogChunk>>;
    fn subscribe(&self, workflow_id: &str, job_id: &str) -> broadcast::Receiver<LogChunk>;
}
```

### WorkerRegistry
```rust
trait WorkerRegistry: Send + Sync + 'static {
    async fn register(&self, worker_id: &str, labels: &[String]) -> Result<()>;
    async fn heartbeat(&self, worker_id: &str) -> Result<()>;
    async fn deregister(&self, worker_id: &str) -> Result<()>;
    async fn list_workers(&self) -> Result<Vec<WorkerInfo>>;
    async fn mark_busy(&self, worker_id: &str, job_id: &str) -> Result<()>;
    async fn mark_idle(&self, worker_id: &str) -> Result<()>;
}
```

## Key Types

```rust
struct Lease { job_id, workflow_id, worker_id, lease_id: String, ttl: Duration, granted_at: Instant }
struct QueuedJob { job_id, workflow_id, command, required_labels, retry_policy, attempt, upstream_outputs, enqueued_at }
struct RetryPolicy { max_retries: u32, backoff: BackoffStrategy }
enum BackoffStrategy { Fixed(Duration), Exponential { base, max }, None }
struct LogChunk { workflow_id, job_id, sequence, data, timestamp_ms, stream: Stdout|Stderr }
struct WorkerInfo { worker_id, labels, registered_at, last_heartbeat_at, current_job, status: Idle|Busy|Offline }
enum JobEvent { Ready, Started, Completed, Failed, Cancelled, LeaseExpired }
```

## Worker HTTP Protocol

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/api/workers/register` | Register worker with labels |
| POST | `/api/workers/{id}/heartbeat` | Worker-level heartbeat |
| POST | `/api/jobs/claim` | Poll for work (atomic claim) |
| POST | `/api/jobs/{lease_id}/heartbeat` | Renew job lease |
| POST | `/api/jobs/{lease_id}/complete` | Report success + outputs |
| POST | `/api/jobs/{lease_id}/fail` | Report failure |
| GET | `/api/jobs/{wf_id}/{job_id}/cancelled` | Check cancellation |
| POST | `/api/jobs/{lease_id}/logs` | Push log chunks |
| GET | `/api/workflows/{wf_id}/jobs/{job_id}/logs` | SSE log stream |
| POST | `/api/workflows/{id}/cancel` | Cancel workflow |
| GET | `/api/workers` | List workers |

## Worker SDK Poll Loop

```
1. REGISTER → POST /api/workers/register
2. POLL → POST /api/jobs/claim (every 1-5s)
3. EXECUTE → spawn sh -c, read output incrementally
4. CONCURRENT: heartbeat (every TTL/3), log push (every 500ms), cancellation check (every 2s)
5. COMPLETE → POST /api/jobs/{lease_id}/complete or /fail
6. GOTO 2
```

## DagScheduler (replaces orchestrator)

Event-driven loop that subscribes to `JobQueue::subscribe()`:
- **On workflow start**: enqueue root jobs (no deps)
- **On Completed**: find downstream jobs with all deps satisfied → enqueue with upstream outputs
- **On Failed (non-retryable)**: mark transitive downstream as Skipped
- **On LeaseExpired**: check retry budget → re-enqueue or mark Failed
- Updates `SharedState` so frontend polling keeps working unchanged

## Server Changes

- `main.rs`: instantiate InMemory* impls, spawn DagScheduler + lease reaper background tasks
- `orchestrator.rs`: strip to just `OrchestratorState` + `JobMeta` (retry policy, attempt, labels)
- `api.rs`: `run_workflow` calls `scheduler.start_workflow()`, add cancel endpoints
- `api_worker.rs`: NEW — worker protocol endpoints
- `api_logs.rs`: NEW — SSE log streaming
- `executor.rs`: DELETE — moves to worker-sdk

## Shared Type Extensions

Add to `Job` (with `#[serde(default)]` for backwards compat):
- `required_labels: Vec<String>`
- `retry_policy: Option<RetryPolicy>`
- `attempt: u32`

Add to YAML parser `JobDef`:
- `labels: Vec<String>` (optional)
- `retries: Option<u32>` (optional, default 0)

## Implementation Phases

### Phase 1: Queue crate — traits + in-memory
1. Create `crates/queue/` with all trait definitions
2. Implement all InMemory* backends
3. Unit tests for claim/complete/fail/reap/cancel

### Phase 2: DagScheduler
1. Event-driven scheduler replaces recursive cascade
2. Wire Completed → enqueue downstream, Failed → skip downstream
3. Lease expiry → retry or fail

### Phase 3: Server integration
1. Wire queue/scheduler/artifacts/logs into server
2. Add worker protocol API endpoints
3. Add SSE log streaming
4. Add cancellation endpoints
5. Strip old orchestrator, delete old executor

### Phase 4: Worker SDK
1. Worker struct with poll/claim/heartbeat/complete loop
2. Incremental output capture + log streaming
3. Cancellation checking + graceful shutdown
4. Standalone binary entry point

### Phase 5: Enhanced YAML schema
1. Add `labels`, `retries` to workflow YAML
2. Propagate through parser → shared types

## Prerequisite: Node Click Events (do first)

Add click detection to the WASM frontend so consumers can wire up actions when a node is clicked. This is separate from drag — a click is a mousedown+mouseup with less than 5px movement.

**Changes to `crates/web/src/lib.rs`:**

1. Add `on_node_click: Option<js_sys::Function>` to `GraphState`
2. Add `mouse_down_pos: Option<(f64, f64)>` to track mousedown position
3. Update `render_workflow()` signature: `render_workflow(canvas_id, workflow_json, on_node_click: Option<js_sys::Function>)`
4. In mousedown handler: record `mouse_down_pos = Some((mx, my))`
5. In mouseup handler: if `mouse_down_pos` is set and distance to current pos < 5px, it's a click:
   - Hit test at mouseup position
   - If a node was hit, call `on_node_click.call1(&JsValue::NULL, &JsValue::from_str(&job_id))`
6. Clear `mouse_down_pos` in mouseup and mouseleave

**Changes to `www/index.js`:**
- Pass callback to `render_workflow()`: `render_workflow('graph', json, (jobId) => { console.log('clicked', jobId); })`

## Log Collection API (by job ID)

Consumers need to fetch logs for a specific job — both historical and live streaming.

**Endpoints:**
- `GET /api/workflows/{wf_id}/jobs/{job_id}/logs` — returns all log chunks as JSON (historical)
- `GET /api/workflows/{wf_id}/jobs/{job_id}/logs/stream` — SSE stream: replays existing chunks then streams live

**WASM API (for click → show logs flow):**
- `get_job_logs(server_url, workflow_id, job_id) -> Promise<Vec<LogChunk>>` — fetch historical logs
- Consumer wires this to the `on_node_click` callback to show a log panel when a node is clicked

**LogSink trait** already covers storage/subscription. The server just exposes it via HTTP.

## Library Design

This project is a **library** that consumers integrate into their own apps, not a standalone application.

**Crate purposes:**
- `workflow-graph-shared` — types crate, used by all consumers (Rust, WASM, workers)
- `workflow-graph-queue` — queue/scheduler library. Consumers instantiate traits with their preferred backend (in-memory, Postgres, Redis). No runtime opinions.
- `workflow-graph-web` — WASM library. Consumers call `render_workflow(canvas_id, json, on_click)` from their own frontend.
- `workflow-graph-worker-sdk` — worker library + optional standalone binary. Consumers can embed the worker loop in their own service or run it standalone.
- `workflow-graph-server` — **example/reference server**. Shows how to wire everything together with Axum. Consumers can either:
  1. Use this server directly (simple setup)
  2. Copy the wiring into their own Axum/Actix/etc. app
  3. Use just the queue/scheduler library with their own HTTP layer

**Simple server setup for consumers:**
```rust
// In their own main.rs:
use workflow_graph_queue::memory::*;
use workflow_graph_server::create_router;

#[tokio::main]
async fn main() {
    let queue = InMemoryJobQueue::new();
    let artifacts = InMemoryArtifactStore::new();
    let logs = InMemoryLogSink::new();
    let workers = InMemoryWorkerRegistry::new();

    let app = create_router(queue, artifacts, logs, workers);
    // ... bind and serve
}
```

The server crate exposes a `create_router()` function that takes trait objects and returns an Axum `Router`. Consumers don't need to know about individual endpoints.

## Branch & Specs Setup

1. Checkout `feat/queue-worker-system` branch from current commit
2. Create `specs/SPRINT.md` with checkboxes tracking all phases
3. Save full plan to `specs/plans/001-queue-worker-architecture.md`

## Verification
1. `cargo test --workspace` — all unit tests pass
2. Start server + worker: `just serve` in one terminal, `cargo run -p workflow-graph-worker-sdk` in another
3. Open frontend, click "Run workflow" — jobs execute via the worker, not the server
4. Kill the worker mid-job — lease expires, job re-queued (if retries > 0)
5. Cancel a running workflow — worker aborts, downstream skipped
6. SSE log stream shows real-time output in browser
