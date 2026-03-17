---
title: Workers Overview
description: How workers execute jobs in workflow-graph
---

Workers are external processes that poll the workflow-graph server for jobs, execute them, and report results. They communicate over HTTP, so you can write workers in **any language**.

## Three Ways to Run a Worker

1. **Standalone binary** — run the pre-built Rust worker with env vars
2. **Embedded SDK** — use the `workflow-graph-worker-sdk` crate as a library in your own Rust binary
3. **Custom HTTP client** — implement the worker protocol in any language

## Standalone Binary

The simplest way to run a worker:

```bash
cargo run -p workflow-graph-worker-sdk
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SERVER_URL` | `http://localhost:3000` | Server base URL |
| `WORKER_LABELS` | (empty) | Comma-separated capability labels |

```bash
SERVER_URL=http://my-server:3000 \
WORKER_LABELS=docker,linux,gpu \
cargo run -p workflow-graph-worker-sdk
```

The standalone binary:
- Executes the job's `command` field via `sh -c`
- Streams stdout/stderr as log chunks
- Sends heartbeats automatically
- Checks for cancellation

**Limitation:** The standalone binary only runs shell commands. For Docker containers, API calls, or structured outputs, use the [Worker SDK](/workflow-graph/workers/sdk/) or a [custom worker](/workflow-graph/workers/custom-workers/).

## Worker Lifecycle

```
Register → Poll for jobs → Claim job → Execute
                ↑              │
                │              ├─ Send heartbeats (concurrent)
                │              ├─ Stream logs (concurrent)
                │              ├─ Check cancellation (concurrent)
                │              │
                │              ▼
                └──── Report result (success/failure)
```

1. **Register** with the server, declaring capability labels
2. **Poll** for available jobs matching your labels
3. **Claim** a job atomically (lease-based, prevents double-claiming)
4. **Execute** the job command while concurrently:
   - Sending heartbeats to keep the lease alive
   - Streaming log output
   - Checking for cancellation
5. **Report** success (with optional outputs) or failure (with retry hint)
6. Loop back to step 2
