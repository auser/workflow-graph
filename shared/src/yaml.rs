//! YAML workflow definition parser.
//!
//! Workflow files use a GitHub Actions-inspired format:
//!
//! ```yaml
//! name: CI Pipeline
//! on: push
//!
//! jobs:
//!   lint:
//!     name: Lint
//!     run: cargo clippy --all-targets
//!
//!   test:
//!     name: Unit Tests
//!     run: cargo test
//!
//!   build:
//!     name: Build
//!     needs: [lint, test]
//!     run: cargo build --release
//!
//!   deploy:
//!     name: Deploy
//!     needs: [build]
//!     steps:
//!       - name: Deploy DB
//!         run: ./scripts/migrate.sh
//!       - name: Deploy App
//!         run: ./scripts/deploy.sh
//! ```

use indexmap::IndexMap;
use serde::Deserialize;

use crate::{Job, JobStatus, Workflow};

/// Top-level YAML workflow definition.
#[derive(Debug, Deserialize)]
pub struct WorkflowDef {
    pub name: String,
    #[serde(rename = "on")]
    pub trigger: TriggerDef,
    #[serde(default)]
    pub env: IndexMap<String, String>,
    pub jobs: IndexMap<String, JobDef>,
}

/// Trigger can be a simple string or a structured definition.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TriggerDef {
    Simple(String),
    List(Vec<String>),
    Structured(IndexMap<String, serde_yaml::Value>),
}

impl TriggerDef {
    pub fn display(&self) -> String {
        match self {
            TriggerDef::Simple(s) => format!("on: {s}"),
            TriggerDef::List(v) => format!("on: [{}]", v.join(", ")),
            TriggerDef::Structured(m) => {
                let keys: Vec<&str> = m.keys().map(|k| k.as_str()).collect();
                format!("on: [{}]", keys.join(", "))
            }
        }
    }
}

/// A single job definition in the workflow YAML.
#[derive(Debug, Deserialize)]
pub struct JobDef {
    /// Display name (defaults to the job key if not set).
    pub name: Option<String>,
    /// Job dependencies — other job IDs that must succeed first.
    #[serde(default)]
    pub needs: Needs,
    /// Shell command to run (simple single-command job).
    pub run: Option<String>,
    /// Multi-step job (used instead of `run`).
    #[serde(default)]
    pub steps: Vec<StepDef>,
    /// Per-job environment variables.
    #[serde(default)]
    pub env: IndexMap<String, String>,
    /// Timeout in seconds.
    pub timeout: Option<u64>,
    /// Condition for running this job (expression string).
    #[serde(rename = "if")]
    pub condition: Option<String>,
    /// Worker labels required to execute this job.
    #[serde(default)]
    pub labels: Vec<String>,
    /// Maximum number of retries on failure (default 0).
    #[serde(default)]
    pub retries: u32,
}

/// Dependencies can be a single string or a list.
#[derive(Debug, Default, Deserialize)]
#[serde(untagged)]
pub enum Needs {
    #[default]
    None,
    Single(String),
    List(Vec<String>),
}

impl Needs {
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            Needs::None => vec![],
            Needs::Single(s) => vec![s.clone()],
            Needs::List(v) => v.clone(),
        }
    }
}

/// A single step within a job.
#[derive(Debug, Deserialize)]
pub struct StepDef {
    pub id: Option<String>,
    pub name: Option<String>,
    pub run: Option<String>,
    #[serde(rename = "if")]
    pub condition: Option<String>,
    #[serde(default)]
    pub env: IndexMap<String, String>,
}

impl WorkflowDef {
    /// Parse a YAML string into a workflow definition.
    pub fn from_yaml(yaml: &str) -> Result<Self, String> {
        serde_yaml::from_str(yaml).map_err(|e| format!("YAML parse error: {e}"))
    }

