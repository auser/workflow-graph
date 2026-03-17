pub mod yaml;

use serde::{Deserialize, Serialize};

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
                },
                Job {
                    id: "build".into(),
                    name: "Build".into(),
                    status: JobStatus::Queued,
                    command: "echo 'Building project' && sleep 3".into(),
                    duration_secs: None,
                    depends_on: vec![
                        "unit-tests".into(),
                        "lint".into(),
                        "typecheck".into(),
                    ],
                    started_at: None,
                    output: None,
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
                },
            ],
        }
    }
}
