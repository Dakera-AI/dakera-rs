//! Agent management for the Dakera client.

use serde::{Deserialize, Serialize};

use crate::error::{ClientError, Result};
use crate::memory::{RecalledMemory, Session};
use crate::types::{
    AgentConsolidateResponse, AgentConsolidationConfig, AgentConsolidationLogEntry,
    ConsolidationConfigPatch,
};
use crate::DakeraClient;

// ============================================================================
// Agent Types
// ============================================================================

/// Summary of an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    /// Agent id.
    pub agent_id: String,
    /// The agent's memories (sentence sub-memories excluded).
    pub memory_count: i64,
    /// Sessions of the agent, ended or not.
    pub session_count: i64,
    /// Sessions of the agent not ended yet.
    pub active_sessions: i64,
    /// Records in the agent's memory namespace (memories, sub-memories,
    /// bookkeeping). Since server v0.12.2 the namespace seed is not counted,
    /// so an empty agent reports 0. `0` from servers that do not send it.
    #[serde(default)]
    pub vector_count: i64,
    /// Why `vector_count` is unknown (`0`): the agent's namespace could not
    /// be counted in time (server v0.12.2+). `None` when it was counted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<String>,
}

/// Body of `POST /v1/agents` (server v0.12.2+).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentRequest {
    /// The agent id (`[A-Za-z0-9][A-Za-z0-9_.-]*`, at most 241 bytes, not
    /// starting with `_dakera_`).
    pub agent_id: String,
}

/// Response of `POST /v1/agents` (server v0.12.2+).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentResponse {
    /// The agent id.
    pub agent_id: String,
    /// The agent's memory namespace (`_dakera_agent_<agent_id>`).
    pub namespace: String,
    /// `true` when the agent was created (HTTP 201); `false` when it already
    /// existed (HTTP 200) and was left untouched.
    pub created: bool,
    /// Dimension of the namespace's vectors (`None` if the server could not
    /// read it).
    #[serde(default)]
    pub dimension: Option<usize>,
    /// The embedding model the agent's memories are embedded with.
    #[serde(default)]
    pub model: Option<String>,
}

/// Options of `GET /v1/agents/{agent_id}/memories`.
///
/// Every field is optional and omitted from the query when unset.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgentMemoriesOptions {
    /// Filter by memory type (sent as `memory_type`).
    pub memory_type: Option<String>,
    /// Page size (server default 50, capped at 1000).
    pub limit: Option<u32>,
    /// Memories to skip.
    pub offset: Option<u32>,
    /// Also list derived records, the sentence sub-memories (server
    /// v0.12.2+). Since v0.12.2 the listing leaves them out by default.
    pub include_derived: Option<bool>,
    /// Cut each memory's `content` to this many characters (1..=10000) and
    /// fill [`RecalledMemory::content_len`] / [`RecalledMemory::content_truncated`]
    /// (server v0.12.2+). Read the whole memory with
    /// [`DakeraClient::get_memory`] when `content_truncated` is true.
    pub content_preview_chars: Option<u32>,
}

impl AgentMemoriesOptions {
    /// The query string pairs for the set fields, in a stable order.
    pub fn query_pairs(&self) -> Vec<(&'static str, String)> {
        let mut q = Vec::new();
        if let Some(t) = &self.memory_type {
            q.push(("memory_type", t.clone()));
        }
        if let Some(l) = self.limit {
            q.push(("limit", l.to_string()));
        }
        if let Some(o) = self.offset {
            q.push(("offset", o.to_string()));
        }
        if let Some(d) = self.include_derived {
            q.push(("include_derived", d.to_string()));
        }
        if let Some(c) = self.content_preview_chars {
            q.push(("content_preview_chars", c.to_string()));
        }
        q
    }
}

/// Options of `GET /v1/agents/{agent_id}/wake-up`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WakeUpOptions {
    /// Maximum memories to return (server default 20, max 100).
    pub top_n: Option<u32>,
    /// Only memories with importance at least this value.
    pub min_importance: Option<f32>,
    /// Also rank derived records, the sentence sub-memories (server
    /// v0.12.2+, which leaves them out by default).
    pub include_derived: Option<bool>,
}

