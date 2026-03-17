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
- [x] Add SSE log streaming endpoint
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

## Phase 5: Enhanced YAML Schema ✅
- [x] Add `labels` field to `JobDef` (optional)
- [x] Add `retries` field to `JobDef` (optional, default 0)
- [x] Propagate through `into_workflow()` to shared types
- [x] Add `required_labels`, `max_retries`, `attempt` to `Job` struct
- [x] Update sample `workflows/ci.yml` with examples

## Phase 6: Log Collection API ✅
- [x] `GET /api/workflows/{wf_id}/jobs/{job_id}/logs` — historical JSON
- [x] `GET /api/workflows/{wf_id}/jobs/{job_id}/logs/stream` — SSE live stream
- [x] `POST /api/jobs/{lease_id}/logs` — worker pushes log chunks
- [x] Wire node click → log fetch in demo frontend (log panel)

---

## Web Component Library ✅

### Config + Events ✅
- [x] Add `on_node_hover` callback (job_id or null)
- [x] Add `on_node_drag_end` callback (job_id, x, y)
- [x] Add `on_edge_click` callback (stub — edge hit testing is future work)
- [x] Add `on_canvas_click` callback (deselect)
- [x] Add `on_selection_change` callback

### Pan & Zoom ✅
- [x] Mouse wheel zoom (centered on cursor)
- [x] Click+drag on empty space to pan
- [x] Zoom level clamping (0.25x to 4x)
- [x] Transform matrix in GraphState (zoom, pan_x, pan_y)

### Selection State ✅
- [x] Click node → selected (blue ring)
- [x] Shift+click → multi-select toggle
- [x] Click empty → deselect all
- [x] Visual feedback for selected nodes (blue border)
- [x] `on_selection_change` fires with selected IDs

### Programmatic Control API ✅
- [x] `select_node(canvas_id, job_id)`
- [x] `deselect_all(canvas_id)`
- [x] `reset_layout(canvas_id)`
- [x] `zoom_to_fit(canvas_id)`
- [x] `set_zoom(canvas_id, level)`
- [x] `get_node_positions(canvas_id) -> JSON`
- [x] `set_node_positions(canvas_id, positions_json)`
- [x] `destroy(canvas_id)`

### NPM Package ✅
- [x] TypeScript wrapper class (`WorkflowGraph`) — `packages/web/`
- [x] Auto WASM init, canvas creation
- [x] TypeScript type definitions
- [x] React adapter component (`<WorkflowGraphComponent />`) — `packages/react/`
- [x] Client SDK (`WorkflowClient` for REST API) — `packages/client/`

### Accessibility ✅
- [x] Canvas `role="img"` + `aria-label`
- [x] `tabindex="0"` for keyboard focus
- [x] Tab/Shift+Tab to cycle through nodes
- [x] Enter/Space to activate selected node (fires click callback)
- [x] Escape to deselect all

---

## Documentation ✅
- [x] Write detailed README.md with project overview, architecture diagram, and feature list
- [x] Quick start guide (install, build, run)
- [x] Library usage guide (WASM/JS API reference in README)
- [x] Workflow YAML/JSON schema reference with examples
- [x] Queue trait implementation guide (pg-boss mapping table)
- [x] Worker SDK usage guide (env vars, standalone binary)
- [x] REST API reference (all endpoints in README table)
- [x] Architecture decision records (in specs/plans/)
