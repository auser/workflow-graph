# Sprint: Queue/Worker System + Web Component Library

## Prerequisite
- [x] Add node click events to WASM frontend (JS callback API)

---

## Phase 1: Queue Crate — Traits + In-Memory ✅
> `crates/queue/` — pure library, no HTTP dependency

- [x] Create `crates/queue/` crate with Cargo.toml
- [x] Define `JobQueue` trait (enqueue, claim, renew, complete, fail, cancel, reap, subscribe)
- [x] Define `ArtifactStore` trait (put_outputs, get_outputs, get_upstream_outputs)
- [x] Define `LogSink` trait (append, get_all, subscribe)
- [x] Define `WorkerRegistry` trait (register, heartbeat, deregister, list, mark_busy/idle)
- [x] Define shared types: `Lease`, `QueuedJob`, `RetryPolicy`, `BackoffStrategy`, `LogChunk`, `WorkerInfo`, `JobEvent`
- [x] Define error types
- [x] Implement `InMemoryJobQueue`
- [x] Implement `InMemoryArtifactStore`
- [x] Implement `InMemoryLogSink`
- [x] Implement `InMemoryWorkerRegistry`
- [x] Unit tests for queue claim/complete/fail/reap/cancel (6 tests)
- [x] Unit tests for artifact store (3 tests)
- [x] Unit tests for log sink subscribe/append (3 tests)
- [x] Unit tests for worker registry (3 tests)

> **pg-boss mapping:** Our `JobQueue` trait maps 1:1 to pg-boss operations.
> pg-boss handles atomic claiming via `SELECT ... FOR UPDATE SKIP LOCKED` —
> no two workers can claim the same job. Expired leases are reaped by pg-boss's
> internal `maintain()` loop. Retry policy is configured per-job at enqueue time.
>
> | Our trait method     | pg-boss equivalent                        |
> |----------------------|-------------------------------------------|
> | `enqueue()`          | `boss.send(queue, data, options)`         |
> | `claim()`            | `boss.fetch(queue)` (atomic FOR UPDATE)   |
> | `renew_lease()`      | extend `expireInSeconds` on active job    |
> | `complete()`         | `boss.complete(jobId, data)`              |
> | `fail()`             | `boss.fail(jobId, data)`                  |
> | `cancel()`           | `boss.cancel(jobId)`                      |
> | `reap_expired()`     | pg-boss `maintain()` (automatic)          |
> | `subscribe()`        | pg-boss `work()` / LISTEN/NOTIFY          |

## Phase 2: DagScheduler ✅
> Event-driven DAG cascade — replaces the inline orchestrator

- [x] Implement `DagScheduler` struct with event-driven loop
- [x] On workflow start: enqueue root jobs (no deps)
- [x] On Completed: find ready downstream → enqueue with upstream outputs
- [x] On Failed (non-retryable): mark transitive downstream as Skipped
- [x] On LeaseExpired: check retry budget → re-enqueue or fail
- [x] Update `SharedState` on each event (frontend compat)
- [x] Unit tests for scheduler (4 tests: start, cascade, failure skip, cancel)

## Phase 3: Server Integration ✅
> Wire queue into server, expose worker protocol via HTTP

- [x] Add `crates/queue` dependency to server
- [x] Modify `main.rs`: create in-memory instances, spawn scheduler + lease reaper
- [x] Expose `create_router()` for library consumers
- [x] Change `run_workflow` to call `scheduler.start_workflow()`
- [x] Delete `orchestrator.rs` (replaced by DagScheduler)
- [x] Delete `executor.rs` from server (moves to worker-sdk)
- [x] Add worker protocol endpoints (register, claim, heartbeat, complete, fail, logs)
- [x] Add cancel endpoints (workflow)
- [x] Add workers list endpoint
- [x] Add log push + get endpoints
- [ ] Add SSE log streaming endpoint (deferred to Phase 6)
- [x] Verify all 22 tests pass

