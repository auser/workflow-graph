# Sprint: Queue/Worker System

## Prerequisite
- [x] Add node click events to WASM frontend (JS callback API)

## Phase 1: Queue Crate — Traits + In-Memory
- [ ] Create `crates/queue/` crate with Cargo.toml
- [ ] Define `JobQueue` trait (enqueue, claim, renew, complete, fail, cancel, reap, subscribe)
- [ ] Define `ArtifactStore` trait (put_outputs, get_outputs, get_upstream_outputs)
- [ ] Define `LogSink` trait (append, get_all, subscribe)
- [ ] Define `WorkerRegistry` trait (register, heartbeat, deregister, list, mark_busy/idle)
- [ ] Define shared types: `Lease`, `QueuedJob`, `RetryPolicy`, `BackoffStrategy`, `LogChunk`, `WorkerInfo`, `JobEvent`
- [ ] Define error types
- [ ] Implement `InMemoryJobQueue`
- [ ] Implement `InMemoryArtifactStore`
- [ ] Implement `InMemoryLogSink`
- [ ] Implement `InMemoryWorkerRegistry`
- [ ] Unit tests for queue claim/complete/fail/reap/cancel
- [ ] Unit tests for artifact store
- [ ] Unit tests for log sink subscribe/append

## Phase 2: DagScheduler
- [ ] Implement `DagScheduler` struct with event-driven loop
- [ ] On workflow start: enqueue root jobs (no deps)
- [ ] On Completed: find ready downstream → enqueue with upstream outputs
- [ ] On Failed (non-retryable): mark transitive downstream as Skipped
- [ ] On LeaseExpired: check retry budget → re-enqueue or fail
- [ ] Update `SharedState` on each event (frontend compat)
- [ ] Unit tests for scheduler with mock queue

## Phase 3: Server Integration
- [ ] Add `crates/queue` dependency to server
- [ ] Modify `main.rs`: create in-memory instances, spawn scheduler + lease reaper
- [ ] Expose `create_router()` for library consumers
- [ ] Change `run_workflow` to call `scheduler.start_workflow()`
- [ ] Strip `orchestrator.rs` to state types only
- [ ] Delete `executor.rs` from server (moves to worker-sdk)
- [ ] Add worker protocol endpoints (`api_worker.rs`)
- [ ] Add SSE log streaming endpoint (`api_logs.rs`)
- [ ] Add cancel endpoints (workflow + individual job)
- [ ] Add workers list endpoint
- [ ] Verify existing frontend still works unchanged

## Phase 4: Worker SDK
- [ ] Create `crates/worker-sdk/` crate
- [ ] Move executor logic from server, enhance with incremental output
- [ ] Implement `Worker` struct with config
- [ ] Implement poll/claim loop
- [ ] Implement heartbeat sender (concurrent task)
- [ ] Implement log streaming (batched push)
- [ ] Implement cancellation checking + graceful child kill
- [ ] Add `main.rs` standalone worker binary
- [ ] Integration test: server + worker end-to-end

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
