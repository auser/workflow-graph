# Postgres / pg-boss Backend

Implement all four backend traits (`JobQueue`, `ArtifactStore`, `LogSink`, `WorkerRegistry`) using Postgres with `sqlx`. The job queue uses pg-boss-style atomic claiming via `SELECT ... FOR UPDATE SKIP LOCKED`.

## Prerequisites

Add to your `Cargo.toml`:

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

Run this migration to create the required tables:

```sql
-- Enable UUID generation
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Job queue: one row per job (pending, active, completed, failed, cancelled)
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
    state           TEXT NOT NULL DEFAULT 'pending',  -- pending, active, completed, failed, cancelled
    worker_id       TEXT,
    lease_id        TEXT UNIQUE,
    lease_expires_at TIMESTAMPTZ,

    CONSTRAINT valid_state CHECK (state IN ('pending', 'active', 'completed', 'failed', 'cancelled'))
);

CREATE INDEX idx_wfg_jobs_pending ON wfg_jobs (enqueued_at) WHERE state = 'pending';
CREATE INDEX idx_wfg_jobs_active ON wfg_jobs (lease_expires_at) WHERE state = 'active';
CREATE INDEX idx_wfg_jobs_workflow ON wfg_jobs (workflow_id);

-- Artifact storage: key-value outputs per job
CREATE TABLE wfg_artifacts (
    workflow_id TEXT NOT NULL,
    job_id      TEXT NOT NULL,
    outputs     JSONB NOT NULL DEFAULT '{}',
    PRIMARY KEY (workflow_id, job_id)
);

-- Log storage: append-only log chunks
CREATE TABLE wfg_logs (
    id          BIGSERIAL PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    job_id      TEXT NOT NULL,
    sequence    BIGINT NOT NULL,
    data        TEXT NOT NULL,
    timestamp_ms BIGINT NOT NULL,
    stream      TEXT NOT NULL DEFAULT 'stdout',  -- stdout or stderr
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
    status          TEXT NOT NULL DEFAULT 'idle'  -- idle, busy, offline
);
```

## PgBackend Struct

A single struct that implements all four traits, sharing a connection pool:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::broadcast;

use workflow_graph_queue::traits::*;
use workflow_graph_queue::error::*;

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

## impl JobQueue for PgBackend

