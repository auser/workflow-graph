# Redis Backend

Implement all four backend traits (`JobQueue`, `ArtifactStore`, `LogSink`, `WorkerRegistry`) using Redis. The job queue uses a Lua script for atomic claiming, and events are distributed via Redis Pub/Sub.

## Prerequisites

Add to your `Cargo.toml`:

```toml
[dependencies]
workflow-graph-queue = { path = "crates/queue" }
redis = { version = "0.27", features = ["tokio-comp", "aio"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
```

## Redis Data Model

| Key Pattern | Type | Purpose |
|-------------|------|---------|
| `wfg:pending` | Sorted Set | Pending jobs, scored by `enqueued_at_ms` (FIFO) |
| `wfg:job:{wf_id}:{job_id}:{attempt}` | Hash | Full `QueuedJob` data |
| `wfg:lease:{lease_id}` | String (with TTL) | Lease data as JSON; Redis TTL = lease expiry |
| `wfg:cancelled` | Set | Members are `{wf_id}:{job_id}` |
| `wfg:artifacts:{wf_id}:{job_id}` | Hash | Key-value outputs |
| `wfg:logs:{wf_id}:{job_id}` | List | JSON-serialized `LogChunk` entries |
| `wfg:workers:{worker_id}` | Hash | `WorkerInfo` fields |
| `wfg:workers:index` | Set | All registered worker IDs |
| `wfg:events` | Pub/Sub channel | `JobEvent` broadcast |
| `wfg:log_events` | Pub/Sub channel | `LogChunk` broadcast |

## RedisBackend Struct

```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use tokio::sync::broadcast;

use workflow_graph_queue::traits::*;
use workflow_graph_queue::error::*;

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

    /// Spawn a background task that subscribes to Redis Pub/Sub and
    /// re-broadcasts events on the in-process channel. Required for
    /// multi-process setups (separate API server and scheduler).
    pub async fn spawn_event_bridge(
        redis_url: &str,
        events: broadcast::Sender<JobEvent>,
    ) -> Result<(), redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        let mut pubsub = client.get_async_pubsub().await?;
        pubsub.subscribe("wfg:events").await?;

        tokio::spawn(async move {
            loop {
                let msg: redis::Msg = match pubsub.on_message().next().await {
                    Some(msg) => msg,
                    None => break,
                };
                if let Ok(payload) = msg.get_payload::<String>() {
                    if let Ok(event) = serde_json::from_str::<JobEvent>(&payload) {
                        events.send(event).ok();
                    }
                }
            }
        });

        Ok(())
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn job_key(wf_id: &str, job_id: &str, attempt: u32) -> String {
    format!("wfg:job:{wf_id}:{job_id}:{attempt}")
}
```

## Atomic Claim via Lua Script

The most critical operation. This Lua script atomically:
1. Pops the oldest pending job from the sorted set
2. Checks if the worker's labels satisfy the job's required labels
3. Creates a lease key with TTL
4. Returns the job and lease data

```lua
-- KEYS[1] = "wfg:pending"
-- ARGV[1] = worker_labels JSON array
-- ARGV[2] = worker_id
-- ARGV[3] = lease_ttl_secs
-- ARGV[4] = lease_id
-- ARGV[5] = now_ms

local candidates = redis.call('ZRANGEBYSCORE', KEYS[1], '-inf', '+inf', 'LIMIT', 0, 20)
local worker_labels = cjson.decode(ARGV[1])

-- Build a set from worker labels for O(1) lookup
local label_set = {}
for _, l in ipairs(worker_labels) do
    label_set[l] = true
end

for _, job_key in ipairs(candidates) do
    local job_json = redis.call('GET', job_key)
    if job_json then
        local job = cjson.decode(job_json)
        local required = job.required_labels or {}

        -- Check if worker has all required labels
        local match = true
        for _, req in ipairs(required) do
            if not label_set[req] then
                match = false
                break
            end
        end

        if match then
            -- Remove from pending set
            redis.call('ZREM', KEYS[1], job_key)

            -- Create lease
            local lease = {
                lease_id = ARGV[4],
                job_id = job.job_id,
                workflow_id = job.workflow_id,
                worker_id = ARGV[2],
                ttl_secs = tonumber(ARGV[3]),
                granted_at_ms = tonumber(ARGV[5])
            }
            local lease_key = 'wfg:lease:' .. ARGV[4]
            redis.call('SET', lease_key, cjson.encode(lease), 'EX', tonumber(ARGV[3]))

            -- Also store a reverse mapping: lease -> job_key (for complete/fail)
            redis.call('SET', 'wfg:lease_job:' .. ARGV[4], job_key, 'EX', tonumber(ARGV[3]))

            return {job_json, cjson.encode(lease)}
        end
    end
end

return nil
```