    /// Parse a JSON string into a workflow definition.
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("JSON parse error: {e}"))
    }

    /// Auto-detect format and parse. Tries JSON first (stricter), falls back to YAML.
    pub fn parse(input: &str) -> Result<Self, String> {
        let trimmed = input.trim_start();
        if trimmed.starts_with('{') {
            Self::from_json(input)
        } else {
            Self::from_yaml(input)
        }
    }

    /// Auto-detect format based on file extension.
    pub fn from_file_contents(contents: &str, filename: &str) -> Result<Self, String> {
        if filename.ends_with(".json") {
            Self::from_json(contents)
        } else if filename.ends_with(".yml") || filename.ends_with(".yaml") {
            Self::from_yaml(contents)
        } else {
            Self::parse(contents)
        }
    }

    /// Convert to the runtime `Workflow` model.
    ///
    /// For jobs with `steps`, the steps are joined into a single shell script
    /// separated by `&&`. For jobs with `run`, that command is used directly.
    pub fn into_workflow(self, id: &str) -> Result<Workflow, String> {
        let trigger = self.trigger.display();
        let mut jobs = Vec::with_capacity(self.jobs.len());

        for (job_id, job_def) in &self.jobs {
            let name = job_def.name.clone().unwrap_or_else(|| job_id.clone());

            let command = build_command(job_def, &self.env)?;
            let depends_on = job_def.needs.to_vec();

            // Validate dependencies exist
            for dep in &depends_on {
                if !self.jobs.contains_key(dep) {
                    return Err(format!(
                        "Job '{job_id}' depends on '{dep}', which doesn't exist"
                    ));
                }
            }

            jobs.push(Job {
                id: job_id.clone(),
                name,
                status: JobStatus::Queued,
                command,
                duration_secs: None,
                started_at: None,
                depends_on,
                output: None,
                required_labels: job_def.labels.clone(),
                max_retries: job_def.retries,
                attempt: 0,
                metadata: std::collections::HashMap::new(),
                ports: vec![],
            });
        }

        Ok(Workflow {
            id: id.to_string(),
            name: self.name,
            trigger,
            jobs,
        })
    }
}

