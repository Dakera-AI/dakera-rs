//! Memory-oriented client methods for Dakera AI Agent Memory Platform
//!
//! Provides high-level methods for storing, recalling, and managing
//! agent memories and sessions through the Dakera API.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::DakeraClient;

// ============================================================================
// Memory Types (client-side)
// ============================================================================

/// Memory type classification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
}

/// Stored memory response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreMemoryResponse {
    pub memory_id: String,
    pub agent_id: String,
    pub namespace: String,
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
}

/// A recalled memory
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

/// Recall response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallResponse {
    pub memories: Vec<RecalledMemory>,
    pub total_found: usize,
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
}

/// Session end request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEndRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
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

/// Request to consolidate memories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
    #[serde(default)]
    pub dry_run: bool,
}

/// Response from consolidation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidateResponse {
    pub consolidated_count: usize,
    pub removed_count: usize,
    pub new_memories: Vec<String>,
}

/// Request for memory feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackRequest {
    pub memory_id: String,
    pub feedback: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relevance_score: Option<f32>,
}

/// Response from feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackResponse {
    pub status: String,
    pub updated_importance: Option<f32>,
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
        let url = format!(
            "{}/v1/agents/{}/memories/consolidate",
            self.base_url, agent_id
        );
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Submit feedback on a memory recall
    pub async fn memory_feedback(
        &self,
        agent_id: &str,
        request: FeedbackRequest,
    ) -> Result<FeedbackResponse> {
        let url = format!(
            "{}/v1/agents/{}/memories/feedback",
            self.base_url, agent_id
        );
        let response = self.client.post(&url).json(&request).send().await?;
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
        self.handle_response(response).await
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
        self.handle_response(response).await
    }

    /// End a session, optionally with a summary
    pub async fn end_session(
        &self,
        session_id: &str,
        summary: Option<String>,
    ) -> Result<Session> {
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
}
