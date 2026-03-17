---
title: Embedding in Your Application
description: Use workflow-graph as a library in your Axum server, React app, or vanilla JS project
---

## Frontend: NPM Packages

### React

```bash
npm install @workflow-graph/react @workflow-graph/web
```

```tsx
import { useRef } from 'react';
import {
  WorkflowGraphComponent,
  darkTheme,
  highContrastTheme,
} from '@workflow-graph/react';
import type { WorkflowGraphHandle } from '@workflow-graph/react';

function App() {
  const ref = useRef<WorkflowGraphHandle>(null);

  return (
    <>
      <WorkflowGraphComponent
        ref={ref}
        workflow={workflowData}
        theme={darkTheme}
        autoResize
        onNodeClick={(id) => console.log('clicked', id)}
        onEdgeClick={(from, to) => console.log('edge', from, to)}
        onError={(err) => console.error('Graph error:', err)}
        // Custom loading skeleton (optional)
        loadingSkeleton={<div>Loading graph...</div>}
      />

      <button onClick={() => ref.current?.zoomToFit()}>Zoom to Fit</button>
      <button onClick={() => ref.current?.setTheme(highContrastTheme)}>
        High Contrast
      </button>
    </>
  );
}
```

#### Props

| Prop | Type | Description |
|------|------|-------------|
| `workflow` | `Workflow` | The workflow data to render |
| `theme` | `ThemeConfig` | Theme configuration (colors, fonts, layout, direction, labels, minimap) |
| `autoResize` | `boolean` | Auto-resize canvas on container resize |
| `onNodeClick` | `(jobId: string) => void` | Node click callback |
| `onNodeHover` | `(jobId: string \| null) => void` | Node hover callback |
| `onCanvasClick` | `() => void` | Empty space click callback |
| `onSelectionChange` | `(ids: string[]) => void` | Selection change callback |
| `onNodeDragEnd` | `(jobId: string, x: number, y: number) => void` | Node drag end callback |
| `onEdgeClick` | `(fromId: string, toId: string) => void` | Edge click callback |
| `onRenderNode` | `(x, y, w, h, job) => boolean` | Custom node rendering |
| `onError` | `(error: Error) => void` | Error callback |
| `loadingSkeleton` | `ReactNode` | Custom loading placeholder |
| `className` | `string` | Container class name |
| `style` | `CSSProperties` | Container inline styles |

#### Imperative Handle (`ref`)

| Method | Description |
|--------|-------------|
| `selectNode(jobId)` | Select a node programmatically |
| `deselectAll()` | Clear selection |
| `resetLayout()` | Reset to auto-computed layout |
| `zoomToFit()` | Fit graph in view |
| `setZoom(level)` | Set zoom (0.25–4.0) |
| `getNodePositions()` | Get positions for persistence |
| `setNodePositions(positions)` | Restore saved positions |
| `setTheme(theme)` | Switch theme at runtime |
| `instance` | Access underlying `WorkflowGraph` |

#### SSR Compatibility

The component guards against server-side rendering — it checks `typeof document !== 'undefined'` before initializing WASM. Works with Next.js, Remix, etc. without `dynamic(() => import(...), { ssr: false })`.

### Vanilla TypeScript / JavaScript

```bash
npm install @workflow-graph/web
```

```typescript
import { WorkflowGraph, darkTheme, setWasmUrl } from '@workflow-graph/web';

// Optional: host WASM binary on a CDN
// setWasmUrl('https://cdn.example.com/wasm/workflow_graph_web_bg.wasm');

const graph = new WorkflowGraph(document.getElementById('container')!, {
  onNodeClick: (jobId) => showJobDetails(jobId),
  onEdgeClick: (from, to) => highlightDependency(from, to),
  theme: {
    ...darkTheme,
    minimap: true,
    direction: 'TopToBottom',
    labels: {
      running: 'Ejecutando',  // Spanish i18n
      success: 'Completado',
    },
    edge_styles: {
      'build->deploy': { color: '#ff6b6b', width: 3, dash: [6, 3] },
    },
  },
  autoResize: true,
});

await graph.setWorkflow(workflowData);

// Poll for status updates
setInterval(async () => {
  const updated = await fetch('/api/workflows/ci-1/status').then(r => r.json());
  graph.updateStatus(updated);
}, 1000);
```

---

## Backend: Rust Server Embedding

The server crate exposes `create_router()` so you can embed the workflow engine in your own Axum application.

### Setup

```toml
[dependencies]
workflow-graph-server = { path = "crates/server" }
workflow-graph-queue = { path = "crates/queue" }
tokio = { version = "1", features = ["full"] }
axum = "0.8"
```

### Basic Example

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

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

### Merging with Your Own Routes

```rust
use axum::Router;

let workflow_router = workflow_graph_server::create_router(app_state);

let app = Router::new()
    .nest("/workflows", workflow_router)
    .route("/health", axum::routing::get(|| async { "ok" }));
```

### API-Only Mode (No Scheduler)

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

### Custom Queue Backends

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
