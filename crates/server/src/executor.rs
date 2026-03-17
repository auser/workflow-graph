use std::time::Instant;
use tokio::process::Command;

pub struct JobResult {
    pub success: bool,
    pub duration_secs: u64,
    pub output: String,
}

pub async fn execute_job(command: &str) -> JobResult {
    let start = Instant::now();

    let result = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .await;

    let duration_secs = start.elapsed().as_secs();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = if stderr.is_empty() {
                stdout.to_string()
            } else {
                format!("{stdout}\n--- stderr ---\n{stderr}")
            };
            // Truncate output to avoid sending huge payloads
            let truncated = if combined.len() > 4096 {
                format!("{}...(truncated)", &combined[..4096])
            } else {
                combined
            };
            JobResult {
                success: output.status.success(),
                duration_secs,
                output: truncated,
            }
        }
        Err(e) => JobResult {
            success: false,
            duration_secs,
            output: format!("Failed to execute command: {e}"),
        },
    }
}
