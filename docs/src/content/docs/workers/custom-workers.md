---
title: Custom Workers
description: Build workers in any language using the HTTP protocol
---

Workers communicate with the server over HTTP using JSON, so you can implement them in any language. This page documents the full protocol.

## Worker Protocol

### Step 1: Register

```http
POST /api/workers/register
Content-Type: application/json

{
  "worker_id": "worker-abc-123",
  "labels": ["docker", "linux"]
}
```

```bash
curl -X POST http://localhost:3000/api/workers/register \
  -H 'Content-Type: application/json' \
  -d '{"worker_id": "w1", "labels": ["docker"]}'
```

### Step 2: Poll for Jobs

```http
POST /api/jobs/claim
Content-Type: application/json

{
  "worker_id": "worker-abc-123",
  "labels": ["docker", "linux"],
  "lease_ttl_secs": 30
}
```

Returns `null` if no matching job is available, or a `{ "job": {...}, "lease": {...} }` object.

If `null`, wait your poll interval and try again.

### Step 3: Execute the Job

Run `job.command`. Access `job.upstream_outputs` for data from dependency jobs. While executing, run steps 4–6 concurrently.

### Step 4: Send Heartbeats (Concurrent)

Every `heartbeat_interval` seconds (must be less than `lease_ttl_secs`):

```http
POST /api/jobs/{lease_id}/heartbeat
```

- **200 OK** — lease renewed
- **409 Conflict** — lease expired; abort immediately

### Step 5: Stream Logs (Concurrent)

Batch stdout/stderr lines and push periodically:

```http
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
    }
  ]
}
```

### Step 6: Check for Cancellation (Concurrent)

Poll every 2 seconds:

```http
GET /api/jobs/{workflow_id}/{job_id}/cancelled
```

Returns `true` or `false`. If `true`, abort execution immediately.

### Step 7: Report Result

On success:

```http
POST /api/jobs/{lease_id}/complete
Content-Type: application/json

{ "outputs": { "artifact_url": "s3://bucket/build.tar.gz" } }
```

On failure:

```http
POST /api/jobs/{lease_id}/fail
Content-Type: application/json

{ "error": "exit code 1: compilation failed", "retryable": true }
```

Set `retryable: true` for transient failures (network, OOM). The server re-enqueues if the retry budget allows. Set `retryable: false` for permanent failures.

After reporting, loop back to Step 2.

## Minimal Python Worker

A complete worker in ~40 lines:

```python
import requests, subprocess, time, uuid, threading

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
               "sequence": 0, "data": result.stdout,
               "timestamp_ms": int(time.time()*1000), "stream": "stdout"}]
    requests.post(f"{SERVER}/api/jobs/{lease_id}/logs", json={"chunks": chunks})

    # Report
    if result.returncode == 0:
        requests.post(f"{SERVER}/api/jobs/{lease_id}/complete",
            json={"outputs": {}})
    else:
        requests.post(f"{SERVER}/api/jobs/{lease_id}/fail",
            json={"error": result.stderr[:4096], "retryable": True})
```
