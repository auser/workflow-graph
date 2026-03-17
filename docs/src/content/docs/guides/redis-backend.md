---
title: Redis Backend
description: Implement the queue traits using Redis for production deployments
---

Use Redis as a production queue backend for workflow-graph. Redis provides atomic operations, pub/sub for events, and sorted sets for priority queuing.

## Prerequisites

```toml
[dependencies]
workflow-graph-queue = { path = "crates/queue" }
redis = { version = "0.27", features = ["tokio-comp", "aio"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
```

## Key Design

Redis keys:

| Key | Type | Purpose |
|-----|------|---------|
| `wfg:jobs:pending` | Sorted Set | Pending jobs, scored by enqueue time |
| `wfg:jobs:active:{lease_id}` | Hash | Active job data + lease metadata |
| `wfg:job:{wf_id}:{job_id}` | Hash | Job details |
| `wfg:leases` | Sorted Set | Lease expiry times (for reaping) |
| `wfg:artifacts:{wf_id}:{job_id}` | Hash | Job outputs |
| `wfg:logs:{wf_id}:{job_id}` | List | Append-only log chunks |
| `wfg:workers` | Hash | Worker registry |
| `wfg:events` | Pub/Sub channel | Job events for the scheduler |

## RedisBackend Struct

```rust
use redis::aio::MultiplexedConnection;
use tokio::sync::broadcast;
use workflow_graph_queue::traits::*;

pub struct RedisBackend {
    conn: MultiplexedConnection,
    events: broadcast::Sender<JobEvent>,
    log_events: broadcast::Sender<LogChunk>,
}

impl RedisBackend {
    pub async fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        let conn = client.get_multiplexed_tokio_connection().await?;
        let (events, _) = broadcast::channel(256);
        let (log_events, _) = broadcast::channel(1024);
        Ok(Self { conn, events, log_events })
    }
}
```

## Atomic Job Claiming

The critical operation — use a Lua script for atomic claim:

```rust
impl JobQueue for RedisBackend {
    async fn claim(
        &self,
        worker_id: &str,
        worker_labels: &[String],
        lease_ttl: Duration,
    ) -> Result<Option<(QueuedJob, Lease)>, QueueError> {
        let mut conn = self.conn.clone();
        let lease_id = uuid::Uuid::new_v4().to_string();

        // Lua script: atomically pop from pending, check labels, move to active
        let script = redis::Script::new(r#"
            local pending = redis.call('ZRANGE', KEYS[1], 0, 0)
            if #pending == 0 then return nil end

            local job_key = pending[1]
            local job_data = redis.call('HGETALL', job_key)
            if #job_data == 0 then
                redis.call('ZREM', KEYS[1], job_key)
                return nil
            end

            -- Parse job data into a table
            local job = {}
            for i = 1, #job_data, 2 do
                job[job_data[i]] = job_data[i+1]
            end

            -- Check label match: required_labels must be subset of worker_labels
            local required = cjson.decode(job['required_labels'] or '[]')
            local worker = cjson.decode(ARGV[1])
            local worker_set = {}
            for _, l in ipairs(worker) do worker_set[l] = true end
            for _, r in ipairs(required) do
                if not worker_set[r] then return nil end
            end

            -- Claim: remove from pending, set active state
            redis.call('ZREM', KEYS[1], job_key)
            redis.call('HSET', job_key, 'state', 'active',
                       'worker_id', ARGV[2], 'lease_id', ARGV[3])
            redis.call('ZADD', KEYS[2], ARGV[4], ARGV[3])

            return cjson.encode(job)
        "#);

        let result: Option<String> = script
            .key("wfg:jobs:pending")
            .key("wfg:leases")
            .arg(serde_json::to_string(worker_labels).unwrap())
            .arg(worker_id)
            .arg(&lease_id)
            .arg(now_epoch_secs() + lease_ttl.as_secs() as f64)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| QueueError::Internal(e.to_string()))?;

        match result {
            None => Ok(None),
            Some(json) => {
                let job: QueuedJob = serde_json::from_str(&json)
                    .map_err(|e| QueueError::Internal(e.to_string()))?;
                let lease = Lease {
                    lease_id,
                    job_id: job.job_id.clone(),
                    workflow_id: job.workflow_id.clone(),
                    worker_id: worker_id.to_string(),
                    ttl_secs: lease_ttl.as_secs(),
                    granted_at_ms: now_ms(),
                };
                Ok(Some((job, lease)))
            }
        }
    }
}
```

## Enqueue

