pub mod executor;

use std::time::Duration;

use workflow_graph_queue::traits::*;
use serde::{Deserialize, Serialize};

/// Configuration for a worker instance.
#[derive(Clone, Debug)]
pub struct WorkerConfig {
    pub server_url: String,
    pub worker_id: String,
    pub labels: Vec<String>,
    pub lease_ttl: Duration,
    pub poll_interval: Duration,
    pub heartbeat_interval: Duration,
    pub cancellation_check_interval: Duration,
    pub log_batch_interval: Duration,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:3000".into(),
            worker_id: uuid::Uuid::new_v4().to_string(),
            labels: vec![],
            lease_ttl: Duration::from_secs(30),
            poll_interval: Duration::from_secs(2),
            heartbeat_interval: Duration::from_secs(10),
            cancellation_check_interval: Duration::from_secs(2),
            log_batch_interval: Duration::from_millis(500),
        }
    }
}

#[derive(Serialize)]
struct RegisterRequest {
    worker_id: String,
    labels: Vec<String>,
}

#[derive(Serialize)]
struct ClaimRequest {
    worker_id: String,
    labels: Vec<String>,
    lease_ttl_secs: u64,
}

#[derive(Deserialize)]
struct ClaimResponse {
    job: QueuedJob,
    lease: Lease,
}

#[derive(Serialize)]
struct CompleteRequest {
    outputs: std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
struct FailRequest {
    error: String,
    retryable: bool,
}

/// A worker that polls the server for jobs and executes them.
pub struct Worker {
    config: WorkerConfig,
    client: reqwest::Client,
}

impl Worker {
    pub fn new(config: WorkerConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Run the worker loop: register, poll for jobs, execute, report results.
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.register().await?;
        println!(
            "Worker {} registered with labels {:?}",
            self.config.worker_id, self.config.labels
        );

        loop {
            match self.poll_and_execute().await {
                Ok(true) => {} // executed a job, poll again immediately
                Ok(false) => {
                    // no job available, wait before polling again
                    tokio::time::sleep(self.config.poll_interval).await;
                }
                Err(e) => {
                    eprintln!("Worker error: {e}");
                    tokio::time::sleep(self.config.poll_interval).await;
                }
            }
        }
    }

    async fn register(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.client
            .post(format!("{}/api/workers/register", self.config.server_url))
            .json(&RegisterRequest {
                worker_id: self.config.worker_id.clone(),
                labels: self.config.labels.clone(),
            })
            .send()
            .await?;
        Ok(())
    }

    /// Poll for a job, execute it if available. Returns true if a job was executed.
    async fn poll_and_execute(&self) -> Result<bool, Box<dyn std::error::Error>> {
        // Claim a job
        let response = self
            .client
            .post(format!("{}/api/jobs/claim", self.config.server_url))
            .json(&ClaimRequest {
                worker_id: self.config.worker_id.clone(),
                labels: self.config.labels.clone(),
                lease_ttl_secs: self.config.lease_ttl.as_secs(),
            })
            .send()
            .await?;

        let claim: Option<ClaimResponse> = response.json().await?;
        let Some(claim) = claim else {
            return Ok(false);
        };

        println!(
            "Claimed job {} (workflow {})",
            claim.job.job_id, claim.job.workflow_id
        );

        // Execute the job with concurrent heartbeat, log streaming, and cancellation checking
        self.execute_job(&claim.job, &claim.lease).await;

        Ok(true)
    }

    async fn execute_job(&self, job: &QueuedJob, lease: &Lease) {
        let lease_id = lease.lease_id.clone();
        let workflow_id = job.workflow_id.clone();
        let job_id = job.job_id.clone();

        // Spawn heartbeat task
        let hb_client = self.client.clone();
        let hb_url = format!(
            "{}/api/jobs/{}/heartbeat",
            self.config.server_url, lease_id
        );
        let hb_interval = self.config.heartbeat_interval;
        let hb_handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(hb_interval).await;
                let res = hb_client.post(&hb_url).send().await;
                if let Ok(resp) = res {
                    if resp.status() == reqwest::StatusCode::CONFLICT {
                        eprintln!("Lease expired, aborting heartbeat");
                        break;
                    }
                }
            }
        });

        // Spawn cancellation checker
        let cancel_client = self.client.clone();
        let cancel_url = format!(
            "{}/api/jobs/{}/{}/cancelled",
            self.config.server_url, workflow_id, job_id
        );
        let cancel_interval = self.config.cancellation_check_interval;
        let cancel_token = tokio_util::sync::CancellationToken::new();
        let cancel_token_clone = cancel_token.clone();
        let cancel_handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(cancel_interval).await;
                if let Ok(resp) = cancel_client.get(&cancel_url).send().await {
                    if let Ok(cancelled) = resp.json::<bool>().await {
                        if cancelled {
                            cancel_token_clone.cancel();
                            break;
                        }
                    }
                }
            }
        });

        // Execute the command
        let result = executor::execute_job_streaming(
            &job.command,
            &self.client,
            &format!(
                "{}/api/jobs/{}/logs",
                self.config.server_url, lease_id
            ),
            &workflow_id,
            &job_id,
            self.config.log_batch_interval,
            cancel_token,
        )
        .await;

        // Cancel background tasks
        hb_handle.abort();
        cancel_handle.abort();

        // Report result
        match result {
            Ok(output) => {
                let url = format!(
                    "{}/api/jobs/{}/complete",
                    self.config.server_url, lease_id
                );
                self.client
                    .post(&url)
                    .json(&CompleteRequest {
                        outputs: output.outputs,
                    })
                    .send()
                    .await
                    .ok();
                println!("Job {} completed successfully", job.job_id);
            }
            Err(e) => {
                let url = format!(
                    "{}/api/jobs/{}/fail",
                    self.config.server_url, lease_id
                );
                self.client
                    .post(&url)
                    .json(&FailRequest {
                        error: e.to_string(),
                        retryable: true,
                    })
                    .send()
                    .await
                    .ok();
                eprintln!("Job {} failed: {e}", job.job_id);
            }
        }
    }
}
