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

---

## Edge Deployment Split (Plan 003) ✅
> Split server into stateless API + standalone scheduler for edge/serverless deployment

- [x] Extract `start_workflow()` / `cancel_workflow()` into `workflow_ops.rs`
- [x] Remove `scheduler` field from `AppState`
- [x] Wire `api.rs` handlers to call `workflow_ops` directly
- [x] Create `crates/scheduler/` standalone scheduler binary
- [x] Add `crates/scheduler` to workspace
- [x] Server two-mode support: all-in-one (default) vs `API_ONLY=1` (edge)

---

## Production Readiness — Customization & Packaging ✅

### Theme Configuration System ✅
> Runtime-configurable colors, fonts, layout dimensions, and direction
- [x] Add `ThemeConfig` struct with `ThemeColors`, `ThemeFonts`, `ThemeLayout`, `LayoutDirection`
- [x] Add `ResolvedTheme` with defaults for all omitted fields
- [x] Wire `ResolvedTheme` through `compute_layout()`, `render()`, and all draw functions
- [x] Replace all hard-coded color/font/dimension constants with theme lookups
- [x] Add `theme_json` parameter to `render_workflow()` WASM function
- [x] Add `set_theme(canvas_id, theme_json)` for runtime theme switching
- [x] Add `get_dark_theme()` WASM function returning dark preset JSON
- [x] Parse running color hex → rgba for spinner animation (dynamic, not hard-coded)

### Dark Mode Preset ✅
- [x] GitHub-accurate dark theme colors (`#0d1117` bg, `#161b22` cards, `#e6edf3` text)
- [x] Exposed as `dark_theme_colors()` in Rust and `darkTheme` / `lightTheme` in TypeScript

### Layout Direction ✅
- [x] `LayoutDirection::LeftToRight` (default) and `LayoutDirection::TopToBottom`
- [x] Edges adapt: LTR uses right→left beziers, TTB uses bottom→top beziers
- [x] `set_theme()` triggers automatic re-layout when direction or dimensions change

### Responsive Canvas (ResizeObserver) ✅
- [x] `set_auto_resize(canvas_id, enabled)` WASM function
- [x] ResizeObserver on canvas parent element, redraws on resize
- [x] Cleanup on destroy
- [x] Exposed as `autoResize` option in TypeScript `GraphOptions`

### TypeScript Wrapper Updates ✅
- [x] Full `ThemeConfig`, `ThemeColors`, `ThemeFonts`, `ThemeLayout`, `LayoutDirection` types
- [x] `WorkflowGraph.setTheme(theme)` method
- [x] `WorkflowGraph.setAutoResize(enabled)` method
- [x] `darkTheme` and `lightTheme` preset exports
- [x] JSDoc on all theme type fields with defaults

### React Component Hardening ✅
- [x] `forwardRef` + `useImperativeHandle` → `WorkflowGraphHandle` ref type
- [x] Imperative API: `selectNode`, `deselectAll`, `resetLayout`, `zoomToFit`, `setZoom`, `getNodePositions`, `setNodePositions`, `setTheme`
- [x] SSR guard (`typeof document === 'undefined'`)
- [x] `onError` callback prop with error state fallback UI
- [x] Reactive `theme` prop — updates theme when prop changes
- [x] Re-exports `darkTheme`, `lightTheme`, and all theme types

### Publish Metadata ✅
- [x] Workspace-level `license`, `repository`, `homepage`, `description` in root `Cargo.toml`
- [x] All 6 crates inherit workspace metadata (`edition.workspace`, `license.workspace`, `repository.workspace`)
- [x] Per-crate descriptions
- [x] npm packages: `repository`, `homepage`, `keywords`, `sideEffects`, `exports` fields
- [x] `@workflow-graph/{web,react,client}` all have proper package metadata

---

## Production Readiness — Full Customization & Quality ✅

### Publish CI ✅
- [x] `.github/workflows/publish.yml` — triggered on tag push (`v*`)
- [x] crates.io publishing in dependency order with 30s index-update delays
- [x] npm publishing with provenance (`--provenance --access public`)
- [x] WASM build step before npm publish
- [x] Node.js 22 + wasm-pack setup

### CHANGELOG & Semver ✅
- [x] `CHANGELOG.md` with Keep a Changelog format
- [x] Semver policy documented (patch/minor/major)
- [x] Pre-1.0 caveat: minor versions may include minor breaking changes
- [x] Initial release entry (v0.1.1)
- [x] Unreleased section with all new features

### High-Contrast Accessibility Theme ✅
- [x] `high_contrast_colors()` Rust preset (WCAG AA 4.5:1+ contrast ratios)
- [x] `highContrastTheme` TypeScript export
- [x] Black borders, pure blue highlight, strong color separation

