---
title: Embedding in Your Application
description: Use workflow-graph as a library in your Axum server, React app, or vanilla JS project
---

:::tip[Working Examples]
Complete runnable examples for each package:
- [**vanilla-web**](https://github.com/auser/workflow-graph/tree/main/examples/vanilla-web) — Plain HTML + Vite, theme switching, minimap, direction toggle
- [**react-app**](https://github.com/auser/workflow-graph/tree/main/examples/react-app) — Vite + React with ref API, custom edge styles, error handling
- [**client-polling**](https://github.com/auser/workflow-graph/tree/main/examples/client-polling) — Node.js script that runs a workflow and polls status

Each example runs with `npm install && npm run dev` (or `npm start`).
:::

## Frontend: NPM Packages

### React

```bash
npm install @auser/workflow-graph-react @auser/workflow-graph-web
```

```tsx
import { useRef, useState } from 'react';
import { setWasmUrl } from '@auser/workflow-graph-web';
import {
  WorkflowGraphComponent,
  darkTheme,
  highContrastTheme,
} from '@auser/workflow-graph-react';
import type { WorkflowGraphHandle, ThemeConfig } from '@auser/workflow-graph-react';

// If using Vite, serve the .wasm file from public/ and set the URL:
setWasmUrl('/workflow_graph_web_bg.wasm');

function App() {
  const ref = useRef<WorkflowGraphHandle>(null);
  const [minimap, setMinimap] = useState(false);

  const theme: ThemeConfig = { ...darkTheme, minimap };

  return (
    <>
      <WorkflowGraphComponent
        ref={ref}
        workflow={workflowData}
        theme={theme}
        autoResize
        onNodeClick={(id) => console.log('clicked', id)}
        onEdgeClick={(from, to) => console.log('edge', from, to)}
        onError={(err) => console.error('Graph error:', err)}
      />

      <button onClick={() => ref.current?.zoomToFit()}>Zoom to Fit</button>
      <button onClick={() => ref.current?.setTheme(highContrastTheme)}>
        High Contrast
      </button>
      <button onClick={() => setMinimap(m => !m)}>Toggle Minimap</button>
    </>
  );
}
```

#### WASM Setup with Vite

When using Vite, the WASM binary needs to be served as a static asset. Copy it to your `public/` directory or configure `publicDir`:

```ts
// vite.config.ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  // Option 1: copy workflow_graph_web_bg.wasm to public/
  // Option 2: point publicDir at the wasm directory
});
```

Then call `setWasmUrl()` before rendering any graph components:

```ts
import { setWasmUrl } from '@auser/workflow-graph-web';
setWasmUrl('/workflow_graph_web_bg.wasm');
```

:::note
Without `setWasmUrl()`, the WASM loader uses `import.meta.url` to resolve the binary path. This works in most bundlers but may fail in Vite dev mode. Always set the URL explicitly for reliable behavior.
:::

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
npm install @auser/workflow-graph-web
```

```typescript
import { WorkflowGraph, darkTheme, setWasmUrl } from '@auser/workflow-graph-web';

// Set WASM URL (required for Vite, recommended for all bundlers)
setWasmUrl('/workflow_graph_web_bg.wasm');

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

### REST API Client (Node.js / Server-side)

```bash
npm install @auser/workflow-graph-client
```

```typescript
import { WorkflowClient, WorkflowApiError } from '@auser/workflow-graph-client';

const client = new WorkflowClient('http://localhost:4000');

// List and run workflows
const workflows = await client.listWorkflows();
await client.runWorkflow(workflows[0].id);

// Poll for status
const status = await client.getStatus(workflows[0].id);

// Stream logs
for await (const chunk of client.streamLogs(wfId, jobId)) {
  process.stdout.write(chunk.data);
}

// Error handling
try {
  await client.getStatus('nonexistent');
} catch (err) {
  if (err instanceof WorkflowApiError) {
    console.error(`API ${err.status}: ${err.message}`);
  }
}
```

The client requires a running server and worker:

```bash
# Terminal 1: Server
PORT=4000 just dev

# Terminal 2: Worker
SERVER_URL=http://localhost:4000 cargo run -p workflow-graph-worker-sdk

# Terminal 3: Client script
cd examples/client-polling && npm install && npm start
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
