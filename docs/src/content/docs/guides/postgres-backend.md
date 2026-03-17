---
title: Postgres / pg-boss Backend
description: Implement all queue traits using Postgres with pg-boss-style atomic claiming
---

Use Postgres as a durable production backend for workflow-graph. This guide implements all four backend traits (`JobQueue`, `ArtifactStore`, `LogSink`, `WorkerRegistry`) using `sqlx` with pg-boss-style atomic claiming via `SELECT ... FOR UPDATE SKIP LOCKED`.

## Prerequisites

```toml
[dependencies]
workflow-graph-queue = { path = "crates/queue" }
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "macros", "json", "chrono", "uuid"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
```

## Database Schema

```sql
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Job queue
CREATE TABLE wfg_jobs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id     TEXT NOT NULL,
    job_id          TEXT NOT NULL,
    command         TEXT NOT NULL,
    required_labels JSONB NOT NULL DEFAULT '[]',
    retry_policy    JSONB NOT NULL DEFAULT '{"max_retries": 0, "backoff": "None"}',
    attempt         INTEGER NOT NULL DEFAULT 0,
    upstream_outputs JSONB NOT NULL DEFAULT '{}',
    enqueued_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    state           TEXT NOT NULL DEFAULT 'pending',
    worker_id       TEXT,
    lease_id        TEXT UNIQUE,
    lease_expires_at TIMESTAMPTZ,
    CONSTRAINT valid_state CHECK (state IN ('pending', 'active', 'completed', 'failed', 'cancelled'))
);

CREATE INDEX idx_wfg_jobs_pending ON wfg_jobs (enqueued_at) WHERE state = 'pending';
CREATE INDEX idx_wfg_jobs_active ON wfg_jobs (lease_expires_at) WHERE state = 'active';
CREATE INDEX idx_wfg_jobs_workflow ON wfg_jobs (workflow_id);

-- Artifact storage
CREATE TABLE wfg_artifacts (
    workflow_id TEXT NOT NULL,
    job_id      TEXT NOT NULL,
    outputs     JSONB NOT NULL DEFAULT '{}',
    PRIMARY KEY (workflow_id, job_id)
);

-- Log storage
CREATE TABLE wfg_logs (
    id          BIGSERIAL PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    job_id      TEXT NOT NULL,
    sequence    BIGINT NOT NULL,
    data        TEXT NOT NULL,
    timestamp_ms BIGINT NOT NULL,
    stream      TEXT NOT NULL DEFAULT 'stdout',
    CONSTRAINT valid_stream CHECK (stream IN ('stdout', 'stderr'))
);

CREATE INDEX idx_wfg_logs_job ON wfg_logs (workflow_id, job_id, sequence);

-- Worker registry
CREATE TABLE wfg_workers (
    worker_id       TEXT PRIMARY KEY,
    labels          JSONB NOT NULL DEFAULT '[]',
    registered_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_heartbeat  TIMESTAMPTZ NOT NULL DEFAULT now(),
    current_job     TEXT,
    status          TEXT NOT NULL DEFAULT 'idle'
);
```

## PgBackend Struct

A single struct implements all four traits, sharing a connection pool:

```rust
use sqlx::PgPool;
use tokio::sync::broadcast;
use workflow_graph_queue::traits::*;

pub struct PgBackend {
    pool: PgPool,
    events: broadcast::Sender<JobEvent>,
    log_events: broadcast::Sender<LogChunk>,
}

impl PgBackend {
    pub fn new(pool: PgPool) -> Self {
        let (events, _) = broadcast::channel(256);
        let (log_events, _) = broadcast::channel(1024);
        Self { pool, events, log_events }
    }
}
```

## Atomic Job Claiming

This is the critical operation. `FOR UPDATE SKIP LOCKED` ensures two workers can never claim the same job:

```rust
impl JobQueue for PgBackend {
    async fn claim(
        &self,
        worker_id: &str,
        worker_labels: &[String],
        lease_ttl: Duration,
    ) -> Result<Option<(QueuedJob, Lease)>, QueueError> {
        let labels_json = serde_json::to_value(worker_labels).unwrap();
        let lease_id = uuid::Uuid::new_v4().to_string();
        let ttl_secs = lease_ttl.as_secs() as f64;

        let row = sqlx::query_as::<_, (
            uuid::Uuid, String, String, String,
            serde_json::Value, serde_json::Value, i32, serde_json::Value,
        )>(
            "WITH candidate AS (
                SELECT id, workflow_id, job_id, command, required_labels,
                       retry_policy, attempt, upstream_outputs
                FROM wfg_jobs
                WHERE state = 'pending'
                  AND $1::jsonb @> required_labels
                ORDER BY enqueued_at ASC
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            UPDATE wfg_jobs j
            SET state = 'active',
                worker_id = $2,
                lease_id = $3,
                lease_expires_at = now() + make_interval(secs => $4)
            FROM candidate c
            WHERE j.id = c.id
            RETURNING c.id, c.workflow_id, c.job_id, c.command,
                      c.required_labels, c.retry_policy, c.attempt,
                      c.upstream_outputs"
        )
        .bind(&labels_json)
        .bind(worker_id)
        .bind(&lease_id)
        .bind(ttl_secs)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QueueError::Internal(e.to_string()))?;

        // ... deserialize and return (QueuedJob, Lease)
    }
}
```

