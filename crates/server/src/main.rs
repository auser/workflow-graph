mod api;
mod state;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::routing::{get, post};
use axum::Router;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use github_graph_queue::memory::*;
use github_graph_queue::scheduler::WorkflowState;
use github_graph_queue::{DagScheduler, JobQueue};
use github_graph_shared::yaml::WorkflowDef;

use state::AppState;

#[tokio::main]
async fn main() {
    let workflow_state = Arc::new(RwLock::new(WorkflowState::new()));

    // Load workflow files
    let workflows_dir = std::env::var("WORKFLOWS_DIR").unwrap_or_else(|_| "workflows".into());
    let loaded = load_workflows_from_dir(&workflows_dir);
    if loaded.is_empty() {
        eprintln!("No workflow files found in '{workflows_dir}', using built-in sample");
        let sample = github_graph_shared::Workflow::sample();
        workflow_state
            .write()
            .await
            .workflows
            .insert(sample.id.clone(), sample);
    } else {
        let mut s = workflow_state.write().await;
        for wf in loaded {
            println!("Loaded workflow: {} ({})", wf.name, wf.id);
            s.workflows.insert(wf.id.clone(), wf);
        }
    }

    // Create queue backends (in-memory for now)
    let queue = Arc::new(InMemoryJobQueue::new());
    let artifacts = Arc::new(InMemoryArtifactStore::new());
    let logs = Arc::new(InMemoryLogSink::new());
    let workers = Arc::new(InMemoryWorkerRegistry::new());

    // Create scheduler
    let scheduler = Arc::new(DagScheduler::new(
        queue.clone(),
        artifacts.clone(),
        workflow_state.clone(),
    ));

    // Spawn scheduler event loop
    let scheduler_handle = scheduler.clone();
    tokio::spawn(async move {
        scheduler_handle.run().await;
    });

    // Spawn lease reaper (checks for expired leases every 5 seconds)
    let reaper_queue = queue.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if let Err(e) = reaper_queue.reap_expired_leases().await {
                eprintln!("Lease reaper error: {e}");
            }
        }
    });

    let app_state = AppState {
        workflow_state,
        queue,
        artifacts,
        logs,
        workers,
        scheduler,
    };

    let app = create_router(app_state);

    let preferred_port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let listener = find_available_port(preferred_port).await;
    let port = listener.local_addr().unwrap().port();
    println!("Server listening on http://localhost:{port}");

    axum::serve(listener, app).await.unwrap();
}

/// Create the Axum router with all API routes.
///
/// Library consumers can call this directly to embed the API in their own server.
pub fn create_router(state: AppState) -> Router {
    let api = Router::new()
        // Workflow management
        .route("/api/workflows", get(api::list_workflows).post(api::create_workflow))
        .route("/api/workflows/sample", post(api::load_sample))
        .route("/api/workflows/{id}/status", get(api::get_workflow_status))
        .route("/api/workflows/{id}/run", post(api::run_workflow))
        .route("/api/workflows/{id}/cancel", post(api::cancel_workflow))
        // Worker protocol
        .route("/api/workers", get(api::list_workers))
        .route("/api/workers/register", post(api::register_worker))
        .route("/api/workers/{id}/heartbeat", post(api::worker_heartbeat))
        // Job claiming & execution
        .route("/api/jobs/claim", post(api::claim_job))
        .route("/api/jobs/{lease_id}/heartbeat", post(api::job_heartbeat))
        .route("/api/jobs/{lease_id}/complete", post(api::complete_job))
        .route("/api/jobs/{lease_id}/fail", post(api::fail_job))
        .route("/api/jobs/{lease_id}/logs", post(api::push_logs))
        // Job queries
        .route("/api/jobs/{wf_id}/{job_id}/cancelled", get(api::check_cancelled))
        .route("/api/workflows/{wf_id}/jobs/{job_id}/logs", get(api::get_job_logs))
        .with_state(state);

    // Static file serving
    let static_files = ServeDir::new("www").fallback(ServeDir::new("."));

    api.fallback_service(static_files)
        .layer(CorsLayer::permissive())
}

fn load_workflows_from_dir(dir: &str) -> Vec<github_graph_shared::Workflow> {
    let path = Path::new(dir);
    if !path.is_dir() {
        return vec![];
    }

    let mut workflows = Vec::new();
    let Ok(entries) = std::fs::read_dir(path) else {
        return vec![];
    };

    for entry in entries.flatten() {
        let file_path = entry.path();
        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if !(filename.ends_with(".yml")
            || filename.ends_with(".yaml")
            || filename.ends_with(".json"))
        {
            continue;
        }

        let Ok(contents) = std::fs::read_to_string(&file_path) else {
            eprintln!("Failed to read {}", file_path.display());
            continue;
        };

        let id = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        match WorkflowDef::from_file_contents(&contents, filename) {
            Ok(def) => match def.into_workflow(&id) {
                Ok(wf) => workflows.push(wf),
                Err(e) => eprintln!("Invalid workflow {}: {e}", file_path.display()),
            },
            Err(e) => eprintln!("Failed to parse {}: {e}", file_path.display()),
        }
    }

    workflows
}

async fn find_available_port(preferred: u16) -> tokio::net::TcpListener {
    if let Ok(listener) = tokio::net::TcpListener::bind(("0.0.0.0", preferred)).await {
        return listener;
    }
    eprintln!("Port {preferred} is taken, searching for an available port...");

    for port in (preferred + 1)..=preferred.saturating_add(100) {
        if let Ok(listener) = tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
            return listener;
        }
    }

    tokio::net::TcpListener::bind(("0.0.0.0", 0u16))
        .await
        .expect("failed to bind to any port")
}