## impl JobQueue for RedisBackend

```rust
const CLAIM_SCRIPT: &str = r#"
local candidates = redis.call('ZRANGEBYSCORE', KEYS[1], '-inf', '+inf', 'LIMIT', 0, 20)
local worker_labels = cjson.decode(ARGV[1])
local label_set = {}
for _, l in ipairs(worker_labels) do label_set[l] = true end

for _, job_key in ipairs(candidates) do
    local job_json = redis.call('GET', job_key)
    if job_json then
        local job = cjson.decode(job_json)
        local required = job.required_labels or {}
        local match = true
        for _, req in ipairs(required) do
            if not label_set[req] then match = false; break end
        end
        if match then
            redis.call('ZREM', KEYS[1], job_key)
            local lease = {
                lease_id = ARGV[4], job_id = job.job_id,
                workflow_id = job.workflow_id, worker_id = ARGV[2],
                ttl_secs = tonumber(ARGV[3]), granted_at_ms = tonumber(ARGV[5])
            }
            local lease_key = 'wfg:lease:' .. ARGV[4]
            redis.call('SET', lease_key, cjson.encode(lease), 'EX', tonumber(ARGV[3]))
            redis.call('SET', 'wfg:lease_job:' .. ARGV[4], job_key, 'EX', tonumber(ARGV[3]))
            return {job_json, cjson.encode(lease)}
        end
    end
end
return nil
"#;

impl JobQueue for RedisBackend {
    async fn enqueue(&self, job: QueuedJob) -> Result<(), QueueError> {
        let mut conn = self.conn.clone();
        let key = job_key(&job.workflow_id, &job.job_id, job.attempt);
        let job_json = serde_json::to_string(&job)
            .map_err(|e| QueueError::Internal(e.to_string()))?;

        // Store job data
        let _: () = conn.set(&key, &job_json).await
            .map_err(|e| QueueError::Internal(e.to_string()))?;

        // Add to pending sorted set (score = enqueued_at for FIFO ordering)
        let _: () = conn.zadd("wfg:pending", &key, job.enqueued_at_ms as f64).await
            .map_err(|e| QueueError::Internal(e.to_string()))?;

        let event = JobEvent::Ready {
            workflow_id: job.workflow_id.clone(),
            job_id: job.job_id.clone(),
        };
        self.publish_event(&mut conn, &event).await;
        self.events.send(event).ok();

        Ok(())
    }

    async fn claim(
        &self,
        worker_id: &str,
        worker_labels: &[String],
        lease_ttl: Duration,
    ) -> Result<Option<(QueuedJob, Lease)>, QueueError> {
        let mut conn = self.conn.clone();
        let lease_id = uuid::Uuid::new_v4().to_string();

        let result: Option<(String, String)> = redis::Script::new(CLAIM_SCRIPT)
            .key("wfg:pending")
            .arg(serde_json::to_string(worker_labels).unwrap())
            .arg(worker_id)
            .arg(lease_ttl.as_secs())
            .arg(&lease_id)
            .arg(now_ms())
            .invoke_async(&mut conn)
            .await
            .map_err(|e| QueueError::Internal(e.to_string()))?;

        let Some((job_json, lease_json)) = result else {
            return Ok(None);
        };

        let job: QueuedJob = serde_json::from_str(&job_json)
            .map_err(|e| QueueError::Internal(e.to_string()))?;
        let lease: Lease = serde_json::from_str(&lease_json)
            .map_err(|e| QueueError::Internal(e.to_string()))?;

        let event = JobEvent::Started {
            workflow_id: job.workflow_id.clone(),
            job_id: job.job_id.clone(),
            worker_id: worker_id.to_string(),
        };
        self.publish_event(&mut conn, &event).await;
        self.events.send(event).ok();

        Ok(Some((job, lease)))
    }

    async fn renew_lease(
        &self,
        lease_id: &str,
        extend_by: Duration,
    ) -> Result<(), QueueError> {
        let mut conn = self.conn.clone();
        let key = format!("wfg:lease:{lease_id}");
        let job_key = format!("wfg:lease_job:{lease_id}");

        // Extend TTL on both the lease and the reverse mapping
        let existed: bool = conn.expire(&key, extend_by.as_secs() as i64).await
            .map_err(|e| QueueError::Internal(e.to_string()))?;

        if !existed {
            return Err(QueueError::LeaseNotFound(lease_id.to_string()));
        }

        let _: () = conn.expire(&job_key, extend_by.as_secs() as i64).await
            .map_err(|e| QueueError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn complete(
        &self,
        lease_id: &str,
        outputs: HashMap<String, String>,
    ) -> Result<(), QueueError> {
        let mut conn = self.conn.clone();
        let lease_key = format!("wfg:lease:{lease_id}");

        // Get and delete lease
        let lease_json: Option<String> = conn.get(&lease_key).await
            .map_err(|e| QueueError::Internal(e.to_string()))?;

        let Some(lease_json) = lease_json else {
            return Err(QueueError::LeaseNotFound(lease_id.to_string()));
        };

        let lease: Lease = serde_json::from_str(&lease_json)
            .map_err(|e| QueueError::Internal(e.to_string()))?;

        // Clean up lease keys
        let _: () = conn.del(&lease_key).await
            .map_err(|e| QueueError::Internal(e.to_string()))?;
        let _: () = conn.del(format!("wfg:lease_job:{lease_id}")).await
            .map_err(|e| QueueError::Internal(e.to_string()))?;

        let event = JobEvent::Completed {
            workflow_id: lease.workflow_id,
            job_id: lease.job_id,
            outputs,
        };
        self.publish_event(&mut conn, &event).await;
        self.events.send(event).ok();

        Ok(())
    }

    async fn fail(
        &self,
        lease_id: &str,
        error: String,
        retryable: bool,
    ) -> Result<(), QueueError> {
        let mut conn = self.conn.clone();
        let lease_key = format!("wfg:lease:{lease_id}");
        let job_map_key = format!("wfg:lease_job:{lease_id}");

        let lease_json: Option<String> = conn.get(&lease_key).await
            .map_err(|e| QueueError::Internal(e.to_string()))?;
        let job_data_key: Option<String> = conn.get(&job_map_key).await
            .map_err(|e| QueueError::Internal(e.to_string()))?;

        let Some(lease_json) = lease_json else {
            return Err(QueueError::LeaseNotFound(lease_id.to_string()));
        };

        let lease: Lease = serde_json::from_str(&lease_json)
            .map_err(|e| QueueError::Internal(e.to_string()))?;

        // Clean up lease
        let _: () = conn.del(&lease_key).await.unwrap_or(());
        let _: () = conn.del(&job_map_key).await.unwrap_or(());

        // Check retry
        let mut should_retry = false;
        if retryable {
            if let Some(ref jk) = job_data_key {
                let job_json: Option<String> = conn.get(jk).await.unwrap_or(None);
                if let Some(job_json) = job_json {
                    if let Ok(job) = serde_json::from_str::<QueuedJob>(&job_json) {
                        if job.attempt < job.retry_policy.max_retries {
                            should_retry = true;
                            // Re-enqueue with incremented attempt
                            let mut retried = job;
                            retried.attempt += 1;
                            retried.enqueued_at_ms = now_ms();
                            // Store under new key and add to pending
                            let new_key = job_key(
                                &retried.workflow_id, &retried.job_id, retried.attempt,
                            );
                            let new_json = serde_json::to_string(&retried).unwrap();
                            let _: () = conn.set(&new_key, &new_json).await.unwrap_or(());
                            let _: () = conn.zadd(
                                "wfg:pending", &new_key, retried.enqueued_at_ms as f64,
                            ).await.unwrap_or(());
                        }
                    }
                }
            }
        }

        let event = JobEvent::Failed {
            workflow_id: lease.workflow_id,
            job_id: lease.job_id,
            error,
            retryable: should_retry,
        };
        self.publish_event(&mut conn, &event).await;
        self.events.send(event).ok();

        Ok(())
    }

    async fn cancel(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> Result<(), QueueError> {
        let mut conn = self.conn.clone();

        // Add to cancelled set
        let _: () = conn.sadd("wfg:cancelled", format!("{workflow_id}:{job_id}")).await
            .map_err(|e| QueueError::Internal(e.to_string()))?;

        // Remove any pending entries for this job (all attempts)
        let members: Vec<String> = conn.zrangebyscore("wfg:pending", "-inf", "+inf").await
            .unwrap_or_default();
        for member in members {
            if member.contains(&format!("{workflow_id}:{job_id}:")) {
                let _: () = conn.zrem("wfg:pending", &member).await.unwrap_or(());
            }
        }

        let event = JobEvent::Cancelled {
            workflow_id: workflow_id.to_string(),
            job_id: job_id.to_string(),
        };
        self.publish_event(&mut conn, &event).await;
        self.events.send(event).ok();

        Ok(())
    }

    async fn cancel_workflow(
        &self,
        workflow_id: &str,
    ) -> Result<(), QueueError> {
        let mut conn = self.conn.clone();

        // Find and remove all pending jobs for this workflow
        let members: Vec<String> = conn.zrangebyscore("wfg:pending", "-inf", "+inf").await
            .unwrap_or_default();

        for member in &members {
            if member.starts_with(&format!("wfg:job:{workflow_id}:")) {
                let _: () = conn.zrem("wfg:pending", member).await.unwrap_or(());

                // Extract job_id from key pattern wfg:job:{wf_id}:{job_id}:{attempt}
                let parts: Vec<&str> = member.split(':').collect();
                if parts.len() >= 4 {
                    let job_id = parts[3];
                    let _: () = conn.sadd(
                        "wfg:cancelled", format!("{workflow_id}:{job_id}"),
                    ).await.unwrap_or(());

                    let event = JobEvent::Cancelled {
                        workflow_id: workflow_id.to_string(),
                        job_id: job_id.to_string(),
                    };
                    self.publish_event(&mut conn, &event).await;
                    self.events.send(event).ok();
                }
            }
        }

        Ok(())
    }

    async fn is_cancelled(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> Result<bool, QueueError> {
        let mut conn = self.conn.clone();
        let result: bool = conn.sismember("wfg:cancelled", format!("{workflow_id}:{job_id}"))
            .await
            .map_err(|e| QueueError::Internal(e.to_string()))?;
        Ok(result)
    }

    async fn reap_expired_leases(&self) -> Result<Vec<JobEvent>, QueueError> {
        // Redis TTL handles lease expiry automatically — when a lease key expires,
        // the job data remains. We scan for job keys that have no corresponding lease.
        //
        // Alternative: maintain a sorted set of active leases by expiry time.
        // For simplicity, this implementation uses a periodic scan.

        // This is a simplified approach. In production, use Redis keyspace
        // notifications (CONFIG SET notify-keyspace-events Ex) to get notified
        // when lease keys expire, then re-enqueue the job.

        Ok(vec![])
    }

    fn subscribe(&self) -> broadcast::Receiver<JobEvent> {
        self.events.subscribe()
    }
}

impl RedisBackend {
    async fn publish_event(&self, conn: &mut MultiplexedConnection, event: &JobEvent) {
        let json = serde_json::to_string(event).unwrap_or_default();
        let _: Result<(), _> = conn.publish("wfg:events", &json).await;
    }
}
```

