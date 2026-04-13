//! Memory-oriented client methods for Dakera AI Agent Memory Platform
//!
//! Provides high-level methods for storing, recalling, and managing
//! agent memories and sessions through the Dakera API.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::types::{
    AgentFeedbackSummary, EdgeType, FeedbackHealthResponse, FeedbackHistoryResponse,
    FeedbackResponse, FeedbackSignal, GraphExport, GraphLinkRequest, GraphLinkResponse,
    GraphOptions, GraphPath, MemoryFeedbackBody, MemoryGraph, MemoryImportancePatch,
};
use crate::DakeraClient;

// ============================================================================
// Memory Types (client-side)
// ============================================================================

/// Memory type classification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    #[default]
    Episodic,
    Semantic,
    Procedural,
    Working,
}

/// Store a memory request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreMemoryRequest {
    pub agent_id: String,
    pub content: String,
    #[serde(default)]
    pub memory_type: MemoryType,
    #[serde(default = "default_importance")]
    pub importance: f32,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Optional TTL in seconds. The memory is hard-deleted after this many
    /// seconds from creation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
    /// Optional explicit expiry as a Unix timestamp (seconds). Takes precedence
    /// over `ttl_seconds` when both are set. The memory is hard-deleted by the
    /// decay engine on expiry (DECAY-3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
}

fn default_importance() -> f32 {
    0.5
}

impl StoreMemoryRequest {
    /// Create a new store memory request
    pub fn new(agent_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            content: content.into(),
            memory_type: MemoryType::default(),
            importance: 0.5,
            tags: Vec::new(),
            session_id: None,
            metadata: None,
            ttl_seconds: None,
            expires_at: None,
        }
    }

    /// Set memory type
    pub fn with_type(mut self, memory_type: MemoryType) -> Self {
        self.memory_type = memory_type;
        self
    }

    /// Set importance score
    pub fn with_importance(mut self, importance: f32) -> Self {
        self.importance = importance.clamp(0.0, 1.0);
        self
    }

    /// Set tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Set session ID
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set TTL in seconds. The memory is hard-deleted after this many seconds
    /// from creation.
    pub fn with_ttl(mut self, ttl_seconds: u64) -> Self {
        self.ttl_seconds = Some(ttl_seconds);
        self
    }

    /// Set an explicit expiry Unix timestamp (seconds). Takes precedence over
    /// `ttl_seconds` when both are set (DECAY-3).
    pub fn with_expires_at(mut self, expires_at: u64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }
}

/// Stored memory response from `POST /v1/memory/store`.
///
/// The server wraps the memory in a nested `memory` object:
/// `{"memory": {"id": "...", "agent_id": "...", ...}, "embedding_time_ms": N}`.
/// The `memory_id` and `agent_id` fields are convenience accessors mapped from
/// `memory.id` and `memory.agent_id` respectively.
#[derive(Debug, Clone, Serialize)]
pub struct StoreMemoryResponse {
    /// Memory ID (mapped from `memory.id`)
    pub memory_id: String,
    /// Agent ID (mapped from `memory.agent_id`)
    pub agent_id: String,
    /// Namespace (mapped from `memory.namespace`, defaults to `"default"`)
    pub namespace: String,
    /// Embedding latency in milliseconds
    pub embedding_time_ms: Option<u64>,
}

impl<'de> serde::Deserialize<'de> for StoreMemoryResponse {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        use serde::de::Error;
        let val = serde_json::Value::deserialize(deserializer)?;

        // Server response: {"memory": {"id":"...","agent_id":"...",...}, "embedding_time_ms": N}
        if let Some(memory) = val.get("memory") {
            let memory_id = memory
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| D::Error::missing_field("memory.id"))?
                .to_string();
            let agent_id = memory
                .get("agent_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let namespace = memory
                .get("namespace")
                .and_then(|v| v.as_str())
                .unwrap_or("default")
                .to_string();
            let embedding_time_ms = val.get("embedding_time_ms").and_then(|v| v.as_u64());
            return Ok(Self {
                memory_id,
                agent_id,
                namespace,
                embedding_time_ms,
            });
        }

        // Legacy / mock format: {"memory_id":"...","agent_id":"...","namespace":"..."}
        let memory_id = val
            .get("memory_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| D::Error::missing_field("memory_id"))?
            .to_string();
        let agent_id = val
            .get("agent_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let namespace = val
            .get("namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();
        Ok(Self {
            memory_id,
            agent_id,
            namespace,
            embedding_time_ms: None,
        })
    }
}

/// Retrieval routing mode for recall and search (CE-10).
///
/// Controls which retrieval index the server uses. `Auto` (default) lets the
/// server pick the best strategy based on the query.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoutingMode {
    /// Server picks the best strategy (default).
    Auto,
    /// Force ANN vector search (HNSW).
    Vector,
    /// Force BM25 full-text search.
    Bm25,
    /// Fuse ANN and BM25 scores (RRF).
    Hybrid,
}

/// Recall memories request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallRequest {
    pub agent_id: String,
    pub query: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<MemoryType>,
    #[serde(default)]
    pub min_importance: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// COG-2: traverse KG depth-1 from recalled memories and include
    /// associatively linked memories in the response (default: false)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub include_associated: bool,
    /// COG-2: max associated memories to return (default: 10, max: 10)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub associated_memories_cap: Option<u32>,
    /// KG-3: KG traversal depth 1–3 (default: 1); requires include_associated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub associated_memories_depth: Option<u8>,
    /// KG-3: minimum edge weight for KG traversal (default: 0.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub associated_memories_min_weight: Option<f32>,
    /// CE-7: only recall memories created at or after this ISO-8601 timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// CE-7: only recall memories created at or before this ISO-8601 timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    /// CE-10: retrieval routing mode. `None` uses the server default (`auto`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing: Option<RoutingMode>,
    /// CE-13: cross-encoder reranking. `None` uses server default (`true` for recall,
    /// `false` for search). Set to `Some(false)` to disable on latency-sensitive paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rerank: Option<bool>,
}

