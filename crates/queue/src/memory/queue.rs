use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

use tokio::sync::{broadcast, Mutex};

use crate::error::QueueError;
use crate::traits::*;

struct Inner {
    pending: VecDeque<QueuedJob>,
    active: HashMap<String, (Lease, QueuedJob)>, // keyed by lease_id
    cancelled: HashSet<(String, String)>,         // (workflow_id, job_id)
}

pub struct InMemoryJobQueue {
    inner: Mutex<Inner>,
    events: broadcast::Sender<JobEvent>,
}

impl InMemoryJobQueue {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            inner: Mutex::new(Inner {
                pending: VecDeque::new(),
                active: HashMap::new(),
                cancelled: HashSet::new(),
            }),
            events: tx,
        }
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

impl Default for InMemoryJobQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl JobQueue for InMemoryJobQueue {
    async fn enqueue(&self, job: QueuedJob) -> Result<(), QueueError> {
        let event = JobEvent::Ready {
            workflow_id: job.workflow_id.clone(),
            job_id: job.job_id.clone(),
        };
        self.inner.lock().await.pending.push_back(job);
        self.events.send(event).ok();
        Ok(())
    }

    async fn claim(
        &self,
        worker_id: &str,
        worker_labels: &[String],
        lease_ttl: Duration,
    ) -> Result<Option<(QueuedJob, Lease)>, QueueError> {
        let mut inner = self.inner.lock().await;

        // Find first pending job whose required_labels are a subset of worker's labels
        let pos = inner.pending.iter().position(|job| {
            job.required_labels
                .iter()
                .all(|label| worker_labels.contains(label))
        });

        let Some(idx) = pos else {
            return Ok(None);
        };

        let job = inner.pending.remove(idx).unwrap();
        let lease = Lease {
            lease_id: uuid::Uuid::new_v4().to_string(),
            job_id: job.job_id.clone(),
            workflow_id: job.workflow_id.clone(),
            worker_id: worker_id.to_string(),
            ttl_secs: lease_ttl.as_secs(),
            granted_at_ms: Self::now_ms(),
        };

        inner
            .active
            .insert(lease.lease_id.clone(), (lease.clone(), job.clone()));

        let event = JobEvent::Started {
            workflow_id: job.workflow_id.clone(),
            job_id: job.job_id.clone(),
            worker_id: worker_id.to_string(),
        };
        drop(inner);
        self.events.send(event).ok();

        Ok(Some((job, lease)))
    }

    async fn renew_lease(
        &self,
        lease_id: &str,
        extend_by: Duration,
    ) -> Result<(), QueueError> {
        let mut inner = self.inner.lock().await;
        let (lease, _) = inner
            .active
            .get_mut(lease_id)
            .ok_or_else(|| QueueError::LeaseNotFound(lease_id.to_string()))?;

        lease.granted_at_ms = Self::now_ms();
        lease.ttl_secs = extend_by.as_secs();
        Ok(())
    }

    async fn complete(
        &self,
        lease_id: &str,
        outputs: HashMap<String, String>,
    ) -> Result<(), QueueError> {
        let mut inner = self.inner.lock().await;
        let (_, job) = inner
            .active
            .remove(lease_id)
            .ok_or_else(|| QueueError::LeaseNotFound(lease_id.to_string()))?;

        let event = JobEvent::Completed {
            workflow_id: job.workflow_id.clone(),
            job_id: job.job_id.clone(),
            outputs,
        };
        drop(inner);
        self.events.send(event).ok();
        Ok(())
    }

    async fn fail(
        &self,
        lease_id: &str,
        error: String,
        retryable: bool,
    ) -> Result<(), QueueError> {
        let mut inner = self.inner.lock().await;
        let (_, job) = inner
            .active
            .remove(lease_id)
            .ok_or_else(|| QueueError::LeaseNotFound(lease_id.to_string()))?;

        let should_retry =
            retryable && job.attempt < job.retry_policy.max_retries;

        if should_retry {
            // Re-enqueue with incremented attempt
            let mut retried = job.clone();
            retried.attempt += 1;
            retried.enqueued_at_ms = Self::now_ms();
            inner.pending.push_back(retried);
        }

        let event = JobEvent::Failed {
            workflow_id: job.workflow_id.clone(),
            job_id: job.job_id.clone(),
            error,
            retryable: should_retry,
        };
        drop(inner);
        self.events.send(event).ok();
        Ok(())
    }

