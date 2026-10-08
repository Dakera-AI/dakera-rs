//! Knowledge graph operations for the Dakera client.

use serde::{Deserialize, Serialize};

use crate::error::{ClientError, Result};
use crate::types::{KgExportResponse, KgPathResponse, KgQueryResponse};
use crate::DakeraClient;

// ============================================================================
// Knowledge Graph Types
// ============================================================================

/// Request to build a knowledge graph
///
/// `memory_id` (the seed memory) is required by the server;
/// [`DakeraClient::knowledge_graph`] refuses a request without it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraphRequest {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_similarity: Option<f32>,
}

/// A node in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeNode {
    pub id: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub importance: Option<f32>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    /// Memory tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Cluster the node belongs to (full graph only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cluster_id: Option<usize>,
    /// Degree centrality (full graph only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub centrality: Option<f32>,
    /// Creation time as the server sends it (full graph: Unix seconds as a string).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Full length of the content in characters (full graph, server v0.12.2+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_len: Option<usize>,
    /// Whether `content` was cut to `content_preview_chars` (full graph,
    /// server v0.12.2+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_truncated: Option<bool>,
}

/// An edge in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEdge {
    pub source: String,
    pub target: String,
    pub similarity: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship: Option<String>,
    /// Tags both memories carry.
    #[serde(default)]
    pub shared_tags: Vec<String>,
}

/// A memory as the knowledge routes return it (`root.memory`,
/// `summary_memory`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeMemory {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub memory_type: String,
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub importance: f32,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub last_accessed_at: u64,
    #[serde(default)]
    pub access_count: u64,
}

/// A memory related to the seed in `POST /v1/knowledge/graph`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeRelated {
    pub memory_id: String,
    pub similarity: f32,
    #[serde(default)]
    pub shared_tags: Vec<String>,
}

/// The seed of `POST /v1/knowledge/graph`: the memory and its related ones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraphRoot {
    pub memory: KnowledgeMemory,
    pub similarity: f32,
    #[serde(default)]
    pub related: Vec<KnowledgeRelated>,
}

/// A cluster of the full knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeCluster {
    pub id: usize,
    pub node_count: usize,
    #[serde(default)]
    pub top_tags: Vec<String>,
    #[serde(default)]
    pub avg_importance: f32,
}

/// Statistics of the full knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraphStats {
    pub total_memories: usize,
    pub included_memories: usize,
    pub total_edges: usize,
    pub cluster_count: usize,
    pub density: f32,
    #[serde(default)]
    pub hub_memory_id: Option<String>,
}

/// Response from knowledge graph operations.
///
/// The two routes answer differently and both are read into this type:
///
/// - `POST /v1/knowledge/graph` answers `{root: {memory, similarity, related},
///   total_nodes}`: `root` is set, `nodes` holds the seed memory, `edges` one
///   edge per related memory (the server sends only ids for those), and
///   `total_nodes` the count.
/// - `POST /v1/knowledge/graph/full` answers `{nodes, edges, clusters, stats}`:
///   `cluster_info` and `stats` hold the server's clusters and statistics, and
///   `clusters` lists each cluster's node ids.
#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeGraphResponse {
    pub nodes: Vec<KnowledgeNode>,
    pub edges: Vec<KnowledgeEdge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clusters: Option<Vec<Vec<String>>>,
    /// Seed and related memories (`/v1/knowledge/graph`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<KnowledgeGraphRoot>,
    /// Number of nodes in the graph.
    pub total_nodes: usize,
    /// The server's clusters (`/v1/knowledge/graph/full`).
    #[serde(default)]
    pub cluster_info: Vec<KnowledgeCluster>,
    /// Graph statistics (`/v1/knowledge/graph/full`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<KnowledgeGraphStats>,
}

impl<'de> Deserialize<'de> for KnowledgeGraphResponse {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        use serde::de::Error;

        let val = serde_json::Value::deserialize(deserializer)?;
        let field = |name: &str| val.get(name).cloned().unwrap_or(serde_json::Value::Null);

