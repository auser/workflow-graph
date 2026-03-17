---
title: Deployment Modes
description: All-in-one vs split deployment for edge and serverless
---

The server supports two deployment modes, controlled by the `API_ONLY` environment variable.

## All-in-One (Default)

Runs the API server, DAG scheduler, and lease reaper in a single process. Best for development and simple deployments.

```bash
cargo run -p workflow-graph-server
```

This mode starts:
- HTTP API on the configured port (default 3000)
- DAG scheduler event loop (subscribes to queue events, cascades jobs)
- Lease reaper (periodically reclaims expired leases)

## Split (Edge / Serverless)

Runs the API server without the scheduler — suitable for edge platforms (Vercel Workers, Cloudflare Workers, Supabase Edge Functions) where functions are request-scoped.

```bash
# Terminal 1: API server (stateless, edge-deployable)
API_ONLY=1 cargo run -p workflow-graph-server

# Terminal 2: Standalone scheduler (long-running)
cargo run -p workflow-graph-scheduler
```

### Why Split?

Edge platforms typically:
- Spin up a new process per request
- Have no persistent background tasks
- Run close to users for low latency

The API server is stateless — it reads/writes to the queue backend and returns. The scheduler needs to run continuously to cascade the DAG, so it runs as a separate long-lived process.

### Scheduler Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `REAP_INTERVAL_SECS` | `5` | How often the lease reaper checks for expired leases |

### Architecture in Split Mode

```
┌─────────────┐     ┌─────────────┐
│ Edge Worker  │────►│  API Server │──┐
│ (request)    │     │ (stateless) │  │
└─────────────┘     └─────────────┘  │
                                      ▼
┌─────────────┐     ┌─────────────┐  ┌──────────┐
│   Worker     │────►│  Job Queue  │◄─│Scheduler │
│ (polls jobs) │     │  (Postgres) │  │(separate)│
└─────────────┘     └─────────────┘  └──────────┘
```

Both the API server and scheduler connect to the same queue backend. The scheduler subscribes to queue events (via `broadcast` channels or Postgres `LISTEN/NOTIFY`) and cascades downstream jobs.