    async fn cancel(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> Result<(), QueueError> {
        let mut inner = self.inner.lock().await;

        // Remove from pending if present
        inner
            .pending
            .retain(|j| !(j.workflow_id == workflow_id && j.job_id == job_id));

        // Mark as cancelled (so active workers can check)
        inner
            .cancelled
            .insert((workflow_id.to_string(), job_id.to_string()));

        let event = JobEvent::Cancelled {
            workflow_id: workflow_id.to_string(),
            job_id: job_id.to_string(),
        };
        drop(inner);
        self.events.send(event).ok();
        Ok(())
    }

    async fn cancel_workflow(&self, workflow_id: &str) -> Result<(), QueueError> {
        let mut inner = self.inner.lock().await;

        // Collect job IDs to cancel
        let pending_ids: Vec<String> = inner
            .pending
            .iter()
            .filter(|j| j.workflow_id == workflow_id)
            .map(|j| j.job_id.clone())
            .collect();
        let active_ids: Vec<String> = inner
            .active
            .values()
            .filter(|(_, j)| j.workflow_id == workflow_id)
            .map(|(_, j)| j.job_id.clone())
            .collect();

        // Remove pending jobs
        inner.pending.retain(|j| j.workflow_id != workflow_id);

        // Mark all as cancelled
        for id in pending_ids.iter().chain(active_ids.iter()) {
            inner
                .cancelled
                .insert((workflow_id.to_string(), id.clone()));
        }

        drop(inner);

        for id in pending_ids.iter().chain(active_ids.iter()) {
            self.events
                .send(JobEvent::Cancelled {
                    workflow_id: workflow_id.to_string(),
                    job_id: id.clone(),
                })
                .ok();
        }

        Ok(())
    }

    async fn is_cancelled(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> Result<bool, QueueError> {
        let inner = self.inner.lock().await;
        Ok(inner
            .cancelled
            .contains(&(workflow_id.to_string(), job_id.to_string())))
    }

    async fn reap_expired_leases(&self) -> Result<Vec<JobEvent>, QueueError> {
        let mut inner = self.inner.lock().await;
        let now = Self::now_ms();
        let mut events = Vec::new();

        let expired_ids: Vec<String> = inner
            .active
            .iter()
            .filter(|(_, (lease, _))| {
                let expires_at = lease.granted_at_ms + lease.ttl_secs * 1000;
                now > expires_at
            })
            .map(|(id, _)| id.clone())
            .collect();

        for lease_id in expired_ids {
            let (lease, job) = inner.active.remove(&lease_id).unwrap();

            events.push(JobEvent::LeaseExpired {
                workflow_id: job.workflow_id.clone(),
                job_id: job.job_id.clone(),
                worker_id: lease.worker_id.clone(),
            });

            // Re-enqueue if retries remain
            if job.attempt < job.retry_policy.max_retries {
                let mut retried = job;
                retried.attempt += 1;
                retried.enqueued_at_ms = now;
                inner.pending.push_back(retried);
            }
        }

        drop(inner);
        for event in &events {
            self.events.send(event.clone()).ok();
        }

        Ok(events)
    }

    fn subscribe(&self) -> broadcast::Receiver<JobEvent> {
        self.events.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enqueue_and_claim() {
        let queue = InMemoryJobQueue::new();
        let job = QueuedJob {
            job_id: "j1".into(),
            workflow_id: "wf1".into(),
            command: "echo hello".into(),
            required_labels: vec![],
            retry_policy: RetryPolicy::default(),
            attempt: 0,
            upstream_outputs: HashMap::new(),
            enqueued_at_ms: 0,
        };

        queue.enqueue(job).await.unwrap();

        let result = queue
            .claim("w1", &[], Duration::from_secs(30))
            .await
            .unwrap();
        assert!(result.is_some());

        let (claimed_job, lease) = result.unwrap();
        assert_eq!(claimed_job.job_id, "j1");
        assert_eq!(lease.worker_id, "w1");

        // Queue should be empty now
        let result2 = queue
            .claim("w2", &[], Duration::from_secs(30))
            .await
            .unwrap();
        assert!(result2.is_none());
    }

    #[tokio::test]
    async fn test_claim_respects_labels() {
        let queue = InMemoryJobQueue::new();
        let job = QueuedJob {
            job_id: "j1".into(),
            workflow_id: "wf1".into(),
            command: "echo hello".into(),
            required_labels: vec!["docker".into()],
            retry_policy: RetryPolicy::default(),
            attempt: 0,
            upstream_outputs: HashMap::new(),
            enqueued_at_ms: 0,
        };

        queue.enqueue(job).await.unwrap();

        // Worker without docker label can't claim
        let result = queue
            .claim("w1", &[], Duration::from_secs(30))
            .await
            .unwrap();
        assert!(result.is_none());

        // Worker with docker label can claim
        let result = queue
            .claim("w2", &["docker".into()], Duration::from_secs(30))
            .await
            .unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_complete() {
        let queue = InMemoryJobQueue::new();
        let mut rx = queue.subscribe();

        let job = QueuedJob {
            job_id: "j1".into(),
            workflow_id: "wf1".into(),
            command: "echo".into(),
            required_labels: vec![],
            retry_policy: RetryPolicy::default(),
            attempt: 0,
            upstream_outputs: HashMap::new(),
            enqueued_at_ms: 0,
        };

        queue.enqueue(job).await.unwrap();
        let _ = rx.recv().await; // Ready event

        let (_, lease) = queue
            .claim("w1", &[], Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        let _ = rx.recv().await; // Started event

        let mut outputs = HashMap::new();
        outputs.insert("result".into(), "success".into());
        queue.complete(&lease.lease_id, outputs).await.unwrap();

        if let Ok(JobEvent::Completed { job_id, outputs, .. }) = rx.recv().await {
            assert_eq!(job_id, "j1");
            assert_eq!(outputs.get("result").unwrap(), "success");
        } else {
            panic!("expected Completed event");
        }
    }

    #[tokio::test]
    async fn test_fail_with_retry() {
        let queue = InMemoryJobQueue::new();
        let job = QueuedJob {
            job_id: "j1".into(),
            workflow_id: "wf1".into(),
            command: "echo".into(),
            required_labels: vec![],
            retry_policy: RetryPolicy {
                max_retries: 2,
                backoff: BackoffStrategy::None,
            },
            attempt: 0,
            upstream_outputs: HashMap::new(),
            enqueued_at_ms: 0,
        };

        queue.enqueue(job).await.unwrap();
        let (_, lease) = queue
            .claim("w1", &[], Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();

        // Fail with retryable — should re-enqueue
        queue
            .fail(&lease.lease_id, "oops".into(), true)
            .await
            .unwrap();

        // Should be available again with attempt=1
        let (retried, _) = queue
            .claim("w1", &[], Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retried.attempt, 1);
    }

    #[tokio::test]
    async fn test_cancel() {
        let queue = InMemoryJobQueue::new();
        let job = QueuedJob {
            job_id: "j1".into(),
            workflow_id: "wf1".into(),
            command: "echo".into(),
            required_labels: vec![],
            retry_policy: RetryPolicy::default(),
            attempt: 0,
            upstream_outputs: HashMap::new(),
            enqueued_at_ms: 0,
        };

        queue.enqueue(job).await.unwrap();
        queue.cancel("wf1", "j1").await.unwrap();

        // Job should be removed from pending
        let result = queue
            .claim("w1", &[], Duration::from_secs(30))
            .await
            .unwrap();
        assert!(result.is_none());

        // Should be marked as cancelled
        assert!(queue.is_cancelled("wf1", "j1").await.unwrap());
    }

    #[tokio::test]
    async fn test_reap_expired_leases() {
        let queue = InMemoryJobQueue::new();
        let job = QueuedJob {
            job_id: "j1".into(),
            workflow_id: "wf1".into(),
            command: "echo".into(),
            required_labels: vec![],
            retry_policy: RetryPolicy {
                max_retries: 1,
                backoff: BackoffStrategy::None,
            },
            attempt: 0,
            upstream_outputs: HashMap::new(),
            enqueued_at_ms: 0,
        };

        queue.enqueue(job).await.unwrap();

        // Claim with 0-second TTL (expires immediately)
        let (_, _lease) = queue
            .claim("w1", &[], Duration::from_secs(0))
            .await
            .unwrap()
            .unwrap();

        // Wait a tick so the lease is definitely expired
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Reap should find the expired lease
        let events = queue.reap_expired_leases().await.unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], JobEvent::LeaseExpired { job_id, .. } if job_id == "j1"));

        // Job should be re-enqueued (retry budget allows it)
        let (retried, _) = queue
            .claim("w2", &[], Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retried.attempt, 1);
    }
}
