mod api;
mod executor;
mod orchestrator;

use std::path::Path;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use github_graph_shared::yaml::WorkflowDef;
use orchestrator::OrchestratorState;

#[tokio::main]
async fn main() {
    let state = Arc::new(RwLock::new(OrchestratorState::new()));

    // Load workflow files from the workflows/ directory (or fall back to sample)
    let workflows_dir = std::env::var("WORKFLOWS_DIR").unwrap_or_else(|_| "workflows".into());
    let loaded = load_workflows_from_dir(&workflows_dir);
    if loaded.is_empty() {
        eprintln!("No workflow files found in '{workflows_dir}', using built-in sample");
        let sample = github_graph_shared::Workflow::sample();
        state.write().await.workflows.insert(sample.id.clone(), sample);
    } else {
        let mut s = state.write().await;
        for wf in loaded {
            println!("Loaded workflow: {} ({})", wf.name, wf.id);
            s.workflows.insert(wf.id.clone(), wf);
        }
    }

    let api_routes = Router::new()
        .route("/api/workflows", get(api::list_workflows).post(api::create_workflow))
        .route("/api/workflows/sample", post(api::load_sample))
        .route("/api/workflows/{id}/status", get(api::get_workflow_status))
        .route("/api/workflows/{id}/run", post(api::run_workflow))
        .route(
            "/api/workflows/{wf_id}/jobs/{job_id}/rerun",
            post(api::rerun_job),
        )
        .with_state(state);

    // Serve WASM pkg at /pkg/ and static files from www/ at root
    let static_files = ServeDir::new("www")
        .fallback(ServeDir::new("."));

    let app = api_routes
        .fallback_service(static_files)
        .layer(CorsLayer::permissive());

    let preferred_port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let listener = find_available_port(preferred_port).await;
    let port = listener.local_addr().unwrap().port();
    println!("Server listening on http://localhost:{port}");

    axum::serve(listener, app).await.unwrap();
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
        let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if !(filename.ends_with(".yml") || filename.ends_with(".yaml") || filename.ends_with(".json"))
        {
            continue;
        }

        let Ok(contents) = std::fs::read_to_string(&file_path) else {
            eprintln!("Failed to read {}", file_path.display());
            continue;
        };

        // Use filename stem as workflow ID
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

    // Let the OS pick
    tokio::net::TcpListener::bind(("0.0.0.0", 0u16))
        .await
        .expect("failed to bind to any port")
}