fn default_top_k() -> usize {
    5
}

impl RecallRequest {
    /// Create a new recall request
    pub fn new(agent_id: impl Into<String>, query: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            query: query.into(),
            top_k: 5,
            memory_type: None,
            min_importance: 0.0,
            session_id: None,
            tags: Vec::new(),
            include_associated: false,
            associated_memories_cap: None,
            associated_memories_depth: None,
            associated_memories_min_weight: None,
            since: None,
            until: None,
            routing: None,
            rerank: None,
        }
    }

    /// Set number of results
    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k = top_k;
        self
    }

    /// Filter by memory type
    pub fn with_type(mut self, memory_type: MemoryType) -> Self {
        self.memory_type = Some(memory_type);
        self
    }

    /// Set minimum importance threshold
    pub fn with_min_importance(mut self, min: f32) -> Self {
        self.min_importance = min;
        self
    }

    /// Filter by session
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Filter by tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// COG-2: include KG depth-1 associated memories in the response
    pub fn with_associated(mut self) -> Self {
        self.include_associated = true;
        self
    }

    /// COG-2: set max associated memories cap (default: 10, max: 10)
    pub fn with_associated_cap(mut self, cap: u32) -> Self {
        self.include_associated = true;
        self.associated_memories_cap = Some(cap);
        self
    }

    /// CE-7: only recall memories created at or after this ISO-8601 timestamp
    pub fn with_since(mut self, since: impl Into<String>) -> Self {
        self.since = Some(since.into());
        self
    }

    /// CE-7: only recall memories created at or before this ISO-8601 timestamp
    pub fn with_until(mut self, until: impl Into<String>) -> Self {
        self.until = Some(until.into());
        self
    }

    /// CE-10: set retrieval routing mode
    pub fn with_routing(mut self, routing: RoutingMode) -> Self {
        self.routing = Some(routing);
        self
    }

    /// CE-13: enable or disable cross-encoder reranking (server default: true for recall)
    pub fn with_rerank(mut self, rerank: bool) -> Self {
        self.rerank = Some(rerank);
        self
    }

    /// KG-3: set KG traversal depth (1–3, default: 1); implies include_associated
    pub fn with_associated_depth(mut self, depth: u8) -> Self {
        self.include_associated = true;
        self.associated_memories_depth = Some(depth);
        self
    }

    /// KG-3: set minimum edge weight for KG traversal (default: 0.0)
    pub fn with_associated_min_weight(mut self, weight: f32) -> Self {
        self.associated_memories_min_weight = Some(weight);
        self
    }
}

/// A recalled memory
#[derive(Debug, Clone, Serialize)]
pub struct RecalledMemory {
    pub id: String,
    pub content: String,
    pub memory_type: MemoryType,
    pub importance: f32,
    pub score: f32,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub created_at: u64,
    pub last_accessed_at: u64,
    pub access_count: u32,
    /// KG-3: hop depth at which this memory was found (only set on associated memories)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u8>,
}

impl<'de> serde::Deserialize<'de> for RecalledMemory {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        use serde::de::Error as _;
        let val = serde_json::Value::deserialize(deserializer)?;

        // Server wraps recall results as {memory:{...}, score, weighted_score, smart_score}.
        // Fall back to flat format for direct memory-get responses.
        let score = val
            .get("score")
            .and_then(|v| v.as_f64())
            .or_else(|| val.get("weighted_score").and_then(|v| v.as_f64()))
            .unwrap_or(0.0) as f32;

        let mem = val.get("memory").unwrap_or(&val);

        let id = mem
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| D::Error::missing_field("id"))?
            .to_string();
        let content = mem
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| D::Error::missing_field("content"))?
            .to_string();
        let memory_type: MemoryType = mem
            .get("memory_type")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or(MemoryType::Episodic);
        let importance = mem
            .get("importance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5) as f32;
        let tags: Vec<String> = mem
            .get("tags")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let session_id = mem
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(String::from);
        let metadata = mem.get("metadata").cloned().filter(|v| !v.is_null());
        let created_at = mem.get("created_at").and_then(|v| v.as_u64()).unwrap_or(0);
        let last_accessed_at = mem
            .get("last_accessed_at")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let access_count = mem
            .get("access_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let depth = mem.get("depth").and_then(|v| v.as_u64()).map(|v| v as u8);

        Ok(Self {
            id,
            content,
            memory_type,
            importance,
            score,
            tags,
            session_id,
            metadata,
            created_at,
            last_accessed_at,
            access_count,
            depth,
        })
    }
}

