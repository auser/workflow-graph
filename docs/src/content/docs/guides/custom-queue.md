---
title: Custom Queue Backend
description: Implement your own storage backend with the queue traits
---

All storage in workflow-graph is abstracted behind four traits. Implement them for your preferred backend (Postgres, Redis, DynamoDB, etc.).

## Traits Overview

| Trait | Purpose |
|-------|---------|
| `JobQueue` | Job lifecycle: enqueue, claim, complete, fail, cancel, reap |
| `ArtifactStore` | Key-value outputs per job (job-to-job communication) |
| `LogSink` | Append-only log chunks with live subscriptions |
| `WorkerRegistry` | Worker registration, heartbeats, status tracking |

## JobQueue Trait

The core trait for job management:

```rust
use workflow_graph_queue::traits::*;

struct MyRedisQueue { /* ... */ }

impl JobQueue for MyRedisQueue {
    async fn enqueue(&self, job: QueuedJob) -> Result<(), QueueError> { /* ... */ }
    async fn claim(&self, worker_id: &str, labels: &[String], ttl: Duration)
        -> Result<Option<(QueuedJob, Lease)>, QueueError> { /* ... */ }
    async fn renew_lease(&self, lease_id: &str, extend_by: Duration)
        -> Result<(), QueueError> { /* ... */ }
    async fn complete(&self, lease_id: &str, outputs: HashMap<String, String>)
        -> Result<(), QueueError> { /* ... */ }
    async fn fail(&self, lease_id: &str, error: String, retryable: bool)
        -> Result<(), QueueError> { /* ... */ }
    async fn cancel(&self, workflow_id: &str, job_id: &str)
        -> Result<(), QueueError> { /* ... */ }
    async fn cancel_workflow(&self, workflow_id: &str)
        -> Result<(), QueueError> { /* ... */ }
    async fn is_cancelled(&self, workflow_id: &str, job_id: &str)
        -> Result<bool, QueueError> { /* ... */ }
    async fn reap_expired_leases(&self) -> Result<Vec<JobEvent>, QueueError> { /* ... */ }
    fn subscribe(&self) -> broadcast::Receiver<JobEvent>;
}
```

### Critical: Atomic Claiming

The `claim` method must be **atomic** — two workers calling `claim` simultaneously must never receive the same job. With Postgres, use `SELECT ... FOR UPDATE SKIP LOCKED`:

```sql
WITH candidate AS (
    SELECT id FROM wfg_jobs
    WHERE state = 'pending'
      AND $1::jsonb @> required_labels
    ORDER BY enqueued_at ASC
    LIMIT 1
    FOR UPDATE SKIP LOCKED
)
UPDATE wfg_jobs j
SET state = 'active', worker_id = $2, lease_id = $3,
    lease_expires_at = now() + make_interval(secs => $4)
FROM candidate c WHERE j.id = c.id
RETURNING ...
```

With Redis, use `RPOPLPUSH` or Lua scripts for atomicity.

## Trait → pg-boss Mapping

If you're familiar with pg-boss, here's how the traits map:

| Trait Method | pg-boss Equivalent |
|-------------|-------------------|
| `enqueue()` | `boss.send(queue, data, options)` |
| `claim()` | `boss.fetch(queue)` — `SELECT FOR UPDATE SKIP LOCKED` |
| `complete()` | `boss.complete(jobId)` |
| `fail()` | `boss.fail(jobId)` |
| `cancel()` | `boss.cancel(jobId)` |
| `reap_expired()` | pg-boss `maintain()` (automatic) |

## ArtifactStore Trait

Stores job outputs for downstream consumption:

```rust
impl ArtifactStore for MyBackend {
    async fn put_outputs(&self, workflow_id: &str, job_id: &str,
        outputs: HashMap<String, String>) -> Result<(), ArtifactError>;
    async fn get_outputs(&self, workflow_id: &str, job_id: &str)
        -> Result<HashMap<String, String>, ArtifactError>;
    async fn get_upstream_outputs(&self, workflow_id: &str, job_ids: &[String])
        -> Result<HashMap<String, HashMap<String, String>>, ArtifactError>;
}
```

## LogSink Trait

Append-only log storage with live subscription support:

```rust
impl LogSink for MyBackend {
    async fn append(&self, chunk: LogChunk) -> Result<(), LogError>;
    async fn get_all(&self, workflow_id: &str, job_id: &str)
        -> Result<Vec<LogChunk>, LogError>;
    fn subscribe(&self, workflow_id: &str, job_id: &str)
        -> broadcast::Receiver<LogChunk>;
}
```

## WorkerRegistry Trait

Track registered workers and their status:

```rust
impl WorkerRegistry for MyBackend {
    async fn register(&self, worker_id: &str, labels: &[String])
        -> Result<(), RegistryError>;
    async fn heartbeat(&self, worker_id: &str) -> Result<(), RegistryError>;
    async fn deregister(&self, worker_id: &str) -> Result<(), RegistryError>;
    async fn list_workers(&self) -> Result<Vec<WorkerInfo>, RegistryError>;
    async fn mark_busy(&self, worker_id: &str, job_id: &str)
        -> Result<(), RegistryError>;
    async fn mark_idle(&self, worker_id: &str) -> Result<(), RegistryError>;
}
```

## Single Backend Struct

A single struct can implement all four traits, sharing a connection pool:

```rust
pub struct PgBackend {
    pool: PgPool,
    events: broadcast::Sender<JobEvent>,
    log_events: broadcast::Sender<LogChunk>,
}

// Then wire it in:
let backend = Arc::new(PgBackend::new(pool));
let app_state = AppState {
    queue: backend.clone(),
    artifacts: backend.clone(),
    logs: backend.clone(),
    workers: backend.clone(),
    ..
};
```

For a complete Postgres implementation, see the [Postgres backend guide](/workflow-graph/guides/custom-queue/) in the project's `docs/guide-postgres.md`.
