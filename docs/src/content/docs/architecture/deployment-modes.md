---
title: Deployment Modes
description: All-in-one vs split deployment for edge and serverless platforms
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

Runs the API server without the scheduler — suitable for edge platforms where functions are request-scoped and cannot run background tasks.

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

---

## Deploying to Edge Platforms

The split architecture lets you deploy the stateless API server to edge platforms while running the scheduler as a separate long-lived process (on a VM, container, or managed service).

All edge deployments share the same pattern:

1. **API server** → deploy to the edge platform (stateless, request-scoped)
2. **Scheduler** → run separately on a VM, container, or always-on service
3. **Queue backend** → use a shared Postgres or Redis instance (not in-memory)

:::caution
The in-memory queue backend does not work in split mode — each process would have its own isolated queue. Use [Postgres](/workflow-graph/guides/postgres-backend/) or [Redis](/workflow-graph/guides/redis-backend/) for production split deployments.
:::

### Prerequisites for All Edge Deployments

Before deploying to any edge platform, you need:

1. A **Postgres or Redis** instance accessible from both the edge platform and the scheduler
2. A custom binary that uses `create_router()` with your chosen backend (see [Embedding](/workflow-graph/guides/embedding/))
3. The scheduler running somewhere with persistent compute

```rust
// shared setup for all edge deployments
use std::sync::Arc;
use workflow_graph_server::state::AppState;

fn build_app_state(pool: PgPool) -> AppState {
    let backend = Arc::new(PgBackend::new(pool));
    let state = Arc::new(RwLock::new(WorkflowState::new()));

    AppState {
        workflow_state: state,
        queue: backend.clone(),
        artifacts: backend.clone(),
        logs: backend.clone(),
        workers: backend.clone(),
    }
}
```

---

### Vercel (Serverless Functions)

Vercel runs each request as an isolated serverless function. Deploy the API server as a Rust-based serverless function using Vercel's [Rust runtime](https://github.com/vercel-community/rust).

#### Project Structure

```
vercel-workflow-api/
├── api/
│   └── handler.rs        # Serverless function entry point
├── Cargo.toml
├── vercel.json
```

#### `vercel.json`

```json
{
  "functions": {
    "api/**/*.rs": {
      "runtime": "vercel-rust@latest"
    }
  },
  "rewrites": [
    { "source": "/api/(.*)", "destination": "/api/handler" }
  ]
}
```

#### `api/handler.rs`

```rust
use vercel_runtime::{run, Body, Error, Request, Response};
use workflow_graph_server::create_router;

// Initialize the router once (reused across warm invocations)
static APP: once_cell::sync::Lazy<axum::Router> = once_cell::sync::Lazy::new(|| {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    let pool = /* connect to Postgres */;
    let app_state = build_app_state(pool);
    create_router(app_state)
});

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(handler).await
}

async fn handler(req: Request) -> Result<Response<Body>, Error> {
    // Convert Vercel request → Axum request → Vercel response
    // Use tower::ServiceExt::oneshot to handle the request
    let response = APP.clone().oneshot(req.into()).await?;
    Ok(response.into())
}
```

#### Environment Variables

Set these in your Vercel project settings:

| Variable | Value |
|----------|-------|
| `DATABASE_URL` | `postgres://user:pass@host:5432/workflow_graph` |
| `API_ONLY` | `1` |

#### Deploy

```bash
vercel deploy
```