## Phase 4: Worker SDK ✅
> `crates/worker-sdk/` — standalone worker binary + embeddable library

- [x] Create `crates/worker-sdk/` crate
- [x] Implement `executor.rs` with incremental stdout/stderr streaming
- [x] Implement `Worker` struct with `WorkerConfig`
- [x] Implement poll/claim loop (HTTP polling)
- [x] Implement heartbeat sender (concurrent tokio task)
- [x] Implement log streaming (batched push via HTTP)
- [x] Implement cancellation checking + graceful child kill (CancellationToken)
- [x] Add `main.rs` standalone worker binary (configurable via env vars)
- [ ] Integration test: server + worker end-to-end (deferred)

## Phase 5: Enhanced YAML Schema
- [ ] Add `labels` field to `JobDef` (optional)
- [ ] Add `retries` field to `JobDef` (optional, default 0)
- [ ] Propagate through `into_workflow()` to shared types
- [ ] Add `required_labels`, `retry_policy`, `attempt` to `Job` struct
- [ ] Update sample `workflows/ci.yml` with examples

## Phase 6: Log Collection API
- [ ] `GET /api/workflows/{wf_id}/jobs/{job_id}/logs` — historical JSON
- [ ] `GET /api/workflows/{wf_id}/jobs/{job_id}/logs/stream` — SSE live stream
- [ ] Wire node click → log fetch in demo frontend

---

## Web Component Library (Plan 002)

### Config + Events
- [ ] Add `GraphConfig` object (theme, layout, behavior options)
- [ ] Add `on_node_hover` callback (job_id or null)
- [ ] Add `on_node_drag_end` callback (job_id, x, y)
- [ ] Add `on_edge_click` callback (from_id, to_id)
- [ ] Add `on_canvas_click` callback (deselect)
- [ ] Add `on_selection_change` callback

### Pan & Zoom
- [ ] Mouse wheel zoom (centered on cursor)
- [ ] Click+drag on empty space to pan
- [ ] Zoom level clamping (0.25x to 4x)
- [ ] Transform matrix in GraphState

### Selection State
- [ ] Click node → selected (blue ring)
- [ ] Shift+click → multi-select toggle
- [ ] Click empty → deselect all
- [ ] Visual feedback for selected nodes
- [ ] `on_selection_change` fires with selected IDs

### Programmatic Control API
- [ ] `select_node(canvas_id, job_id)`
- [ ] `deselect_all(canvas_id)`
- [ ] `reset_layout(canvas_id)`
- [ ] `zoom_to_fit(canvas_id)`
- [ ] `set_zoom(canvas_id, level)`
- [ ] `get_node_positions(canvas_id) -> JSON`
- [ ] `set_node_positions(canvas_id, positions_json)`
- [ ] `destroy(canvas_id)`

### NPM Package
- [ ] TypeScript wrapper class (`WorkflowGraph`)
- [ ] Auto WASM init, canvas creation
- [ ] TypeScript type definitions
- [ ] React adapter component (`<WorkflowGraph />`)
- [ ] Client SDK (`WorkflowClient` for REST API)

### Accessibility
- [ ] Canvas `role="img"` + `aria-label`
- [ ] Hidden DOM overlay with focusable node elements
- [ ] Tab/arrow key navigation
- [ ] Enter/Space to select

---

## Documentation
- [ ] Write detailed README.md with project overview, architecture diagram, and feature list
- [ ] Quick start guide (install, build, run)
- [ ] Library usage guide (Rust, WASM/JS, React)
- [ ] Workflow YAML/JSON schema reference with examples
- [ ] Queue trait implementation guide (how to write a Redis/Postgres backend)
- [ ] Worker SDK usage guide (standalone binary + embedded)
- [ ] REST API reference (all endpoints, request/response examples)
- [ ] Configuration reference (GraphConfig options, theme customization)
- [ ] Architecture decision records (why Canvas, why traits, why HTTP polling)
- [ ] Contributing guide