/// Largest `content_preview_chars` the server accepts.
pub const MAX_CONTENT_PREVIEW_CHARS: u32 = 10_000;

/// `Err(InvalidRequest)` unless `preview` is `None` or in `1..=10000`.
pub(crate) fn check_content_preview(preview: Option<u32>) -> Result<()> {
    match preview {
        Some(n) if n == 0 || n > MAX_CONTENT_PREVIEW_CHARS => {
            Err(ClientError::InvalidRequest(format!(
                "content_preview_chars must be between 1 and {MAX_CONTENT_PREVIEW_CHARS} (got {n})"
            )))
        }
        _ => Ok(()),
    }
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
    /// Creation time of the oldest memory. The server sends Unix seconds as
    /// a number; it is kept here as its decimal string.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_string_or_number"
    )]
    pub oldest_memory_at: Option<String>,
    /// Creation time of the newest memory (Unix seconds as a decimal string).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_string_or_number"
    )]
    pub newest_memory_at: Option<String>,
}

/// Reads a timestamp the server sends as a number (Unix seconds) or a
/// string into `Option<String>`.
fn opt_string_or_number<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match v {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) => Some(s),
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        Some(other) => Some(other.to_string()),
    })
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

    /// Create an agent — its memory namespace — before its first memory
    /// (server v0.12.2+; `POST /v1/agents`).
    ///
    /// Needs Write on `_dakera_agent_<agent_id>` (a key granted
    /// `_dakera_agent_mlx-*` can create `mlx-dev`). Idempotent:
    /// [`CreateAgentResponse::created`] is `false` for an agent that already
    /// existed, which is left untouched.
    pub async fn create_agent(&self, agent_id: &str) -> Result<CreateAgentResponse> {
        let url = format!("{}/v1/agents", self.base_url);
        let body = CreateAgentRequest {
            agent_id: agent_id.to_string(),
        };
        let response = self.client.post(&url).json(&body).send().await?;
        self.handle_response(response).await
    }

    /// Get memories for an agent
    ///
    /// Since server v0.12.2 the listing leaves the sentence sub-memories out;
    /// use [`agent_memories_with`](Self::agent_memories_with) and
    /// [`AgentMemoriesOptions::include_derived`] to list them too.
    pub async fn agent_memories(
        &self,
        agent_id: &str,
        memory_type: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<RecalledMemory>> {
        let options = AgentMemoriesOptions {
            memory_type: memory_type.map(str::to_string),
            limit,
            ..Default::default()
        };
        self.agent_memories_with(agent_id, &options).await
    }

    /// Get memories for an agent with every listing option
    /// (`GET /v1/agents/{agent_id}/memories`).
    ///
    /// An out-of-range [`AgentMemoriesOptions::content_preview_chars`] is
    /// refused with [`ClientError::InvalidRequest`] before anything is sent.
    pub async fn agent_memories_with(
        &self,
        agent_id: &str,
        options: &AgentMemoriesOptions,
    ) -> Result<Vec<RecalledMemory>> {
        check_content_preview(options.content_preview_chars)?;
        let url = format!("{}/v1/agents/{}/memories", self.base_url, agent_id);
        let response = self
            .client
            .get(&url)
            .query(&options.query_pairs())
            .send()
            .await?;
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
        let options = WakeUpOptions {
            top_n,
            min_importance,
            include_derived: None,
        };
        self.wake_up_with(agent_id, &options).await
    }

    /// [`wake_up`](Self::wake_up) with every option, including
    /// [`WakeUpOptions::include_derived`] (server v0.12.2+, which leaves the
    /// sentence sub-memories out by default; `total_available` then counts
    /// memories only).
    pub async fn wake_up_with(
        &self,
        agent_id: &str,
        options: &WakeUpOptions,
    ) -> Result<WakeUpResponse> {
        let url = format!("{}/v1/agents/{}/wake-up", self.base_url, agent_id);
        let mut query: Vec<(&str, String)> = Vec::new();
        if let Some(n) = options.top_n {
            query.push(("top_n", n.to_string()));
        }
        if let Some(mi) = options.min_importance {
            query.push(("min_importance", mi.to_string()));
        }
        if let Some(d) = options.include_derived {
            query.push(("include_derived", d.to_string()));
        }
        let response = self.client.get(&url).query(&query).send().await?;
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

    /// Alias for [`compress`](Self::compress) matching Python/JS/Go SDK naming.
    pub async fn compress_agent(&self, agent_id: &str) -> Result<CompressResponse> {
        self.compress(agent_id).await
    }

    /// Consolidate memories for an agent using the agent-scoped endpoint.
    #[tracing::instrument(skip(self))]
    pub async fn consolidate_agent(&self, agent_id: &str) -> Result<AgentConsolidateResponse> {
        let url = format!("{}/v1/agents/{}/consolidate", self.base_url, agent_id);
        let response = self.client.post(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get the consolidation execution log for an agent.
    #[tracing::instrument(skip(self))]
    pub async fn get_consolidation_log(
        &self,
        agent_id: &str,
    ) -> Result<Vec<AgentConsolidationLogEntry>> {
        let url = format!("{}/v1/agents/{}/consolidation/log", self.base_url, agent_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Update the consolidation configuration for an agent.
    #[tracing::instrument(skip(self, patch))]
    pub async fn patch_consolidation_config(
        &self,
        agent_id: &str,
        patch: ConsolidationConfigPatch,
    ) -> Result<AgentConsolidationConfig> {
        let url = format!(
            "{}/v1/agents/{}/consolidation/config",
            self.base_url, agent_id
        );
        let response = self.client.patch(&url).json(&patch).send().await?;
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
    /// Creation time. The server sends Unix seconds as a number; it is kept
    /// here as its decimal string (an ISO 8601 string is kept as is).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_string_or_number"
    )]
    pub created_at: Option<String>,
    /// Last update time (same encoding as `created_at`).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_string_or_number"
    )]
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
/// server runs the DBSCAN compression pass (`POST /v1/agents/{id}/compress`).
///
/// Server returns: `{"agent_id":"...","memories_scanned":N,"originals_deprecated":N,
/// "clusters_found":N,"summaries_created":N,...}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressResponse {
    /// The agent whose namespace was compressed
    pub agent_id: String,
    /// Memories scanned (server field `memories_scanned`)
    #[serde(default)]
    pub memories_scanned: i64,
    /// Memories removed via DBSCAN clustering (server field `originals_deprecated`)
    #[serde(default, alias = "removed_count")]
    pub originals_deprecated: i64,
    /// DBSCAN clusters found
    #[serde(default)]
    pub clusters_found: i64,
    /// Summary memories created from cluster centroids
    #[serde(default)]
    pub summaries_created: i64,
    /// IDs of memories that were deprecated
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deprecated_ids: Vec<String>,
    /// Wall-clock duration of the compression pass in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
    /// IDs of the summary memories written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub summary_ids: Vec<String>,
    /// Summaries the server refused or could not store (server v0.12.2+);
    /// the originals of their clusters are NOT deprecated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub summaries_skipped: Vec<SkippedSummary>,
}

/// A compression summary that was not written (`summaries_skipped[]`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedSummary {
    /// The id the summary would have had.
    pub summary_id: String,
    /// Why it was skipped (a validation message naming the field).
    pub reason: String,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // AgentSummary
    // -------------------------------------------------------------------------

    #[test]
    fn test_agent_summary_deserializes() {
        let json = r#"{
            "agent_id": "agent-xyz",
            "memory_count": 42,
            "session_count": 7,
            "active_sessions": 2
        }"#;
        let s: AgentSummary = serde_json::from_str(json).unwrap();
        assert_eq!(s.agent_id, "agent-xyz");
        assert_eq!(s.memory_count, 42);
        assert_eq!(s.session_count, 7);
        assert_eq!(s.active_sessions, 2);
    }

    // -------------------------------------------------------------------------
    // AgentStats
    // -------------------------------------------------------------------------

    #[test]
    fn test_agent_stats_memories_by_type_defaults_empty() {
        let json =
            r#"{"agent_id": "a", "total_memories": 0, "total_sessions": 0, "active_sessions": 0}"#;
        let s: AgentStats = serde_json::from_str(json).unwrap();
        assert!(s.memories_by_type.is_empty());
        assert!(s.avg_importance.is_none());
        assert!(s.oldest_memory_at.is_none());
        assert!(s.newest_memory_at.is_none());
    }

    #[test]
    fn test_agent_stats_with_type_distribution() {
        let json = r#"{
            "agent_id": "a",
            "total_memories": 10,
            "total_sessions": 3,
            "active_sessions": 1,
            "memories_by_type": {"episodic": 5, "semantic": 5},
            "avg_importance": 0.72
        }"#;
        let s: AgentStats = serde_json::from_str(json).unwrap();
        assert_eq!(s.memories_by_type["episodic"], 5);
        assert!((s.avg_importance.unwrap() - 0.72).abs() < 1e-6);
    }

    // -------------------------------------------------------------------------
    // Memory struct
    // -------------------------------------------------------------------------

    #[test]
    fn test_memory_optional_fields_omitted_in_serialize() {
        let m = Memory {
            id: "mem-1".to_string(),
            content: "hello".to_string(),
            memory_type: "episodic".to_string(),
            importance: 0.8,
            metadata: None,
            created_at: None,
            updated_at: None,
            access_count: None,
        };
        let json = serde_json::to_string(&m).unwrap();
        assert!(!json.contains("metadata"));
        assert!(!json.contains("created_at"));
        assert!(!json.contains("updated_at"));
        assert!(!json.contains("access_count"));
    }

    #[test]
    fn test_memory_with_all_optional_fields() {
        let json = r#"{
            "id": "m1",
            "content": "test",
            "memory_type": "semantic",
            "importance": 0.9,
            "metadata": {"key": "val"},
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-02T00:00:00Z",
            "access_count": 5
        }"#;
        let m: Memory = serde_json::from_str(json).unwrap();
        assert!(m.metadata.is_some());
        assert_eq!(m.access_count, Some(5));
    }

    // -------------------------------------------------------------------------
    // WakeUpResponse
    // -------------------------------------------------------------------------

    #[test]
    fn test_wake_up_response_deserializes() {
        let json = r#"{
            "agent_id": "agent-1",
            "memories": [],
            "total_available": 50
        }"#;
        let r: WakeUpResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.agent_id, "agent-1");
        assert!(r.memories.is_empty());
        assert_eq!(r.total_available, 50);
    }

    // -------------------------------------------------------------------------
    // CompressResponse — alias removed_count → originals_deprecated
    // -------------------------------------------------------------------------

    #[test]
    fn test_compress_response_defaults_zero() {
        let json = r#"{"agent_id": "a"}"#;
        let r: CompressResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.memories_scanned, 0);
        assert_eq!(r.originals_deprecated, 0);
        assert_eq!(r.clusters_found, 0);
        assert_eq!(r.summaries_created, 0);
        assert!(r.deprecated_ids.is_empty());
        assert!(r.duration_ms.is_none());
    }

    #[test]
    fn test_compress_response_alias_removed_count() {
        let json = r#"{
            "agent_id": "a",
            "memories_scanned": 100,
            "removed_count": 30,
            "clusters_found": 10,
            "summaries_created": 10
        }"#;
        let r: CompressResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.originals_deprecated, 30);
    }

    #[test]
    fn test_compress_response_deprecated_ids_omitted_when_empty() {
        let r = CompressResponse {
            agent_id: "a".to_string(),
            memories_scanned: 0,
            originals_deprecated: 0,
            clusters_found: 0,
            summaries_created: 0,
            deprecated_ids: vec![],
            duration_ms: None,
            summary_ids: vec![],
            summaries_skipped: vec![],
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("deprecated_ids"));
    }

    #[test]
    fn test_compress_response_with_duration_and_ids() {
        let json = r#"{
            "agent_id": "a",
            "deprecated_ids": ["m1", "m2"],
            "duration_ms": 42.5
        }"#;
        let r: CompressResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.deprecated_ids.len(), 2);
        assert!((r.duration_ms.unwrap() - 42.5).abs() < 1e-6);
    }
}