### Lease Expiry via Keyspace Notifications

For production use, enable Redis keyspace notifications to detect expired leases:

```rust
// At startup, enable notifications for expired keys:
let _: () = redis::cmd("CONFIG")
    .arg("SET")
    .arg("notify-keyspace-events")
    .arg("Ex")
    .query_async(&mut conn)
    .await?;

// Subscribe to expiry events in a background task:
let mut pubsub = client.get_async_pubsub().await?;
pubsub.psubscribe("__keyevent@0__:expired").await?;

tokio::spawn(async move {
    loop {
        let msg: redis::Msg = match pubsub.on_message().next().await {
            Some(msg) => msg,
            None => break,
        };
        let key: String = msg.get_payload().unwrap_or_default();
        if key.starts_with("wfg:lease:") {
            let lease_id = key.strip_prefix("wfg:lease:").unwrap();
            // Look up the job via wfg:lease_job:{lease_id} (also expired, but
            // we stored the job data separately). Re-enqueue if retries remain.
        }
    }
});
```

## impl ArtifactStore for RedisBackend

```rust
impl ArtifactStore for RedisBackend {
    async fn put_outputs(
        &self,
        workflow_id: &str,
        job_id: &str,
        outputs: HashMap<String, String>,
    ) -> Result<(), ArtifactError> {
        let mut conn = self.conn.clone();
        let key = format!("wfg:artifacts:{workflow_id}:{job_id}");

        if outputs.is_empty() {
            return Ok(());
        }

        let pairs: Vec<(String, String)> = outputs.into_iter().collect();
        let _: () = conn.hset_multiple(&key, &pairs).await
            .map_err(|e| ArtifactError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn get_outputs(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> Result<HashMap<String, String>, ArtifactError> {
        let mut conn = self.conn.clone();
        let key = format!("wfg:artifacts:{workflow_id}:{job_id}");

        let result: HashMap<String, String> = conn.hgetall(&key).await
            .map_err(|e| ArtifactError::Internal(e.to_string()))?;

        Ok(result)
    }

    async fn get_upstream_outputs(
        &self,
        workflow_id: &str,
        job_ids: &[String],
    ) -> Result<HashMap<String, HashMap<String, String>>, ArtifactError> {
        let mut conn = self.conn.clone();
        let mut result = HashMap::new();

        // Use pipeline for efficiency
        let mut pipe = redis::pipe();
        for job_id in job_ids {
            let key = format!("wfg:artifacts:{workflow_id}:{job_id}");
            pipe.hgetall(key);
        }

        let values: Vec<HashMap<String, String>> = pipe.query_async(&mut conn).await
            .map_err(|e| ArtifactError::Internal(e.to_string()))?;

        for (job_id, outputs) in job_ids.iter().zip(values) {
            result.insert(job_id.clone(), outputs);
        }

        Ok(result)
    }
}
```

