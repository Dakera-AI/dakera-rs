//! High-level session helper for LLM chat comparison patterns.
//!
//! [`ChatMemorySession`] wraps the low-level session/memory API into the
//! three-step pattern used by the playground LLM chat comparison feature:
//!
//! 1. Create a session bound to an agent.
//! 2. Store conversation turns with [`ChatMemorySession::store`].
//! 3. Recall relevant context before generating the next response.
//!
//! # Example
//!
//! ```no_run
//! use std::sync::Arc;
//! use dakera_client::{DakeraClient, ChatMemorySession};
//!
//! # async fn run() -> dakera_client::Result<()> {
//! let client = Arc::new(
//!     DakeraClient::builder("http://localhost:3000")
//!         .api_key("dk-mykey")
//!         .build()?,
//! );
//!
//! let session = ChatMemorySession::create(Arc::clone(&client), "chat-agent").await?;
//! session.store("user", "My name is Alice and I like Rust.").await?;
//! let context = session.recall("user preferences").await?;
//! // Pass context to your LLM — or skip it for the baseline comparison arm.
//! session.close().await?;
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;

use crate::memory::{RecallRequest, RecalledMemory, StoreMemoryRequest, StoreMemoryResponse};
use crate::{DakeraClient, Result};

/// High-level session helper for LLM chat comparison patterns.
///
/// Groups conversation turns under a single Dakera session so that:
///
/// * Every stored message is associated with `session_id` for scoped retrieval.
/// * [`recall`][ChatMemorySession::recall] queries the agent's **full** memory —
///   not just this session — so prior conversations inform the current exchange.
///
/// Create via [`ChatMemorySession::create`]; close via [`ChatMemorySession::close`].
pub struct ChatMemorySession {
    client: Arc<DakeraClient>,
    agent_id: String,
    session_id: String,
}

impl ChatMemorySession {
    // -------------------------------------------------------------------------
    // Factory
    // -------------------------------------------------------------------------

    /// Create a new Dakera session and return a [`ChatMemorySession`].
    ///
    /// # Arguments
    ///
    /// * `client`   – Shared [`DakeraClient`] instance.
    /// * `agent_id` – Identifier for the agent whose memory to use.
    pub async fn create(
        client: Arc<DakeraClient>,
        agent_id: impl Into<String>,
    ) -> Result<ChatMemorySession> {
        let agent_id = agent_id.into();
        let session = client.start_session(&agent_id).await?;
        Ok(ChatMemorySession {
            client,
            agent_id,
            session_id: session.id,
        })
    }

    /// Create a session with attached metadata.
    pub async fn create_with_metadata(
        client: Arc<DakeraClient>,
        agent_id: impl Into<String>,
        metadata: serde_json::Value,
    ) -> Result<ChatMemorySession> {
        let agent_id = agent_id.into();
        let session = client
            .start_session_with_metadata(&agent_id, metadata)
            .await?;
        Ok(ChatMemorySession {
            client,
            agent_id,
            session_id: session.id,
        })
    }

    // -------------------------------------------------------------------------
    // Core operations
    // -------------------------------------------------------------------------

    /// Store a conversation turn in the session with default importance (0.6).
    ///
    /// The `role` (e.g. `"user"` or `"assistant"`) is appended to the memory's
    /// tags automatically.
    pub async fn store(&self, role: &str, content: &str) -> Result<StoreMemoryResponse> {
        self.store_with_opts(role, content, 0.6, &[]).await
    }

    /// Store a conversation turn with custom importance and additional tags.
    pub async fn store_with_opts(
        &self,
        role: &str,
        content: &str,
        importance: f32,
        extra_tags: &[&str],
    ) -> Result<StoreMemoryResponse> {
        let mut tags: Vec<String> = extra_tags.iter().map(|&t| t.to_owned()).collect();
        if !tags.iter().any(|t| t == role) {
            tags.push(role.to_owned());
        }
        let request = StoreMemoryRequest::new(&self.agent_id, content)
            .with_importance(importance)
            .with_tags(tags)
            .with_session(self.session_id.clone());
        self.client.store_memory(request).await
    }

    /// Recall up to 5 memories relevant to `query` for this agent.
    ///
    /// Searches the agent's **full** memory, not just the current session, so
    /// context from prior conversations is surfaced when relevant.
    pub async fn recall(&self, query: &str) -> Result<Vec<RecalledMemory>> {
        self.recall_top_k(query, 5).await
    }

    /// Recall up to `top_k` memories relevant to `query`.
    pub async fn recall_top_k(&self, query: &str, top_k: usize) -> Result<Vec<RecalledMemory>> {
        let request = RecallRequest::new(&self.agent_id, query).with_top_k(top_k);
        let response = self.client.recall(request).await?;
        Ok(response.memories)
    }

    /// End the Dakera session.
    pub async fn close(self) -> Result<()> {
        self.client.end_session(&self.session_id, None).await?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Properties
    // -------------------------------------------------------------------------

    /// The underlying Dakera session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// The agent ID this session is bound to.
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }
}
