//! SSE streaming event types and client support (CE-1).
//!
//! ## Usage
//!
//! ```rust,no_run
//! use dakera_client::DakeraClient;
//! use tokio::sync::mpsc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = DakeraClient::new("http://localhost:3000")?;
//!
//!     let mut rx = client.stream_namespace_events("my-ns").await?;
//!     while let Some(result) = rx.recv().await {
//!         let event = result?;
//!         println!("Event: {:?}", event);
//!     }
//!     Ok(())
//! }
//! ```

use serde::{Deserialize, Serialize};

/// Operation status for [`DakeraEvent::OperationProgress`] events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// Vector mutation operation type for [`DakeraEvent::VectorsMutated`] events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorMutationOp {
    Upserted,
    Deleted,
}

/// An event received from a Dakera SSE stream.
///
/// Mirrors the server-side `DakeraEvent` enum.  Events are delivered over
/// `GET /v1/namespaces/:ns/events` (namespace-scoped, Read scope) or
/// `GET /ops/events` (global, Admin scope).
///
/// Use [`DakeraClient::stream_namespace_events`] or
/// [`DakeraClient::stream_global_events`] to subscribe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DakeraEvent {
    /// A new namespace was created.
    NamespaceCreated { namespace: String, dimension: usize },

    /// A namespace was deleted.
    NamespaceDeleted { namespace: String },

    /// Progress update for a long-running operation (progress: 0–100).
    OperationProgress {
        operation_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        namespace: Option<String>,
        op_type: String,
        /// Progress percentage 0–100.
        progress: u8,
        status: OpStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        /// Unix milliseconds.
        updated_at: u64,
    },

    /// A background job changed status.
    JobProgress {
        job_id: String,
        job_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        namespace: Option<String>,
        progress: u8,
        status: String,
    },

    /// Vectors were upserted or deleted in bulk (threshold: >100 vectors).
    VectorsMutated {
        namespace: String,
        op: VectorMutationOp,
        count: usize,
    },

    /// Subscriber fell too far behind — some events were dropped.
    /// Reconnect to resume the stream.
    StreamLagged { dropped: u64, hint: String },

    /// Emitted immediately on stream subscription to confirm the connection is live.
    ///
    /// Clients can use this to distinguish *connected-and-idle* from *not-yet-connected*.
    Connected {
        /// Unix milliseconds when the connection was confirmed.
        timestamp: u64,
    },
}

/// A memory lifecycle event received from the `GET /v1/events/stream` SSE endpoint (DASH-B).
///
/// The `event_type` field identifies the operation:
/// - `connected` — emitted immediately on subscription; `agent_id` will be an empty string
/// - `stored` — a memory was stored (content, importance, tags present)
/// - `recalled` — a memory was recalled
/// - `forgotten` — a memory was deleted
/// - `consolidated` — memories were merged
/// - `importance_updated` — importance score changed
/// - `session_started` / `session_ended` — agent session lifecycle
/// - `stream_lagged` — consumer fell behind; some events were dropped
///
/// Use [`DakeraClient::stream_memory_events`] to subscribe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEvent {
    /// Event type. The `connected` handshake event uses the JSON `"type"` key
    /// rather than `"event_type"` — the SDK normalises this automatically.
    #[serde(alias = "type", default)]
    pub event_type: String,
    /// Agent that owns the memory. Empty string for `connected` handshake events.
    #[serde(default)]
    pub agent_id: String,
    /// Unix milliseconds.
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub importance: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl DakeraEvent {
    /// Returns the SSE `event` type string for this event variant.
    pub fn event_type(&self) -> &'static str {
        match self {
            DakeraEvent::NamespaceCreated { .. } => "namespace_created",
            DakeraEvent::NamespaceDeleted { .. } => "namespace_deleted",
            DakeraEvent::OperationProgress { .. } => "operation_progress",
            DakeraEvent::JobProgress { .. } => "job_progress",
            DakeraEvent::VectorsMutated { .. } => "vectors_mutated",
            DakeraEvent::StreamLagged { .. } => "stream_lagged",
            DakeraEvent::Connected { .. } => "connected",
        }
    }
}
