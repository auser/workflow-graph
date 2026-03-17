---
title: Architecture Overview
description: Crate structure and system architecture
---

## System Architecture

```
                     All-in-one (dev)          Split (edge/prod)
                     ────────────────          ─────────────────
┌──────────┐  poll   ┌──────────────┐         ┌───────────┐  ┌───────────┐
│  Worker   │◄──────►│   Server     │         │ API Server│  │ Scheduler │
│  (SDK)    │  HTTP  │ API+Scheduler│         │ (stateless│  │ (separate │
└──────────┘        └──────┬───────┘         │  process) │  │  process) │
                           │                  └─────┬─────┘  └─────┬─────┘
                      ┌────┴────┐                   │              │
                      │JobQueue │◄──────────────────┴──────────────┘
                      │LogSink  │
                      │Artifacts│
                      └─────────┘
```

Workers communicate with the server over HTTP. The server can run in **all-in-one** mode (API + scheduler in one process) or **split** mode (stateless API + separate scheduler) for edge deployments.

## Crate Structure

| Crate | Purpose |
|-------|---------|
| `workflow-graph-shared` | Core types: `Job`, `Workflow`, `JobStatus`, YAML parser |
| `workflow-graph-queue` | Queue traits + in-memory implementations, DagScheduler |
| `workflow-graph-web` | WASM Canvas renderer with interactive graph |
| `workflow-graph-worker-sdk` | Worker binary + embeddable library |
| `workflow-graph-server` | Reference Axum server (stateless API, embeddable) |
| `workflow-graph-scheduler` | Standalone scheduler binary for split deployments |

## NPM Packages

| Package | Purpose |
|---------|---------|
| `@workflow-graph/web` | TypeScript wrapper for WASM, auto-inits, manages canvas |
| `@workflow-graph/react` | React component adapter (`<WorkflowGraphComponent />`) |
| `@workflow-graph/client` | TypeScript client for the REST API |

## Key Design Decisions

### Trait-Based Backends

All storage is abstracted behind four traits:

- **`JobQueue`** — enqueue, claim (atomic), renew, complete, fail, cancel, reap, subscribe
- **`ArtifactStore`** — put/get job outputs (job-to-job communication)
- **`LogSink`** — append log chunks, get all, subscribe for live streaming
- **`WorkerRegistry`** — register, heartbeat, list, mark busy/idle

The in-memory implementations ship by default. Swap them for Postgres, Redis, or your own backend.

### Event-Driven DAG Scheduler

The `DagScheduler` subscribes to queue events and cascades the DAG:

1. When a job **completes** → enqueue downstream jobs whose dependencies are all satisfied
2. When a job **fails** (non-retryable) → skip all transitive downstream jobs
3. When a **lease expires** → retry if budget allows, otherwise fail

### Stateless API Server

All API handlers call `workflow_ops` functions that directly manipulate queue backends. The server holds no in-process state beyond the trait object references, making it suitable for edge/serverless deployment.