The scheduler must run separately — use a VM, a container on [Railway](https://railway.app), or [Fly.io](https://fly.io):

```bash
# On your always-on server
DATABASE_URL=postgres://... cargo run -p workflow-graph-scheduler
```

---

### Cloudflare Workers

Cloudflare Workers run on the V8 runtime at the edge. Since workflow-graph is a Rust/Axum server, you compile it to WebAssembly using [`workers-rs`](https://github.com/cloudflare/workers-rs).

#### Project Structure

```
cf-workflow-api/
├── src/
│   └── lib.rs            # Worker entry point
├── Cargo.toml
├── wrangler.toml
```

#### `wrangler.toml`

```toml
name = "workflow-graph-api"
main = "build/worker/shim.mjs"
compatibility_date = "2024-01-01"

[vars]
API_ONLY = "1"

# Bind to a Hyperdrive (Postgres connection pooler)
[[hyperdrive]]
binding = "DB"
id = "<your-hyperdrive-id>"
```

#### `src/lib.rs`

```rust
use worker::*;
use workflow_graph_server::create_router;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // Get the Postgres connection string from Hyperdrive
    let db_url = env.hyperdrive("DB")?.connection_string();
    let pool = /* connect to Postgres via db_url */;
    let app_state = build_app_state(pool);
    let router = create_router(app_state);

    // Convert Worker request → Axum request
    let axum_req = convert_request(req).await?;
    let axum_resp = router.oneshot(axum_req).await.map_err(|e| e.to_string())?;

    // Convert Axum response → Worker response
    convert_response(axum_resp).await
}
```

#### Cloudflare Hyperdrive

Cloudflare Workers can't hold persistent TCP connections, so use [Hyperdrive](https://developers.cloudflare.com/hyperdrive/) for Postgres connection pooling:

```bash
# Create a Hyperdrive config pointing to your Postgres
wrangler hyperdrive create workflow-db \
  --connection-string="postgres://user:pass@host:5432/workflow_graph"
```

#### Deploy

```bash
wrangler deploy
```

#### Scheduler

Run the scheduler on a Cloudflare Worker with [Cron Triggers](https://developers.cloudflare.com/workers/configuration/cron-triggers/) for lease reaping, or run it on a separate always-on server:

```bash
DATABASE_URL=postgres://... cargo run -p workflow-graph-scheduler
```

---

### Supabase Edge Functions

Supabase Edge Functions run on Deno Deploy at the edge. You can call the workflow-graph API from a Supabase Edge Function that proxies requests to your Rust API server, or implement a thin TypeScript wrapper that talks directly to Supabase's Postgres instance.

#### Option A: TypeScript Wrapper with Direct Postgres Access

Since Supabase Edge Functions run on Deno and have direct access to the project's Postgres database, you can use the `@workflow-graph/client` TypeScript package or make direct SQL calls.

##### `supabase/functions/workflow-api/index.ts`

```typescript
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";
import { serve } from "https://deno.land/std/http/server.ts";

const supabase = createClient(
  Deno.env.get("SUPABASE_URL")!,
  Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!,
);

serve(async (req: Request) => {
  const url = new URL(req.url);
  const path = url.pathname;

  // Route: POST /claim — atomic job claiming via Postgres
  if (path === "/claim" && req.method === "POST") {
    const { worker_id, labels, lease_ttl_secs } = await req.json();
    const { data, error } = await supabase.rpc("wfg_claim_job", {
      p_worker_id: worker_id,
      p_labels: labels,
      p_ttl_secs: lease_ttl_secs,
    });

    if (error) return new Response(JSON.stringify({ error }), { status: 500 });
    return new Response(JSON.stringify(data));
  }

  // Route: POST /complete
  if (path === "/complete" && req.method === "POST") {
    const { lease_id, outputs } = await req.json();
    const { error } = await supabase.rpc("wfg_complete_job", {
      p_lease_id: lease_id,
      p_outputs: outputs,
    });

    if (error) return new Response(JSON.stringify({ error }), { status: 500 });
    return new Response(JSON.stringify({ ok: true }));
  }

  return new Response("Not Found", { status: 404 });
});
```

##### Postgres Functions

Create RPC functions in Supabase that wrap the `FOR UPDATE SKIP LOCKED` claiming logic:

```sql
-- In Supabase SQL Editor
CREATE OR REPLACE FUNCTION wfg_claim_job(
  p_worker_id TEXT,
  p_labels JSONB,
  p_ttl_secs INTEGER
)
RETURNS JSONB AS $$
DECLARE
  v_job RECORD;
  v_lease_id TEXT := gen_random_uuid()::TEXT;
BEGIN
  SELECT * INTO v_job
  FROM wfg_jobs
  WHERE state = 'pending'
    AND p_labels @> required_labels
  ORDER BY enqueued_at ASC
  LIMIT 1
  FOR UPDATE SKIP LOCKED;

  IF NOT FOUND THEN
    RETURN NULL;
  END IF;

  UPDATE wfg_jobs
  SET state = 'active',
      worker_id = p_worker_id,
      lease_id = v_lease_id,
      lease_expires_at = now() + make_interval(secs => p_ttl_secs)
  WHERE id = v_job.id;

  RETURN jsonb_build_object(
    'job', row_to_json(v_job),
    'lease', jsonb_build_object(
      'lease_id', v_lease_id,
      'job_id', v_job.job_id,
      'workflow_id', v_job.workflow_id,
      'worker_id', p_worker_id,
      'ttl_secs', p_ttl_secs
    )
  );
END;
$$ LANGUAGE plpgsql;
```

#### Option B: Proxy to Rust API Server

Deploy the Rust API server on [Fly.io](https://fly.io) or [Railway](https://railway.app), then proxy from Supabase Edge Functions:

```typescript
import { serve } from "https://deno.land/std/http/server.ts";

const API_SERVER = Deno.env.get("WORKFLOW_API_URL")!;

serve(async (req: Request) => {
  const url = new URL(req.url);
  // Forward to the Rust API server
  const upstream = new URL(url.pathname + url.search, API_SERVER);
  return fetch(upstream.toString(), {
    method: req.method,
    headers: req.headers,
    body: req.body,
  });
});
```

#### Deploy

```bash
supabase functions deploy workflow-api
```

#### Scheduler

For Supabase, the scheduler can run as:
- A separate process on Fly.io / Railway
- A Supabase [pg_cron](https://supabase.com/docs/guides/database/extensions/pg_cron) job for lease reaping, combined with Postgres `LISTEN/NOTIFY` triggers for DAG cascading

```sql
-- Reap expired leases every 10 seconds via pg_cron
SELECT cron.schedule(
  'reap-expired-leases',
  '10 seconds',
  $$UPDATE wfg_jobs SET state = 'pending', worker_id = NULL, lease_id = NULL
    WHERE state = 'active' AND lease_expires_at < now()$$
);
```

---

## Platform Comparison

| Platform | Runtime | Postgres Access | Scheduler Option | Cold Start |
|----------|---------|-----------------|------------------|------------|
| **Self-hosted** | Native Rust | Direct | In-process | None |
| **Vercel** | Serverless (Rust → WASM or native) | Via connection string | Separate VM/container | ~200ms |
| **Cloudflare Workers** | V8 (Rust → WASM) | Via Hyperdrive | Separate VM or Cron Triggers | ~5ms |
| **Supabase Edge** | Deno Deploy | Direct (same Postgres) | pg_cron + LISTEN/NOTIFY | ~50ms |
| **Fly.io** | Native Rust | Direct | In-process (all-in-one) | None |
| **Railway** | Native Rust | Direct | In-process (all-in-one) | None |

:::tip
For the simplest production deployment, use **Fly.io** or **Railway** in all-in-one mode with a managed Postgres instance. Split mode is only needed when you specifically want edge latency for the API layer.
:::
