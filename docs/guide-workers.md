# Writing Workers

Workers are external processes that poll the workflow-graph server for jobs, execute them, and report results. They communicate over HTTP, so you can write workers in any language.

There are three ways to run a worker:

1. **Standalone binary** — run the pre-built Rust worker with env vars
2. **Embedded SDK** — use the `workflow-graph-worker-sdk` crate as a library in your own Rust binary
3. **Custom HTTP client** — implement the worker protocol in any language

## Standalone Binary

The simplest way to run a worker:

```bash
cargo run -p workflow-graph-worker-sdk
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SERVER_URL` | `http://localhost:3000` | Server base URL |
| `WORKER_LABELS` | (empty) | Comma-separated capability labels |

```bash
SERVER_URL=http://my-server:3000 \
WORKER_LABELS=docker,linux,gpu \
cargo run -p workflow-graph-worker-sdk
```

The standalone binary executes the job's `command` field via `sh -c`, streams stdout/stderr as log chunks, sends heartbeats, and checks for cancellation automatically.

**Limitation:** the standalone binary only runs shell commands. It cannot run Docker containers, call APIs, or extract structured outputs. For those, use the embedded SDK or a custom worker.

## Embedding the Worker SDK

Add the SDK as a dependency:

```toml
[dependencies]
workflow-graph-worker-sdk = { path = "crates/worker-sdk" }
tokio = { version = "1", features = ["full"] }
```

### WorkerConfig

```rust
use std::time::Duration;
use workflow_graph_worker_sdk::{Worker, WorkerConfig};

let config = WorkerConfig {
    server_url: "http://localhost:3000".into(),
    worker_id: uuid::Uuid::new_v4().to_string(), // auto-generated if using Default
    labels: vec!["docker".into(), "linux".into()],
    lease_ttl: Duration::from_secs(30),           // how long the server waits before reclaiming
    poll_interval: Duration::from_secs(2),        // delay between polls when idle
    heartbeat_interval: Duration::from_secs(10),  // must be < lease_ttl
    cancellation_check_interval: Duration::from_secs(2),
    log_batch_interval: Duration::from_millis(500), // flush logs every 500ms
};

let worker = Worker::new(config);
worker.run().await?;
```

### Running Multiple Workers

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

### Graceful Shutdown

```rust
tokio::select! {
    result = worker.run() => {
        if let Err(e) = result {
            eprintln!("Worker failed: {e}");
        }
    }
    _ = tokio::signal::ctrl_c() => {
        println!("Shutting down...");
        // The current job will finish or its lease will expire and be retried
    }
}
```

## Custom Executors

The built-in executor runs shell commands via `sh -c`. To execute Docker containers, WASM modules, or API calls instead, you have two options:

### Option A: Fork the Executor

Copy `executor::execute_job_streaming` and replace the shell execution with your logic. The function signature stays the same — it receives a command string, streams logs, and returns `JobOutput` or `JobError`.

```rust
use workflow_graph_worker_sdk::executor::{JobOutput, JobError};
use workflow_graph_queue::traits::{LogChunk, LogStream};

pub async fn execute_docker(
    command: &str,
    client: &reqwest::Client,
    logs_url: &str,
    workflow_id: &str,
    job_id: &str,
    batch_interval: std::time::Duration,
    cancel_token: tokio_util::sync::CancellationToken,
) -> Result<JobOutput, JobError> {
    // Parse the command as a Docker image + args
    let parts: Vec<&str> = command.splitn(2, ' ').collect();
    let image = parts[0];
    let args = parts.get(1).unwrap_or(&"");

    // Run via Docker CLI (or use bollard crate for the Docker API)
    let mut child = tokio::process::Command::new("docker")
        .args(["run", "--rm", image, "sh", "-c", args])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| JobError {
            message: format!("docker spawn failed: {e}"),
            exit_code: None,
        })?;

    // ... stream stdout/stderr as LogChunks (same pattern as the built-in executor)
    // ... handle cancellation via cancel_token
    // ... return JobOutput with extracted outputs

    todo!("wire up log streaming — see executor.rs for the pattern")
}
```

### Option B: Custom Worker Loop

