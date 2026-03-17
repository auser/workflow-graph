use std::collections::HashMap;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};

use workflow_graph_shared::Workflow;
use workflow_graph_queue::traits::*;

use crate::state::AppState;

// ─── Workflow Management ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateWorkflow {
    pub name: String,
    pub trigger: String,
    pub jobs: Vec<workflow_graph_shared::Job>,
}

pub async fn create_workflow(
    State(state): State<AppState>,
    Json(payload): Json<CreateWorkflow>,
) -> (StatusCode, Json<Workflow>) {
    let id = uuid::Uuid::new_v4().to_string();
    let workflow = Workflow {
        id: id.clone(),
        name: payload.name,
        trigger: payload.trigger,
        jobs: payload.jobs,
    };

    state
        .workflow_state
        .write()
        .await
        .workflows
        .insert(id, workflow.clone());
    (StatusCode::CREATED, Json(workflow))
}

pub async fn list_workflows(State(state): State<AppState>) -> Json<Vec<Workflow>> {
    let s = state.workflow_state.read().await;
    Json(s.workflows.values().cloned().collect())
}

pub async fn get_workflow_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Workflow>, StatusCode> {
    let s = state.workflow_state.read().await;
    s.workflows
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn run_workflow(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    crate::workflow_ops::start_workflow(&state.workflow_state, state.queue.as_ref(), &id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(StatusCode::ACCEPTED)
}

pub async fn cancel_workflow(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    crate::workflow_ops::cancel_workflow(&state.workflow_state, state.queue.as_ref(), &id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(StatusCode::ACCEPTED)
}

pub async fn load_sample(
    State(state): State<AppState>,
) -> (StatusCode, Json<Workflow>) {
    let sample = Workflow::sample();
    let id = sample.id.clone();
    state
        .workflow_state
        .write()
        .await
        .workflows
        .insert(id, sample.clone());
    (StatusCode::CREATED, Json(sample))
}

// ─── Worker Protocol ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterWorker {
    pub worker_id: String,
    pub labels: Vec<String>,
}

pub async fn register_worker(
    State(state): State<AppState>,
    Json(payload): Json<RegisterWorker>,
) -> StatusCode {
    state
        .workers
        .register(&payload.worker_id, &payload.labels)
        .await
        .ok();
    StatusCode::OK
}

pub async fn worker_heartbeat(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
) -> StatusCode {
    match state.workers.heartbeat(&worker_id).await {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::NOT_FOUND,
    }
}

pub async fn list_workers(State(state): State<AppState>) -> Json<Vec<WorkerInfo>> {
    Json(state.workers.list_workers().await.unwrap_or_default())
}

// ─── Job Claiming & Execution ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ClaimRequest {
    pub worker_id: String,
    pub labels: Vec<String>,
    #[serde(default = "default_lease_ttl")]
    pub lease_ttl_secs: u64,
}

fn default_lease_ttl() -> u64 {
    30
}

#[derive(Serialize)]
pub struct ClaimResponse {
    pub job: QueuedJob,
    pub lease: Lease,
}

pub async fn claim_job(
    State(state): State<AppState>,
    Json(payload): Json<ClaimRequest>,
) -> Result<Json<Option<ClaimResponse>>, StatusCode> {
    let ttl = Duration::from_secs(payload.lease_ttl_secs);
    match state
        .queue
        .claim(&payload.worker_id, &payload.labels, ttl)
        .await
    {
        Ok(Some((job, lease))) => {
            state
                .workers
                .mark_busy(&payload.worker_id, &job.job_id)
                .await
                .ok();
            Ok(Json(Some(ClaimResponse { job, lease })))
        }
        Ok(None) => Ok(Json(None)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn job_heartbeat(
    State(state): State<AppState>,
    Path(lease_id): Path<String>,
) -> StatusCode {
    match state
        .queue
        .renew_lease(&lease_id, Duration::from_secs(30))
        .await
    {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::CONFLICT, // lease expired
    }
}

#[derive(Deserialize)]
pub struct CompleteRequest {
    #[serde(default)]
    pub outputs: HashMap<String, String>,
}

pub async fn complete_job(
    State(state): State<AppState>,
    Path(lease_id): Path<String>,
    Json(payload): Json<CompleteRequest>,
) -> StatusCode {
    match state.queue.complete(&lease_id, payload.outputs).await {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::CONFLICT,
    }
}

#[derive(Deserialize)]
pub struct FailRequest {
    pub error: String,
    #[serde(default)]
    pub retryable: bool,
}

pub async fn fail_job(
    State(state): State<AppState>,
    Path(lease_id): Path<String>,
    Json(payload): Json<FailRequest>,
) -> StatusCode {
    match state
        .queue
        .fail(&lease_id, payload.error, payload.retryable)
        .await
    {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::CONFLICT,
    }
}

pub async fn check_cancelled(
    State(state): State<AppState>,
    Path((wf_id, job_id)): Path<(String, String)>,
) -> Json<bool> {
    Json(
        state
            .queue
            .is_cancelled(&wf_id, &job_id)
            .await
            .unwrap_or(false),
    )
}

// ─── Log Streaming ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PushLogsRequest {
    pub chunks: Vec<LogChunk>,
}

pub async fn push_logs(
    State(state): State<AppState>,
    Path(_lease_id): Path<String>,
    Json(payload): Json<PushLogsRequest>,
) -> StatusCode {
    for chunk in payload.chunks {
        state.logs.append(chunk).await.ok();
    }
    StatusCode::OK
}

pub async fn get_job_logs(
    State(state): State<AppState>,
    Path((wf_id, job_id)): Path<(String, String)>,
) -> Result<Json<Vec<LogChunk>>, StatusCode> {
    match state.logs.get_all(&wf_id, &job_id).await {
        Ok(chunks) => Ok(Json(chunks)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// SSE log stream: replays existing chunks then streams live.
pub async fn stream_job_logs(
    State(state): State<AppState>,
    Path((wf_id, job_id)): Path<(String, String)>,
) -> axum::response::Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>> {
    use axum::response::sse::Event;
    use tokio_stream::StreamExt;
    use tokio_stream::wrappers::BroadcastStream;

    // Get existing chunks for catch-up
    let existing = state.logs.get_all(&wf_id, &job_id).await.unwrap_or_default();

    // Subscribe to live chunks
    let rx = state.logs.subscribe(&wf_id, &job_id);
    let live_stream = BroadcastStream::new(rx).filter_map(|result| {
        result.ok().map(|chunk| {
            Ok(Event::default()
                .event("log")
                .data(serde_json::to_string(&chunk).unwrap_or_default()))
        })
    });

    // Replay existing, then stream live
    let replay = tokio_stream::iter(existing.into_iter().map(|chunk| {
        Ok(Event::default()
            .event("log")
            .data(serde_json::to_string(&chunk).unwrap_or_default()))
    }));

    axum::response::Sse::new(replay.chain(live_stream))
}
