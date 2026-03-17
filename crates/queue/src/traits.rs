use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::error::{ArtifactError, LogError, QueueError, RegistryError};

// ─── Queue Types ─────────────────────────────────────────────────────────────

/// A lease proving a worker has exclusively claimed a job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lease {
    pub lease_id: String,
    pub job_id: String,
    pub workflow_id: String,
    pub worker_id: String,
    pub ttl_secs: u64,
    /// Epoch milliseconds when the lease was granted/last renewed.
    pub granted_at_ms: u64,
}

/// A job sitting in the queue, ready to be claimed by a worker.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueuedJob {
    pub job_id: String,
    pub workflow_id: String,
    pub command: String,
    pub required_labels: Vec<String>,
    pub retry_policy: RetryPolicy,
    pub attempt: u32,
    /// Outputs from upstream jobs, keyed by job_id then output key.
    pub upstream_outputs: HashMap<String, HashMap<String, String>>,
    pub enqueued_at_ms: u64,
    /// Epoch milliseconds before which this job should not be claimed (backoff delay).
    #[serde(default)]
    pub delayed_until_ms: u64,
}

/// Configurable retry behavior per job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff: BackoffStrategy,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 0,
            backoff: BackoffStrategy::None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BackoffStrategy {
    None,
    Fixed { delay_secs: u64 },
    Exponential { base_secs: u64, max_secs: u64 },
}

impl BackoffStrategy {
    /// Calculate delay in milliseconds for the given attempt number.
    pub fn delay_ms(&self, attempt: u32) -> u64 {
        match self {
            BackoffStrategy::None => 0,
            BackoffStrategy::Fixed { delay_secs } => delay_secs * 1000,
            BackoffStrategy::Exponential { base_secs, max_secs } => {
                let delay = base_secs.saturating_mul(2u64.saturating_pow(attempt));
                delay.min(*max_secs) * 1000
            }
        }
    }
}

/// Events emitted by the queue for the scheduler to react to.
#[derive(Clone, Debug)]
pub enum JobEvent {
    Ready {
        workflow_id: String,
        job_id: String,
    },
    Started {
        workflow_id: String,
        job_id: String,
        worker_id: String,
    },
    Completed {
        workflow_id: String,
        job_id: String,
        outputs: HashMap<String, String>,
    },
    Failed {
        workflow_id: String,
        job_id: String,
        error: String,
        retryable: bool,
    },
    Cancelled {
        workflow_id: String,
        job_id: String,
    },
    LeaseExpired {
        workflow_id: String,
        job_id: String,
        worker_id: String,
    },
}

// ─── Log Types ───────────────────────────────────────────────────────────────

/// A chunk of log output from a running job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogChunk {
    pub workflow_id: String,
    pub job_id: String,
    pub sequence: u64,
    pub data: String,
    pub timestamp_ms: u64,
    pub stream: LogStream,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogStream {
    Stdout,
    Stderr,
}

// ─── Worker Types ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerInfo {
    pub worker_id: String,
    pub labels: Vec<String>,
    pub registered_at_ms: u64,
    pub last_heartbeat_ms: u64,
    pub current_job: Option<String>,
    pub status: WorkerStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Idle,
    Busy,
    Offline,
}

// ─── Trait: JobQueue ─────────────────────────────────────────────────────────