        // `/v1/knowledge/graph`: {root, total_nodes}
        if val.get("root").is_some() {
            let root: KnowledgeGraphRoot =
                serde_json::from_value(field("root")).map_err(D::Error::custom)?;
            let m = &root.memory;
            let nodes = vec![KnowledgeNode {
                id: m.id.clone(),
                content: m.content.clone(),
                memory_type: Some(m.memory_type.clone()),
                importance: Some(m.importance),
                metadata: m.metadata.clone().unwrap_or(serde_json::Value::Null),
                tags: m.tags.clone(),
                cluster_id: None,
                centrality: None,
                created_at: Some(m.created_at.to_string()),
                content_len: None,
                content_truncated: None,
            }];
            let edges = root
                .related
                .iter()
                .map(|r| KnowledgeEdge {
                    source: m.id.clone(),
                    target: r.memory_id.clone(),
                    similarity: r.similarity,
                    relationship: None,
                    shared_tags: r.shared_tags.clone(),
                })
                .collect();
            let total_nodes = val
                .get("total_nodes")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize)
                .unwrap_or(1 + root.related.len());
            return Ok(Self {
                nodes,
                edges,
                clusters: None,
                root: Some(root),
                total_nodes,
                cluster_info: Vec::new(),
                stats: None,
            });
        }

        // `/v1/knowledge/graph/full` (or the legacy {nodes, edges, clusters}).
        let nodes: Vec<KnowledgeNode> = if val.get("nodes").is_some() {
            serde_json::from_value(field("nodes")).map_err(D::Error::custom)?
        } else {
            return Err(D::Error::missing_field("nodes"));
        };
        let edges: Vec<KnowledgeEdge> = match val.get("edges") {
            Some(_) => serde_json::from_value(field("edges")).map_err(D::Error::custom)?,
            None => Vec::new(),
        };
        let stats: Option<KnowledgeGraphStats> = match val.get("stats") {
            Some(v) if !v.is_null() => {
                Some(serde_json::from_value(v.clone()).map_err(D::Error::custom)?)
            }
            _ => None,
        };
        let mut cluster_info = Vec::new();
        let clusters = match val.get("clusters") {
            Some(serde_json::Value::Array(items))
                if items.first().map(|c| c.is_object()).unwrap_or(false) =>
            {
                cluster_info = serde_json::from_value::<Vec<KnowledgeCluster>>(field("clusters"))
                    .map_err(D::Error::custom)?;
                Some(
                    cluster_info
                        .iter()
                        .map(|c| {
                            nodes
                                .iter()
                                .filter(|n| n.cluster_id == Some(c.id))
                                .map(|n| n.id.clone())
                                .collect()
                        })
                        .collect(),
                )
            }
            Some(serde_json::Value::Array(items)) if items.is_empty() => Some(Vec::new()),
            Some(serde_json::Value::Null) | None => None,
            Some(_) => Some(serde_json::from_value(field("clusters")).map_err(D::Error::custom)?),
        };
        let total_nodes = nodes.len();
        Ok(Self {
            nodes,
            edges,
            clusters,
            root: None,
            total_nodes,
            cluster_info,
            stats,
        })
    }
}

/// Request to build a full knowledge graph
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FullKnowledgeGraphRequest {
    /// Agent whose memories form the graph.
    pub agent_id: String,
    /// Most nodes (1..=500).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_nodes: Option<u32>,
    /// Smallest similarity of an edge (0..=1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_similarity: Option<f32>,
    /// Similarity that puts two nodes in one cluster (0..=1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_threshold: Option<f32>,
    /// Most edges per node (1..=50).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_edges_per_node: Option<u32>,
    /// Cut each node's `content` to this many characters (1..=10000; server
    /// v0.12.2+, which then fills `content_len` / `content_truncated`).
    /// `None`: full content. An older server ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_preview_chars: Option<u32>,
}

