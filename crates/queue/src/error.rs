use thiserror::Error;

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("job not found: {0}")]
    JobNotFound(String),
    #[error("lease not found or expired: {0}")]
    LeaseNotFound(String),
    #[error("lease expired for job {0}")]
    LeaseExpired(String),
    #[error("workflow not found: {0}")]
    WorkflowNotFound(String),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("artifact not found for {0}/{1}")]
    NotFound(String, String),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
pub enum LogError {
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("worker not found: {0}")]
    WorkerNotFound(String),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("workflow not found: {0}")]
    WorkflowNotFound(String),
    #[error("queue error: {0}")]
    Queue(#[from] QueueError),
    #[error("artifact error: {0}")]
    Artifact(#[from] ArtifactError),
    #[error("internal error: {0}")]
    Internal(String),
}