/// Recall response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallResponse {
    pub memories: Vec<RecalledMemory>,
    #[serde(default)]
    pub total_found: usize,
    /// COG-2 / KG-3: KG associated memories at configurable depth (only present when include_associated was true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub associated_memories: Option<Vec<RecalledMemory>>,
}

/// Forget (delete) memories request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgetRequest {
    pub agent_id: String,
    #[serde(default)]
    pub memory_ids: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_timestamp: Option<u64>,
}

impl ForgetRequest {
    /// Forget specific memories by ID
    pub fn by_ids(agent_id: impl Into<String>, ids: Vec<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            memory_ids: ids,
            tags: Vec::new(),
            session_id: None,
            before_timestamp: None,
        }
    }

    /// Forget memories with specific tags
    pub fn by_tags(agent_id: impl Into<String>, tags: Vec<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            memory_ids: Vec::new(),
            tags,
            session_id: None,
            before_timestamp: None,
        }
    }

    /// Forget all memories in a session
    pub fn by_session(agent_id: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            memory_ids: Vec::new(),
            tags: Vec::new(),
            session_id: Some(session_id.into()),
            before_timestamp: None,
        }
    }
}

/// Forget response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgetResponse {
    pub deleted_count: u64,
}

/// Session start request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStartRequest {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub agent_id: String,
    pub started_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Cached count of memories in this session
    #[serde(default)]
    pub memory_count: usize,
}

/// Session end request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEndRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Response from `POST /v1/sessions/start`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStartResponse {
    pub session: Session,
}

/// Response from `POST /v1/sessions/{id}/end`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEndResponse {
    pub session: Session,
    pub memory_count: usize,
}

/// Request to update a memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMemoryRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<MemoryType>,
}

/// Request to update memory importance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateImportanceRequest {
    pub memory_ids: Vec<String>,
    pub importance: f32,
}

/// DBSCAN algorithm config for adaptive consolidation (CE-6).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConsolidationConfig {
    /// Clustering algorithm: `"dbscan"` (default) or `"greedy"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,
    /// Minimum cluster samples for DBSCAN.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_samples: Option<u32>,
    /// Epsilon distance parameter for DBSCAN.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eps: Option<f32>,
}

/// One step in the consolidation execution log (CE-6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationLogEntry {
    pub step: String,
    pub memories_before: usize,
    pub memories_after: usize,
    pub duration_ms: f64,
}

/// Request to consolidate memories
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConsolidateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
    #[serde(default)]
    pub dry_run: bool,
    /// Optional DBSCAN algorithm configuration (CE-6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<ConsolidationConfig>,
}

/// Response from consolidation (`POST /v1/memory/consolidate`).
///
/// The server returns `{"memories_removed": N, "source_memory_ids": [...], "consolidated_memory": {...}}`.
/// `consolidated_count` is mapped from `memories_removed` for backward compat.
#[derive(Debug, Clone, Serialize)]
pub struct ConsolidateResponse {
    /// Number of source memories removed (= `memories_removed` from server)
    pub consolidated_count: usize,
    /// Alias for consolidated_count
    pub removed_count: usize,
    /// IDs of source memories that were removed
    #[serde(default)]
    pub new_memories: Vec<String>,
    /// Step-by-step consolidation log (CE-6, optional).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub log: Vec<ConsolidationLogEntry>,
}

impl<'de> serde::Deserialize<'de> for ConsolidateResponse {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let val = serde_json::Value::deserialize(deserializer)?;
        // Server format: {"consolidated_memory":{...}, "source_memory_ids":[...], "memories_removed": N}
        let removed = val
            .get("memories_removed")
            .and_then(|v| v.as_u64())
            .or_else(|| val.get("removed_count").and_then(|v| v.as_u64()))
            .or_else(|| val.get("consolidated_count").and_then(|v| v.as_u64()))
            .unwrap_or(0) as usize;
        let source_ids: Vec<String> = val
            .get("source_memory_ids")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self {
            consolidated_count: removed,
            removed_count: removed,
            new_memories: source_ids,
            log: vec![],
        })
    }
}

// ============================================================================
// DX-1: Memory Import / Export
// ============================================================================

/// Response from `POST /v1/import` (DX-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryImportResponse {
    pub imported_count: usize,
    pub skipped_count: usize,
    #[serde(default)]
    pub errors: Vec<String>,
}

/// Response from `GET /v1/export` (DX-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryExportResponse {
    pub data: Vec<serde_json::Value>,
    pub format: String,
    pub count: usize,
}

// ============================================================================
// OBS-1: Business-Event Audit Log
// ============================================================================

/// A single business-event entry from the audit log (OBS-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub timestamp: u64,
    #[serde(default)]
    pub details: serde_json::Value,
}