## impl LogSink for RedisBackend

```rust
impl LogSink for RedisBackend {
    async fn append(&self, chunk: LogChunk) -> Result<(), LogError> {
        let mut conn = self.conn.clone();
        let key = format!("wfg:logs:{}:{}", chunk.workflow_id, chunk.job_id);
        let chunk_json = serde_json::to_string(&chunk)
            .map_err(|e| LogError::Internal(e.to_string()))?;

        // Append to list
        let _: () = conn.rpush(&key, &chunk_json).await
            .map_err(|e| LogError::Internal(e.to_string()))?;

        // Publish for live streaming
        let _: Result<(), _> = conn.publish("wfg:log_events", &chunk_json).await;

        // Broadcast in-process
        self.log_events.send(chunk).ok();

        Ok(())
    }

    async fn get_all(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> Result<Vec<LogChunk>, LogError> {
        let mut conn = self.conn.clone();
        let key = format!("wfg:logs:{workflow_id}:{job_id}");

        let items: Vec<String> = conn.lrange(&key, 0, -1).await
            .map_err(|e| LogError::Internal(e.to_string()))?;

        let chunks: Vec<LogChunk> = items
            .iter()
            .filter_map(|s| serde_json::from_str(s).ok())
            .collect();

        Ok(chunks)
    }

    fn subscribe(&self, _workflow_id: &str, _job_id: &str) -> broadcast::Receiver<LogChunk> {
        self.log_events.subscribe()
    }
}
```

