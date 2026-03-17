use std::collections::HashMap;

use std::sync::Mutex as StdMutex;

use tokio::sync::{broadcast, Mutex};

use crate::error::LogError;
use crate::traits::{LogChunk, LogSink};

pub struct InMemoryLogSink {
    /// Stored log chunks, keyed by (workflow_id, job_id).
    store: Mutex<HashMap<(String, String), Vec<LogChunk>>>,
    /// Per-job broadcast channels for live streaming.
    /// Uses std::sync::Mutex since it's held only briefly and needs sync access in subscribe().
    channels: StdMutex<HashMap<(String, String), broadcast::Sender<LogChunk>>>,
}

impl InMemoryLogSink {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
            channels: StdMutex::new(HashMap::new()),
        }
    }

    fn get_or_create_channel(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> broadcast::Sender<LogChunk> {
        let key = (workflow_id.to_string(), job_id.to_string());
        self.channels
            .lock()
            .unwrap()
            .entry(key)
            .or_insert_with(|| broadcast::channel(256).0)
            .clone()
    }
}

impl Default for InMemoryLogSink {
    fn default() -> Self {
        Self::new()
    }
}

impl LogSink for InMemoryLogSink {
    async fn append(&self, chunk: LogChunk) -> Result<(), LogError> {
        let key = (chunk.workflow_id.clone(), chunk.job_id.clone());

        // Store the chunk
        self.store
            .lock()
            .await
            .entry(key)
            .or_default()
            .push(chunk.clone());

        // Broadcast to live subscribers
        let tx = self.get_or_create_channel(&chunk.workflow_id, &chunk.job_id);
        tx.send(chunk).ok(); // ok if no subscribers

        Ok(())
    }

    async fn get_all(
        &self,
        workflow_id: &str,
        job_id: &str,
    ) -> Result<Vec<LogChunk>, LogError> {
        let store = self.store.lock().await;
        let key = (workflow_id.to_string(), job_id.to_string());
        Ok(store.get(&key).cloned().unwrap_or_default())
    }

    fn subscribe(&self, workflow_id: &str, job_id: &str) -> broadcast::Receiver<LogChunk> {
        let key = (workflow_id.to_string(), job_id.to_string());
        // Use try_lock for sync context; if contended, create a new channel
        // (the append path will get_or_create the canonical one)
        let mut channels = self.channels.lock().unwrap();
        let tx = channels
            .entry(key)
            .or_insert_with(|| broadcast::channel(256).0);
        tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::LogStream;

    fn make_chunk(wf: &str, job: &str, seq: u64, data: &str) -> LogChunk {
        LogChunk {
            workflow_id: wf.into(),
            job_id: job.into(),
            sequence: seq,
            data: data.into(),
            timestamp_ms: 0,
            stream: LogStream::Stdout,
        }
    }

    #[tokio::test]
    async fn test_append_and_get_all() {
        let sink = InMemoryLogSink::new();
        sink.append(make_chunk("wf1", "j1", 0, "line 1\n"))
            .await
            .unwrap();
        sink.append(make_chunk("wf1", "j1", 1, "line 2\n"))
            .await
            .unwrap();

        let logs = sink.get_all("wf1", "j1").await.unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].data, "line 1\n");
        assert_eq!(logs[1].data, "line 2\n");
    }

    #[tokio::test]
    async fn test_subscribe_receives_live() {
        let sink = InMemoryLogSink::new();
        let mut rx = sink.subscribe("wf1", "j1");

        sink.append(make_chunk("wf1", "j1", 0, "hello\n"))
            .await
            .unwrap();

        let chunk = rx.recv().await.unwrap();
        assert_eq!(chunk.data, "hello\n");
    }

    #[tokio::test]
    async fn test_separate_jobs_isolated() {
        let sink = InMemoryLogSink::new();
        sink.append(make_chunk("wf1", "j1", 0, "job1"))
            .await
            .unwrap();
        sink.append(make_chunk("wf1", "j2", 0, "job2"))
            .await
            .unwrap();

        let logs1 = sink.get_all("wf1", "j1").await.unwrap();
        let logs2 = sink.get_all("wf1", "j2").await.unwrap();
        assert_eq!(logs1.len(), 1);
        assert_eq!(logs2.len(), 1);
        assert_eq!(logs1[0].data, "job1");
        assert_eq!(logs2[0].data, "job2");
    }
}