### ARIA Live Regions ✅
- [x] Hidden `<div aria-live="polite" role="status">` injected next to canvas
- [x] Visually hidden (1px clip) but screen-reader accessible
- [x] Announces job status changes: "Unit Tests: Running. Build: Success."
- [x] Deduplication: same message not announced twice in a row
- [x] Cleaned up on `destroy()`

### Edge Click Handler ✅
- [x] Bezier curve hit testing via 21-point sampling (6px threshold)
- [x] Direction-aware: adapts for LTR and TTB layouts
- [x] `set_on_edge_click(canvas_id, callback)` WASM function
- [x] `onEdgeClick` in TypeScript `GraphOptions`
- [x] Fires `(fromId, toId)` callback on click
- [x] Priority: edge click checked before canvas click

### Per-Edge Style Customization ✅
- [x] `EdgeStyle` struct: `color`, `width`, `dash` pattern
- [x] `edge_styles` map in `ThemeConfig` keyed by `"from_id->to_id"`
- [x] Overrides applied per-edge during rendering
- [x] Dash pattern via Canvas `setLineDash()`, reset after each edge
- [x] Falls back to theme defaults for omitted fields

### Internationalization (i18n) ✅
- [x] `Labels` struct with status names + duration format strings
- [x] `labels` field in `ThemeConfig` and `ResolvedTheme`
- [x] Placeholder-based duration formatting: `{m}m {s}s`, `{s}s`
- [x] Labels used in renderer, a11y announcements
- [x] TypeScript `Labels` interface with JSDoc defaults

### Custom Node Rendering ✅
- [x] `set_on_render_node(canvas_id, callback)` WASM function
- [x] `onRenderNode` in TypeScript `GraphOptions`
- [x] Callback receives `(x, y, w, h, job)` — return `true` to skip default rendering
- [x] Called per-node during render loop via `render_with_callbacks`
- [x] Job data serialized to JS object via `serde_wasm_bindgen`

### Minimap Overlay ✅
- [x] `minimap: true` in `ThemeConfig` enables overlay
- [x] 160×100px semi-transparent panel in bottom-right corner
- [x] Nodes colored by status (success=green, failure=red, running=amber)
- [x] Viewport indicator rectangle showing current pan/zoom position
- [x] Scales to fit entire graph, rendered in screen space (not affected by zoom)

### Loading Skeleton (React) ✅
- [x] Animated gradient pulse placeholder shown during WASM initialization
- [x] `role="progressbar"` with `aria-label` for accessibility
- [x] `loadingSkeleton` prop for custom placeholder override
- [x] Canvas hidden (visibility:hidden, height:0) while loading
- [x] Skeleton removed once WASM renders successfully

### Integration Tests ✅
- [x] `test_full_workflow_lifecycle` — 8-job CI pipeline: start → claim → complete → cascade
- [x] `test_failure_cascading_integration` — root failure skips all downstream dependents
- [x] `test_cancel_workflow_integration` — cancel marks remaining jobs as cancelled
- [x] `test_concurrent_workers_no_double_claim` — 5 workers, 3 root jobs, no duplicates

### Performance Benchmarks ✅
- [x] `bench_enqueue_throughput` — 10/50/100/500 jobs, sub-1µs/op
- [x] `bench_claim_throughput` — 10/50/100/500 claims, sub-1µs/op
- [x] `bench_scheduler_cascade` — 10/50/100 node diamond workflows
- [x] `PERFORMANCE.md` — documented limits, recommendations, optimization tips
- [x] Test count: 38 total (19 unit + 4 integration + 3 performance + 12 YAML parsing)

---

## Production Hardening ✅

### NPM Build Pipeline ✅
- [x] `tsconfig.base.json` with `strict: true`, ES2020 target, declaration output
- [x] Per-package `tsconfig.json` extending base for `web`, `client`, `react`
- [x] `build` + `prepublishOnly` scripts in all package.json files
- [x] `main`/`module`/`types` pointing to `dist/` compiled output
- [x] `engines: { node: ">=18" }` on all packages
- [x] `exports` field with proper types/import/default conditions
- [x] React peer deps tightened: `^18.0.0 || ^19.0.0`

### TypeScript Strictness ✅
- [x] Remove all `any` types from `@workflow-graph/web`
- [x] `WasmModule` interface with typed WASM function signatures
- [x] `ensureWasm()` returns `Promise<WasmModule>` instead of `Promise<any>`
- [x] All `res.json()` calls cast with `as Promise<T>`

### Client Error Handling ✅
- [x] `WorkflowApiError` class with `status`, `statusText` fields
- [x] `assertOk()` helper checks response status on every API call
- [x] Error messages include response body for debugging
- [x] URL parameters encoded with `encodeURIComponent()`