## impl WorkerRegistry for RedisBackend

```rust
impl WorkerRegistry for RedisBackend {
    async fn register(
        &self,
        worker_id: &str,
        labels: &[String],
    ) -> Result<(), RegistryError> {
        let mut conn = self.conn.clone();
        let key = format!("wfg:workers:{worker_id}");

        let _: () = redis::pipe()
            .hset(&key, "worker_id", worker_id)
            .hset(&key, "labels", serde_json::to_string(labels).unwrap())
            .hset(&key, "registered_at_ms", now_ms().to_string())
            .hset(&key, "last_heartbeat_ms", now_ms().to_string())
            .hset(&key, "status", "idle")
            .sadd("wfg:workers:index", worker_id)
            .query_async(&mut conn)
            .await
            .map_err(|e| RegistryError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn heartbeat(&self, worker_id: &str) -> Result<(), RegistryError> {
        let mut conn = self.conn.clone();
        let key = format!("wfg:workers:{worker_id}");

        let exists: bool = conn.exists(&key).await
            .map_err(|e| RegistryError::Internal(e.to_string()))?;

        if !exists {
            return Err(RegistryError::NotFound(worker_id.to_string()));
        }

        let _: () = conn.hset(&key, "last_heartbeat_ms", now_ms().to_string()).await
            .map_err(|e| RegistryError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn deregister(&self, worker_id: &str) -> Result<(), RegistryError> {
        let mut conn = self.conn.clone();
        let _: () = redis::pipe()
            .del(format!("wfg:workers:{worker_id}"))
            .srem("wfg:workers:index", worker_id)
            .query_async(&mut conn)
            .await
            .map_err(|e| RegistryError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn list_workers(&self) -> Result<Vec<WorkerInfo>, RegistryError> {
        let mut conn = self.conn.clone();

        let ids: Vec<String> = conn.smembers("wfg:workers:index").await
            .map_err(|e| RegistryError::Internal(e.to_string()))?;

        let mut workers = Vec::new();
        for id in ids {
            let key = format!("wfg:workers:{id}");
            let data: HashMap<String, String> = conn.hgetall(&key).await.unwrap_or_default();

            if data.is_empty() {
                continue;
            }

            workers.push(WorkerInfo {
                worker_id: id,
                labels: data.get("labels")
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default(),
                registered_at_ms: data.get("registered_at_ms")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                last_heartbeat_ms: data.get("last_heartbeat_ms")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                current_job: data.get("current_job").cloned(),
                status: match data.get("status").map(|s| s.as_str()) {
                    Some("busy") => WorkerStatus::Busy,
                    Some("offline") => WorkerStatus::Offline,
                    _ => WorkerStatus::Idle,
                },
            });
        }

        Ok(workers)
    }

    async fn mark_busy(
        &self,
        worker_id: &str,
        job_id: &str,
    ) -> Result<(), RegistryError> {
        let mut conn = self.conn.clone();
        let key = format!("wfg:workers:{worker_id}");

        let _: () = redis::pipe()
            .hset(&key, "status", "busy")
            .hset(&key, "current_job", job_id)
            .query_async(&mut conn)
            .await
            .map_err(|e| RegistryError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn mark_idle(&self, worker_id: &str) -> Result<(), RegistryError> {
        let mut conn = self.conn.clone();
        let key = format!("wfg:workers:{worker_id}");

        let _: () = redis::pipe()
            .hset(&key, "status", "idle")
            .hdel(&key, "current_job")
            .query_async(&mut conn)
            .await
            .map_err(|e| RegistryError::Internal(e.to_string()))?;

        Ok(())
    }
}
```

