use std::collections::HashMap;

use tokio::sync::Mutex;

use crate::error::RegistryError;
use crate::traits::{WorkerInfo, WorkerRegistry, WorkerStatus};

pub struct InMemoryWorkerRegistry {
    workers: Mutex<HashMap<String, WorkerInfo>>,
}

impl InMemoryWorkerRegistry {
    pub fn new() -> Self {
        Self {
            workers: Mutex::new(HashMap::new()),
        }
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

impl Default for InMemoryWorkerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerRegistry for InMemoryWorkerRegistry {
    async fn register(
        &self,
        worker_id: &str,
        labels: &[String],
    ) -> Result<(), RegistryError> {
        let now = Self::now_ms();
        self.workers.lock().await.insert(
            worker_id.to_string(),
            WorkerInfo {
                worker_id: worker_id.to_string(),
                labels: labels.to_vec(),
                registered_at_ms: now,
                last_heartbeat_ms: now,
                current_job: None,
                status: WorkerStatus::Idle,
            },
        );
        Ok(())
    }

    async fn heartbeat(&self, worker_id: &str) -> Result<(), RegistryError> {
        let mut workers = self.workers.lock().await;
        let worker = workers
            .get_mut(worker_id)
            .ok_or_else(|| RegistryError::WorkerNotFound(worker_id.to_string()))?;
        worker.last_heartbeat_ms = Self::now_ms();
        Ok(())
    }

    async fn deregister(&self, worker_id: &str) -> Result<(), RegistryError> {
        self.workers.lock().await.remove(worker_id);
        Ok(())
    }

    async fn list_workers(&self) -> Result<Vec<WorkerInfo>, RegistryError> {
        Ok(self.workers.lock().await.values().cloned().collect())
    }

    async fn mark_busy(
        &self,
        worker_id: &str,
        job_id: &str,
    ) -> Result<(), RegistryError> {
        let mut workers = self.workers.lock().await;
        let worker = workers
            .get_mut(worker_id)
            .ok_or_else(|| RegistryError::WorkerNotFound(worker_id.to_string()))?;
        worker.status = WorkerStatus::Busy;
        worker.current_job = Some(job_id.to_string());
        worker.last_heartbeat_ms = Self::now_ms();
        Ok(())
    }

    async fn mark_idle(&self, worker_id: &str) -> Result<(), RegistryError> {
        let mut workers = self.workers.lock().await;
        let worker = workers
            .get_mut(worker_id)
            .ok_or_else(|| RegistryError::WorkerNotFound(worker_id.to_string()))?;
        worker.status = WorkerStatus::Idle;
        worker.current_job = None;
        worker.last_heartbeat_ms = Self::now_ms();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_list() {
        let registry = InMemoryWorkerRegistry::new();
        registry
            .register("w1", &["docker".into(), "linux".into()])
            .await
            .unwrap();

        let workers = registry.list_workers().await.unwrap();
        assert_eq!(workers.len(), 1);
        assert_eq!(workers[0].worker_id, "w1");
        assert_eq!(workers[0].labels, vec!["docker", "linux"]);
        assert_eq!(workers[0].status, WorkerStatus::Idle);
    }

    #[tokio::test]
    async fn test_mark_busy_and_idle() {
        let registry = InMemoryWorkerRegistry::new();
        registry.register("w1", &[]).await.unwrap();

        registry.mark_busy("w1", "j1").await.unwrap();
        let workers = registry.list_workers().await.unwrap();
        assert_eq!(workers[0].status, WorkerStatus::Busy);
        assert_eq!(workers[0].current_job.as_deref(), Some("j1"));

        registry.mark_idle("w1").await.unwrap();
        let workers = registry.list_workers().await.unwrap();
        assert_eq!(workers[0].status, WorkerStatus::Idle);
        assert!(workers[0].current_job.is_none());
    }

    #[tokio::test]
    async fn test_deregister() {
        let registry = InMemoryWorkerRegistry::new();
        registry.register("w1", &[]).await.unwrap();
        registry.deregister("w1").await.unwrap();

        let workers = registry.list_workers().await.unwrap();
        assert!(workers.is_empty());
    }
}
