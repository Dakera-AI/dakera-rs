//! Dakera Rust Client SDK
//!
//! A high-level Rust client for interacting with Dakera AI Agent Memory Platform.
//!
//! # Quick Start (HTTP)
//!
//! ```rust,no_run
//! use dakera_client::{DakeraClient, UpsertRequest, QueryRequest};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a client
//!     let client = DakeraClient::new("http://localhost:3000")?;
//!
//!     // Check health
//!     let health = client.health().await?;
//!     println!("Server healthy: {}", health.healthy);
//!
//!     // Upsert vectors
//!     let request = UpsertRequest {
//!         vectors: vec![
//!             dakera_client::Vector {
//!                 id: "vec1".to_string(),
//!                 values: vec![0.1, 0.2, 0.3, 0.4],
//!                 metadata: None,
//!             },
//!         ],
//!     };
//!     client.upsert("my-namespace", request).await?;
//!
//!     // Query for similar vectors
//!     let query = QueryRequest {
//!         vector: vec![0.1, 0.2, 0.3, 0.4],
//!         top_k: 10,
//!         filter: None,
//!         include_metadata: true,
//!         include_vectors: false,
//!         distance_metric: Default::default(),
//!         consistency: Default::default(),
//!         staleness_config: None,
//!     };
//!     let results = client.query("my-namespace", query).await?;
//!
//!     for match_ in results.results {
//!         println!("ID: {}, Score: {}", match_.id, match_.score);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # gRPC Client with Connection Pooling
//!
//! Enable the `grpc` feature for high-performance gRPC communication:
//!
//! ```rust,ignore
//! use dakera_client::grpc::{GrpcClient, GrpcClientConfig, GrpcConnectionPool};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Single client with HTTP/2 multiplexing
//!     let config = GrpcClientConfig::default()
//!         .with_endpoint("http://localhost:50051")
//!         .with_concurrency_limit(100);
//!     let client = GrpcClient::connect(config).await?;
//!
//!     // Or use a connection pool for even higher throughput
//!     let pool = GrpcConnectionPool::new(GrpcClientConfig::default(), 4).await?;
//!     let client = pool.get();
//!
//!     Ok(())
//! }
//! ```

pub mod admin;
pub mod agents;
pub mod analytics;
pub mod attachments;
mod capabilities;
#[cfg(feature = "http-client")]
mod client;
mod error;
pub mod events;
pub mod filter;
pub mod keys;
pub mod knowledge;
pub mod memory;
pub mod records;
pub mod session;
mod types;

// gRPC client with connection pooling
#[cfg(feature = "grpc")]
mod grpc_client;
#[cfg(feature = "grpc")]
mod grpc_proto;

pub use events::{DakeraEvent, MemoryEvent, OpStatus, VectorMutationOp};

