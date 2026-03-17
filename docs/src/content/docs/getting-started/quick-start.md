---
title: Quick Start
description: Get workflow-graph running in under a minute
---

## Start the Server

```bash
just dev
```

This builds the WASM frontend and starts the server. Open `http://localhost:3000/index.html` and click **Run workflow** to see the demo.

## Run a Worker

In a separate terminal:

```bash
cargo run -p workflow-graph-worker-sdk
```

The worker registers with the server, polls for jobs, and executes them. You'll see jobs transition from queued to running to success in the browser.

### Worker with Labels

Workers can declare capabilities via labels:

```bash
SERVER_URL=http://localhost:3000 \
WORKER_LABELS=docker,linux \
cargo run -p workflow-graph-worker-sdk
```

Jobs with matching `labels` in the workflow YAML will only be claimed by workers whose labels are a superset.

## Interact via API

```bash
# List workflows
curl -s http://localhost:3000/api/workflows | python3 -m json.tool

# Run a workflow
curl -s -X POST http://localhost:3000/api/workflows/ci-1/run

# Check status
curl -s http://localhost:3000/api/workflows/ci-1/status | python3 -m json.tool
```

## Use via NPM

If you're building a web app, install the published packages:

```bash
npm install @auser/workflow-graph-web    # Vanilla JS/TS
npm install @auser/workflow-graph-react  # React component
```

```typescript
import { WorkflowGraph, darkTheme, setWasmUrl } from '@auser/workflow-graph-web';

// Serve the .wasm file from public/ and set the URL
setWasmUrl('/workflow_graph_web_bg.wasm');

const graph = new WorkflowGraph(document.getElementById('container')!, {
  theme: darkTheme,
  autoResize: true,
});
await graph.setWorkflow(workflowData);
```

See the [WASM API reference](/workflow-graph/api/wasm-api/) for the full API, the [embedding guide](/workflow-graph/guides/embedding/) for React examples, and the [examples directory](https://github.com/auser/workflow-graph/tree/main/examples) for complete working apps.

## Run Tests

```bash
just test              # Run all tests (38 tests)
just check             # Type-check workspace
cargo test -p workflow-graph-queue   # Queue + scheduler tests only
```
