---
title: Labels & Outputs
description: Worker label routing and job-to-job data passing
---

## Worker Labels

Workers declare capability labels when they register. Jobs declare required labels in the workflow YAML.

```yaml
jobs:
  build:
    run: cargo build
    labels: [linux, docker]
```

**Matching rule:** A job's `required_labels` must be a **subset** of the worker's `labels`. A job with `labels: [linux, docker]` will only be claimed by workers whose labels include both `linux` AND `docker`.

Jobs with no `labels` (the default) can be claimed by any worker.

### Label Strategy Patterns

| Pattern | Labels | Use Case |
|---------|--------|----------|
| Environment | `staging`, `production` | Route deploys to the right env |
| Capability | `docker`, `gpu`, `arm64` | Match hardware/software requirements |
| Team | `team-infra`, `team-ml` | Isolate workloads per team |
| Region | `us-east`, `eu-west` | Run jobs near data |

### Example: Multi-Environment Deploy

```yaml
jobs:
  build:
    run: cargo build --release

  deploy-staging:
    needs: [build]
    labels: [staging]
    run: ./deploy.sh staging

  deploy-prod:
    needs: [build]
    labels: [production]
    run: ./deploy.sh production
```

```bash
# Staging worker
WORKER_LABELS=staging cargo run -p workflow-graph-worker-sdk

# Production worker
WORKER_LABELS=production cargo run -p workflow-graph-worker-sdk
```

## Upstream Outputs

When a job completes with outputs, downstream jobs receive them automatically via `upstream_outputs`.

### How It Works

1. A job completes and reports outputs via `POST /api/jobs/{lease_id}/complete`
2. The DAG scheduler stores outputs in the `ArtifactStore`
3. When a downstream job's dependencies are all satisfied, the scheduler fetches all upstream outputs
4. The downstream job receives them in `job.upstream_outputs` when claimed

### Example

```yaml
jobs:
  build:
    run: |
      cargo build --release
      echo "::set-output artifact_url=s3://bucket/build.tar.gz"
      echo "::set-output version=1.2.3"

  deploy:
    needs: [build]
    run: deploy.sh
```

When `deploy` is claimed, it receives:

```json
{
  "upstream_outputs": {
    "build": {
      "artifact_url": "s3://bucket/build.tar.gz",
      "version": "1.2.3"
    }
  }
}
```

### Reading Outputs in Workers

**Python:**
```python
upstream = job["upstream_outputs"]
artifact_url = upstream["build"]["artifact_url"]
subprocess.run(["deploy.sh", artifact_url])
```

**Rust:**
```rust
let artifact_url = job.upstream_outputs
    .get("build")
    .and_then(|o| o.get("artifact_url"))
    .expect("build should produce artifact_url");
```
