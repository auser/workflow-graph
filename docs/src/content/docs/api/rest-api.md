---
title: REST API
description: HTTP API endpoints for workflow management and worker protocol
---

All endpoints use JSON request/response bodies. The base URL defaults to `http://localhost:3000`.

## Workflow Management

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/workflows` | List all workflows |
| `POST` | `/api/workflows` | Create workflow |
| `GET` | `/api/workflows/{id}/status` | Get workflow status with all job states |
| `POST` | `/api/workflows/{id}/run` | Run a workflow (enqueues root jobs) |
| `POST` | `/api/workflows/{id}/cancel` | Cancel all pending/active jobs |

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
    "enqueued_at_ms": 1710000000000
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

Set `retryable: true` for transient failures (network, OOM). The server will re-enqueue if the job's retry budget allows.
