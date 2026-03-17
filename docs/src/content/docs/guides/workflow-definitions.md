---
title: Workflow Definitions
description: YAML and JSON format for defining workflows
---

Workflows are defined in YAML or JSON and placed in the `workflows/` directory. The format is inspired by GitHub Actions.

## Basic Structure

```yaml
name: CI Pipeline
on: push

jobs:
  test:
    name: Unit Tests
    run: cargo test

  lint:
    name: Lint
    run: cargo clippy

  build:
    name: Build
    needs: [test, lint]
    run: cargo build --release
```

## Job Fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | No | Display name (defaults to job key) |
| `run` | Yes | Shell command to execute |
| `needs` | No | List of dependency job keys |
| `labels` | No | Required worker labels |
| `retries` | No | Max retry attempts on failure |
| `steps` | No | List of named steps (alternative to `run`) |

## Dependencies

Use `needs` to declare dependencies between jobs. The DAG scheduler ensures jobs only run after all dependencies succeed.

```yaml
jobs:
  a:
    run: echo "first"
  b:
    run: echo "second"
  c:
    needs: [a, b]
    run: echo "runs after both a and b"
```

If a dependency fails (and retries are exhausted), all downstream jobs are automatically **skipped**.

## Retries

```yaml
jobs:
  flaky-test:
    run: ./run-integration-tests.sh
    retries: 3
```

When a job fails and is marked `retryable` by the worker, the scheduler re-enqueues it up to `retries` times.

## Worker Labels

```yaml
jobs:
  deploy:
    run: ./deploy.sh
    labels: [linux, aws]
```

Only workers whose labels are a superset of the job's labels can claim it. See [Labels & Outputs](/workflow-graph/workers/labels-and-outputs/) for details.

## Multi-Step Jobs

```yaml
jobs:
  deploy:
    needs: [build]
    labels: [linux, aws]
    steps:
      - name: Migrate DB
        run: ./scripts/migrate.sh
      - name: Deploy App
        run: ./scripts/deploy.sh
```

## Full Example

```yaml
name: CI
on: push

jobs:
  unit-tests:
    name: Unit Tests
    run: echo 'Running unit tests' && sleep 2
    retries: 1

  lint:
    name: Lint
    run: echo 'Running linter' && sleep 1

  build:
    name: Build
    needs: [unit-tests, lint]
    run: echo 'Building project' && sleep 3
    labels: [linux]

  deploy-web:
    name: Deploy Web
    needs: [build]
    run: echo 'Deploying' && sleep 3
    labels: [linux, aws]
```

## JSON Format

You can also define workflows in JSON:

```json
{
  "name": "CI Pipeline",
  "on": "push",
  "jobs": {
    "test": {
      "name": "Unit Tests",
      "run": "cargo test"
    },
    "build": {
      "name": "Build",
      "needs": ["test"],
      "run": "cargo build --release"
    }
  }
}
```

Place files with `.yml`, `.yaml`, or `.json` extensions in the `workflows/` directory.
