---
title: Worker SDK
description: Embedding the Rust worker SDK in your own binary
---

The Worker SDK lets you embed a job executor in your own Rust application with full control over configuration.

## Setup

Add the dependency:

```toml
[dependencies]
workflow-graph-worker-sdk = { path = "crates/worker-sdk" }
tokio = { version = "1", features = ["full"] }
```

## WorkerConfig

```rust
use std::time::Duration;
use workflow_graph_worker_sdk::{Worker, WorkerConfig};

let config = WorkerConfig {
    server_url: "http://localhost:3000".into(),
    worker_id: uuid::Uuid::new_v4().to_string(),
    labels: vec!["docker".into(), "linux".into()],
    lease_ttl: Duration::from_secs(30),
    poll_interval: Duration::from_secs(2),
    heartbeat_interval: Duration::from_secs(10),  // must be < lease_ttl
    cancellation_check_interval: Duration::from_secs(2),
    log_batch_interval: Duration::from_millis(500),
};

let worker = Worker::new(config);
worker.run().await?;
```

| Field | Default | Description |
|-------|---------|-------------|
| `server_url` | `http://localhost:3000` | Server base URL |
| `worker_id` | Auto-generated UUID | Unique worker identifier |
| `labels` | `[]` | Capability labels |
| `lease_ttl` | 30s | How long the server waits before reclaiming |
| `poll_interval` | 2s | Delay between polls when idle |
| `heartbeat_interval` | 10s | Must be less than `lease_ttl` |
| `cancellation_check_interval` | 2s | How often to check for cancellation |
| `log_batch_interval` | 500ms | How often to flush log batches |

## Running Multiple Workers

Spawn workers with different labels in the same process:

```rust
#[tokio::main]
async fn main() {
    let docker_worker = Worker::new(WorkerConfig {
        labels: vec!["docker".into()],
        ..Default::default()
    });

    let gpu_worker = Worker::new(WorkerConfig {
        labels: vec!["gpu".into(), "linux".into()],
        ..Default::default()
    });

    tokio::spawn(async move { docker_worker.run().await });
    tokio::spawn(async move { gpu_worker.run().await });

    // Block forever (or until Ctrl+C)
    tokio::signal::ctrl_c().await.ok();
}
```

## Graceful Shutdown

The worker handles SIGTERM/SIGINT automatically — it finishes the current job before exiting:

```rust
// worker.run() handles Ctrl+C / SIGTERM internally:
// 1. Receives signal
// 2. Finishes executing the current job (if any)
// 3. Reports result to server
// 4. Returns Ok(())
worker.run().await?;
```

For custom shutdown logic, use `tokio::select!`:

```rust
tokio::select! {
    result = worker.run() => {
        if let Err(e) = result {
            eprintln!("Worker failed: {e}");
        }
    }
    _ = custom_shutdown_signal() => {
        println!("Custom shutdown...");
    }
}
```

## Retry Backoff

Jobs support configurable backoff strategies when retries are enabled:

| Strategy | Config | Behavior |
|----------|--------|----------|
| None | `BackoffStrategy::None` | Retry immediately (default) |
| Fixed | `BackoffStrategy::Fixed { delay_secs: 5 }` | Wait 5s between each retry |
| Exponential | `BackoffStrategy::Exponential { base_secs: 2, max_secs: 60 }` | Wait 2s, 4s, 8s, 16s... capped at 60s |

Backoff is configured per-job via `RetryPolicy` and enforced by the queue — jobs with pending backoff delay are not claimable until the delay elapses.

## Custom Executors

The built-in executor runs shell commands via `sh -c`. To execute Docker containers, WASM modules, or API calls, fork the executor:

```rust
use workflow_graph_worker_sdk::executor::{JobOutput, JobError};

pub async fn execute_docker(
    command: &str,
    client: &reqwest::Client,
    logs_url: &str,
    workflow_id: &str,
    job_id: &str,
    batch_interval: std::time::Duration,
    cancel_token: tokio_util::sync::CancellationToken,
) -> Result<JobOutput, JobError> {
    let parts: Vec<&str> = command.splitn(2, ' ').collect();
    let image = parts[0];
    let args = parts.get(1).unwrap_or(&"");

    let mut child = tokio::process::Command::new("docker")
        .args(["run", "--rm", image, "sh", "-c", args])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| JobError {
            message: format!("docker spawn failed: {e}"),
            exit_code: None,
        })?;

    // Stream stdout/stderr as LogChunks (same pattern as executor.rs)
    // Handle cancellation via cancel_token
    // Return JobOutput with extracted outputs
    todo!("wire up log streaming — see executor.rs for the pattern")
}
```

### Extracting Structured Outputs

Parse special lines from stdout (GitHub Actions-style) to pass data between jobs:

```rust
let mut outputs = HashMap::new();
for line in stdout_lines {
    if let Some(rest) = line.strip_prefix("::set-output ") {
        if let Some((name, value)) = rest.split_once('=') {
            outputs.insert(name.to_string(), value.to_string());
        }
    }
}
Ok(JobOutput { outputs })
```

Then in your workflow YAML:

```yaml
jobs:
  build:
    run: |
      cargo build --release
      echo "::set-output artifact_url=s3://bucket/build-123.tar.gz"
  deploy:
    needs: [build]
    run: deploy.sh
```