/// Request to summarize memories
///
/// The server needs at least two `memory_ids` and always stores the summary
/// as a new memory; it has no dry run. [`DakeraClient::summarize`] refuses
/// fewer than two ids or `dry_run: true` before sending anything.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizeRequest {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_type: Option<String>,
    /// Not supported by the server (it would write the summary anyway):
    /// `true` makes [`DakeraClient::summarize`] return an error. Never sent.
    #[serde(default, skip_serializing)]
    pub dry_run: bool,
}

/// Response from summarization
///
/// The server answers `{summary_memory, source_count}`; `summary` and
/// `new_memory_id` are read from `summary_memory.content` / `.id`.
#[derive(Debug, Clone, Serialize)]
pub struct SummarizeResponse {
    pub summary: String,
    pub source_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_memory_id: Option<String>,
    /// The stored summary memory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary_memory: Option<KnowledgeMemory>,
}

impl<'de> Deserialize<'de> for SummarizeResponse {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        use serde::de::Error;

        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            summary_memory: Option<KnowledgeMemory>,
            #[serde(default)]
            summary: Option<String>,
            source_count: usize,
            #[serde(default)]
            new_memory_id: Option<String>,
        }

        let raw = Raw::deserialize(deserializer)?;
        let summary = match (&raw.summary, &raw.summary_memory) {
            (Some(s), _) => s.clone(),
            (None, Some(m)) => m.content.clone(),
            (None, None) => return Err(D::Error::missing_field("summary_memory")),
        };
        let new_memory_id = raw
            .new_memory_id
            .or_else(|| raw.summary_memory.as_ref().map(|m| m.id.clone()));
        Ok(Self {
            summary,
            source_count: raw.source_count,
            new_memory_id,
            summary_memory: raw.summary_memory,
        })
    }
}

/// Request to deduplicate memories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeduplicateRequest {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
}

/// A group of near-duplicate memories found by deduplication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    /// The memory that is kept.
    pub canonical_id: String,
    /// The memories merged into it (or that would be, on a dry run).
    pub duplicate_ids: Vec<String>,
    #[serde(default)]
    pub avg_similarity: f32,
}

/// Response from deduplication
///
/// The server answers `{groups: [{canonical_id, duplicate_ids,
/// avg_similarity}], duplicates_found, duplicates_merged}`: `removed_count`
/// is `duplicates_merged` (0 on a dry run), `groups` lists each group's ids
/// (canonical first) and `duplicate_groups` keeps the server's groups.
#[derive(Debug, Clone, Serialize)]
pub struct DeduplicateResponse {
    pub duplicates_found: usize,
    pub removed_count: usize,
    pub groups: Vec<Vec<String>>,
    /// The server's duplicate groups.
    pub duplicate_groups: Vec<DuplicateGroup>,
    /// Duplicates not merged because a record changed (edited, expired,
    /// forgotten) between the scan and the write (server v0.12.2+; `0` on a
    /// dry run and from older servers).
    pub duplicates_skipped_changed: usize,
}

impl<'de> Deserialize<'de> for DeduplicateResponse {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        use serde::de::Error;

        let val = serde_json::Value::deserialize(deserializer)?;
        let count = |names: &[&str]| -> Option<usize> {
            names
                .iter()
                .find_map(|n| val.get(*n).and_then(|v| v.as_u64()))
                .map(|n| n as usize)
        };
        let duplicates_found = count(&["duplicates_found"])
            .ok_or_else(|| D::Error::missing_field("duplicates_found"))?;
        let removed_count = count(&["duplicates_merged", "removed_count"]).unwrap_or(0);
        let raw_groups = val
            .get("groups")
            .cloned()
            .unwrap_or(serde_json::Value::Array(vec![]));
        let (groups, duplicate_groups) = match &raw_groups {
            serde_json::Value::Array(items) if items.iter().all(|g| g.is_object()) => {
                let dg: Vec<DuplicateGroup> =
                    serde_json::from_value(raw_groups.clone()).map_err(D::Error::custom)?;
                let ids = dg
                    .iter()
                    .map(|g| {
                        std::iter::once(g.canonical_id.clone())
                            .chain(g.duplicate_ids.iter().cloned())
                            .collect()
                    })
                    .collect();
                (ids, dg)
            }
            _ => (
                serde_json::from_value(raw_groups).map_err(D::Error::custom)?,
                Vec::new(),
            ),
        };
        let duplicates_skipped_changed = count(&["duplicates_skipped_changed"]).unwrap_or(0);
        Ok(Self {
            duplicates_found,
            removed_count,
            groups,
            duplicate_groups,
            duplicates_skipped_changed,
        })
    }
}