### Touch/Mobile Support ✅
- [x] `touchstart` / `touchmove` / `touchend` event handlers
- [x] `touch-action: none` on canvas to prevent browser scroll/zoom
- [x] Touch drag (node repositioning) and pan (canvas movement)
- [x] Touch tap detection (click threshold) with node selection

### WASM Binary Optimization ✅
- [x] `[profile.release]` with `opt-level = "z"`, `lto = true`, `strip = true`, `codegen-units = 1`
- [x] `wasm-pack build --release` in Justfile

### License Fix ✅
- [x] LICENSE file replaced: Apache 2.0 → MIT (matching Cargo.toml)

### CORS Configuration ✅
- [x] `CORS_ORIGINS` env var for production origin whitelist
- [x] Comma-separated list of allowed origins
- [x] Falls back to permissive for development when unset

### Worker Graceful Shutdown ✅
- [x] `tokio::signal::ctrl_c()` handler in worker run loop
- [x] `tokio::select!` biased to check shutdown before polling
- [x] Finishes current job before exiting

### API Pagination ✅
- [x] `?limit=N&offset=M` query parameters on `GET /api/workflows`
- [x] `?status=running` filter by job status
- [x] Default limit 100, max 1000

### YAML Parser Edge Case Tests ✅
- [x] Job without `run` or `steps` returns error
- [x] Empty jobs map produces empty workflow
- [x] Single string dependency (`needs: a`)
- [x] Special characters in job IDs and names
- [x] Labels and retries parsed correctly
- [x] Environment variables in commands
- [x] JSON format parsing
- [x] Malformed YAML returns error
- [x] Shell quoting handles single quotes

### GitHub Repository Templates ✅
- [x] `.github/ISSUE_TEMPLATE/bug_report.md`
- [x] `.github/ISSUE_TEMPLATE/feature_request.md`
- [x] `.github/pull_request_template.md`

---

## Final Polish ✅

### Closure Memory Leak Fix ✅
- [x] `StoredListener` struct holds `js_sys::Function` ref + `Box<dyn Any>` for type-erased closure
- [x] `GraphInstance` holds `Vec<StoredListener>` alongside state
- [x] `attach_event_handlers` returns owned listeners via `add_listener!` macro
- [x] `destroy()` calls `removeEventListener` for all 9 handlers before dropping
- [x] No more `Closure::forget()` for canvas event listeners

### Retry Backoff Implementation ✅
- [x] `BackoffStrategy::delay_ms(attempt)` method: None=0, Fixed=constant, Exponential=2^n capped
- [x] `QueuedJob::delayed_until_ms` field — jobs not claimable before this timestamp
- [x] `claim()` skips jobs where `delayed_until_ms > now`
- [x] `fail()` applies backoff delay when re-enqueueing retried jobs
- [x] `reap_expired_leases()` also applies backoff on re-enqueue

### DPR Change Listener ✅
- [x] `matchMedia("(resolution: Xdppx)")` listener in `render_workflow`
- [x] Updates `dpr` and redraws when display resolution changes
- [x] Handles multi-monitor scenarios (window moved between displays)

### Docs Site Updated ✅
- [x] WASM API doc rewritten with all new functions
- [x] Theme config table with all fields
- [x] i18n labels example (Spanish)
- [x] Edge styles example
- [x] Minimap section
- [x] Custom node rendering section
- [x] Edge click section
- [x] Auto resize section
- [x] React component example with ref, theme, onError
- [x] Interaction features list updated (touch, keyboard, a11y, HiDPI)

### Infrastructure ✅
- [x] `packages/*/dist/` and `packages/web/wasm/` in `.gitignore`
- [x] `.github/dependabot.yml` — Cargo weekly, GitHub Actions weekly, npm monthly
- [x] Test count: 38 total (19 unit + 4 integration + 3 performance + 12 YAML parsing)

### WASM Binary Distribution Fix ✅
- [x] WASM JS glue + `.wasm` binary copied into `packages/web/wasm/` during build
- [x] `copy-wasm` script in package.json copies from `crates/web/pkg/`
- [x] Import path changed from monorepo-relative `../../crates/web/pkg/` to `../wasm/`
- [x] `wasm/` added to `files` array — ships with npm package
- [x] `setWasmUrl(url)` export for consumers who host WASM on a CDN or custom path
- [x] `wasm.d.ts` type declarations for the dynamic import
- [x] `typescript: ^5.0.0` devDependency on all 3 packages
- [x] `just build-packages` recipe chains WASM build → TS build for all packages
- [x] Publish CI updated: builds WASM, then builds each TS package before publishing
- [x] `exports` map includes `./wasm/*` for direct WASM file access
