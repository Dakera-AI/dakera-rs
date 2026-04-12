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
    ) -> crate::error::Result<
        tokio::sync::mpsc::Receiver<crate::error::Result<crate::events::MemoryEvent>>,
    > {
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

    /// Return top-N wake-up context memories for an agent (DAK-1690).
    ///
    /// Calls `GET /v1/agents/{agent_id}/wake-up`. Returns memories ranked by
    /// `importance × exp(-ln2 × age / 14d)` — no embedding inference, served
    /// from the metadata index for sub-millisecond latency.
    ///
    /// Requires Read scope on the agent namespace.
    ///
    /// # Arguments
    /// * `agent_id` — Agent identifier.
    /// * `top_n` — Maximum memories to return (default 20, max 100). Pass `None` to use default.
    /// * `min_importance` — Only return memories with importance ≥ this value. Pass `None` for 0.0.
    pub async fn wake_up(
        &self,
        agent_id: &str,
        top_n: Option<u32>,
        min_importance: Option<f32>,
    ) -> Result<WakeUpResponse> {
        let mut url = format!("{}/v1/agents/{}/wake-up", self.base_url, agent_id);
        let mut params = Vec::new();
        if let Some(n) = top_n {
            params.push(format!("top_n={}", n));
        }
        if let Some(mi) = min_importance {
            params.push(format!("min_importance={}", mi));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Compress the memory namespace for an agent (CE-12).
    ///
    /// Runs a server-side compression pass that removes low-value or redundant
    /// memories, returning statistics about the operation.
    ///
    /// # Arguments
    /// * `agent_id` — Agent identifier.
    pub async fn compress(&self, agent_id: &str) -> Result<CompressResponse> {
        let url = format!("{}/v1/agents/{}/compress", self.base_url, agent_id);
        let response = self.client.post(&url).send().await?;
        self.handle_response(response).await
    }
}

// ============================================================================
// Wake-Up Types (DAK-1690)
// ============================================================================

/// A stored memory returned by agent endpoints (non-recall, no similarity score).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Memory ID
    pub id: String,
    /// Memory content
    pub content: String,
    /// Memory type (episodic, semantic, procedural, working)
    pub memory_type: String,
    /// Importance score (0.0–1.0)
    pub importance: f32,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Creation timestamp (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Last update timestamp (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// Number of times this memory has been accessed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_count: Option<i64>,
}

/// Response from `GET /v1/agents/{agent_id}/wake-up` (DAK-1690).
///
/// Contains top-N memories ranked by recency-weighted importance for fast
/// agent start-up context loading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakeUpResponse {
    /// The agent whose memories are returned
    pub agent_id: String,
    /// Top-N memories ranked by `importance × exp(-ln2 × age / 14d)`
    pub memories: Vec<Memory>,
    /// Total memories available before `top_n` cap was applied
    pub total_available: i64,
}

// ============================================================================
// Compress Types (CE-12)
// ============================================================================

/// Response from `POST /v1/agents/{agent_id}/compress` (CE-12).
///
/// Contains compression statistics for the agent's memory namespace after the
/// server runs the compression pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressResponse {
    /// The agent whose namespace was compressed
    pub agent_id: String,
    /// Number of memories before compression
    pub memories_before: i64,
    /// Number of memories after compression
    pub memories_after: i64,
    /// Number of memories removed during compression
    pub removed_count: i64,
    /// Wall-clock duration of the compression pass in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
}