// ============================================================================
// Cross-Agent Network Types (DASH-A)
// ============================================================================

/// Request to build a cross-agent knowledge network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossAgentNetworkRequest {
    /// Agent IDs to include; `None` means all agents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_ids: Option<Vec<String>>,
    /// Minimum cosine similarity for cross-agent edges (default 0.3).
    pub min_similarity: f32,
    /// Maximum memory nodes returned per agent (default 50).
    pub max_nodes_per_agent: usize,
    /// Minimum importance score for included nodes (default 0.0).
    pub min_importance: f32,
    /// Maximum cross-agent edges in the response (default 200).
    pub max_cross_edges: usize,
    /// Cut each node's `content` to this many characters (1..=10000; server
    /// v0.12.2+). `None`: full content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_preview_chars: Option<u32>,
}

impl Default for CrossAgentNetworkRequest {
    fn default() -> Self {
        Self {
            agent_ids: None,
            min_similarity: 0.3,
            max_nodes_per_agent: 50,
            min_importance: 0.0,
            max_cross_edges: 200,
            content_preview_chars: None,
        }
    }
}

/// Summary information about a single agent in the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNetworkInfo {
    pub agent_id: String,
    pub memory_count: usize,
    pub avg_importance: f32,
}

/// A memory node in the cross-agent network graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNetworkNode {
    /// Memory id.
    pub id: String,
    /// The agent the memory belongs to.
    pub agent_id: String,
    /// Content (cut to `content_preview_chars` when requested).
    pub content: String,
    /// Importance (0..=1).
    pub importance: f32,
    /// Tags.
    pub tags: Vec<String>,
    /// Memory type.
    pub memory_type: String,
    /// Unix milliseconds.
    pub created_at: u64,
    /// Full length of the content in characters (server v0.12.2+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_len: Option<usize>,
    /// Whether `content` was cut to `content_preview_chars` (server v0.12.2+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_truncated: Option<bool>,
}

/// A cross-agent similarity edge between two memory nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNetworkEdge {
    pub source: String,
    pub target: String,
    pub source_agent: String,
    pub target_agent: String,
    pub similarity: f32,
}

/// Aggregate statistics for the cross-agent network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNetworkStats {
    pub total_agents: usize,
    pub total_nodes: usize,
    pub total_cross_edges: usize,
    pub density: f32,
}

/// Response from the cross-agent network endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossAgentNetworkResponse {
    pub agents: Vec<AgentNetworkInfo>,
    pub nodes: Vec<AgentNetworkNode>,
    pub edges: Vec<AgentNetworkEdge>,
    pub stats: AgentNetworkStats,
    /// Total number of memory nodes in the network (added in server v0.6.2).
    #[serde(default)]
    pub node_count: usize,
}

// ============================================================================
// Knowledge Graph Client Methods
// ============================================================================

