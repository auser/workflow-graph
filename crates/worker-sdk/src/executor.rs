use std::collections::HashMap;
use std::time::Duration;

use github_graph_queue::traits::{LogChunk, LogStream};
use serde::Serialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Result of a successful job execution.
pub struct JobOutput {
    pub outputs: HashMap<String, String>,
}

/// Error from job execution.
#[derive(Debug)]
pub struct JobError {
    pub message: String,
    pub exit_code: Option<i32>,
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(code) = self.exit_code {
            write!(f, "exit code {code}: {}", self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for JobError {}

#[derive(Serialize)]
struct PushLogsRequest {
    chunks: Vec<LogChunk>,
}

/// Execute a shell command, streaming output to the server as log chunks.
pub async fn execute_job_streaming(
    command: &str,
    client: &reqwest::Client,
    logs_url: &str,
    workflow_id: &str,
    job_id: &str,
    batch_interval: Duration,
    cancel_token: tokio_util::sync::CancellationToken,
) -> Result<JobOutput, JobError> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| JobError {
            message: format!("failed to spawn: {e}"),
            exit_code: None,
        })?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let wf_id = workflow_id.to_string();
    let j_id = job_id.to_string();
    let client = client.clone();
    let logs_url = logs_url.to_string();

    // Collect output lines and batch-send as log chunks
    let log_handle = {
        let wf_id = wf_id.clone();
        let j_id = j_id.clone();
        let client = client.clone();
        let logs_url = logs_url.clone();

        tokio::spawn(async move {
            let mut stdout_reader = BufReader::new(stdout).lines();
            let mut stderr_reader = BufReader::new(stderr).lines();
            let mut sequence: u64 = 0;
            let mut batch: Vec<LogChunk> = Vec::new();
            let mut last_flush = tokio::time::Instant::now();
            let mut full_output = String::new();

            loop {
                tokio::select! {
                    line = stdout_reader.next_line() => {
                        match line {
                            Ok(Some(text)) => {
                                full_output.push_str(&text);
                                full_output.push('\n');
                                batch.push(LogChunk {
                                    workflow_id: wf_id.clone(),
                                    job_id: j_id.clone(),
                                    sequence,
                                    data: format!("{text}\n"),
                                    timestamp_ms: now_ms(),
                                    stream: LogStream::Stdout,
                                });
                                sequence += 1;
                            }
                            Ok(None) => break, // stdout closed
                            Err(_) => break,
                        }
                    }
                    line = stderr_reader.next_line() => {
                        match line {
                            Ok(Some(text)) => {
                                full_output.push_str(&text);
                                full_output.push('\n');
                                batch.push(LogChunk {
                                    workflow_id: wf_id.clone(),
                                    job_id: j_id.clone(),
                                    sequence,
                                    data: format!("{text}\n"),
                                    timestamp_ms: now_ms(),
                                    stream: LogStream::Stderr,
                                });
                                sequence += 1;
                            }
                            Ok(None) => {} // stderr closed before stdout, keep going
                            Err(_) => {}
                        }
                    }
                    _ = tokio::time::sleep_until(last_flush + batch_interval) => {
                        // Flush batch
                        if !batch.is_empty() {
                            flush_logs(&client, &logs_url, &mut batch).await;
                            last_flush = tokio::time::Instant::now();
                        }
                    }
                }
            }

            // Final flush
            if !batch.is_empty() {
                flush_logs(&client, &logs_url, &mut batch).await;
            }

            full_output
        })
    };

    // Wait for either completion or cancellation
    tokio::select! {
        status = child.wait() => {
            let output = log_handle.await.unwrap_or_default();

            match status {
                Ok(exit) if exit.success() => Ok(JobOutput {
                    outputs: HashMap::new(),
                }),
                Ok(exit) => Err(JobError {
                    message: output.chars().take(4096).collect(),
                    exit_code: exit.code(),
                }),
                Err(e) => Err(JobError {
                    message: format!("wait failed: {e}"),
                    exit_code: None,
                }),
            }
        }
        _ = cancel_token.cancelled() => {
            // Kill the child process
            child.kill().await.ok();
            log_handle.abort();
            Err(JobError {
                message: "cancelled".into(),
                exit_code: None,
            })
        }
    }
}

async fn flush_logs(client: &reqwest::Client, url: &str, batch: &mut Vec<LogChunk>) {
    let chunks = std::mem::take(batch);
    client
        .post(url)
        .json(&PushLogsRequest { chunks })
        .send()
        .await
        .ok();
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