/// Response from `GET /v1/audit` (OBS-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditListResponse {
    pub events: Vec<AuditEvent>,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Response from `POST /v1/audit/export` (OBS-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditExportResponse {
    pub data: String,
    pub format: String,
    pub count: usize,
}

/// Query parameters for the audit log (OBS-1).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

// ============================================================================
// EXT-1: External Extraction Providers
// ============================================================================

/// Result from `POST /v1/extract` (EXT-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub entities: Vec<serde_json::Value>,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub duration_ms: f64,
}

/// Metadata for an available extraction provider (EXT-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionProviderInfo {
    pub name: String,
    pub available: bool,
    #[serde(default)]
    pub models: Vec<String>,
}

/// Response from `GET /v1/extract/providers` (EXT-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExtractProvidersResponse {
    List(Vec<ExtractionProviderInfo>),
    Object {
        providers: Vec<ExtractionProviderInfo>,
    },
}

// ============================================================================
// SEC-3: AES-256-GCM Encryption Key Rotation
// ============================================================================

/// Request body for `POST /v1/admin/encryption/rotate-key` (SEC-3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotateEncryptionKeyRequest {
    /// New passphrase or 64-char hex key to rotate to.
    pub new_key: String,
    /// If set, rotate only memories in this namespace. Omit to rotate all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

/// Response from `POST /v1/admin/encryption/rotate-key` (SEC-3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotateEncryptionKeyResponse {
    pub rotated: usize,
    pub skipped: usize,
    #[serde(default)]
    pub namespaces: Vec<String>,
}

/// Request for memory feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackRequest {
    pub memory_id: String,
    pub feedback: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relevance_score: Option<f32>,
}

/// Response from legacy feedback endpoint (POST /v1/agents/:id/memories/feedback)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyFeedbackResponse {
    pub status: String,
    pub updated_importance: Option<f32>,
}

// ============================================================================
// CE-2: Batch Recall / Forget Types
// ============================================================================

/// Filter predicates for batch memory operations (CE-2).
///
/// All fields are optional.  For [`BatchForgetRequest`] at least one must be
/// set (server-side safety guard).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatchMemoryFilter {
    /// Restrict to memories that carry **all** listed tags.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Minimum importance (inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_importance: Option<f32>,
    /// Maximum importance (inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_importance: Option<f32>,
    /// Only memories created at or after this Unix timestamp (seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_after: Option<u64>,
    /// Only memories created before or at this Unix timestamp (seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_before: Option<u64>,
    /// Restrict to a specific memory type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<MemoryType>,
    /// Restrict to memories from a specific session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl BatchMemoryFilter {
    /// Convenience: filter by tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    /// Convenience: filter by minimum importance.
    pub fn with_min_importance(mut self, min: f32) -> Self {
        self.min_importance = Some(min);
        self
    }

    /// Convenience: filter by maximum importance.
    pub fn with_max_importance(mut self, max: f32) -> Self {
        self.max_importance = Some(max);
        self
    }

    /// Convenience: filter by session.
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}

/// Request body for `POST /v1/memories/recall/batch`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRecallRequest {
    /// Agent whose memory namespace to search.
    pub agent_id: String,
    /// Filter predicates to apply.
    #[serde(default)]
    pub filter: BatchMemoryFilter,
    /// Maximum number of results to return (default: 100).
    #[serde(default = "default_batch_limit")]
    pub limit: usize,
}

fn default_batch_limit() -> usize {
    100
}

impl BatchRecallRequest {
    /// Create a new batch recall request for an agent.
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            filter: BatchMemoryFilter::default(),
            limit: 100,
        }
    }

    /// Set filter predicates.
    pub fn with_filter(mut self, filter: BatchMemoryFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Set result limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

/// Response from `POST /v1/memories/recall/batch`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRecallResponse {
    pub memories: Vec<RecalledMemory>,
    /// Total memories in the agent namespace.
    pub total: usize,
    /// Number of memories that passed the filter.
    pub filtered: usize,
}

/// Request body for `DELETE /v1/memories/forget/batch`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchForgetRequest {
    /// Agent whose memory namespace to purge from.
    pub agent_id: String,
    /// Filter predicates — **at least one must be set** (server safety guard).
    pub filter: BatchMemoryFilter,
}

impl BatchForgetRequest {
    /// Create a new batch forget request with the given filter.
    pub fn new(agent_id: impl Into<String>, filter: BatchMemoryFilter) -> Self {
        Self {
            agent_id: agent_id.into(),
            filter,
        }
    }
}

/// Response from `DELETE /v1/memories/forget/batch`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchForgetResponse {
    pub deleted_count: usize,
}

// ============================================================================
// Memory Client Methods
// ============================================================================

impl DakeraClient {
    // ========================================================================
    // Memory Operations
    // ========================================================================