## Wiring into the Server

Same pattern as the [Postgres guide](guide-postgres.md#wiring-into-the-server) — replace `InMemory*` types with `RedisBackend`:

```rust
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1/".into());

    let backend = Arc::new(
        RedisBackend::new(&redis_url).await.expect("Redis connect failed")
    );

    let state = SharedState::new(WorkflowState::new());

    let scheduler = Arc::new(DagScheduler::new(
        backend.clone(),
        backend.clone(),
        state.clone(),
    ));
    tokio::spawn(scheduler.clone().run());

    // Optional: spawn Pub/Sub bridge for multi-process setups
    // RedisBackend::spawn_event_bridge(&redis_url, backend.events.clone()).await.ok();

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

## Performance Considerations

### Connection Pooling

`MultiplexedConnection` multiplexes commands over a single TCP connection. This works well for most workloads. For high-throughput scenarios, use `ConnectionManager` which reconnects automatically:

```rust
let manager = redis::aio::ConnectionManager::new(client).await?;
```

### Pipeline Batching

Use `redis::pipe()` to batch multiple commands in a single round-trip. This is used in the `WorkerRegistry` and `ArtifactStore` implementations above. For log chunks, consider batching multiple `RPUSH` calls:

```rust
let mut pipe = redis::pipe();
for chunk in chunks {
    pipe.rpush(&key, serde_json::to_string(&chunk).unwrap());
}
pipe.query_async(&mut conn).await?;
```

### Key Expiry and Cleanup

Set TTLs on job data keys to prevent unbounded growth:

```rust
// After completing a job, expire its data after 24 hours
let _: () = conn.expire(&job_key, 86400).await?;

// Expire log data after 7 days
let _: () = conn.expire(&log_key, 604800).await?;
```

### Sorted Set Scanning

The `cancel_workflow` implementation scans the entire pending sorted set. For large queues, consider using a secondary index:

```
wfg:pending:{workflow_id}  →  Set of job keys for this workflow
```

This allows O(1) lookup instead of scanning all pending jobs.
