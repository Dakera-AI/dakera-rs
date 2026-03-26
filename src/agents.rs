//! Agent management for the Dakera client.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::memory::{RecalledMemory, Session};
use crate::DakeraClient;

// ============================================================================
// Agent Types
// ============================================================================

/// Summary of an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub agent_id: String,
    pub memory_count: i64,
    pub session_count: i64,
    pub active_sessions: i64,
}

/// Detailed stats for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStats {
    pub agent_id: String,
    pub total_memories: i64,
    #[serde(default)]
    pub memories_by_type: std::collections::HashMap<String, i64>,
    pub total_sessions: i64,
    pub active_sessions: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_importance: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_memory_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newest_memory_at: Option<String>,
}

// ============================================================================
// Agent Client Methods
// ============================================================================

impl DakeraClient {
    /// List all agents
    pub async fn list_agents(&self) -> Result<Vec<AgentSummary>> {
        let url = format!("{}/v1/agents", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get memories for an agent
    pub async fn agent_memories(
        &self,
        agent_id: &str,
        memory_type: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<RecalledMemory>> {
        let mut url = format!("{}/v1/agents/{}/memories", self.base_url, agent_id);
        let mut params = Vec::new();
        if let Some(t) = memory_type {
            params.push(format!("memory_type={}", t));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get stats for an agent
    pub async fn agent_stats(&self, agent_id: &str) -> Result<AgentStats> {
        let url = format!("{}/v1/agents/{}/stats", self.base_url, agent_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Subscribe to real-time memory lifecycle events for a specific agent.
    ///
    /// Opens a long-lived connection to `GET /v1/events/stream` and returns a
    /// [`tokio::sync::mpsc::Receiver`] that yields [`MemoryEvent`] results filtered
    /// to the given `agent_id`.  An optional `tags` list further restricts events
    /// to those whose tags have at least one overlap with the filter.
    ///
    /// The background task reconnects automatically on stream error.  It exits
    /// when the returned receiver is dropped.
    ///
    /// Requires a Read-scoped API key.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dakera_client::DakeraClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = DakeraClient::new("http://localhost:3000")?;
    ///     let mut rx = client.subscribe_agent_events("my-bot", None).await?;
    ///     while let Some(result) = rx.recv().await {
    ///         let event = result?;
    ///         println!("{}: {:?}", event.event_type, event.memory_id);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn subscribe_agent_events(
        &self,
        agent_id: &str,
        tags: Option<Vec<String>>,
    ) -> crate::error::Result<tokio::sync::mpsc::Receiver<crate::error::Result<crate::events::MemoryEvent>>> {
        let (tx, rx) = tokio::sync::mpsc::channel(64);
        let client = self.clone();
        let agent_id = agent_id.to_owned();

        tokio::spawn(async move {
            loop {
                match client.stream_memory_events().await {
                    Err(_) => {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        continue;
                    }
                    Ok(mut inner_rx) => {
                        while let Some(result) = inner_rx.recv().await {
                            match result {
                                Err(e) => {
                                    // Send the error but don't kill the reconnect loop.
                                    let _ = tx.send(Err(e)).await;
                                    break;
                                }
                                Ok(event) => {
                                    if event.event_type == "connected" {
                                        continue;
                                    }
                                    if event.agent_id != agent_id {
                                        continue;
                                    }
                                    if let Some(ref filter_tags) = tags {
                                        let event_tags = event.tags.as_deref().unwrap_or(&[]);
                                        if !filter_tags.iter().any(|t| event_tags.contains(t)) {
                                            continue;
                                        }
                                    }
                                    if tx.send(Ok(event)).await.is_err() {
                                        return; // Receiver dropped — exit.
                                    }
                                }
                            }
                        }
                    }
                }
                // Reconnect after a short delay.
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });

        Ok(rx)
    }

    /// Get sessions for an agent
    pub async fn agent_sessions(
        &self,
        agent_id: &str,
        active_only: Option<bool>,
        limit: Option<u32>,
    ) -> Result<Vec<Session>> {
        let mut url = format!("{}/v1/agents/{}/sessions", self.base_url, agent_id);
        let mut params = Vec::new();
        if let Some(active) = active_only {
            params.push(format!("active_only={}", active));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }
}
