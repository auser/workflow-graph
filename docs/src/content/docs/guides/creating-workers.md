---
title: Creating Workers
description: Step-by-step guide to building custom workers in any language
---

Workers are external processes that execute jobs. This guide walks through creating workers from simple to advanced, in multiple languages.

## Choosing an Approach

| Approach | Effort | Flexibility | Best For |
|----------|--------|-------------|----------|
| [Standalone binary](#standalone-binary) | None | Shell commands only | Quick start, simple jobs |
| [Rust SDK](#rust-worker) | Low | Full Rust ecosystem | Production Rust services |
| [Custom HTTP (any language)](#any-language-worker) | Medium | Unlimited | Python, Go, Node.js, etc. |

## Standalone Binary

Zero code required — just run the pre-built binary:

```bash
cargo run -p workflow-graph-worker-sdk
```

Configure with environment variables:

```bash
SERVER_URL=http://my-server:3000 \
WORKER_LABELS=docker,linux,gpu \
cargo run -p workflow-graph-worker-sdk
```

The binary executes `job.command` via `sh -c`, streams logs, sends heartbeats, and checks for cancellation automatically.

**Limitation:** Only runs shell commands. For Docker, API calls, or structured outputs, build a custom worker.

## Rust Worker

### Minimal Example

```rust
use workflow_graph_worker_sdk::{Worker, WorkerConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let worker = Worker::new(WorkerConfig {
        server_url: "http://localhost:3000".into(),
        labels: vec!["rust".into()],
        ..Default::default()
    });

    worker.run().await?;
    Ok(())
}
```

### With Custom Execution Logic

Replace the built-in shell executor with your own:

```rust
use std::collections::HashMap;
use workflow_graph_worker_sdk::executor::{JobOutput, JobError};

async fn my_executor(command: &str) -> Result<JobOutput, JobError> {
    // Parse the command as a Docker image
    let output = tokio::process::Command::new("docker")
        .args(["run", "--rm", command])
        .output()
        .await
        .map_err(|e| JobError {
            message: format!("docker failed: {e}"),
            exit_code: None,
        })?;

    if !output.status.success() {
        return Err(JobError {
            message: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
        });
    }

    // Extract outputs from stdout
    let mut outputs = HashMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some(rest) = line.strip_prefix("::set-output ") {
            if let Some((k, v)) = rest.split_once('=') {
                outputs.insert(k.to_string(), v.to_string());
            }
        }
    }

    Ok(JobOutput { outputs })
}
```

### Graceful Shutdown

```rust
let worker = Worker::new(config);

tokio::select! {
    result = worker.run() => {
        if let Err(e) = result {
            eprintln!("Worker error: {e}");
        }
    }
    _ = tokio::signal::ctrl_c() => {
        println!("Shutting down gracefully...");
        // Current job finishes or lease expires and gets retried
    }
}
```

### Running Multiple Workers

```rust
#[tokio::main]
async fn main() {
    let docker_worker = Worker::new(WorkerConfig {
        labels: vec!["docker".into()],
        ..Default::default()
    });

    let gpu_worker = Worker::new(WorkerConfig {
        labels: vec!["gpu".into(), "cuda".into()],
        ..Default::default()
    });

    tokio::spawn(async move { docker_worker.run().await });
    tokio::spawn(async move { gpu_worker.run().await });

    tokio::signal::ctrl_c().await.ok();
}
```

## Any-Language Worker

The worker protocol is plain HTTP + JSON — build a worker in any language that can make HTTP requests.

### Worker Lifecycle

```
1. Register    POST /api/workers/register
2. Poll loop:
   a. Claim    POST /api/jobs/claim
   b. Execute  (your logic)
      ├─ Heartbeat  POST /api/jobs/{lease_id}/heartbeat  (concurrent)
      ├─ Logs       POST /api/jobs/{lease_id}/logs        (concurrent)
      └─ Cancel     GET  /api/jobs/{wf_id}/{job_id}/cancelled (concurrent)
   c. Report   POST /api/jobs/{lease_id}/complete  or  /fail
3. Goto 2
```

### Python Worker

```python
import requests
import subprocess
import time
import uuid
import threading

SERVER = "http://localhost:3000"
WORKER_ID = f"py-{uuid.uuid4().hex[:8]}"
LABELS = ["python"]

# 1. Register
requests.post(f"{SERVER}/api/workers/register",
    json={"worker_id": WORKER_ID, "labels": LABELS})

while True:
    # 2a. Claim a job
    resp = requests.post(f"{SERVER}/api/jobs/claim",
        json={
            "worker_id": WORKER_ID,
            "labels": LABELS,
            "lease_ttl_secs": 60,
        })
    claim = resp.json()

    if claim is None:
        time.sleep(2)  # No jobs available
        continue

    job = claim["job"]
    lease = claim["lease"]
    lease_id = lease["lease_id"]
    print(f"[{WORKER_ID}] Claimed: {job['job_id']}")

    # Heartbeat thread (keeps lease alive)
    stop = threading.Event()
    def send_heartbeats():
        while not stop.is_set():
            time.sleep(10)
            r = requests.post(f"{SERVER}/api/jobs/{lease_id}/heartbeat")
            if r.status_code == 409:
                break

    hb_thread = threading.Thread(target=send_heartbeats, daemon=True)
    hb_thread.start()

    # Execute
    result = subprocess.run(
        ["sh", "-c", job["command"]],
        capture_output=True, text=True, timeout=300,
    )
    stop.set()

    # Push logs
    chunks = []
    if result.stdout:
        chunks.append({
            "workflow_id": job["workflow_id"],
            "job_id": job["job_id"],
            "sequence": 0,
            "data": result.stdout,
            "timestamp_ms": int(time.time() * 1000),
            "stream": "stdout",
        })
    if chunks:
        requests.post(f"{SERVER}/api/jobs/{lease_id}/logs",
            json={"chunks": chunks})

    # Report
    if result.returncode == 0:
        requests.post(f"{SERVER}/api/jobs/{lease_id}/complete",
            json={"outputs": {}})
    else:
        requests.post(f"{SERVER}/api/jobs/{lease_id}/fail",
            json={"error": result.stderr[:4096], "retryable": True})
```

### TypeScript / Node.js Worker

```typescript
import { execSync } from "node:child_process";

const SERVER = process.env.SERVER_URL || "http://localhost:3000";
const WORKER_ID = `node-${Math.random().toString(36).slice(2, 10)}`;
const LABELS = (process.env.WORKER_LABELS || "node").split(",");

async function post(path: string, body?: unknown) {
  return fetch(`${SERVER}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: body ? JSON.stringify(body) : undefined,
  });
}

