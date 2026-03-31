//! Knowledge graph operations for the Dakera client.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::types::{KgExportResponse, KgPathResponse, KgQueryResponse};
use crate::DakeraClient;

// ============================================================================
// Knowledge Graph Types
// ============================================================================

/// Request to build a knowledge graph
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
}

/// An edge in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEdge {
    pub source: String,
    pub target: String,
    pub similarity: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship: Option<String>,
}

/// Response from knowledge graph operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraphResponse {
    pub nodes: Vec<KnowledgeNode>,
    pub edges: Vec<KnowledgeEdge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clusters: Option<Vec<Vec<String>>>,
}

/// Request to build a full knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullKnowledgeGraphRequest {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_nodes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_similarity: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_edges_per_node: Option<u32>,
}

/// Request to summarize memories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizeRequest {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_type: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
}

/// Response from summarization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizeResponse {
    pub summary: String,
    pub source_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_memory_id: Option<String>,
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

/// Response from deduplication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeduplicateResponse {
    pub duplicates_found: usize,
    pub removed_count: usize,
    pub groups: Vec<Vec<String>>,
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
}

impl Default for CrossAgentNetworkRequest {
    fn default() -> Self {
        Self {
            agent_ids: None,
            min_similarity: 0.3,
            max_nodes_per_agent: 50,
            min_importance: 0.0,
            max_cross_edges: 200,
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
    pub id: String,
    pub agent_id: String,
    pub content: String,
    pub importance: f32,
    pub tags: Vec<String>,
    pub memory_type: String,
    /// Unix milliseconds.
    pub created_at: u64,
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
    /// Build a knowledge graph from a seed memory
    pub async fn knowledge_graph(
        &self,
        request: KnowledgeGraphRequest,
    ) -> Result<KnowledgeGraphResponse> {
        let url = format!("{}/v1/knowledge/graph", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Build a full knowledge graph for an agent
    pub async fn full_knowledge_graph(
        &self,
        request: FullKnowledgeGraphRequest,
    ) -> Result<KnowledgeGraphResponse> {
        let url = format!("{}/v1/knowledge/graph/full", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Summarize memories
    pub async fn summarize(&self, request: SummarizeRequest) -> Result<SummarizeResponse> {
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