impl DakeraClient {
    /// Build a knowledge graph from a seed memory (`request.memory_id`,
    /// required by the server: a request without it is refused with
    /// [`ClientError::InvalidRequest`] before it is sent).
    pub async fn knowledge_graph(
        &self,
        request: KnowledgeGraphRequest,
    ) -> Result<KnowledgeGraphResponse> {
        if request.memory_id.as_deref().is_none_or(str::is_empty) {
            return Err(ClientError::InvalidRequest(
                "knowledge_graph needs memory_id (the seed memory); the server requires it"
                    .to_string(),
            ));
        }
        let url = format!("{}/v1/knowledge/graph", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Build a full knowledge graph for an agent
    ///
    /// An out-of-range `content_preview_chars` is refused with
    /// [`ClientError::InvalidRequest`] before anything is sent.
    pub async fn full_knowledge_graph(
        &self,
        request: FullKnowledgeGraphRequest,
    ) -> Result<KnowledgeGraphResponse> {
        crate::agents::check_content_preview(request.content_preview_chars)?;
        let url = format!("{}/v1/knowledge/graph/full", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Summarize memories into a new summary memory.
    ///
    /// The server needs at least two `memory_ids` and always stores the
    /// summary; it has no dry run. Fewer than two ids or `dry_run: true` is
    /// refused with [`ClientError::InvalidRequest`] before anything is sent.
    pub async fn summarize(&self, request: SummarizeRequest) -> Result<SummarizeResponse> {
        if request.dry_run {
            return Err(ClientError::InvalidRequest(
                "summarize has no dry run: the server always stores the summary memory".to_string(),
            ));
        }
        let ids = request.memory_ids.as_ref().map_or(0, Vec::len);
        if ids < 2 {
            return Err(ClientError::InvalidRequest(format!(
                "summarize needs at least 2 memory_ids (got {ids})"
            )));
        }
        let url = format!("{}/v1/knowledge/summarize", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Deduplicate memories
    pub async fn deduplicate(&self, request: DeduplicateRequest) -> Result<DeduplicateResponse> {
        let url = format!("{}/v1/knowledge/deduplicate", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Build a cross-agent knowledge network (DASH-A).
    ///
    /// Calls `POST /v1/knowledge/network/cross-agent` (Admin scope) and returns
    /// a graph of memory nodes and cross-agent similarity edges.
    pub async fn cross_agent_network(
        &self,
        request: CrossAgentNetworkRequest,
    ) -> Result<CrossAgentNetworkResponse> {
        crate::agents::check_content_preview(request.content_preview_chars)?;
        let url = format!("{}/v1/knowledge/network/cross-agent", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // =========================================================================
    // KG-2: Graph Query & Export
    // =========================================================================

    /// Query the memory knowledge graph using a filter DSL (KG-2).
    ///
    /// Calls `GET /v1/knowledge/query`.
    ///
    /// # Arguments
    /// - `agent_id` — agent whose graph to query.
    /// - `root_id` — optional root memory ID for BFS traversal.
    /// - `edge_type` — comma-separated edge types to filter (e.g. `"related_to,shares_entity"`).
    /// - `min_weight` — minimum edge weight (0.0–1.0).
    /// - `max_depth` — BFS depth when `root_id` is set (1–5, default 3).
    /// - `limit` — maximum edges to return (default 100, max 1000).
    pub async fn knowledge_query(
        &self,
        agent_id: &str,
        root_id: Option<&str>,
        edge_type: Option<&str>,
        min_weight: Option<f32>,
        max_depth: Option<u32>,
        limit: Option<usize>,
    ) -> Result<KgQueryResponse> {
        let mut url = format!("{}/v1/knowledge/query?agent_id={}", self.base_url, agent_id);
        if let Some(v) = root_id {
            url.push_str(&format!("&root_id={}", v));
        }
        if let Some(v) = edge_type {
            url.push_str(&format!("&edge_type={}", v));
        }
        if let Some(v) = min_weight {
            url.push_str(&format!("&min_weight={}", v));
        }
        if let Some(v) = max_depth {
            url.push_str(&format!("&max_depth={}", v));
        }
        if let Some(v) = limit {
            url.push_str(&format!("&limit={}", v));
        }
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Find the BFS shortest path between two memory IDs (KG-2).
    ///
    /// Calls `GET /v1/knowledge/path`.
    ///
    /// Returns an error if no path exists between the two memories.
    pub async fn knowledge_path(
        &self,
        agent_id: &str,
        from_id: &str,
        to_id: &str,
    ) -> Result<KgPathResponse> {
        let url = format!(
            "{}/v1/knowledge/path?agent_id={}&from={}&to={}",
            self.base_url, agent_id, from_id, to_id
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Export the memory knowledge graph as JSON or GraphML (KG-2).
    ///
    /// Calls `GET /v1/knowledge/export`.
    ///
    /// For `format = "graphml"` the server returns `application/xml`. This
    /// method deserializes JSON only — use a raw HTTP client for GraphML.
    pub async fn knowledge_export(
        &self,
        agent_id: &str,
        format: Option<&str>,
    ) -> Result<KgExportResponse> {
        let fmt = format.unwrap_or("json");
        let url = format!(
            "{}/v1/knowledge/export?agent_id={}&format={}",
            self.base_url, agent_id, fmt
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // KnowledgeGraphRequest serialization
    // -------------------------------------------------------------------------

    #[test]
    fn test_knowledge_graph_request_minimal_omits_optional() {
        let req = KnowledgeGraphRequest {
            agent_id: "agent-1".to_string(),
            memory_id: None,
            depth: None,
            min_similarity: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"agent_id\":\"agent-1\""));
        assert!(!json.contains("memory_id"));
        assert!(!json.contains("depth"));
        assert!(!json.contains("min_similarity"));
    }

    #[test]
    fn test_knowledge_graph_request_with_all_fields() {
        let req = KnowledgeGraphRequest {
            agent_id: "agent-1".to_string(),
            memory_id: Some("mem-abc".to_string()),
            depth: Some(3),
            min_similarity: Some(0.7),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"memory_id\":\"mem-abc\""));
        assert!(json.contains("\"depth\":3"));
        assert!(json.contains("\"min_similarity\":0.7"));
    }

    // -------------------------------------------------------------------------
    // KnowledgeNode deserialization
    // -------------------------------------------------------------------------

    #[test]
    fn test_knowledge_node_deserializes_minimal() {
        let json = r#"{
            "id": "n1",
            "content": "user likes coffee"
        }"#;
        let node: KnowledgeNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.id, "n1");
        assert!(node.memory_type.is_none());
        assert!(node.importance.is_none());
        assert_eq!(node.metadata, serde_json::Value::Null);
    }

    #[test]
    fn test_knowledge_node_deserializes_with_optional_fields() {
        let json = r#"{
            "id": "n2",
            "content": "works at Dakera",
            "memory_type": "semantic",
            "importance": 0.9
        }"#;
        let node: KnowledgeNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.memory_type.as_deref(), Some("semantic"));
        assert!((node.importance.unwrap() - 0.9).abs() < 1e-6);
    }

    // -------------------------------------------------------------------------
    // KnowledgeEdge serialization
    // -------------------------------------------------------------------------

    #[test]
    fn test_knowledge_edge_without_relationship_omits_field() {
        let edge = KnowledgeEdge {
            source: "n1".to_string(),
            target: "n2".to_string(),
            similarity: 0.85,
            relationship: None,
            shared_tags: Vec::new(),
        };
        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("\"similarity\":0.85"));
        assert!(!json.contains("relationship"));
    }

    #[test]
    fn test_knowledge_edge_with_relationship() {
        let edge = KnowledgeEdge {
            source: "n1".to_string(),
            target: "n2".to_string(),
            similarity: 0.92,
            relationship: Some("colleague".to_string()),
            shared_tags: Vec::new(),
        };
        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("\"relationship\":\"colleague\""));
    }

    // -------------------------------------------------------------------------
    // KnowledgeGraphResponse
    // -------------------------------------------------------------------------

    #[test]
    fn test_knowledge_graph_response_deserializes_empty() {
        let json = r#"{"nodes": [], "edges": []}"#;
        let resp: KnowledgeGraphResponse = serde_json::from_str(json).unwrap();
        assert!(resp.nodes.is_empty());
        assert!(resp.edges.is_empty());
        assert!(resp.clusters.is_none());
    }

    // -------------------------------------------------------------------------
    // FullKnowledgeGraphRequest serialization
    // -------------------------------------------------------------------------

    #[test]
    fn test_full_knowledge_graph_request_all_optional_omitted() {
        let req = FullKnowledgeGraphRequest {
            agent_id: "a".to_string(),
            max_nodes: None,
            min_similarity: None,
            cluster_threshold: None,
            max_edges_per_node: None,
            content_preview_chars: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("max_nodes"));
        assert!(!json.contains("min_similarity"));
        assert!(!json.contains("cluster_threshold"));
        assert!(!json.contains("content_preview_chars"));
    }

    // -------------------------------------------------------------------------
    // SummarizeRequest
    // -------------------------------------------------------------------------

    #[test]
    fn test_summarize_request_dry_run_default_false() {
        let req = SummarizeRequest {
            agent_id: "a".to_string(),
            memory_ids: None,
            target_type: None,
            dry_run: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        // The server has no dry run for summarize: the flag is never sent.
        assert!(!json.contains("dry_run"));
        assert!(!json.contains("memory_ids"));
        assert!(!json.contains("target_type"));
    }

    #[test]
    fn test_summarize_request_with_memory_ids() {
        let req = SummarizeRequest {
            agent_id: "a".to_string(),
            memory_ids: Some(vec!["m1".to_string(), "m2".to_string()]),
            target_type: Some("semantic".to_string()),
            dry_run: true,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("dry_run"));
        assert!(json.contains("\"m1\""));
        assert!(json.contains("\"target_type\":\"semantic\""));
    }

    // -------------------------------------------------------------------------
    // SummarizeResponse
    // -------------------------------------------------------------------------

    #[test]
    fn test_summarize_response_deserializes() {
        let json = r#"{"summary": "user is a developer", "source_count": 5}"#;
        let resp: SummarizeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.summary, "user is a developer");
        assert_eq!(resp.source_count, 5);
        assert!(resp.new_memory_id.is_none());
    }

    // -------------------------------------------------------------------------
    // DeduplicateRequest
    // -------------------------------------------------------------------------

    #[test]
    fn test_deduplicate_request_dry_run_default_false() {
        let req = DeduplicateRequest {
            agent_id: "a".to_string(),
            threshold: None,
            memory_type: None,
            dry_run: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"dry_run\":false"));
        assert!(!json.contains("threshold"));
        assert!(!json.contains("memory_type"));
    }

    #[test]
    fn test_deduplicate_request_with_threshold() {
        let req = DeduplicateRequest {
            agent_id: "a".to_string(),
            threshold: Some(0.92),
            memory_type: Some("episodic".to_string()),
            dry_run: true,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"threshold\":0.92"));
        assert!(json.contains("\"memory_type\":\"episodic\""));
    }

    // -------------------------------------------------------------------------
    // DeduplicateResponse
    // -------------------------------------------------------------------------

    #[test]
    fn test_deduplicate_response_deserializes() {
        let json =
            r#"{"duplicates_found": 3, "removed_count": 2, "groups": [["a","b"],["c","d","e"]]}"#;
        let resp: DeduplicateResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.duplicates_found, 3);
        assert_eq!(resp.removed_count, 2);
        assert_eq!(resp.groups.len(), 2);
    }

    // -------------------------------------------------------------------------
    // CrossAgentNetworkRequest defaults
    // -------------------------------------------------------------------------

    #[test]
    fn test_cross_agent_network_request_default_values() {
        let req = CrossAgentNetworkRequest::default();
        assert!(req.agent_ids.is_none());
        assert!((req.min_similarity - 0.3).abs() < 1e-6);
        assert_eq!(req.max_nodes_per_agent, 50);
        assert!((req.min_importance - 0.0).abs() < 1e-6);
        assert_eq!(req.max_cross_edges, 200);
    }

    // -------------------------------------------------------------------------
    // CrossAgentNetworkResponse
    // -------------------------------------------------------------------------

    #[test]
    fn test_cross_agent_network_response_node_count_defaults_zero() {
        let json = r#"{
            "agents": [],
            "nodes": [],
            "edges": [],
            "stats": {
                "total_agents": 0,
                "total_nodes": 0,
                "total_cross_edges": 0,
                "density": 0.0
            }
        }"#;
        let resp: CrossAgentNetworkResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.node_count, 0);
        assert!(resp.agents.is_empty());
    }
}