### Why `FOR UPDATE SKIP LOCKED`?

When multiple workers call `claim()` simultaneously:

- **`FOR UPDATE`** locks the selected row so no other transaction can modify it
- **`SKIP LOCKED`** tells concurrent transactions to skip already-locked rows instead of waiting

Without this, two workers could claim the same job. This is exactly what pg-boss uses internally.

## Enqueue

```rust
async fn enqueue(&self, job: QueuedJob) -> Result<(), QueueError> {
    sqlx::query(
        "INSERT INTO wfg_jobs (workflow_id, job_id, command, required_labels,
         retry_policy, attempt, upstream_outputs, state)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending')"
    )
    .bind(&job.workflow_id)
    .bind(&job.job_id)
    .bind(&job.command)
    .bind(serde_json::to_value(&job.required_labels).unwrap())
    .bind(serde_json::to_value(&job.retry_policy).unwrap())
    .bind(job.attempt as i32)
    .bind(serde_json::to_value(&job.upstream_outputs).unwrap())
    .execute(&self.pool)
    .await
    .map_err(|e| QueueError::Internal(e.to_string()))?;

    self.events.send(JobEvent::Ready {
        workflow_id: job.workflow_id,
        job_id: job.job_id,
    }).ok();

    Ok(())
}
```

## Complete / Fail

```rust
async fn complete(
    &self,
    lease_id: &str,
    outputs: HashMap<String, String>,
) -> Result<(), QueueError> {
    let row = sqlx::query_as::<_, (String, String)>(
        "UPDATE wfg_jobs
         SET state = 'completed', lease_expires_at = NULL
         WHERE lease_id = $1 AND state = 'active'
         RETURNING workflow_id, job_id"
    )
    .bind(lease_id)
    .fetch_optional(&self.pool)
    .await
    .map_err(|e| QueueError::Internal(e.to_string()))?;

    let Some((workflow_id, job_id)) = row else {
        return Err(QueueError::LeaseNotFound(lease_id.to_string()));
    };

    self.events.send(JobEvent::Completed {
        workflow_id, job_id, outputs,
    }).ok();

    Ok(())
}

async fn fail(
    &self,
    lease_id: &str,
    error: String,
    retryable: bool,
) -> Result<(), QueueError> {
    let row = sqlx::query_as::<_, (String, String, serde_json::Value, i32)>(
        "UPDATE wfg_jobs
         SET state = 'failed', lease_expires_at = NULL
         WHERE lease_id = $1 AND state = 'active'
         RETURNING workflow_id, job_id, retry_policy, attempt"
    )
    .bind(lease_id)
    .fetch_optional(&self.pool)
    .await
    .map_err(|e| QueueError::Internal(e.to_string()))?;

    let Some((workflow_id, job_id, policy_json, attempt)) = row else {
        return Err(QueueError::LeaseNotFound(lease_id.to_string()));
    };

    let policy: RetryPolicy = serde_json::from_value(policy_json).unwrap_or_default();
    let should_retry = retryable && (attempt as u32) < policy.max_retries;

    if should_retry {
        sqlx::query(
            "INSERT INTO wfg_jobs (workflow_id, job_id, command, required_labels,
             retry_policy, attempt, upstream_outputs, state)
             SELECT workflow_id, job_id, command, required_labels,
                    retry_policy, $1, upstream_outputs, 'pending'
             FROM wfg_jobs WHERE lease_id = $2"
        )
        .bind(attempt + 1)
        .bind(lease_id)
        .execute(&self.pool)
        .await
        .map_err(|e| QueueError::Internal(e.to_string()))?;
    }

    self.events.send(JobEvent::Failed {
        workflow_id, job_id, error, retryable: should_retry,
    }).ok();

    Ok(())
}
```

## ArtifactStore