pub use admin::{
    AutoPilotConfig, AutoPilotConfigRequest, AutoPilotConfigResponse, AutoPilotConsolidationResult,
    AutoPilotDedupResult, AutoPilotStatusResponse, AutoPilotTriggerAction, AutoPilotTriggerRequest,
    AutoPilotTriggerResponse, BackupInfo, BackupListResponse, CacheStats, ClearCacheRequest,
    ClearCacheResponse, ClusterStatus, ConsolidationResultSnapshot, CreateBackupRequest,
    CreateBackupResponse, DecayConfigResponse, DecayConfigUpdateRequest, DecayConfigUpdateResponse,
    DecayStatsResponse, DedupResultSnapshot, FulltextReindexNamespaceResult,
    FulltextReindexResponse, IndexStats, IndexStatsResponse, KpiSnapshot, LastDecayCycleStats,
    NodeInfo, NodeListResponse, OpsStats, RestoreBackupRequest, RestoreBackupResponse,
    RuntimeConfig, SlowQueryListResponse, TtlCleanupRequest, TtlCleanupResponse, TtlStats,
    TtlStatsResponse,
};
pub use agents::{AgentStats, AgentSummary, CompressResponse, Memory, WakeUpResponse};
pub use analytics::{
    AnalyticsOverview, LatencyAnalytics, LatencyBucket, NamespaceStorage, NamespaceStorageInfo,
    StorageAnalytics, ThroughputAnalytics, ThroughputDataPoint,
};
// v0.12: attachments, speech-to-text, image indexing
pub use attachments::{
    AttachmentDownload, AttachmentEntry, AttachmentJobAccepted, AttachmentListResponse,
    AttachmentUploadResponse, IndexImageAccepted, IndexImageRequest, TranscribeAccepted,
    TranscribeRequest,
};
// R9 / DAK-10004: GET /v1/capabilities
pub use capabilities::{
    parse_accepted_values, AttachmentCapability, CapabilityKind, LateInteractionCapability,
    ModelCapability, RecordCapabilities, ScoringCapability, ServerCapabilities,
    TranscriptionCapability, VisionCapability,
};
#[cfg(feature = "http-client")]
pub use client::{DakeraClient, DakeraClientBuilder};
pub use error::{ClientError, Result, ServerErrorCode};
pub use keys::{
    ApiKeyUsageResponse, CreateKeyRequest, CreateKeyResponse, CreateNamespaceKeyRequest,
    CreateNamespaceKeyResponse, KeyInfo, ListKeysResponse, ListNamespaceKeysResponse,
    NamespaceKeyInfo, NamespaceKeyUsageResponse, RotateKeyResponse,
};
pub use knowledge::{
    AgentNetworkEdge, AgentNetworkInfo, AgentNetworkNode, AgentNetworkStats,
    CrossAgentNetworkRequest, CrossAgentNetworkResponse, DeduplicateRequest, DeduplicateResponse,
    DuplicateGroup, FullKnowledgeGraphRequest, KnowledgeCluster, KnowledgeEdge,
    KnowledgeGraphRequest, KnowledgeGraphResponse, KnowledgeGraphRoot, KnowledgeGraphStats,
    KnowledgeMemory, KnowledgeNode, KnowledgeRelated, SummarizeRequest, SummarizeResponse,
};
pub use memory::{
    // OBS-1: Business-Event Audit Log
    AuditEvent,
    AuditExportResponse,
    AuditListResponse,
    AuditQuery,
    BatchForgetRequest,
    BatchForgetResponse,
    BatchMemoryFilter,
    BatchRecallRequest,
    BatchRecallResponse,
    // CE-6: DBSCAN Adaptive Consolidation
    ConsolidationConfig,
    ConsolidationLogEntry,
    // EXT-1: External Extraction Providers
    ExtractionResult,
    ForgetRequest,
    ForgetResponse,
    MemoryExportResponse,
    // DX-1: Memory Import / Export
    MemoryImportResponse,
    MemoryType,
    RecallRequest,
    RecallResponse,
    RecalledMemory,
    // SEC-3: AES-256-GCM Encryption Key Rotation
    RotateEncryptionKeyRequest,
    RotateEncryptionKeyResponse,
    // CE-10: Routing mode
    RoutingMode,
    Session,
    SessionEndResponse,
    SessionStartResponse,
    StoreMemoryRequest,
    StoreMemoryResponse,
};
// v0.12: records with named representations
pub use records::{
    RecordInput, RecordUpsertRequest, RecordUpsertResponse, RecordView, RepresentationInfo,
    RepresentationInput,
};
#[cfg(feature = "http-client")]
pub use session::ChatMemorySession;
pub use types::*;

// gRPC exports
#[cfg(feature = "grpc")]
pub mod grpc {
    //! gRPC client with connection pooling for high-performance scenarios.
    pub use crate::grpc_client::{GrpcClient, GrpcClientConfig, GrpcConnectionPool, PoolStats};
    pub use crate::grpc_proto::*;
}

// Re-export reqwest for CLI and other consumers
#[cfg(feature = "http-client")]
pub use reqwest;

/// Compiles the README's Rust examples as doctests (`cargo test`), so they
/// cannot rot.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
