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
//!     };
//!     let results = client.query("my-namespace", query).await?;
//!
//!     for match_ in results.matches {
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

#[cfg(feature = "http-client")]
mod client;
mod error;
pub mod admin;
pub mod agents;
pub mod analytics;
pub mod keys;
pub mod knowledge;
pub mod memory;
mod types;

// gRPC client with connection pooling
#[cfg(feature = "grpc")]
mod grpc_proto;
#[cfg(feature = "grpc")]
mod grpc_client;

#[cfg(feature = "http-client")]
pub use client::{DakeraClient, DakeraClientBuilder};
pub use error::{ClientError, Result};
pub use admin::{
    ClusterStatus, NodeInfo, NodeListResponse, IndexStats, IndexStatsResponse,
    CacheStats, RuntimeConfig, BackupInfo, BackupListResponse,
    CreateBackupRequest, CreateBackupResponse, RestoreBackupRequest, RestoreBackupResponse,
    QuotaConfig, QuotaStatus, QuotaListResponse, SlowQueryListResponse,
};
pub use agents::{AgentSummary, AgentStats};
pub use analytics::{AnalyticsOverview, LatencyAnalytics, ThroughputAnalytics, StorageAnalytics};
pub use keys::{
    CreateKeyRequest, CreateKeyResponse, KeyInfo, ListKeysResponse,
    RotateKeyResponse, ApiKeyUsageResponse,
};
pub use knowledge::{
    KnowledgeGraphRequest, KnowledgeGraphResponse, KnowledgeNode, KnowledgeEdge,
    FullKnowledgeGraphRequest, SummarizeRequest, SummarizeResponse,
    DeduplicateRequest, DeduplicateResponse,
};
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
