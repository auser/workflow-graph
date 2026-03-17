use workflow_graph_worker_sdk::{Worker, WorkerConfig};

#[tokio::main]
async fn main() {
    let server_url = std::env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:3000".into());

    let labels: Vec<String> = std::env::var("WORKER_LABELS")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().to_string())
        .collect();

    let config = WorkerConfig {
        server_url,
        labels,
        ..Default::default()
    };

    println!("Starting worker {} ...", config.worker_id);

    let worker = Worker::new(config);
    if let Err(e) = worker.run().await {
        eprintln!("Worker failed: {e}");
        std::process::exit(1);
    }
}
