# workflow-graph

A GitHub Actions-style workflow DAG visualizer and job execution engine, built with Rust + WebAssembly.

Render interactive workflow graphs in the browser with pixel-perfect GitHub Octicon icons, drag-and-drop nodes, pan & zoom, path highlighting, and real-time job status updates. Execute workflows via a pluggable queue system with external workers.

[![crates.io](https://img.shields.io/crates/v/workflow-graph-shared)](https://crates.io/crates/workflow-graph-shared)
[![npm](https://img.shields.io/npm/v/@auser/workflow-graph-web)](https://www.npmjs.com/package/@auser/workflow-graph-web)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Features

### Visualization (WASM)
- **GitHub-accurate DAG rendering** — Octicon SVG icons via Canvas Path2D
- **Interactive nodes** — drag to reposition, click to select, shift+click for multi-select
- **Pan & zoom** — mouse wheel zoom (0.25x–4x), click+drag empty space to pan
- **Touch support** — full touch interaction: drag, pan, tap on mobile devices
- **Path highlighting** — hover a node to highlight its full upstream/downstream path in blue
- **Animated status icons** — spinning ring for running jobs, live elapsed timer
- **Theming** — light, dark, and high-contrast presets; fully customizable colors, fonts, and dimensions
- **Layout direction** — left-to-right (default) or top-to-bottom
- **Minimap** — optional overview overlay for large graphs
- **Custom node rendering** — callback to draw custom node content
- **Edge click & styles** — clickable edges with per-edge color, width, and dash patterns
- **i18n** — configurable status labels and duration formats for any language
- **Accessibility** — ARIA live region for status announcements, keyboard navigation, high-contrast theme
- **Auto-resize** — ResizeObserver adapts canvas to container size changes
- **HiDPI** — adapts to devicePixelRatio changes (multi-monitor)

### Job Execution (Server + Workers)
- **Pluggable queue** — trait-based: in-memory (dev), Postgres/pg-boss, Redis
- **DAG scheduler** — event-driven cascade: downstream jobs auto-start when deps succeed
- **External workers** — poll for jobs via HTTP, execute shell commands, stream logs
- **Atomic job claiming** — lease-based with TTL, prevents double-claiming
- **Heartbeats** — workers send periodic heartbeats; expired leases trigger re-queue
- **Retry policy** — configurable per-job retries with Fixed or Exponential backoff
- **Cancellation** — cancel workflows/jobs; workers detect and abort gracefully
- **Graceful shutdown** — workers handle SIGTERM/SIGINT, finish current job before exit
- **Worker labels** — jobs require labels, workers register capabilities
- **Log streaming** — workers push log chunks, server stores and serves them
- **Artifact outputs** — jobs publish key-value outputs, downstream jobs read them

### Library Design
- **6 crates** — shared types, queue engine, WASM frontend, worker SDK, reference server, standalone scheduler
- **3 npm packages** — `@auser/workflow-graph-web`, `@auser/workflow-graph-react`, `@auser/workflow-graph-client`
- **Trait-based backends** — `JobQueue`, `ArtifactStore`, `LogSink`, `WorkerRegistry`
- **YAML/JSON workflows** — GitHub Actions-inspired definition format
- **Embeddable** — use `create_router()` to embed the API in your own Axum server
- **Edge-deployable** — API server is stateless; run scheduler separately or in-process
- **38 tests** — unit, integration, performance, and YAML parsing

## Architecture

```
                     All-in-one (dev)          Split (edge/prod)
                     ────────────────          ─────────────────
┌──────────┐  poll   ┌──────────────┐         ┌───────────┐  ┌───────────┐
│  Worker   │◄──────►│   Server     │         │ API Server│  │ Scheduler │
│  (SDK)    │  HTTP  │ API+Scheduler│         │ (stateless│  │ (separate │
└──────────┘        └──────┬───────┘         │  process) │  │  process) │
                           │                  └─────┬─────┘  └─────┬─────┘
                      ┌────┴────┐                   │              │
                      │JobQueue │◄──────────────────┴──────────────┘
                      │LogSink  │
                      │Artifacts│
                      └─────────┘
```

## Documentation

Full documentation is available at **[auser.github.io/workflow-graph](https://auser.github.io/workflow-graph/)**.

| Guide | Description |
|-------|-------------|
| [Getting Started](https://auser.github.io/workflow-graph/getting-started/installation/) | Installation, prerequisites, quick start |
| [Creating Workers](https://auser.github.io/workflow-graph/guides/creating-workers/) | Build workers in Rust, Python, TypeScript, or Go |
| [Worker SDK](https://auser.github.io/workflow-graph/workers/sdk/) | Embed the Rust worker SDK in your own binary |
| [Worker Protocol](https://auser.github.io/workflow-graph/workers/custom-workers/) | HTTP protocol reference for any-language workers |
| [Postgres Backend](https://auser.github.io/workflow-graph/guides/postgres-backend/) | pg-boss-style atomic claiming with sqlx |
| [Redis Backend](https://auser.github.io/workflow-graph/guides/redis-backend/) | Lua-scripted atomic claiming and Pub/Sub events |
| [Custom Queue Backend](https://auser.github.io/workflow-graph/guides/custom-queue/) | Implement the four queue traits for your own storage |
| [Embedding](https://auser.github.io/workflow-graph/guides/embedding/) | Use `create_router()` in your own Axum server |
| [Deployment Modes](https://auser.github.io/workflow-graph/architecture/deployment-modes/) | All-in-one vs split (edge/serverless) |
| [REST API](https://auser.github.io/workflow-graph/api/rest-api/) | Full HTTP API reference |
| [WASM API](https://auser.github.io/workflow-graph/api/wasm-api/) | JavaScript API for the canvas renderer |

## NPM Packages

```bash
npm install @auser/workflow-graph-web      # WASM + Canvas renderer
npm install @auser/workflow-graph-react    # React component
npm install @auser/workflow-graph-client   # REST API client
```

### TypeScript / Vanilla JS

```typescript
import { WorkflowGraph, darkTheme, setWasmUrl } from '@auser/workflow-graph-web';

// Optional: set custom WASM URL if hosting separately
// setWasmUrl('https://cdn.example.com/wasm/workflow_graph_web_bg.wasm');

const graph = new WorkflowGraph(document.getElementById('container')!, {
  onNodeClick: (jobId) => console.log('clicked', jobId),
  onEdgeClick: (from, to) => console.log('edge', from, to),
  theme: darkTheme,
  autoResize: true,
});
await graph.setWorkflow(workflowData);

// Update statuses (preserves positions, zoom, selection)
await graph.updateStatus(newWorkflowData);

// Runtime theme switching
await graph.setTheme({ minimap: true, direction: 'TopToBottom' });
```

### React

```tsx
import { WorkflowGraphComponent, darkTheme } from '@auser/workflow-graph-react';
import type { WorkflowGraphHandle } from '@auser/workflow-graph-react';

const ref = useRef<WorkflowGraphHandle>(null);

<WorkflowGraphComponent
  ref={ref}
  workflow={workflowData}
  theme={darkTheme}
  autoResize
  onNodeClick={(id) => console.log(id)}
  onError={(err) => console.error(err)}
/>

// Imperative control via ref
ref.current?.zoomToFit();
ref.current?.setTheme({ minimap: true });
```

### REST API Client

```typescript
import { WorkflowClient } from '@auser/workflow-graph-client';

const client = new WorkflowClient('http://localhost:3000');
const workflows = await client.listWorkflows();
await client.runWorkflow(workflows[0].id);

// Stream logs
for await (const chunk of client.streamLogs(wfId, jobId)) {
  console.log(chunk.data);
}
```

## Examples

Working examples for each package are in the [`examples/`](examples/) directory:

| Example | Package | Description |
|---------|---------|-------------|
| [vanilla-web](examples/vanilla-web/) | `@auser/workflow-graph-web` | Plain HTML + ES modules with theme switching, minimap, direction toggle |
| [react-app](examples/react-app/) | `@auser/workflow-graph-react` | React app with ref API, theme switching, custom edge styles, error handling |
| [client-polling](examples/client-polling/) | `@auser/workflow-graph-client` | Node.js script that runs a workflow and polls for status with live output |

## Theming

Three built-in presets:

```typescript
import { darkTheme, lightTheme, highContrastTheme } from '@auser/workflow-graph-web';
```

Full customization via `ThemeConfig`:

```typescript
const theme: ThemeConfig = {
  colors: { bg: '#1a1a2e', node_bg: '#16213e', text: '#e0e0e0' },
  fonts: { family: 'JetBrains Mono, monospace', size_name: 12 },
  layout: { node_width: 220, node_height: 48, h_gap: 80 },
  direction: 'TopToBottom',
  labels: { running: 'En curso', success: 'Listo' },  // i18n
  edge_styles: { 'build->deploy': { color: '#ff0', dash: [5, 3] } },
  minimap: true,
};
```

## Crate Structure

| Crate | Purpose |
|-------|---------|
| `workflow-graph-shared` | Core types: `Job`, `Workflow`, `JobStatus`, YAML parser |
| `workflow-graph-queue` | Queue traits + in-memory implementations, DagScheduler |
| `workflow-graph-web` | WASM Canvas renderer with interactive graph |
| `workflow-graph-worker-sdk` | Worker binary + embeddable library |
| `workflow-graph-server` | Reference Axum server (stateless API, embeddable) |
| `workflow-graph-scheduler` | Standalone scheduler binary for split deployments |

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
just build-wasm      # Build WASM (release, optimized)
just serve           # Start server (auto-finds port if 3000 is taken)
just build-packages  # Build all TypeScript packages

# Development with auto-reload:
just watch            # cargo-watch restarts server on changes
```

Open `http://localhost:3000/index.html` and click **Run workflow**.

### Run a Worker (separate terminal)

```bash
cargo run -p workflow-graph-worker-sdk
# Or with custom server URL and labels:
SERVER_URL=http://localhost:3000 WORKER_LABELS=docker,linux cargo run -p workflow-graph-worker-sdk
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

## REST API

### Workflow Management

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/workflows` | List workflows (`?limit=N&offset=M&status=running`) |
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

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3000` | Server port (auto-finds next available if taken) |
| `API_ONLY` | unset | Set to `1` or `true` for API-only mode (no scheduler) |
| `WORKFLOWS_DIR` | `workflows/` | Directory to load workflow YAML/JSON files from |
| `CORS_ORIGINS` | unset | Comma-separated allowed origins (permissive if unset) |
| `REAP_INTERVAL_SECS` | `5` | Lease reaper interval (standalone scheduler) |
| `SERVER_URL` | `http://localhost:3000` | Worker SDK: API server address |
| `WORKER_LABELS` | unset | Worker SDK: comma-separated capabilities |

## Deployment Modes

### All-in-one (default)

Runs the API server, DAG scheduler, and lease reaper in a single process. Best for development and simple deployments.

```bash
cargo run -p workflow-graph-server
```

### Split (edge/serverless)

Runs the API server without the scheduler — suitable for edge platforms where functions are request-scoped.

```bash
# Terminal 1: API server (stateless, edge-deployable)
API_ONLY=1 cargo run -p workflow-graph-server

# Terminal 2: Standalone scheduler (long-running)
cargo run -p workflow-graph-scheduler
```

## Embedding in Your Server

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
    let scheduler = Arc::new(DagScheduler::new(queue.clone(), artifacts.clone(), state.clone()));
    tokio::spawn(scheduler.clone().run());

    // Build your router with the stateless API
    let app = workflow_graph_server::create_router(AppState {
        workflow_state: state,
        queue, artifacts, logs, workers,
    });

    axum::serve(listener, app).await.unwrap();
}
```

## Performance

| Scenario | Recommended Max | Notes |
|----------|----------------|-------|
| Interactive editing (60fps) | 100 nodes | Canvas2D redraws entire scene |
| Static display (no animation) | 500 nodes | No animation loop overhead |
| With minimap enabled | 200 nodes | Minimap adds second render pass |
| Queue throughput (in-memory) | < 1µs/op | Enqueue and claim operations |

See [PERFORMANCE.md](PERFORMANCE.md) for benchmarks and optimization tips.

## Testing

```bash
just test              # Run all tests (38 tests)
just check             # Type-check workspace
cargo test -p workflow-graph-queue                    # Queue + scheduler unit tests
cargo test --test integration -p workflow-graph-queue # Integration tests
cargo test --test performance -p workflow-graph-queue -- --nocapture # Benchmarks
```

## License

MIT
