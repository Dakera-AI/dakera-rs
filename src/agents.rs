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