/// Build the shell command for a job, combining env vars and steps/run.
fn build_command(job: &JobDef, global_env: &IndexMap<String, String>) -> Result<String, String> {
    // Collect env var exports
    let mut env_exports = Vec::new();
    for (k, v) in global_env {
        env_exports.push(format!("export {k}={}", shell_quote(v)));
    }
    for (k, v) in &job.env {
        env_exports.push(format!("export {k}={}", shell_quote(v)));
    }

    let commands = if !job.steps.is_empty() {
        // Multi-step: join step commands
        let step_cmds: Result<Vec<String>, String> = job
            .steps
            .iter()
            .enumerate()
            .filter_map(|(i, step)| {
                step.run.as_ref().map(|cmd| {
                    let mut parts = Vec::new();
                    // Per-step env
                    for (k, v) in &step.env {
                        parts.push(format!("export {k}={}", shell_quote(v)));
                    }
                    let default_label = format!("step {}", i + 1);
                    let label = step
                        .name
                        .as_deref()
                        .or(step.id.as_deref())
                        .unwrap_or(&default_label);
                    parts.push(format!("echo '=== {label} ==='"));
                    parts.push(cmd.trim().to_string());
                    Ok(parts.join(" && "))
                })
            })
            .collect();
        step_cmds?
    } else if let Some(run) = &job.run {
        vec![run.trim().to_string()]
    } else {
        return Err("Job must have either 'run' or 'steps'".to_string());
    };

    let mut full = env_exports;
    full.extend(commands);
    Ok(full.join(" && "))
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_workflow() {
        let yaml = r#"
name: CI
on: push

jobs:
  lint:
    name: Lint
    run: cargo clippy

  test:
    name: Test
    run: cargo test

  build:
    name: Build
    needs: [lint, test]
    run: cargo build --release
"#;
        let def = WorkflowDef::from_yaml(yaml).unwrap();
        let wf = def.into_workflow("ci-1").unwrap();

        assert_eq!(wf.name, "CI");
        assert_eq!(wf.trigger, "on: push");
        assert_eq!(wf.jobs.len(), 3);
        assert_eq!(wf.jobs[2].depends_on, vec!["lint", "test"]);
    }

    #[test]
    fn parse_steps_workflow() {
        let yaml = r#"
name: Deploy
on: push

jobs:
  deploy:
    name: Deploy All
    steps:
      - name: Migrate DB
        run: ./migrate.sh
      - name: Deploy App
        run: ./deploy.sh
"#;
        let def = WorkflowDef::from_yaml(yaml).unwrap();
        let wf = def.into_workflow("deploy-1").unwrap();

        assert_eq!(wf.jobs.len(), 1);
        assert!(wf.jobs[0].command.contains("Migrate DB"));
        assert!(wf.jobs[0].command.contains("./deploy.sh"));
    }

    #[test]
    fn invalid_dependency_errors() {
        let yaml = r#"
name: Bad
on: push

jobs:
  build:
    needs: [nonexistent]
    run: echo hi
"#;
        let def = WorkflowDef::from_yaml(yaml).unwrap();
        let result = def.into_workflow("bad-1");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("nonexistent"));
    }

    #[test]
    fn job_without_run_or_steps_errors() {
        let yaml = r#"
name: Bad
on: push

jobs:
  empty:
    name: Empty Job
"#;
        let def = WorkflowDef::from_yaml(yaml).unwrap();
        let result = def.into_workflow("bad-2");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("must have either 'run' or 'steps'")
        );
    }

    #[test]
    fn empty_jobs_map() {
        let yaml = r#"
name: Empty
on: push

jobs: {}
"#;
        let def = WorkflowDef::from_yaml(yaml).unwrap();
        let wf = def.into_workflow("empty-1").unwrap();
        assert_eq!(wf.jobs.len(), 0);
    }

    #[test]
    fn single_string_dependency() {
        let yaml = r#"
name: Single Dep
on: push

jobs:
  a:
    run: echo a
  b:
    needs: a
    run: echo b
"#;
        let def = WorkflowDef::from_yaml(yaml).unwrap();
        let wf = def.into_workflow("single-1").unwrap();
        assert_eq!(wf.jobs[1].depends_on, vec!["a"]);
    }

    #[test]
    fn special_characters_in_job_names() {
        let yaml = r#"
name: Special Chars
on: push

jobs:
  build-linux_x86:
    name: "Build (Linux x86_64)"
    run: echo "building"
"#;
        let def = WorkflowDef::from_yaml(yaml).unwrap();
        let wf = def.into_workflow("special-1").unwrap();
        assert_eq!(wf.jobs[0].id, "build-linux_x86");
        assert_eq!(wf.jobs[0].name, "Build (Linux x86_64)");
    }

    #[test]
    fn labels_and_retries_parsed() {
        let yaml = r#"
name: Config
on: push

jobs:
  deploy:
    name: Deploy
    run: ./deploy.sh
    labels: [linux, aws]
    retries: 3
"#;
        let def = WorkflowDef::from_yaml(yaml).unwrap();
        let wf = def.into_workflow("config-1").unwrap();
        assert_eq!(wf.jobs[0].required_labels, vec!["linux", "aws"]);
        assert_eq!(wf.jobs[0].max_retries, 3);
    }

    #[test]
    fn env_vars_in_command() {
        let yaml = r#"
name: Env
on: push

env:
  GLOBAL: "value"

jobs:
  test:
    run: echo test
    env:
      LOCAL: "local_value"
"#;
        let def = WorkflowDef::from_yaml(yaml).unwrap();
        let wf = def.into_workflow("env-1").unwrap();
        assert!(wf.jobs[0].command.contains("export GLOBAL="));
        assert!(wf.jobs[0].command.contains("export LOCAL="));
    }

    #[test]
    fn json_format_parsing() {
        let json = r#"{
            "name": "JSON Workflow",
            "on": "push",
            "jobs": {
                "test": {
                    "run": "echo test"
                }
            }
        }"#;
        let def = WorkflowDef::from_json(json).unwrap();
        let wf = def.into_workflow("json-1").unwrap();
        assert_eq!(wf.name, "JSON Workflow");
        assert_eq!(wf.jobs.len(), 1);
    }

    #[test]
    fn malformed_yaml_returns_error() {
        let yaml = "this is not valid yaml: [[[";
        assert!(WorkflowDef::from_yaml(yaml).is_err());
    }

    #[test]
    fn shell_quote_handles_single_quotes() {
        let result = super::shell_quote("it's a test");
        assert_eq!(result, "'it'\\''s a test'");
    }
}
