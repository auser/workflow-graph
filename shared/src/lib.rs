pub mod yaml;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Direction of a port on a node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortDirection {
    Input,
    Output,
}

/// A typed input or output port on a node.
/// Ports define connection points — edges connect from an output port to an input port.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Port {
    /// Unique identifier within the node (e.g., "message", "response").
    pub id: String,
    /// Display label.
    pub label: String,
    /// Whether this is an input or output port.
    pub direction: PortDirection,
    /// Type tag for connection compatibility (e.g., "text", "json", "tool_call").
    /// Only ports with matching types can be connected.
    #[serde(default)]
    pub port_type: String,
    /// Optional color override for the port dot.
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Success,
    Failure,
    Skipped,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub name: String,
    pub status: JobStatus,
    pub command: String,
    pub duration_secs: Option<u64>,
    /// Epoch milliseconds when the job started running (for live timer).
    #[serde(default)]
    pub started_at: Option<f64>,
    pub depends_on: Vec<String>,
    pub output: Option<String>,
    /// Worker labels required to execute this job.
    #[serde(default)]
    pub required_labels: Vec<String>,
    /// Maximum number of retries on failure.
    #[serde(default)]
    pub max_retries: u32,
    /// Current attempt number (0-indexed).
    #[serde(default)]
    pub attempt: u32,
    /// Arbitrary metadata for custom renderers (e.g., node_type, icon, color).
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Input and output ports for node-graph-style connections.
    #[serde(default)]
    pub ports: Vec<Port>,
    /// If this is a compound node (node group), contains the child nodes.
    /// When collapsed, renders as a single node with aggregated ports.
    /// When expanded, renders children with a dashed border.
    #[serde(default)]
    pub children: Option<Vec<Job>>,
    /// Whether this compound node is collapsed (shows as single node).
    #[serde(default)]
    pub collapsed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub trigger: String,
    pub jobs: Vec<Job>,
}