/// Pluggable job queue backend.
///
/// Implementations: InMemoryJobQueue, PgBossJobQueue (Postgres), RedisJobQueue.
///
/// pg-boss mapping:
/// - `enqueue()` → `boss.send(queue, data, options)`
/// - `claim()`   → `boss.fetch(queue)` (SELECT ... FOR UPDATE SKIP LOCKED)
/// - `complete()` → `boss.complete(jobId)`
/// - `fail()`    → `boss.fail(jobId)`
/// - `cancel()`  → `boss.cancel(jobId)`
pub trait JobQueue: Send + Sync + 'static {
    /// Enqueue a job for execution. Called by the scheduler when dependencies are met.
    fn enqueue(&self, job: QueuedJob) -> impl Future<Output = Result<(), QueueError>> + Send;

    /// Atomically claim the next available job matching the worker's labels.
    /// Returns `None` if no matching job is available.
    fn claim(
        &self,
        worker_id: &str,
        worker_labels: &[String],
        lease_ttl: Duration,
    ) -> impl Future<Output = Result<Option<(QueuedJob, Lease)>, QueueError>> + Send;

    /// Renew a lease (heartbeat). Returns error if the lease has already expired.
    fn renew_lease(
        &self,
        lease_id: &str,
        extend_by: Duration,
    ) -> impl Future<Output = Result<(), QueueError>> + Send;

    /// Complete a job successfully. Releases the lease and stores outputs.
    fn complete(
        &self,
        lease_id: &str,
        outputs: HashMap<String, String>,
    ) -> impl Future<Output = Result<(), QueueError>> + Send;

    /// Fail a job. The queue decides whether to re-enqueue based on RetryPolicy.
    fn fail(
        &self,
        lease_id: &str,
        error: String,
        retryable: bool,
    ) -> impl Future<Output = Result<(), QueueError>> + Send;

    /// Cancel a specific job. If currently claimed, marks it for cancellation.
    fn cancel(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> impl Future<Output = Result<(), QueueError>> + Send;

    /// Cancel all jobs for a workflow.
    fn cancel_workflow(
        &self,
        workflow_id: &str,
    ) -> impl Future<Output = Result<(), QueueError>> + Send;

    /// Check if a job has been marked for cancellation (workers poll this).
    fn is_cancelled(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> impl Future<Output = Result<bool, QueueError>> + Send;

    /// Collect expired leases and emit LeaseExpired events.
    /// Called periodically by the server's monitor task.
    fn reap_expired_leases(&self)
    -> impl Future<Output = Result<Vec<JobEvent>, QueueError>> + Send;

    /// Subscribe to job events for event-driven processing.
    fn subscribe(&self) -> broadcast::Receiver<JobEvent>;
}

// ─── Trait: ArtifactStore ────────────────────────────────────────────────────

/// Pluggable artifact/output storage.
///
/// Jobs publish key-value outputs; downstream jobs read upstream outputs.
/// Implementations: InMemoryArtifactStore, S3ArtifactStore, FsArtifactStore.
pub trait ArtifactStore: Send + Sync + 'static {
    /// Store key-value outputs for a completed job.
    fn put_outputs(
        &self,
        workflow_id: &str,
        job_id: &str,
        outputs: HashMap<String, String>,
    ) -> impl Future<Output = Result<(), ArtifactError>> + Send;

    /// Retrieve outputs for a specific job.
    fn get_outputs(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> impl Future<Output = Result<HashMap<String, String>, ArtifactError>> + Send;

    /// Retrieve outputs for multiple upstream jobs at once.
    fn get_upstream_outputs(
        &self,
        workflow_id: &str,
        job_ids: &[String],
    ) -> impl Future<Output = Result<HashMap<String, HashMap<String, String>>, ArtifactError>> + Send;
}

// ─── Trait: LogSink ──────────────────────────────────────────────────────────

/// Pluggable log storage and streaming backend.
///
/// Workers push log chunks; the server streams them to clients via SSE.
/// Implementations: InMemoryLogSink, FileLogSink, S3LogSink.
pub trait LogSink: Send + Sync + 'static {
    /// Append a log chunk from a worker.
    fn append(&self, chunk: LogChunk) -> impl Future<Output = Result<(), LogError>> + Send;

    /// Get all log chunks for a job (for catch-up on SSE connect).
    fn get_all(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> impl Future<Output = Result<Vec<LogChunk>, LogError>> + Send;

    /// Subscribe to live log chunks for a specific job.
    fn subscribe(&self, workflow_id: &str, job_id: &str) -> broadcast::Receiver<LogChunk>;
}

// ─── Trait: WorkerRegistry ───────────────────────────────────────────────────

/// Registry of connected workers and their capabilities.
///
/// Used for monitoring and matching jobs to capable workers.
pub trait WorkerRegistry: Send + Sync + 'static {
    /// Register a worker with its capability labels.
    fn register(
        &self,
        worker_id: &str,
        labels: &[String],
    ) -> impl Future<Output = Result<(), RegistryError>> + Send;

    /// Record a heartbeat from a worker.
    fn heartbeat(&self, worker_id: &str) -> impl Future<Output = Result<(), RegistryError>> + Send;

    /// Remove a worker from the registry.
    fn deregister(&self, worker_id: &str)
    -> impl Future<Output = Result<(), RegistryError>> + Send;

    /// List all registered workers.
    fn list_workers(&self) -> impl Future<Output = Result<Vec<WorkerInfo>, RegistryError>> + Send;

    /// Mark a worker as busy with a specific job.
    fn mark_busy(
        &self,
        worker_id: &str,
        job_id: &str,
    ) -> impl Future<Output = Result<(), RegistryError>> + Send;

    /// Mark a worker as idle (finished or released a job).
    fn mark_idle(&self, worker_id: &str) -> impl Future<Output = Result<(), RegistryError>> + Send;
}
