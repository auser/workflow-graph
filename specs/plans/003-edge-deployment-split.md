# Plan 003: Split Server for Edge Deployment

## Context
The server bundles the HTTP API and DagScheduler in one process, preventing deployment to edge/serverless platforms (Vercel Workers, Cloudflare Workers, Supabase Edge Functions) where functions are request-scoped with no long-lived background tasks.

## Decision
Split into two independently deployable pieces:
- **API server** (stateless) — all HTTP handlers, reads/writes to queue backends directly
- **Scheduler service** (long-running) — event loop listening for queue events, drives DAG cascade

## Key Changes
1. Extract `start_workflow()` and `cancel_workflow()` into `workflow_ops.rs` — stateless functions that enqueue jobs directly without needing the scheduler
2. Remove `scheduler` field from `AppState` — API is fully stateless
3. Create `crates/scheduler/` — standalone scheduler binary
4. Server `main.rs` supports two modes: all-in-one (dev) and api-only (edge), controlled by `API_ONLY=1` env var

## Rename
All crates renamed from the old name to `workflow-graph-*` (done in parallel session).

## Status
Complete.

### Checklist
- [x] Extract `start_workflow()` / `cancel_workflow()` into `workflow_ops.rs`
- [x] Remove `scheduler` field from `AppState` (`state.rs`)
- [x] Wire `api.rs` handlers to call `workflow_ops` instead of `state.scheduler`
- [x] Create `crates/scheduler/` standalone binary with event loop + lease reaper
- [x] Add workspace member `crates/scheduler` to root `Cargo.toml`
- [x] Server `main.rs` two-mode support: all-in-one (default) vs `API_ONLY=1` (edge)
- [x] Workspace compiles cleanly (`cargo check`)