```rust
impl JobQueue for PgBackend {
    async fn enqueue(&self, job: QueuedJob) -> Result<(), QueueError> {
        sqlx::query(
            "INSERT INTO wfg_jobs (workflow_id, job_id, command, required_labels, retry_policy, attempt, upstream_outputs, enqueued_at, state)
             VALUES ($1, $2, $3, $4, $5, $6, $7, now(), 'pending')"
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

    async fn claim(
        &self,
        worker_id: &str,
        worker_labels: &[String],
        lease_ttl: Duration,
    ) -> Result<Option<(QueuedJob, Lease)>, QueueError> {
        let labels_json = serde_json::to_value(worker_labels).unwrap();
        let lease_id = uuid::Uuid::new_v4().to_string();
        let ttl_secs = lease_ttl.as_secs() as i64;

        // Atomic claim: find a pending job whose required_labels are a subset
        // of the worker's labels, lock it, and update in one transaction.
        //
        // The @> operator checks if worker_labels contains all required_labels.
        // FOR UPDATE SKIP LOCKED prevents multiple workers from claiming the same job.
        let row = sqlx::query_as::<_, (
            uuid::Uuid,       // id
            String,            // workflow_id
            String,            // job_id
            String,            // command
            serde_json::Value, // required_labels
            serde_json::Value, // retry_policy
            i32,               // attempt
            serde_json::Value, // upstream_outputs
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
                      c.required_labels, c.retry_policy, c.attempt, c.upstream_outputs"
        )
        .bind(&labels_json)
        .bind(worker_id)
        .bind(&lease_id)
        .bind(ttl_secs as f64)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QueueError::Internal(e.to_string()))?;

        let Some((_id, workflow_id, job_id, command, labels, policy, attempt, upstream)) = row
        else {
            return Ok(None);
        };

        let queued_job = QueuedJob {
            workflow_id: workflow_id.clone(),
            job_id: job_id.clone(),
            command,
            required_labels: serde_json::from_value(labels).unwrap_or_default(),
            retry_policy: serde_json::from_value(policy).unwrap_or_default(),
            attempt: attempt as u32,
            upstream_outputs: serde_json::from_value(upstream).unwrap_or_default(),
            enqueued_at_ms: now_ms(),
        };

        let lease = Lease {
            lease_id: lease_id.clone(),
            job_id: job_id.clone(),
            workflow_id: workflow_id.clone(),
            worker_id: worker_id.to_string(),
            ttl_secs: lease_ttl.as_secs(),
            granted_at_ms: now_ms(),
        };

        self.events.send(JobEvent::Started {
            workflow_id,
            job_id,
            worker_id: worker_id.to_string(),
        }).ok();

        Ok(Some((queued_job, lease)))
    }

    async fn renew_lease(
        &self,
        lease_id: &str,
        extend_by: Duration,
    ) -> Result<(), QueueError> {
        let rows = sqlx::query(
            "UPDATE wfg_jobs
             SET lease_expires_at = now() + make_interval(secs => $1)
             WHERE lease_id = $2 AND state = 'active'"
        )
        .bind(extend_by.as_secs() as f64)
        .bind(lease_id)
        .execute(&self.pool)
        .await
        .map_err(|e| QueueError::Internal(e.to_string()))?;

        if rows.rows_affected() == 0 {
            return Err(QueueError::LeaseNotFound(lease_id.to_string()));
        }
        Ok(())
    }

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
            workflow_id,
            job_id,
            outputs,
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
            // Re-enqueue with incremented attempt
            sqlx::query(
                "INSERT INTO wfg_jobs (workflow_id, job_id, command, required_labels, retry_policy, attempt, upstream_outputs, state)
                 SELECT workflow_id, job_id, command, required_labels, retry_policy, $1, upstream_outputs, 'pending'
                 FROM wfg_jobs WHERE lease_id = $2"
            )
            .bind(attempt + 1)
            .bind(lease_id)
            .execute(&self.pool)
            .await
            .map_err(|e| QueueError::Internal(e.to_string()))?;
        }

        self.events.send(JobEvent::Failed {
            workflow_id,
            job_id,
            error,
            retryable: should_retry,
        }).ok();

        Ok(())
    }

    async fn cancel(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> Result<(), QueueError> {
        sqlx::query(
            "UPDATE wfg_jobs SET state = 'cancelled'
             WHERE workflow_id = $1 AND job_id = $2 AND state IN ('pending', 'active')"
        )
        .bind(workflow_id)
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|e| QueueError::Internal(e.to_string()))?;

        self.events.send(JobEvent::Cancelled {
            workflow_id: workflow_id.to_string(),
            job_id: job_id.to_string(),
        }).ok();

        Ok(())
    }

    async fn cancel_workflow(
        &self,
        workflow_id: &str,
    ) -> Result<(), QueueError> {
        let rows = sqlx::query_as::<_, (String,)>(
            "UPDATE wfg_jobs SET state = 'cancelled'
             WHERE workflow_id = $1 AND state IN ('pending', 'active')
             RETURNING job_id"
        )
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QueueError::Internal(e.to_string()))?;

        for (job_id,) in rows {
            self.events.send(JobEvent::Cancelled {
                workflow_id: workflow_id.to_string(),
                job_id,
            }).ok();
        }

        Ok(())
    }

    async fn is_cancelled(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> Result<bool, QueueError> {
        let row = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM wfg_jobs
             WHERE workflow_id = $1 AND job_id = $2 AND state = 'cancelled'"
        )
        .bind(workflow_id)
        .bind(job_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| QueueError::Internal(e.to_string()))?;

        Ok(row.0 > 0)
    }

    async fn reap_expired_leases(&self) -> Result<Vec<JobEvent>, QueueError> {
        // Find active jobs with expired leases
        let expired = sqlx::query_as::<_, (String, String, String, serde_json::Value, i32)>(
            "UPDATE wfg_jobs
             SET state = 'pending', worker_id = NULL, lease_id = NULL, lease_expires_at = NULL
             WHERE state = 'active' AND lease_expires_at < now()
             RETURNING workflow_id, job_id, COALESCE(worker_id, ''), retry_policy, attempt"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QueueError::Internal(e.to_string()))?;

        let mut events = Vec::new();
        for (workflow_id, job_id, worker_id, policy_json, attempt) in expired {
            let policy: RetryPolicy = serde_json::from_value(policy_json).unwrap_or_default();

            events.push(JobEvent::LeaseExpired {
                workflow_id: workflow_id.clone(),
                job_id: job_id.clone(),
                worker_id: worker_id.clone(),
            });

            // If no retries left, mark as failed instead
            if (attempt as u32) >= policy.max_retries {
                sqlx::query(
                    "UPDATE wfg_jobs SET state = 'failed'
                     WHERE workflow_id = $1 AND job_id = $2 AND state = 'pending' AND attempt = $3"
                )
                .bind(&workflow_id)
                .bind(&job_id)
                .bind(attempt)
                .execute(&self.pool)
                .await
                .ok();
            }
        }

        for event in &events {
            self.events.send(event.clone()).ok();
        }

        Ok(events)
    }

    fn subscribe(&self) -> broadcast::Receiver<JobEvent> {
        self.events.subscribe()
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
```

### Why `FOR UPDATE SKIP LOCKED`?

