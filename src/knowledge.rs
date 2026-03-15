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
    pub async fn summarize(
        &self,
        request: SummarizeRequest,
    ) -> Result<SummarizeResponse> {
        let url = format!("{}/v1/knowledge/summarize", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Deduplicate memories
    pub async fn deduplicate(
        &self,
        request: DeduplicateRequest,
    ) -> Result<DeduplicateResponse> {
        let url = format!("{}/v1/knowledge/deduplicate", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }
}
