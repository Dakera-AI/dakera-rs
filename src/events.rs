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
    NamespaceCreated {
        namespace: String,
        dimension: usize,
    },

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
        }
    }
}