This is the critical piece for concurrent workers. When multiple workers call `claim()` simultaneously:

- `FOR UPDATE` locks the selected row so no other transaction can modify it
- `SKIP LOCKED` tells other transactions to skip already-locked rows instead of waiting

Without this, two workers could claim the same job. This is exactly what pg-boss uses internally.

## impl ArtifactStore for PgBackend

```rust
impl ArtifactStore for PgBackend {
    async fn put_outputs(
        &self,
        workflow_id: &str,
        job_id: &str,
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
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> Result<HashMap<String, String>, ArtifactError> {
        let row = sqlx::query_as::<_, (serde_json::Value,)>(
            "SELECT outputs FROM wfg_artifacts WHERE workflow_id = $1 AND job_id = $2"
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

    async fn get_upstream_outputs(
        &self,
        workflow_id: &str,
        job_ids: &[String],
    ) -> Result<HashMap<String, HashMap<String, String>>, ArtifactError> {
        if job_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = sqlx::query_as::<_, (String, serde_json::Value)>(
            "SELECT job_id, outputs FROM wfg_artifacts
             WHERE workflow_id = $1 AND job_id = ANY($2)"
        )
        .bind(workflow_id)
        .bind(job_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ArtifactError::Internal(e.to_string()))?;

        let mut result = HashMap::new();
        for (job_id, val) in rows {
            let outputs: HashMap<String, String> =
                serde_json::from_value(val).unwrap_or_default();
            result.insert(job_id, outputs);
        }
        Ok(result)
    }
}
```