// Register
await post("/api/workers/register", {
  worker_id: WORKER_ID,
  labels: LABELS,
});

while (true) {
  const resp = await post("/api/jobs/claim", {
    worker_id: WORKER_ID,
    labels: LABELS,
    lease_ttl_secs: 60,
  });

  const claim = await resp.json();
  if (!claim) {
    await new Promise((r) => setTimeout(r, 2000));
    continue;
  }

  const { job, lease } = claim;
  console.log(`Claimed: ${job.job_id}`);

  // Heartbeat interval
  const hb = setInterval(() => {
    post(`/api/jobs/${lease.lease_id}/heartbeat`);
  }, 10_000);

  try {
    const stdout = execSync(job.command, {
      encoding: "utf-8",
      timeout: 300_000,
    });

    await post(`/api/jobs/${lease.lease_id}/logs`, {
      chunks: [
        {
          workflow_id: job.workflow_id,
          job_id: job.job_id,
          sequence: 0,
          data: stdout,
          timestamp_ms: Date.now(),
          stream: "stdout",
        },
      ],
    });

    await post(`/api/jobs/${lease.lease_id}/complete`, { outputs: {} });
  } catch (err: any) {
    await post(`/api/jobs/${lease.lease_id}/fail`, {
      error: err.message.slice(0, 4096),
      retryable: true,
    });
  } finally {
    clearInterval(hb);
  }
}
```

### Go Worker

```go
package main

import (
    "bytes"
    "encoding/json"
    "fmt"
    "net/http"
    "os/exec"
    "time"
)

const server = "http://localhost:3000"

func main() {
    workerID := fmt.Sprintf("go-%d", time.Now().UnixNano()%100000)
    labels := []string{"go"}

    // Register
    postJSON("/api/workers/register", map[string]any{
        "worker_id": workerID, "labels": labels,
    })

    for {
        // Claim
        resp := postJSON("/api/jobs/claim", map[string]any{
            "worker_id": workerID, "labels": labels, "lease_ttl_secs": 60,
        })
        if resp == nil {
            time.Sleep(2 * time.Second)
            continue
        }

        job := resp["job"].(map[string]any)
        lease := resp["lease"].(map[string]any)
        leaseID := lease["lease_id"].(string)
        fmt.Printf("[%s] Claimed: %s\n", workerID, job["job_id"])

        // Heartbeat goroutine
        done := make(chan struct{})
        go func() {
            for {
                select {
                case <-done:
                    return
                case <-time.After(10 * time.Second):
                    postJSON(fmt.Sprintf("/api/jobs/%s/heartbeat", leaseID), nil)
                }
            }
        }()

        // Execute
        cmd := exec.Command("sh", "-c", job["command"].(string))
        output, err := cmd.CombinedOutput()
        close(done)

        // Report
        if err == nil {
            postJSON(fmt.Sprintf("/api/jobs/%s/complete", leaseID),
                map[string]any{"outputs": map[string]string{}})
        } else {
            postJSON(fmt.Sprintf("/api/jobs/%s/fail", leaseID),
                map[string]any{"error": string(output), "retryable": true})
        }
    }
}

func postJSON(path string, body any) map[string]any {
    var buf bytes.Buffer
    if body != nil {
        json.NewEncoder(&buf).Encode(body)
    }
    resp, err := http.Post(server+path, "application/json", &buf)
    if err != nil { return nil }
    defer resp.Body.Close()
    var result map[string]any
    json.NewDecoder(resp.Body).Decode(&result)
    return result
}
```

## Best Practices

### Heartbeat Interval

Set heartbeat interval to **less than half** of `lease_ttl`. If the lease is 30s, heartbeat every 10s. This gives a buffer for network latency.

### Idempotent Jobs

Design jobs to be safely re-executed. If a worker crashes mid-job, the lease expires and the job gets re-queued. Your job command should handle being run twice.

### Structured Outputs

Use the `::set-output key=value` convention in stdout to pass data to downstream jobs:

```bash
echo "::set-output version=1.2.3"
echo "::set-output artifact_url=s3://bucket/build.tar.gz"
```

Downstream jobs receive these in `job.upstream_outputs`. See [Labels & Outputs](/workflow-graph/workers/labels-and-outputs/).

### Error Handling

- `retryable: true` — transient failures (network, OOM, timeouts). Server re-enqueues up to `max_retries`.
- `retryable: false` — permanent failures (bad config, missing deps). Job is marked failed immediately.

### Label Strategy

Route jobs to the right workers with labels:

```yaml
jobs:
  build:
    labels: [linux, docker]
  deploy-staging:
    labels: [staging]
  gpu-train:
    labels: [gpu, cuda]
```