```rust
impl ArtifactStore for PgBackend {
    async fn put_outputs(
        &self, workflow_id: &str, job_id: &str,
        outputs: HashMap<String, String>,
    ) -> Result<(), ArtifactError> {
        sqlx::query(
            "INSERT INTO wfg_artifacts (workflow_id, job_id, outputs)
             VALUES ($1, $2, $3)
             ON CONFLICT (workflow_id, job_id) DO UPDATE SET outputs = $3"
        )
        .bind(workflow_id)
        .bind(job_id)
        .bind(serde_json::to_value(&outputs).unwrap())
        .execute(&self.pool)
        .await
        .map_err(|e| ArtifactError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn get_outputs(
        &self, workflow_id: &str, job_id: &str,
    ) -> Result<HashMap<String, String>, ArtifactError> {
        let row = sqlx::query_as::<_, (serde_json::Value,)>(
            "SELECT outputs FROM wfg_artifacts
             WHERE workflow_id = $1 AND job_id = $2"
        )
        .bind(workflow_id)
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ArtifactError::Internal(e.to_string()))?;

        match row {
            Some((val,)) => Ok(serde_json::from_value(val).unwrap_or_default()),
            None => Ok(HashMap::new()),
        }
    }
}
```

## LogSink

```rust
impl LogSink for PgBackend {
    async fn append(&self, chunk: LogChunk) -> Result<(), LogError> {
        let stream_str = match chunk.stream {
            LogStream::Stdout => "stdout",
            LogStream::Stderr => "stderr",
        };
        sqlx::query(
            "INSERT INTO wfg_logs (workflow_id, job_id, sequence, data, timestamp_ms, stream)
             VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(&chunk.workflow_id)
        .bind(&chunk.job_id)
        .bind(chunk.sequence as i64)
        .bind(&chunk.data)
        .bind(chunk.timestamp_ms as i64)
        .bind(stream_str)
        .execute(&self.pool)
        .await
        .map_err(|e| LogError::Internal(e.to_string()))?;

        self.log_events.send(chunk).ok();
        Ok(())
    }

    async fn get_all(
        &self, workflow_id: &str, job_id: &str,
    ) -> Result<Vec<LogChunk>, LogError> {
        let rows = sqlx::query_as::<_, (String, String, i64, String, i64, String)>(
            "SELECT workflow_id, job_id, sequence, data, timestamp_ms, stream
             FROM wfg_logs WHERE workflow_id = $1 AND job_id = $2
             ORDER BY sequence ASC"
        )
        .bind(workflow_id)
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LogError::Internal(e.to_string()))?;

        Ok(rows.into_iter().map(|(wf, j, seq, data, ts, stream)| LogChunk {
            workflow_id: wf, job_id: j,
            sequence: seq as u64, data,
            timestamp_ms: ts as u64,
            stream: if stream == "stderr" { LogStream::Stderr } else { LogStream::Stdout },
        }).collect())
    }
}
```

## Multi-Process Event Distribution

The `broadcast::Sender<JobEvent>` is in-process only. For split deployments (separate API + scheduler), use Postgres `LISTEN/NOTIFY`:

```rust
// In PgBackend::enqueue, after the INSERT:
sqlx::query("SELECT pg_notify('wfg_events', $1)")
    .bind(serde_json::to_string(&event).unwrap())
    .execute(&self.pool)
    .await
    .ok();

// In the scheduler process:
let mut listener = sqlx::postgres::PgListener::connect_with(&pool).await?;
listener.listen("wfg_events").await?;
loop {
    let notification = listener.recv().await?;
    let event: JobEvent = serde_json::from_str(notification.payload())?;
    events_tx.send(event).ok();
}
```

## Wiring into the Server

```rust
use sqlx::PgPool;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/workflow_graph".into());

    let pool = PgPool::connect(&database_url)
        .await
        .expect("DB connect failed");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migration failed");

    let backend = Arc::new(PgBackend::new(pool));
    let state = SharedState::new(WorkflowState::new());

    let scheduler = Arc::new(DagScheduler::new(
        backend.clone(),
        backend.clone(),
        state.clone(),
    ));
    tokio::spawn(scheduler.clone().run());

    // Lease reaper
    let reaper = backend.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            reaper.reap_expired_leases().await.ok();
        }
    });

    let app = workflow_graph_server::create_router(AppState {
        workflow_state: state,
        queue: backend.clone(),
        artifacts: backend.clone(),
        logs: backend.clone(),
        workers: backend.clone(),
    });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

## pg-boss Compatibility

This schema is pg-boss-**inspired** but not pg-boss-compatible. The key patterns are the same:

| Concept | pg-boss | This schema |
|---------|---------|-------------|
| Atomic claim | `SELECT ... FOR UPDATE SKIP LOCKED` | Same |
| Lease expiry | `expireIn` option | `lease_expires_at` column |
| Retry | `retryLimit` + `retryDelay` | `retry_policy` JSONB |
| State machine | `created → active → completed/failed` | `pending → active → completed/failed/cancelled` |

If you want to use the actual pg-boss npm package alongside Rust workers, you'd need to adapt queries to use pg-boss's internal tables (`pgboss.job`, `pgboss.version`, etc.).