## impl LogSink for PgBackend

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

        // Broadcast for SSE subscribers in this process
        self.log_events.send(chunk).ok();

        Ok(())
    }

    async fn get_all(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> Result<Vec<LogChunk>, LogError> {
        let rows = sqlx::query_as::<_, (String, String, i64, String, i64, String)>(
            "SELECT workflow_id, job_id, sequence, data, timestamp_ms, stream
             FROM wfg_logs
             WHERE workflow_id = $1 AND job_id = $2
             ORDER BY sequence ASC"
        )
        .bind(workflow_id)
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LogError::Internal(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|(wf, j, seq, data, ts, stream)| LogChunk {
                workflow_id: wf,
                job_id: j,
                sequence: seq as u64,
                data,
                timestamp_ms: ts as u64,
                stream: if stream == "stderr" {
                    LogStream::Stderr
                } else {
                    LogStream::Stdout
                },
            })
            .collect())
    }

    fn subscribe(&self, _workflow_id: &str, _job_id: &str) -> broadcast::Receiver<LogChunk> {
        // Returns all log events; filter on the consumer side.
        // For per-job channels, use a DashMap<(wf_id, job_id), Sender>.
        self.log_events.subscribe()
    }
}
```

## impl WorkerRegistry for PgBackend

```rust
impl WorkerRegistry for PgBackend {
    async fn register(
        &self,
        worker_id: &str,
        labels: &[String],
    ) -> Result<(), RegistryError> {
        sqlx::query(
            "INSERT INTO wfg_workers (worker_id, labels, status)
             VALUES ($1, $2, 'idle')
             ON CONFLICT (worker_id) DO UPDATE
             SET labels = $2, last_heartbeat = now(), status = 'idle'"
        )
        .bind(worker_id)
        .bind(serde_json::to_value(labels).unwrap())
        .execute(&self.pool)
        .await
        .map_err(|e| RegistryError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn heartbeat(&self, worker_id: &str) -> Result<(), RegistryError> {
        let rows = sqlx::query(
            "UPDATE wfg_workers SET last_heartbeat = now() WHERE worker_id = $1"
        )
        .bind(worker_id)
        .execute(&self.pool)
        .await
        .map_err(|e| RegistryError::Internal(e.to_string()))?;

        if rows.rows_affected() == 0 {
            return Err(RegistryError::NotFound(worker_id.to_string()));
        }
        Ok(())
    }

    async fn deregister(&self, worker_id: &str) -> Result<(), RegistryError> {
        sqlx::query("DELETE FROM wfg_workers WHERE worker_id = $1")
            .bind(worker_id)
            .execute(&self.pool)
            .await
            .map_err(|e| RegistryError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn list_workers(&self) -> Result<Vec<WorkerInfo>, RegistryError> {
        let rows = sqlx::query_as::<_, (String, serde_json::Value, i64, i64, Option<String>, String)>(
            "SELECT worker_id, labels,
                    EXTRACT(EPOCH FROM registered_at)::bigint * 1000,
                    EXTRACT(EPOCH FROM last_heartbeat)::bigint * 1000,
                    current_job, status
             FROM wfg_workers"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RegistryError::Internal(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|(id, labels, reg, hb, job, status)| WorkerInfo {
                worker_id: id,
                labels: serde_json::from_value(labels).unwrap_or_default(),
                registered_at_ms: reg as u64,
                last_heartbeat_ms: hb as u64,
                current_job: job,
                status: match status.as_str() {
                    "busy" => WorkerStatus::Busy,
                    "offline" => WorkerStatus::Offline,
                    _ => WorkerStatus::Idle,
                },
            })
            .collect())
    }

    async fn mark_busy(
        &self,
        worker_id: &str,
        job_id: &str,
    ) -> Result<(), RegistryError> {
        sqlx::query(
            "UPDATE wfg_workers SET status = 'busy', current_job = $2 WHERE worker_id = $1"
        )
        .bind(worker_id)
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|e| RegistryError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn mark_idle(&self, worker_id: &str) -> Result<(), RegistryError> {
        sqlx::query(
            "UPDATE wfg_workers SET status = 'idle', current_job = NULL WHERE worker_id = $1"
        )
        .bind(worker_id)
        .execute(&self.pool)
        .await
        .map_err(|e| RegistryError::Internal(e.to_string()))?;

        Ok(())
    }
}
```

## Wiring into the Server

The traits use `impl Future` return types (not `dyn`-safe), so you must use concrete types. Replace the `InMemory*` types in `AppState` with `PgBackend`:

```rust
// crates/server/src/state.rs
use std::sync::Arc;
use workflow_graph_queue::scheduler::SharedState;

// Replace InMemory* imports with your PgBackend
use your_crate::PgBackend;

#[derive(Clone)]
pub struct AppState {
    pub workflow_state: SharedState,
    pub queue: Arc<PgBackend>,       // was Arc<InMemoryJobQueue>
    pub artifacts: Arc<PgBackend>,   // was Arc<InMemoryArtifactStore>
    pub logs: Arc<PgBackend>,        // was Arc<InMemoryLogSink>
    pub workers: Arc<PgBackend>,     // was Arc<InMemoryWorkerRegistry>
}
```

Since `PgBackend` implements all four traits, you can share a single `Arc<PgBackend>`:

```rust
// crates/server/src/main.rs
use sqlx::PgPool;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/workflow_graph".into());

    let pool = PgPool::connect(&database_url).await.expect("DB connect failed");

    // Run migrations (or use sqlx-cli)
    sqlx::migrate!("./migrations").run(&pool).await.expect("migration failed");

    let backend = Arc::new(PgBackend::new(pool));
    let state = SharedState::new(WorkflowState::new());

    let scheduler = Arc::new(DagScheduler::new(
        backend.clone(),     // as JobQueue
        backend.clone(),     // as ArtifactStore
        state.clone(),
    ));
    tokio::spawn(scheduler.clone().run());

    // Lease reaper
    let reaper_backend = backend.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            reaper_backend.reap_expired_leases().await.ok();
        }
    });

    let app_state = AppState {
        workflow_state: state,
        queue: backend.clone(),
        artifacts: backend.clone(),
        logs: backend.clone(),
        workers: backend.clone(),
    };

    let app = workflow_graph_server::create_router(app_state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

## pg-boss Compatibility Notes

### Relationship to pg-boss

This schema is **pg-boss-inspired** but not pg-boss-compatible. The key patterns are the same:

| Concept | pg-boss | This schema |
|---------|---------|-------------|
| Atomic claim | `SELECT ... FOR UPDATE SKIP LOCKED` | Same |
| Lease expiry | `expireIn` option | `lease_expires_at` column |
| Retry | `retryLimit` + `retryDelay` | `retry_policy` JSONB |
| State machine | `created → active → completed/failed` | `pending → active → completed/failed/cancelled` |

If you want to use the actual pg-boss npm package (e.g., from a Node.js scheduler) alongside Rust workers, you'd need to adapt the queries to use pg-boss's internal tables (`pgboss.job`, `pgboss.version`, etc.) instead.

### Multi-Process Event Distribution

The `broadcast::Sender<JobEvent>` in `PgBackend` is in-process only. If you run the API server and scheduler as separate processes (edge deployment), you need to bridge events across processes.

Use Postgres LISTEN/NOTIFY:

```rust
// In PgBackend::enqueue, after the INSERT:
sqlx::query("SELECT pg_notify('wfg_events', $1)")
    .bind(serde_json::to_string(&event).unwrap())
    .execute(&self.pool)
    .await
    .ok();

// In the scheduler process, spawn a listener:
let mut listener = sqlx::postgres::PgListener::connect_with(&pool).await?;
listener.listen("wfg_events").await?;
loop {
    let notification = listener.recv().await?;
    let event: JobEvent = serde_json::from_str(notification.payload())?;
    events_tx.send(event).ok();
}
```

This way the scheduler receives events even when the API server is a separate process (or running on an edge platform).
