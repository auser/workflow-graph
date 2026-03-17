---
title: REST API
description: HTTP API endpoints for workflow management and worker protocol
---

All endpoints use JSON request/response bodies. The base URL defaults to `http://localhost:3000`.

## TypeScript Client

The `@auser/workflow-graph-client` package provides a typed client with error handling:

```typescript
import { WorkflowClient, WorkflowApiError } from '@auser/workflow-graph-client';

const client = new WorkflowClient('http://localhost:3000');

try {
  const workflows = await client.listWorkflows();
  await client.runWorkflow(workflows[0].id);
} catch (err) {
  if (err instanceof WorkflowApiError) {
    console.error(`API error ${err.status}: ${err.message}`);
  }
}
```

## Workflow Management

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/workflows` | List workflows (supports pagination) |
| `POST` | `/api/workflows` | Create workflow |
| `GET` | `/api/workflows/{id}/status` | Get workflow status with all job states |
| `POST` | `/api/workflows/{id}/run` | Run a workflow (enqueues root jobs) |
| `POST` | `/api/workflows/{id}/cancel` | Cancel all pending/active jobs |

### Pagination & Filtering

`GET /api/workflows` supports query parameters:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | number | 100 | Max workflows to return (capped at 1000) |
| `offset` | number | 0 | Skip this many workflows |
| `status` | string | — | Filter: only workflows with at least one job in this status |

```bash
# Get first 10 workflows
curl 'http://localhost:3000/api/workflows?limit=10'

# Get running workflows, page 2
curl 'http://localhost:3000/api/workflows?status=running&limit=10&offset=10'
```

### Example: Create and Run a Workflow

```bash
# List available workflows (loaded from workflows/ directory)
curl -s http://localhost:3000/api/workflows | python3 -m json.tool

# Run a workflow
curl -s -X POST http://localhost:3000/api/workflows/ci-1/run

# Check status
curl -s http://localhost:3000/api/workflows/ci-1/status | python3 -m json.tool
```

## Worker Protocol

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/workers/register` | Register worker with labels |
| `POST` | `/api/jobs/claim` | Claim next available job (atomic) |
| `POST` | `/api/jobs/{lease_id}/heartbeat` | Renew job lease |
| `POST` | `/api/jobs/{lease_id}/complete` | Report job success + outputs |
| `POST` | `/api/jobs/{lease_id}/fail` | Report job failure |
| `POST` | `/api/jobs/{lease_id}/logs` | Push log chunks |
| `GET` | `/api/jobs/{wf_id}/{job_id}/cancelled` | Check if job was cancelled |

## Log Streaming

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/workflows/{wf_id}/jobs/{job_id}/logs` | Get historical log chunks |
| `GET` | `/api/workflows/{wf_id}/jobs/{job_id}/logs/stream` | SSE live log stream |

## Worker Registry

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/workers` | List all registered workers |

## Request/Response Examples

### Claim a Job

```bash
curl -X POST http://localhost:3000/api/jobs/claim \
  -H 'Content-Type: application/json' \
  -d '{"worker_id": "w1", "labels": ["docker"], "lease_ttl_secs": 30}'
```

Response when a job is available:
```json
{
  "job": {
    "job_id": "build",
    "workflow_id": "wf-uuid",
    "command": "cargo build --release",
    "required_labels": ["docker"],
    "retry_policy": { "max_retries": 2, "backoff": "None" },
    "attempt": 0,
    "upstream_outputs": {
      "test": { "coverage": "94%" }
    },
    "enqueued_at_ms": 1710000000000,
    "delayed_until_ms": 0
  },
  "lease": {
    "lease_id": "lease-uuid",
    "job_id": "build",
    "workflow_id": "wf-uuid",
    "worker_id": "w1",
    "ttl_secs": 30,
    "granted_at_ms": 1710000000000
  }
}
```

Response when no job is available: `null`

### Complete a Job

```bash
curl -X POST http://localhost:3000/api/jobs/lease-uuid/complete \
  -H 'Content-Type: application/json' \
  -d '{"outputs": {"artifact_url": "s3://bucket/build.tar.gz"}}'
```

### Report Failure

```bash
curl -X POST http://localhost:3000/api/jobs/lease-uuid/fail \
  -H 'Content-Type: application/json' \
  -d '{"error": "exit code 1: compilation failed", "retryable": true}'
```

Set `retryable: true` for transient failures (network, OOM). The server will re-enqueue with backoff delay if the job's retry budget allows.

### Retry Backoff

Jobs support three backoff strategies configured in the `retry_policy`:

| Strategy | Behavior |
|----------|----------|
| `None` | Retry immediately (default) |
| `Fixed { delay_secs: N }` | Wait N seconds between retries |
| `Exponential { base_secs: N, max_secs: M }` | Wait N×2^attempt seconds, capped at M |

Jobs with backoff delay are not claimable until `delayed_until_ms` has elapsed.

## Error Handling

The TypeScript client throws `WorkflowApiError` for non-OK responses:

```typescript
class WorkflowApiError extends Error {
  status: number;      // HTTP status code
  statusText: string;  // HTTP status text
}
```

All API endpoints return standard HTTP status codes:
- `200` — Success
- `201` — Created (workflow creation)
- `202` — Accepted (workflow run/cancel)
- `404` — Not found (unknown workflow/job ID)
- `409` — Conflict (expired lease)
- `500` — Internal server error

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3000` | Server port (auto-finds next available if taken) |
| `API_ONLY` | unset | Set to `1` for API-only mode (no scheduler) |
| `WORKFLOWS_DIR` | `workflows/` | Directory to load workflow files from |
| `CORS_ORIGINS` | unset | Comma-separated allowed origins (permissive if unset) |
| `REAP_INTERVAL_SECS` | `5` | Lease reaper interval (standalone scheduler) |

### CORS

By default, the server allows all origins (suitable for development). For production, set `CORS_ORIGINS`:

```bash
CORS_ORIGINS=https://app.example.com,https://admin.example.com cargo run -p workflow-graph-server
```
