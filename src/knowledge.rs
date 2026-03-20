//! Knowledge graph operations for the Dakera client.

use serde::{Deserialize, Serialize};

use crate::error::Result;
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
}
