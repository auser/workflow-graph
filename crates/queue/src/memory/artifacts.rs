use std::collections::HashMap;

use tokio::sync::Mutex;

use crate::error::ArtifactError;
use crate::traits::ArtifactStore;

pub struct InMemoryArtifactStore {
    /// Keyed by (workflow_id, job_id).
    store: Mutex<HashMap<(String, String), HashMap<String, String>>>,
}

impl InMemoryArtifactStore {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryArtifactStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ArtifactStore for InMemoryArtifactStore {
    async fn put_outputs(
        &self,
        workflow_id: &str,
        job_id: &str,
        outputs: HashMap<String, String>,
    ) -> Result<(), ArtifactError> {
        self.store
            .lock()
            .await
            .insert((workflow_id.to_string(), job_id.to_string()), outputs);
        Ok(())
    }

    async fn get_outputs(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> Result<HashMap<String, String>, ArtifactError> {
        let store = self.store.lock().await;
        Ok(store
            .get(&(workflow_id.to_string(), job_id.to_string()))
            .cloned()
            .unwrap_or_default())
    }

    async fn get_upstream_outputs(
        &self,
        workflow_id: &str,
        job_ids: &[String],
    ) -> Result<HashMap<String, HashMap<String, String>>, ArtifactError> {
        let store = self.store.lock().await;
        let mut result = HashMap::new();
        for job_id in job_ids {
            let key = (workflow_id.to_string(), job_id.clone());
            if let Some(outputs) = store.get(&key) {
                result.insert(job_id.clone(), outputs.clone());
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_put_and_get() {
        let store = InMemoryArtifactStore::new();
        let mut outputs = HashMap::new();
        outputs.insert("version".into(), "1.0".into());

        store.put_outputs("wf1", "build", outputs).await.unwrap();

        let retrieved = store.get_outputs("wf1", "build").await.unwrap();
        assert_eq!(retrieved.get("version").unwrap(), "1.0");
    }

    #[tokio::test]
    async fn test_get_upstream_outputs() {
        let store = InMemoryArtifactStore::new();

        let mut o1 = HashMap::new();
        o1.insert("hash".into(), "abc123".into());
        store.put_outputs("wf1", "build", o1).await.unwrap();

        let mut o2 = HashMap::new();
        o2.insert("passed".into(), "true".into());
        store.put_outputs("wf1", "test", o2).await.unwrap();

        let upstream = store
            .get_upstream_outputs("wf1", &["build".into(), "test".into(), "missing".into()])
            .await
            .unwrap();

        assert_eq!(upstream.len(), 2);
        assert_eq!(upstream["build"]["hash"], "abc123");
        assert_eq!(upstream["test"]["passed"], "true");
    }

    #[tokio::test]
    async fn test_missing_returns_empty() {
        let store = InMemoryArtifactStore::new();
        let result = store.get_outputs("wf1", "nonexistent").await.unwrap();
        assert!(result.is_empty());
    }
}
