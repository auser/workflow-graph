use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;

use github_graph_shared::Workflow;

use crate::orchestrator::{self, SharedState};

#[derive(Deserialize)]
pub struct CreateWorkflow {
    pub name: String,
    pub trigger: String,
    pub jobs: Vec<github_graph_shared::Job>,
}

pub async fn create_workflow(
    State(state): State<SharedState>,
    Json(payload): Json<CreateWorkflow>,
) -> (StatusCode, Json<Workflow>) {
    let id = uuid::Uuid::new_v4().to_string();
    let workflow = Workflow {
        id: id.clone(),
        name: payload.name,
        trigger: payload.trigger,
        jobs: payload.jobs,
    };

    state.write().await.workflows.insert(id, workflow.clone());
    (StatusCode::CREATED, Json(workflow))
}

pub async fn get_workflow_status(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Workflow>, StatusCode> {
    let s = state.read().await;
    s.workflows
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn run_workflow(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    {
        let s = state.read().await;
        if !s.workflows.contains_key(&id) {
            return Err(StatusCode::NOT_FOUND);
        }
    }
    orchestrator::run_workflow(state, id);
    Ok(StatusCode::ACCEPTED)
}

pub async fn rerun_job(
    State(state): State<SharedState>,
    Path((wf_id, job_id)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    {
        let s = state.read().await;
        let wf = s.workflows.get(&wf_id).ok_or(StatusCode::NOT_FOUND)?;
        if !wf.jobs.iter().any(|j| j.id == job_id) {
            return Err(StatusCode::NOT_FOUND);
        }
    }

    // Reset this job and all downstream, then re-run
    {
        let mut s = state.write().await;
        let wf = s.workflows.get_mut(&wf_id).unwrap();
        let downstream = find_downstream_inclusive(wf, &job_id);
        for j in &mut wf.jobs {
            if downstream.contains(&j.id) {
                j.status = github_graph_shared::JobStatus::Queued;
                j.duration_secs = None;
                j.output = None;
            }
        }
    }

    // Start execution from this job
    let state_clone = state.clone();
    tokio::spawn(async move {
        orchestrator::run_workflow(state_clone, wf_id);
    });

    Ok(StatusCode::ACCEPTED)
}

pub async fn load_sample(
    State(state): State<SharedState>,
) -> (StatusCode, Json<Workflow>) {
    let sample = Workflow::sample();
    let id = sample.id.clone();
    state.write().await.workflows.insert(id, sample.clone());
    (StatusCode::CREATED, Json(sample))
}

pub async fn list_workflows(
    State(state): State<SharedState>,
) -> Json<Vec<Workflow>> {
    let s = state.read().await;
    Json(s.workflows.values().cloned().collect())
}

fn find_downstream_inclusive(wf: &Workflow, job_id: &str) -> Vec<String> {
    let mut result = vec![job_id.to_string()];
    let mut stack = vec![job_id.to_string()];

    while let Some(current) = stack.pop() {
        for job in &wf.jobs {
            if job.depends_on.contains(&current) && !result.contains(&job.id) {
                result.push(job.id.clone());
                stack.push(job.id.clone());
            }
        }
    }

    result
}