Build a worker that uses the HTTP protocol directly but replaces the execution step. See the [Worker Protocol Reference](#worker-protocol-reference) below.

### Extracting Structured Outputs

The built-in executor returns an empty `HashMap` for outputs. To pass data between jobs, parse special lines from stdout (GitHub Actions-style):

```rust
// In your custom executor, scan stdout lines for output directives:
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

Then in your workflow YAML, jobs can emit outputs:

```yaml
jobs:
  build:
    run: |
      cargo build --release
      echo "::set-output artifact_url=s3://bucket/build-123.tar.gz"

  deploy:
    needs: [build]
    # upstream_outputs["build"]["artifact_url"] = "s3://bucket/build-123.tar.gz"
    run: deploy.sh
```

## Worker Protocol Reference

The full HTTP protocol for building a worker in any language. All requests use JSON bodies and the server base URL (e.g., `http://localhost:3000`).

### Step 1: Register

```
POST /api/workers/register
Content-Type: application/json

{
  "worker_id": "worker-abc-123",
  "labels": ["docker", "linux"]
}

→ 200 OK
```

```bash
curl -X POST http://localhost:3000/api/workers/register \
  -H 'Content-Type: application/json' \
  -d '{"worker_id": "w1", "labels": ["docker"]}'
```

### Step 2: Poll for Jobs

```
POST /api/jobs/claim
Content-Type: application/json

{
  "worker_id": "worker-abc-123",
  "labels": ["docker", "linux"],
  "lease_ttl_secs": 30
}

→ 200 OK
→ Body: null                    (no matching job available)
→ Body: { "job": {...}, "lease": {...} }  (job claimed)
```

The `job` object:
```json
{
  "job_id": "build",
  "workflow_id": "wf-uuid",
  "command": "cargo build --release",
  "required_labels": ["docker"],
  "retry_policy": { "max_retries": 2, "backoff": "None" },
  "attempt": 0,
  "upstream_outputs": {
    "test": { "coverage": "94%" }
  },
  "enqueued_at_ms": 1710000000000
}
```

The `lease` object:
```json
{
  "lease_id": "lease-uuid",
  "job_id": "build",
  "workflow_id": "wf-uuid",
  "worker_id": "worker-abc-123",
  "ttl_secs": 30,
  "granted_at_ms": 1710000000000
}
```

```bash
curl -X POST http://localhost:3000/api/jobs/claim \
  -H 'Content-Type: application/json' \
  -d '{"worker_id": "w1", "labels": ["docker"], "lease_ttl_secs": 30}'
```

If `null` is returned, wait `poll_interval` seconds and try again.

### Step 3: Execute the Job

Run `job.command`. Access `job.upstream_outputs` for data from dependency jobs. While executing, run steps 4-6 concurrently.

### Step 4: Send Heartbeats (Concurrent)

Every `heartbeat_interval` seconds (must be less than `lease_ttl_secs`):

```
POST /api/jobs/{lease_id}/heartbeat

→ 200 OK          (lease renewed)
→ 409 CONFLICT     (lease expired — abort the job immediately)
```

```bash
curl -X POST http://localhost:3000/api/jobs/lease-uuid/heartbeat
```

If you get 409, the server has already reclaimed the job. Stop execution immediately.

### Step 5: Stream Logs (Concurrent)

Batch stdout/stderr lines and push every 500ms:

```
POST /api/jobs/{lease_id}/logs
Content-Type: application/json

{
  "chunks": [
    {
      "workflow_id": "wf-uuid",
      "job_id": "build",
      "sequence": 0,
      "data": "Compiling workflow-graph v0.1.0\n",
      "timestamp_ms": 1710000001000,
      "stream": "stdout"
    },
    {
      "workflow_id": "wf-uuid",
      "job_id": "build",
      "sequence": 1,
      "data": "warning: unused variable\n",
      "timestamp_ms": 1710000001500,
      "stream": "stderr"
    }
  ]
}

→ 200 OK
```

### Step 6: Check for Cancellation (Concurrent)

Poll every 2 seconds:

```
GET /api/jobs/{workflow_id}/{job_id}/cancelled

→ 200 OK
→ Body: false    (keep running)
→ Body: true     (abort immediately)
```

```bash
curl http://localhost:3000/api/jobs/wf-uuid/build/cancelled
```

### Step 7: Report Result

On success:
```
POST /api/jobs/{lease_id}/complete
Content-Type: application/json

{
  "outputs": {
    "artifact_url": "s3://bucket/build.tar.gz"
  }
}

→ 200 OK
```

On failure:
```
POST /api/jobs/{lease_id}/fail
Content-Type: application/json

{
  "error": "exit code 1: compilation failed",
  "retryable": true
}

→ 200 OK
```

Set `retryable: true` for transient failures (network, OOM). The server will re-enqueue if the job's retry budget allows. Set `retryable: false` for permanent failures (bad config, missing deps).

After reporting, loop back to Step 2 to claim the next job.

### Minimal Python Worker

A complete worker in ~40 lines of Python:

```python
import requests, subprocess, time, uuid, threading, json

SERVER = "http://localhost:3000"
WORKER_ID = f"py-{uuid.uuid4().hex[:8]}"
LABELS = ["python"]

# Register
requests.post(f"{SERVER}/api/workers/register",
    json={"worker_id": WORKER_ID, "labels": LABELS})

while True:
    # Poll
    resp = requests.post(f"{SERVER}/api/jobs/claim",
        json={"worker_id": WORKER_ID, "labels": LABELS, "lease_ttl_secs": 60})
    claim = resp.json()
    if claim is None:
        time.sleep(2)
        continue

    job, lease = claim["job"], claim["lease"]
    lease_id = lease["lease_id"]
    print(f"Claimed job {job['job_id']}")

    # Heartbeat thread
    stop = threading.Event()
    def heartbeat():
        while not stop.is_set():
            time.sleep(10)
            r = requests.post(f"{SERVER}/api/jobs/{lease_id}/heartbeat")
            if r.status_code == 409:
                break
    hb = threading.Thread(target=heartbeat, daemon=True)
    hb.start()

    # Execute
    result = subprocess.run(["sh", "-c", job["command"]],
        capture_output=True, text=True)

    stop.set()

    # Push logs
    chunks = [{"workflow_id": job["workflow_id"], "job_id": job["job_id"],
               "sequence": 0, "data": result.stdout, "timestamp_ms": int(time.time()*1000),
               "stream": "stdout"}]
    requests.post(f"{SERVER}/api/jobs/{lease_id}/logs", json={"chunks": chunks})

    # Report
    if result.returncode == 0:
        requests.post(f"{SERVER}/api/jobs/{lease_id}/complete", json={"outputs": {}})
    else:
        requests.post(f"{SERVER}/api/jobs/{lease_id}/fail",
            json={"error": result.stderr[:4096], "retryable": True})
```

## Worker Labels and Job Routing

Workers declare capability labels when they register. Jobs declare required labels in the workflow YAML:

```yaml
jobs:
  build:
    run: cargo build
    labels: [linux, docker]
```

**Matching rule:** a job's `required_labels` must be a **subset** of the worker's `labels`. A job with `required_labels: [linux, docker]` will only be claimed by workers whose labels include both `linux` AND `docker`.

Jobs with empty `required_labels` (the default) can be claimed by any worker.

### Label Strategy Patterns

| Pattern | Labels | Use Case |
|---------|--------|----------|
| Environment | `staging`, `production` | Route deploys to the right env |
| Capability | `docker`, `gpu`, `arm64` | Match hardware/software requirements |
| Team | `team-infra`, `team-ml` | Isolate workloads per team |
| Region | `us-east`, `eu-west` | Run jobs near data |

## Handling Upstream Outputs

When a job is claimed, `job.upstream_outputs` contains outputs from all dependency jobs:

```json
{
  "upstream_outputs": {
    "build": {
      "artifact_url": "s3://bucket/build.tar.gz",
      "version": "1.2.3"
    },
    "test": {
      "coverage": "94%"
    }
  }
}
```

This is populated automatically by the DAG scheduler. When a job completes with outputs (via the `/complete` endpoint), the scheduler stores them in the `ArtifactStore`. When a downstream job's dependencies are all satisfied, the scheduler fetches all upstream outputs and includes them in the `QueuedJob`.

Your worker can read these to make decisions:

```python
# In your Python worker:
upstream = job["upstream_outputs"]
artifact_url = upstream["build"]["artifact_url"]
subprocess.run(["deploy.sh", artifact_url])
```

```rust
// In your Rust worker:
let artifact_url = job.upstream_outputs
    .get("build")
    .and_then(|o| o.get("artifact_url"))
    .expect("build should produce artifact_url");
```