    /// Store a memory for an agent
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dakera_client::{DakeraClient, memory::StoreMemoryRequest};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = DakeraClient::new("http://localhost:3000")?;
    ///
    /// let request = StoreMemoryRequest::new("agent-1", "The user prefers dark mode")
    ///     .with_importance(0.8)
    ///     .with_tags(vec!["preferences".to_string()]);
    ///
    /// let response = client.store_memory(request).await?;
    /// println!("Stored memory: {}", response.memory_id);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn store_memory(&self, request: StoreMemoryRequest) -> Result<StoreMemoryResponse> {
        let url = format!("{}/v1/memory/store", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Recall memories by semantic query
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dakera_client::{DakeraClient, memory::RecallRequest};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = DakeraClient::new("http://localhost:3000")?;
    ///
    /// let request = RecallRequest::new("agent-1", "user preferences")
    ///     .with_top_k(10);
    ///
    /// let response = client.recall(request).await?;
    /// for memory in response.memories {
    ///     println!("{}: {} (score: {})", memory.id, memory.content, memory.score);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn recall(&self, request: RecallRequest) -> Result<RecallResponse> {
        let url = format!("{}/v1/memory/recall", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Simple recall with just agent_id and query (convenience method)
    pub async fn recall_simple(
        &self,
        agent_id: &str,
        query: &str,
        top_k: usize,
    ) -> Result<RecallResponse> {
        self.recall(RecallRequest::new(agent_id, query).with_top_k(top_k))
            .await
    }

    /// Get a specific memory by ID
    pub async fn get_memory(&self, memory_id: &str) -> Result<RecalledMemory> {
        let url = format!("{}/v1/memory/get/{}", self.base_url, memory_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Forget (delete) memories
    pub async fn forget(&self, request: ForgetRequest) -> Result<ForgetResponse> {
        let url = format!("{}/v1/memory/forget", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Search memories with advanced filters
    pub async fn search_memories(&self, request: RecallRequest) -> Result<RecallResponse> {
        let url = format!("{}/v1/memory/search", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Update an existing memory
    pub async fn update_memory(
        &self,
        agent_id: &str,
        memory_id: &str,
        request: UpdateMemoryRequest,
    ) -> Result<StoreMemoryResponse> {
        let url = format!(
            "{}/v1/agents/{}/memories/{}",
            self.base_url, agent_id, memory_id
        );
        let response = self.client.put(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Update importance of memories
    pub async fn update_importance(
        &self,
        agent_id: &str,
        request: UpdateImportanceRequest,
    ) -> Result<serde_json::Value> {
        let url = format!(
            "{}/v1/agents/{}/memories/importance",
            self.base_url, agent_id
        );
        let response = self.client.put(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Consolidate memories for an agent
    pub async fn consolidate(
        &self,
        agent_id: &str,
        request: ConsolidateRequest,
    ) -> Result<ConsolidateResponse> {
        // Server endpoint: POST /v1/memory/consolidate with agent_id in body
        let url = format!("{}/v1/memory/consolidate", self.base_url);
        let mut body = serde_json::to_value(&request)?;
        body["agent_id"] = serde_json::Value::String(agent_id.to_string());
        let response = self.client.post(&url).json(&body).send().await?;
        self.handle_response(response).await
    }

    /// Submit feedback on a memory recall
    pub async fn memory_feedback(
        &self,
        agent_id: &str,
        request: FeedbackRequest,
    ) -> Result<LegacyFeedbackResponse> {
        let url = format!("{}/v1/agents/{}/memories/feedback", self.base_url, agent_id);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // Memory Feedback Loop — INT-1
    // ========================================================================

    /// Submit upvote/downvote/flag feedback on a memory (INT-1).
    ///
    /// # Arguments
    /// * `memory_id` – The memory to give feedback on.
    /// * `agent_id` – The agent that owns the memory.
    /// * `signal` – [`FeedbackSignal`] value: `Upvote`, `Downvote`, or `Flag`.
    ///
    /// # Example
    /// ```no_run
    /// # use dakera_client::{DakeraClient, FeedbackSignal};
    /// # async fn example(client: &DakeraClient) -> dakera_client::Result<()> {
    /// let resp = client.feedback_memory("mem-abc", "agent-1", FeedbackSignal::Upvote).await?;
    /// println!("new importance: {}", resp.new_importance);
    /// # Ok(()) }
    /// ```
    pub async fn feedback_memory(
        &self,
        memory_id: &str,
        agent_id: &str,
        signal: FeedbackSignal,
    ) -> Result<FeedbackResponse> {
        let url = format!("{}/v1/memories/{}/feedback", self.base_url, memory_id);
        let body = MemoryFeedbackBody {
            agent_id: agent_id.to_string(),
            signal,
        };
        let response = self.client.post(&url).json(&body).send().await?;
        self.handle_response(response).await
    }

    /// Get the full feedback history for a memory (INT-1).
    pub async fn get_memory_feedback_history(
        &self,
        memory_id: &str,
    ) -> Result<FeedbackHistoryResponse> {
        let url = format!("{}/v1/memories/{}/feedback", self.base_url, memory_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get aggregate feedback counts and health score for an agent (INT-1).
    pub async fn get_agent_feedback_summary(&self, agent_id: &str) -> Result<AgentFeedbackSummary> {
        let url = format!("{}/v1/agents/{}/feedback/summary", self.base_url, agent_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Directly override a memory's importance score (INT-1).
    ///
    /// # Arguments
    /// * `memory_id` – The memory to update.
    /// * `agent_id` – The agent that owns the memory.
    /// * `importance` – New importance value (0.0–1.0).
    pub async fn patch_memory_importance(
        &self,
        memory_id: &str,
        agent_id: &str,
        importance: f32,
    ) -> Result<FeedbackResponse> {
        let url = format!("{}/v1/memories/{}/importance", self.base_url, memory_id);
        let body = MemoryImportancePatch {
            agent_id: agent_id.to_string(),
            importance,
        };
        let response = self.client.patch(&url).json(&body).send().await?;
        self.handle_response(response).await
    }

    /// Get overall feedback health score for an agent (INT-1).
    ///
    /// The health score is the mean importance of all non-expired memories (0.0–1.0).
    /// A higher score indicates a healthier, more relevant memory store.
    pub async fn get_feedback_health(&self, agent_id: &str) -> Result<FeedbackHealthResponse> {
        let url = format!("{}/v1/feedback/health?agent_id={}", self.base_url, agent_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // Memory Knowledge Graph Operations (CE-5 / SDK-9)
    // ========================================================================

    /// Traverse the knowledge graph from a memory node.
    ///
    /// Requires CE-5 (Memory Knowledge Graph) on the server.
    ///
    /// # Arguments
    /// * `memory_id` – Root memory ID to start traversal from.
    /// * `options` – Traversal options (depth, edge type filters).
    ///
    /// # Example
    /// ```no_run
    /// # use dakera_client::{DakeraClient, GraphOptions};
    /// # async fn example(client: &DakeraClient) -> dakera_client::Result<()> {
    /// let graph = client.memory_graph("mem-abc", GraphOptions::new().depth(2)).await?;
    /// println!("{} nodes, {} edges", graph.nodes.len(), graph.edges.len());
    /// # Ok(()) }
    /// ```
    pub async fn memory_graph(
        &self,
        memory_id: &str,
        options: GraphOptions,
    ) -> Result<MemoryGraph> {
        let mut url = format!("{}/v1/memories/{}/graph", self.base_url, memory_id);
        let depth = options.depth.unwrap_or(1);
        url.push_str(&format!("?depth={}", depth));
        if let Some(types) = &options.types {
            let type_strs: Vec<String> = types
                .iter()
                .map(|t| {
                    serde_json::to_value(t)
                        .unwrap()
                        .as_str()
                        .unwrap_or("")
                        .to_string()
                })
                .collect();
            if !type_strs.is_empty() {
                url.push_str(&format!("&types={}", type_strs.join(",")));
            }
        }
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Find the shortest path between two memories in the knowledge graph.
    ///
    /// Requires CE-5 (Memory Knowledge Graph) on the server.
    ///
    /// # Example
    /// ```no_run
    /// # use dakera_client::DakeraClient;
    /// # async fn example(client: &DakeraClient) -> dakera_client::Result<()> {
    /// let path = client.memory_path("mem-abc", "mem-xyz").await?;
    /// println!("{} hops: {:?}", path.hops, path.path);
    /// # Ok(()) }
    /// ```
    pub async fn memory_path(&self, source_id: &str, target_id: &str) -> Result<GraphPath> {
        let url = format!(
            "{}/v1/memories/{}/path?target={}",
            self.base_url,
            source_id,
            urlencoding::encode(target_id)
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Create an explicit edge between two memories.
    ///
    /// Requires CE-5 (Memory Knowledge Graph) on the server.
    ///
    /// # Example
    /// ```no_run
    /// # use dakera_client::{DakeraClient, EdgeType};
    /// # async fn example(client: &DakeraClient) -> dakera_client::Result<()> {
    /// let resp = client.memory_link("mem-abc", "mem-xyz", EdgeType::LinkedBy).await?;
    /// println!("Created edge: {}", resp.edge.id);
    /// # Ok(()) }
    /// ```
    pub async fn memory_link(
        &self,
        source_id: &str,
        target_id: &str,
        edge_type: EdgeType,
    ) -> Result<GraphLinkResponse> {
        let url = format!("{}/v1/memories/{}/links", self.base_url, source_id);
        let request = GraphLinkRequest {
            target_id: target_id.to_string(),
            edge_type,
        };
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Export the full knowledge graph for an agent.
    ///
    /// Requires CE-5 (Memory Knowledge Graph) on the server.
    ///
    /// # Arguments
    /// * `agent_id` – Agent whose graph to export.
    /// * `format` – Export format: `"json"` (default), `"graphml"`, or `"csv"`.
    pub async fn agent_graph_export(&self, agent_id: &str, format: &str) -> Result<GraphExport> {
        let url = format!(
            "{}/v1/agents/{}/graph/export?format={}",
            self.base_url, agent_id, format
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // Session Operations
    // ========================================================================

    /// Start a new session for an agent
    pub async fn start_session(&self, agent_id: &str) -> Result<Session> {
        let url = format!("{}/v1/sessions/start", self.base_url);
        let request = SessionStartRequest {
            agent_id: agent_id.to_string(),
            metadata: None,
        };
        let response = self.client.post(&url).json(&request).send().await?;
        let resp: SessionStartResponse = self.handle_response(response).await?;
        Ok(resp.session)
    }

    /// Start a session with metadata
    pub async fn start_session_with_metadata(
        &self,
        agent_id: &str,
        metadata: serde_json::Value,
    ) -> Result<Session> {
        let url = format!("{}/v1/sessions/start", self.base_url);
        let request = SessionStartRequest {
            agent_id: agent_id.to_string(),
            metadata: Some(metadata),
        };
        let response = self.client.post(&url).json(&request).send().await?;
        let resp: SessionStartResponse = self.handle_response(response).await?;
        Ok(resp.session)
    }

    /// End a session, optionally with a summary.
    /// Returns the session state and the total memory count at close.
    pub async fn end_session(
        &self,
        session_id: &str,
        summary: Option<String>,
    ) -> Result<SessionEndResponse> {
        let url = format!("{}/v1/sessions/{}/end", self.base_url, session_id);
        let request = SessionEndRequest { summary };
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Get a session by ID
    pub async fn get_session(&self, session_id: &str) -> Result<Session> {
        let url = format!("{}/v1/sessions/{}", self.base_url, session_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// List sessions for an agent
    pub async fn list_sessions(&self, agent_id: &str) -> Result<Vec<Session>> {
        let url = format!("{}/v1/sessions?agent_id={}", self.base_url, agent_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get memories in a session
    pub async fn session_memories(&self, session_id: &str) -> Result<RecallResponse> {
        let url = format!("{}/v1/sessions/{}/memories", self.base_url, session_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // CE-2: Batch Recall / Forget
    // ========================================================================

    /// Bulk-recall memories using filter predicates (CE-2).
    ///
    /// Uses `POST /v1/memories/recall/batch` — no embedding required.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dakera_client::{DakeraClient, memory::{BatchRecallRequest, BatchMemoryFilter}};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = DakeraClient::new("http://localhost:3000")?;
    ///
    /// let filter = BatchMemoryFilter::default().with_min_importance(0.7);
    /// let req = BatchRecallRequest::new("agent-1").with_filter(filter).with_limit(50);
    /// let resp = client.batch_recall(req).await?;
    /// println!("Found {} memories", resp.filtered);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn batch_recall(&self, request: BatchRecallRequest) -> Result<BatchRecallResponse> {
        let url = format!("{}/v1/memories/recall/batch", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Bulk-delete memories using filter predicates (CE-2).
    ///
    /// Uses `DELETE /v1/memories/forget/batch`.  The server requires at least
    /// one filter predicate to be set as a safety guard.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dakera_client::{DakeraClient, memory::{BatchForgetRequest, BatchMemoryFilter}};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = DakeraClient::new("http://localhost:3000")?;
    ///
    /// let filter = BatchMemoryFilter::default().with_min_importance(0.0).with_max_importance(0.2);
    /// let resp = client.batch_forget(BatchForgetRequest::new("agent-1", filter)).await?;
    /// println!("Deleted {} memories", resp.deleted_count);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn batch_forget(&self, request: BatchForgetRequest) -> Result<BatchForgetResponse> {
        let url = format!("{}/v1/memories/forget/batch", self.base_url);
        let response = self.client.delete(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // DX-1: Memory Import / Export
    // ========================================================================

    /// Import memories from an external format (DX-1).
    ///
    /// Supported formats: `"jsonl"`, `"mem0"`, `"zep"`, `"csv"`.
    ///
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = dakera_client::DakeraClient::new("http://localhost:3000")?;
    /// let data = serde_json::json!([{"content": "hello", "agent_id": "agent-1"}]);
    /// let resp = client.import_memories(data, "jsonl", None, None).await?;
    /// println!("Imported {} memories", resp.imported_count);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn import_memories(
        &self,
        data: serde_json::Value,
        format: &str,
        agent_id: Option<&str>,
        namespace: Option<&str>,
    ) -> Result<MemoryImportResponse> {
        let mut body = serde_json::json!({"data": data, "format": format});
        if let Some(aid) = agent_id {
            body["agent_id"] = serde_json::Value::String(aid.to_string());
        }
        if let Some(ns) = namespace {
            body["namespace"] = serde_json::Value::String(ns.to_string());
        }
        let url = format!("{}/v1/import", self.base_url);
        let response = self.client.post(&url).json(&body).send().await?;
        self.handle_response(response).await
    }

    /// Export memories in a portable format (DX-1).
    ///
    /// Supported formats: `"jsonl"`, `"mem0"`, `"zep"`, `"csv"`.
    pub async fn export_memories(
        &self,
        format: &str,
        agent_id: Option<&str>,
        namespace: Option<&str>,
        limit: Option<u32>,
    ) -> Result<MemoryExportResponse> {
        let mut params = vec![("format", format.to_string())];
        if let Some(aid) = agent_id {
            params.push(("agent_id", aid.to_string()));
        }
        if let Some(ns) = namespace {
            params.push(("namespace", ns.to_string()));
        }
        if let Some(l) = limit {
            params.push(("limit", l.to_string()));
        }
        let url = format!("{}/v1/export", self.base_url);
        let response = self.client.get(&url).query(&params).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // OBS-1: Business-Event Audit Log
    // ========================================================================

    /// List paginated audit log entries (OBS-1).
    pub async fn list_audit_events(&self, query: AuditQuery) -> Result<AuditListResponse> {
        let url = format!("{}/v1/audit", self.base_url);
        let response = self.client.get(&url).query(&query).send().await?;
        self.handle_response(response).await
    }

    /// Stream live audit events via SSE (OBS-1).
    ///
    /// Returns a [`tokio::sync::mpsc::Receiver`] that yields [`DakeraEvent`] results.
    pub async fn stream_audit_events(
        &self,
        agent_id: Option<&str>,
        event_type: Option<&str>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<crate::events::DakeraEvent>>> {
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(aid) = agent_id {
            params.push(("agent_id", aid.to_string()));
        }
        if let Some(et) = event_type {
            params.push(("event_type", et.to_string()));
        }
        let base = format!("{}/v1/audit/stream", self.base_url);
        let url = if params.is_empty() {
            base
        } else {
            let qs = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
                .collect::<Vec<_>>()
                .join("&");
            format!("{}?{}", base, qs)
        };
        self.stream_sse(url).await
    }

    /// Bulk-export audit log entries (OBS-1).
    pub async fn export_audit(
        &self,
        format: &str,
        agent_id: Option<&str>,
        event_type: Option<&str>,
        from_ts: Option<u64>,
        to_ts: Option<u64>,
    ) -> Result<AuditExportResponse> {
        let mut body = serde_json::json!({"format": format});
        if let Some(aid) = agent_id {
            body["agent_id"] = serde_json::Value::String(aid.to_string());
        }
        if let Some(et) = event_type {
            body["event_type"] = serde_json::Value::String(et.to_string());
        }
        if let Some(f) = from_ts {
            body["from"] = serde_json::Value::Number(f.into());
        }
        if let Some(t) = to_ts {
            body["to"] = serde_json::Value::Number(t.into());
        }
        let url = format!("{}/v1/audit/export", self.base_url);
        let response = self.client.post(&url).json(&body).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // EXT-1: External Extraction Providers
    // ========================================================================

    /// Extract entities from text using a pluggable provider (EXT-1).
    ///
    /// Provider hierarchy: per-request > namespace default > GLiNER (bundled).
    /// Supported providers: `"gliner"`, `"openai"`, `"anthropic"`, `"openrouter"`, `"ollama"`.
    pub async fn extract_text(
        &self,
        text: &str,
        namespace: Option<&str>,
        provider: Option<&str>,
        model: Option<&str>,
    ) -> Result<ExtractionResult> {
        let mut body = serde_json::json!({"text": text});
        if let Some(ns) = namespace {
            body["namespace"] = serde_json::Value::String(ns.to_string());
        }
        if let Some(p) = provider {
            body["provider"] = serde_json::Value::String(p.to_string());
        }
        if let Some(m) = model {
            body["model"] = serde_json::Value::String(m.to_string());
        }
        let url = format!("{}/v1/extract", self.base_url);
        let response = self.client.post(&url).json(&body).send().await?;
        self.handle_response(response).await
    }

    /// List available extraction providers and their models (EXT-1).
    pub async fn list_extract_providers(&self) -> Result<Vec<ExtractionProviderInfo>> {
        let url = format!("{}/v1/extract/providers", self.base_url);
        let response = self.client.get(&url).send().await?;
        let result: ExtractProvidersResponse = self.handle_response(response).await?;
        Ok(match result {
            ExtractProvidersResponse::List(v) => v,
            ExtractProvidersResponse::Object { providers } => providers,
        })
    }

    /// Set the default extraction provider for a namespace (EXT-1).
    pub async fn configure_namespace_extractor(
        &self,
        namespace: &str,
        provider: &str,
        model: Option<&str>,
    ) -> Result<serde_json::Value> {
        let mut body = serde_json::json!({"provider": provider});
        if let Some(m) = model {
            body["model"] = serde_json::Value::String(m.to_string());
        }
        let url = format!(
            "{}/v1/namespaces/{}/extractor",
            self.base_url,
            urlencoding::encode(namespace)
        );
        let response = self.client.patch(&url).json(&body).send().await?;
        self.handle_response(response).await
    }

    // =========================================================================
    // SEC-3: AES-256-GCM Encryption Key Rotation
    // =========================================================================

    /// Re-encrypt all memory content blobs with a new AES-256-GCM key (SEC-3).
    ///
    /// After this call the new key is active in the running process.
    /// The operator must update `DAKERA_ENCRYPTION_KEY` and restart to make
    /// the rotation durable across restarts.
    ///
    /// Requires Admin scope.
    ///
    /// # Arguments
    /// * `new_key` - New passphrase or 64-char hex key.
    /// * `namespace` - If `Some`, rotate only this namespace. `None` rotates all.
    pub async fn rotate_encryption_key(
        &self,
        new_key: &str,
        namespace: Option<&str>,
    ) -> Result<RotateEncryptionKeyResponse> {
        let body = RotateEncryptionKeyRequest {
            new_key: new_key.to_string(),
            namespace: namespace.map(|s| s.to_string()),
        };
        let url = format!("{}/v1/admin/encryption/rotate-key", self.base_url);
        let response = self.client.post(&url).json(&body).send().await?;
        self.handle_response(response).await
    }
}