impl Workflow {
    /// Returns a sample workflow matching the GitHub Actions screenshot.
    pub fn sample() -> Self {
        Workflow {
            id: "ci-1".into(),
            name: "ci.yml".into(),
            trigger: "on: push".into(),
            jobs: vec![
                Job {
                    id: "unit-tests".into(),
                    name: "Unit Tests".into(),
                    status: JobStatus::Queued,
                    command: "echo 'Running unit tests' && sleep 2".into(),
                    duration_secs: None,
                    depends_on: vec![],
                    started_at: None,
                    output: None,
                    required_labels: vec![],
                    max_retries: 0,
                    attempt: 0,
                    metadata: HashMap::new(),
                    ports: vec![],
                    children: None,
                    collapsed: false,
                },
                Job {
                    id: "lint".into(),
                    name: "Lint".into(),
                    status: JobStatus::Queued,
                    command: "echo 'Running linter' && sleep 1".into(),
                    duration_secs: None,
                    depends_on: vec![],
                    started_at: None,
                    output: None,
                    required_labels: vec![],
                    max_retries: 0,
                    attempt: 0,
                    metadata: HashMap::new(),
                    ports: vec![],
                    children: None,
                    collapsed: false,
                },
                Job {
                    id: "typecheck".into(),
                    name: "Typecheck".into(),
                    status: JobStatus::Queued,
                    command: "echo 'Running typecheck' && sleep 2".into(),
                    duration_secs: None,
                    depends_on: vec![],
                    started_at: None,
                    output: None,
                    required_labels: vec![],
                    max_retries: 0,
                    attempt: 0,
                    metadata: HashMap::new(),
                    ports: vec![],
                    children: None,
                    collapsed: false,
                },
                Job {
                    id: "build".into(),
                    name: "Build".into(),
                    status: JobStatus::Queued,
                    command: "echo 'Building project' && sleep 3".into(),
                    duration_secs: None,
                    depends_on: vec!["unit-tests".into(), "lint".into(), "typecheck".into()],
                    started_at: None,
                    output: None,
                    required_labels: vec![],
                    max_retries: 0,
                    attempt: 0,
                    metadata: HashMap::new(),
                    ports: vec![],
                    children: None,
                    collapsed: false,
                },
                Job {
                    id: "deploy-db".into(),
                    name: "Deploy DB Migrations".into(),
                    status: JobStatus::Queued,
                    command: "echo 'Deploying DB migrations' && sleep 1".into(),
                    duration_secs: None,
                    depends_on: vec!["build".into()],
                    started_at: None,
                    output: None,
                    required_labels: vec![],
                    max_retries: 0,
                    attempt: 0,
                    metadata: HashMap::new(),
                    ports: vec![],
                    children: None,
                    collapsed: false,
                },
                Job {
                    id: "e2e-tests".into(),
                    name: "E2E Tests".into(),
                    status: JobStatus::Queued,
                    command: "echo 'Running E2E tests' && sleep 5".into(),
                    duration_secs: None,
                    depends_on: vec!["build".into()],
                    started_at: None,
                    output: None,
                    required_labels: vec![],
                    max_retries: 0,
                    attempt: 0,
                    metadata: HashMap::new(),
                    ports: vec![],
                    children: None,
                    collapsed: false,
                },
                Job {
                    id: "deploy-preview".into(),
                    name: "Deploy Preview".into(),
                    status: JobStatus::Queued,
                    command: "echo 'Deploying preview' && sleep 1".into(),
                    duration_secs: None,
                    depends_on: vec!["build".into()],
                    started_at: None,
                    output: None,
                    required_labels: vec![],
                    max_retries: 0,
                    attempt: 0,
                    metadata: HashMap::new(),
                    ports: vec![],
                    children: None,
                    collapsed: false,
                },
                Job {
                    id: "deploy-web".into(),
                    name: "Deploy Web".into(),
                    status: JobStatus::Queued,
                    command: "echo 'Deploying to production' && sleep 3".into(),
                    duration_secs: None,
                    depends_on: vec!["deploy-db".into()],
                    started_at: None,
                    output: None,
                    required_labels: vec![],
                    max_retries: 0,
                    attempt: 0,
                    metadata: HashMap::new(),
                    ports: vec![],
                    children: None,
                    collapsed: false,
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_job(id: &str, metadata: HashMap<String, serde_json::Value>) -> Job {
        Job {
            id: id.into(),
            name: id.into(),
            status: JobStatus::Queued,
            command: "echo test".into(),
            duration_secs: None,
            started_at: None,
            depends_on: vec![],
            output: None,
            required_labels: vec![],
            max_retries: 0,
            attempt: 0,
            metadata,
            ports: vec![],
            children: None,
            collapsed: false,
        }
    }

    #[test]
    fn job_metadata_serializes_roundtrip() {
        let mut meta = HashMap::new();
        meta.insert("node_type".into(), serde_json::json!("deploy"));
        meta.insert("icon".into(), serde_json::json!("rocket"));
        meta.insert("priority".into(), serde_json::json!(42));

        let job = make_job("deploy-1", meta);
        let json = serde_json::to_string(&job).unwrap();
        let deserialized: Job = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.metadata.len(), 3);
        assert_eq!(
            deserialized.metadata["node_type"],
            serde_json::json!("deploy")
        );
        assert_eq!(deserialized.metadata["icon"], serde_json::json!("rocket"));
        assert_eq!(deserialized.metadata["priority"], serde_json::json!(42));
    }

    #[test]
    fn job_metadata_defaults_to_empty() {
        let json = r#"{
            "id": "test",
            "name": "Test",
            "status": "queued",
            "command": "echo hi",
            "depends_on": []
        }"#;
        let job: Job = serde_json::from_str(json).unwrap();
        assert!(job.metadata.is_empty());
    }

    #[test]
    fn job_metadata_with_nested_values() {
        let mut meta = HashMap::new();
        meta.insert(
            "config".into(),
            serde_json::json!({"timeout": 30, "retries": true}),
        );
        meta.insert("tags".into(), serde_json::json!(["ci", "deploy"]));

        let job = make_job("complex", meta);
        let json = serde_json::to_string(&job).unwrap();
        let deserialized: Job = serde_json::from_str(&json).unwrap();

        assert_eq!(
            deserialized.metadata["config"],
            serde_json::json!({"timeout": 30, "retries": true})
        );
        assert_eq!(
            deserialized.metadata["tags"],
            serde_json::json!(["ci", "deploy"])
        );
    }

    #[test]
    fn job_metadata_from_json_string() {
        let json = r##"{
            "id": "styled",
            "name": "Styled Node",
            "status": "running",
            "command": "echo hi",
            "depends_on": [],
            "metadata": {
                "color": "#ff0000",
                "weight": 1.5,
                "visible": true
            }
        }"##;
        let job: Job = serde_json::from_str(json).unwrap();
        assert_eq!(job.metadata.len(), 3);
        assert_eq!(job.metadata["color"], serde_json::json!("#ff0000"));
        assert_eq!(job.metadata["weight"], serde_json::json!(1.5));
        assert_eq!(job.metadata["visible"], serde_json::json!(true));
    }

    #[test]
    fn workflow_sample_has_empty_metadata() {
        let wf = Workflow::sample();
        for job in &wf.jobs {
            assert!(
                job.metadata.is_empty(),
                "Expected empty metadata for job '{}'",
                job.id
            );
        }
    }
}