```rust
async fn enqueue(&self, job: QueuedJob) -> Result<(), QueueError> {
    let mut conn = self.conn.clone();
    let job_key = format!("wfg:job:{}:{}", job.workflow_id, job.job_id);
    let score = job.enqueued_at_ms as f64;

    // Store job data
    redis::pipe()
        .hset_multiple(&job_key, &[
            ("workflow_id", &job.workflow_id),
            ("job_id", &job.job_id),
            ("command", &job.command),
            ("required_labels", &serde_json::to_string(&job.required_labels).unwrap()),
            ("retry_policy", &serde_json::to_string(&job.retry_policy).unwrap()),
            ("attempt", &job.attempt.to_string()),
            ("upstream_outputs", &serde_json::to_string(&job.upstream_outputs).unwrap()),
            ("state", &"pending".to_string()),
        ])
        .zadd("wfg:jobs:pending", &job_key, score)
        .exec_async(&mut conn)
        .await
        .map_err(|e| QueueError::Internal(e.to_string()))?;

    // Publish event for scheduler
    let event = JobEvent::Ready {
        workflow_id: job.workflow_id.clone(),
        job_id: job.job_id.clone(),
    };
    redis::cmd("PUBLISH")
        .arg("wfg:events")
        .arg(serde_json::to_string(&event).unwrap())
        .exec_async(&mut conn)
        .await
        .ok();

    self.events.send(event).ok();
    Ok(())
}
```

## Lease Renewal

```rust
async fn renew_lease(
    &self,
    lease_id: &str,
    extend_by: Duration,
) -> Result<(), QueueError> {
    let mut conn = self.conn.clone();
    let new_expiry = now_epoch_secs() + extend_by.as_secs() as f64;

    // Update the lease expiry in the sorted set
    let updated: i64 = redis::cmd("ZADD")
        .arg("wfg:leases")
        .arg("XX")  // Only update existing
        .arg(new_expiry)
        .arg(lease_id)
        .query_async(&mut conn)
        .await
        .map_err(|e| QueueError::Internal(e.to_string()))?;

    if updated == 0 {
        return Err(QueueError::LeaseNotFound(lease_id.to_string()));
    }
    Ok(())
}
```

## Reaping Expired Leases

```rust
async fn reap_expired_leases(&self) -> Result<Vec<JobEvent>, QueueError> {
    let mut conn = self.conn.clone();
    let now = now_epoch_secs();

    // Find expired leases
    let expired: Vec<String> = redis::cmd("ZRANGEBYSCORE")
        .arg("wfg:leases")
        .arg("-inf")
        .arg(now)
        .query_async(&mut conn)
        .await
        .map_err(|e| QueueError::Internal(e.to_string()))?;

    let mut events = Vec::new();
    for lease_id in expired {
        // Move job back to pending (or fail if retries exhausted)
        // Remove from leases sorted set
        redis::cmd("ZREM")
            .arg("wfg:leases")
            .arg(&lease_id)
            .exec_async(&mut conn)
            .await
            .ok();

        // ... find job by lease_id, re-enqueue or fail
    }
    Ok(events)
}
```

## Event Distribution

For split deployments (separate API server + scheduler), use Redis Pub/Sub:

```rust
// In the scheduler process:
pub async fn listen_for_events(
    redis_url: &str,
    events_tx: broadcast::Sender<JobEvent>,
) {
    let client = redis::Client::open(redis_url).unwrap();
    let mut pubsub = client.get_async_pubsub().await.unwrap();
    pubsub.subscribe("wfg:events").await.unwrap();

    loop {
        let msg = pubsub.on_message().next().await;
        if let Some(msg) = msg {
            let payload: String = msg.get_payload().unwrap();
            if let Ok(event) = serde_json::from_str::<JobEvent>(&payload) {
                events_tx.send(event).ok();
            }
        }
    }
}
```

## Wiring into the Server

```rust
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".into());

    let backend = Arc::new(
        RedisBackend::new(&redis_url).await.expect("Redis connection failed")
    );

    let state = SharedState::new(WorkflowState::new());

    let scheduler = Arc::new(DagScheduler::new(
        backend.clone(),
        backend.clone(),
        state.clone(),
    ));
    tokio::spawn(scheduler.clone().run());

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

## Redis vs In-Memory vs Postgres

| Feature | In-Memory | Redis | Postgres |
|---------|-----------|-------|----------|
| Persistence | None (dev only) | Optional (RDB/AOF) | Full ACID |
| Atomic claiming | Mutex | Lua scripts | `FOR UPDATE SKIP LOCKED` |
| Event distribution | In-process broadcast | Pub/Sub | `LISTEN/NOTIFY` |
| Latency | ~0ms | ~1ms | ~5ms |
| Horizontal scaling | No | Redis Cluster | Connection pooling |
| Best for | Development | Low-latency production | Durable production |
