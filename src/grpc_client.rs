//! gRPC Client for Dakera with Connection Pooling
//!
//! Provides a high-performance gRPC client with:
//! - Connection pooling via HTTP/2 multiplexing
//! - Configurable concurrency limits
//! - Timeout support
//! - Automatic reconnection
//!
//! # Example
//!
//! ```rust,no_run
//! use dakera_client::grpc::{GrpcClient, GrpcClientConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = GrpcClientConfig::default()
//!         .with_endpoint("http://localhost:50051")
//!         .with_concurrency_limit(100)
//!         .with_timeout_ms(30000);
//!
//!     let client = GrpcClient::connect(config).await?;
//!
//!     // Check health
//!     let health = client.health().await?;
//!     println!("Healthy: {}", health.healthy);
//!
//!     Ok(())
//! }
//! ```

#![cfg(feature = "grpc")]

use std::sync::Arc;
use std::time::Duration;

use http_body_util::BodyExt;
use prost::Message;
use tokio::sync::RwLock;
use tonic::transport::{Channel, Endpoint};
use tower::Service;
use tracing::{debug, info};

use crate::error::{ClientError, Result};
use crate::grpc_proto::*;
use crate::types::{
    DeleteResponse, HealthResponse as ClientHealthResponse, Match,
    NamespaceInfo as ClientNamespaceInfo, QueryResponse as ClientQueryResponse,
    UpsertResponse as ClientUpsertResponse, Vector,
};

/// Configuration for the gRPC client
#[derive(Debug, Clone)]
pub struct GrpcClientConfig {
    /// Server endpoint (e.g., "http://localhost:50051")
    pub endpoint: String,
    /// Maximum concurrent requests per connection
    pub concurrency_limit: usize,
    /// Request timeout in milliseconds
    pub timeout_ms: u64,
    /// Connection timeout in milliseconds
    pub connect_timeout_ms: u64,
    /// Keep-alive interval in seconds
    pub keep_alive_interval_secs: u64,
    /// Keep-alive timeout in seconds
    pub keep_alive_timeout_secs: u64,
    /// Enable HTTP/2 adaptive window for better throughput
    pub http2_adaptive_window: bool,
    /// Initial connection window size
    pub initial_connection_window_size: u32,
    /// Initial stream window size
    pub initial_stream_window_size: u32,
}

impl Default for GrpcClientConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:50051".to_string(),
            concurrency_limit: 100,
            timeout_ms: 30_000,
            connect_timeout_ms: 5_000,
            keep_alive_interval_secs: 30,
            keep_alive_timeout_secs: 10,
            http2_adaptive_window: true,
            initial_connection_window_size: 1024 * 1024, // 1MB
            initial_stream_window_size: 1024 * 1024,     // 1MB
        }
    }
}

impl GrpcClientConfig {
    /// Create a new config with the given endpoint
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            ..Default::default()
        }
    }

    /// Set the endpoint
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Set the concurrency limit
    pub fn with_concurrency_limit(mut self, limit: usize) -> Self {
        self.concurrency_limit = limit;
        self
    }

    /// Set the request timeout in milliseconds
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Set the connection timeout in milliseconds
    pub fn with_connect_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.connect_timeout_ms = timeout_ms;
        self
    }

    /// Set keep-alive interval in seconds
    pub fn with_keep_alive_interval(mut self, secs: u64) -> Self {
        self.keep_alive_interval_secs = secs;
        self
    }

    /// Set keep-alive timeout in seconds
    pub fn with_keep_alive_timeout(mut self, secs: u64) -> Self {
        self.keep_alive_timeout_secs = secs;
        self
    }

    /// Enable or disable HTTP/2 adaptive window
    pub fn with_http2_adaptive_window(mut self, enabled: bool) -> Self {
        self.http2_adaptive_window = enabled;
        self
    }
}

/// Connection pool statistics
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    /// Total number of requests made
    pub total_requests: u64,
    /// Number of successful requests
    pub successful_requests: u64,
    /// Number of failed requests
    pub failed_requests: u64,
    /// Number of reconnection attempts
    pub reconnects: u64,
}

/// gRPC client with connection pooling
///
/// Uses HTTP/2 multiplexing for efficient connection reuse.
/// A single connection can handle multiple concurrent requests.
pub struct GrpcClient {
    config: GrpcClientConfig,
    channel: Channel,
    stats: Arc<RwLock<PoolStats>>,
}

impl GrpcClient {
    /// Connect to the gRPC server with the given configuration
    pub async fn connect(config: GrpcClientConfig) -> Result<Self> {
        info!("Connecting to gRPC server at {}", config.endpoint);

        let endpoint = Endpoint::from_shared(config.endpoint.clone())
            .map_err(|e| ClientError::Connection(format!("Invalid endpoint: {}", e)))?
            .connect_timeout(Duration::from_millis(config.connect_timeout_ms))
            .timeout(Duration::from_millis(config.timeout_ms))
            .http2_keep_alive_interval(Duration::from_secs(config.keep_alive_interval_secs))
            .keep_alive_timeout(Duration::from_secs(config.keep_alive_timeout_secs))
            .http2_adaptive_window(config.http2_adaptive_window)
            .initial_connection_window_size(config.initial_connection_window_size)
            .initial_stream_window_size(config.initial_stream_window_size);

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| ClientError::Connection(format!("Failed to connect: {}", e)))?;

        info!("Successfully connected to gRPC server");

        Ok(Self {
            config,
            channel,
            stats: Arc::new(RwLock::new(PoolStats::default())),
        })
    }

    /// Connect with default configuration
    pub async fn connect_default(endpoint: impl Into<String>) -> Result<Self> {
        Self::connect(GrpcClientConfig::new(endpoint)).await
    }

    /// Get the current configuration
    pub fn config(&self) -> &GrpcClientConfig {
        &self.config
    }

    /// Get connection pool statistics
    pub async fn stats(&self) -> PoolStats {
        self.stats.read().await.clone()
    }

    /// Reset statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = PoolStats::default();
    }

    /// Internal helper to track request success
    async fn track_success(&self) {
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;
        stats.successful_requests += 1;
    }

    /// Send a raw gRPC request and decode the response
    async fn send_request<Req: Message, Resp: Message + Default>(
        &self,
        path: &str,
        request: Req,
    ) -> Result<Resp> {
        let mut client = self.channel.clone();

        // Encode the request with gRPC framing (1 byte compression flag + 4 bytes length)
        let encoded = request.encode_to_vec();
        let mut body_bytes = Vec::with_capacity(5 + encoded.len());
        body_bytes.push(0); // No compression
        body_bytes.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
        body_bytes.extend_from_slice(&encoded);

        // Build HTTP/2 request for gRPC
        let http_request = http::Request::builder()
            .method(http::Method::POST)
            .uri(path)
            .header("content-type", "application/grpc")
            .header("te", "trailers")
            .body(tonic::body::Body::new(
                http_body_util::Full::new(bytes::Bytes::from(body_bytes))
                    .map_err(|_: std::convert::Infallible| tonic::Status::internal("body error")),
            ))
            .map_err(|e| ClientError::Grpc(format!("Failed to build request: {}", e)))?;

        // Call the service
        let response = client
            .call(http_request)
            .await
            .map_err(|e| ClientError::Grpc(format!("gRPC call failed: {}", e)))?;

        // Extract the body
        let body = response.into_body();
        let collected = body
            .collect()
            .await
            .map_err(|e| ClientError::Grpc(format!("Failed to collect body: {}", e)))?;

        let response_bytes = collected.to_bytes();

        // Skip the 5-byte gRPC header (compression flag + message length)
        if response_bytes.len() < 5 {
            return Err(ClientError::Grpc("Response too short".to_string()));
        }

        let message_bytes = &response_bytes[5..];
        let resp = Resp::decode(message_bytes)
            .map_err(|e| ClientError::Grpc(format!("Failed to decode response: {}", e)))?;

        Ok(resp)
    }

    // =========================================================================
    // Public API Methods
    // =========================================================================

    /// Check server health
    pub async fn health(&self) -> Result<ClientHealthResponse> {
        debug!("Checking server health");

        let request = HealthRequest {};
        let response: HealthResponse = self
            .send_request("/dakera.VectorService/Health", request)
            .await
            .inspect_err(|_e| {
                let _ = self.stats.try_write().map(|mut s| s.failed_requests += 1);
            })?;

        self.track_success().await;

        Ok(ClientHealthResponse {
            healthy: response.status == "healthy",
            version: Some(response.version),
            uptime_seconds: None, // gRPC health doesn't include uptime
        })
    }

    /// Get namespace information
    pub async fn get_namespace(&self, namespace: &str) -> Result<ClientNamespaceInfo> {
        debug!("Getting namespace info: {}", namespace);

        let request = GetNamespaceRequest {
            namespace: namespace.to_string(),
        };

        let response: NamespaceInfo = self
            .send_request("/dakera.VectorService/GetNamespace", request)
            .await
            .inspect_err(|_e| {
                let _ = self.stats.try_write().map(|mut s| s.failed_requests += 1);
            })?;

        self.track_success().await;

        Ok(ClientNamespaceInfo {
            name: response.name,
            vector_count: response.vector_count,
            dimensions: response.dimension,
            index_type: None,
            created: None,
        })
    }

    /// Delete a namespace
    pub async fn delete_namespace(&self, namespace: &str) -> Result<bool> {
        debug!("Deleting namespace: {}", namespace);

        let request = DeleteNamespaceRequest {
            namespace: namespace.to_string(),
        };

        let response: DeleteNamespaceResponse = self
            .send_request("/dakera.VectorService/DeleteNamespace", request)
            .await
            .inspect_err(|_e| {
                let _ = self.stats.try_write().map(|mut s| s.failed_requests += 1);
            })?;

        self.track_success().await;

        Ok(response.success)
    }

    /// Upsert vectors into a namespace
    pub async fn upsert(
        &self,
        namespace: &str,
        vectors: Vec<Vector>,
    ) -> Result<ClientUpsertResponse> {
        debug!(
            "Upserting {} vectors to namespace: {}",
            vectors.len(),
            namespace
        );

        let proto_vectors: Vec<ProtoVector> = vectors
            .into_iter()
            .map(|v| ProtoVector {
                id: v.id,
                values: v.values,
                metadata_json: v
                    .metadata
                    .map(|m| serde_json::to_string(&m).unwrap_or_default()),
            })
            .collect();

        let request = GrpcUpsertRequest {
            namespace: namespace.to_string(),
            vectors: proto_vectors,
        };

        let response: UpsertResponse = self
            .send_request("/dakera.VectorService/Upsert", request)
            .await
            .inspect_err(|_e| {
                let _ = self.stats.try_write().map(|mut s| s.failed_requests += 1);
            })?;

        self.track_success().await;

        Ok(ClientUpsertResponse {
            upserted_count: response.upserted_count,
        })
    }

    /// Query for similar vectors
    pub async fn query(
        &self,
        namespace: &str,
        vector: Vec<f32>,
        top_k: u32,
        distance_metric: &str,
        include_metadata: bool,
        include_vectors: bool,
    ) -> Result<ClientQueryResponse> {
        debug!("Querying namespace {} for top {} vectors", namespace, top_k);

        let request = GrpcQueryRequest {
            namespace: namespace.to_string(),
            vector,
            top_k,
            distance_metric: distance_metric.to_string(),
            include_metadata,
            include_vectors,
        };

        let response: QueryResponse = self
            .send_request("/dakera.VectorService/Query", request)
            .await
            .inspect_err(|_e| {
                let _ = self.stats.try_write().map(|mut s| s.failed_requests += 1);
            })?;

        self.track_success().await;

        let matches: Vec<Match> = response
            .results
            .into_iter()
            .map(|r| Match {
                id: r.id,
                score: r.score,
                metadata: r.metadata_json.and_then(|s| serde_json::from_str(&s).ok()),
            })
            .collect();

        Ok(ClientQueryResponse { results: matches })
    }

    /// Delete vectors by ID
    pub async fn delete_vectors(
        &self,
        namespace: &str,
        ids: Vec<String>,
    ) -> Result<DeleteResponse> {
        debug!(
            "Deleting {} vectors from namespace: {}",
            ids.len(),
            namespace
        );

        let request = DeleteVectorsRequest {
            namespace: namespace.to_string(),
            ids,
        };

        let response: DeleteVectorsResponse = self
            .send_request("/dakera.VectorService/DeleteVectors", request)
            .await
            .inspect_err(|_e| {
                let _ = self.stats.try_write().map(|mut s| s.failed_requests += 1);
            })?;

        self.track_success().await;

        Ok(DeleteResponse {
            deleted_count: response.deleted_count,
        })
    }

    /// Warm the cache for specific vectors
    pub async fn warm_cache(&self, namespace: &str, vector_ids: Vec<String>) -> Result<u64> {
        debug!(
            "Warming cache for {} vectors in namespace: {}",
            vector_ids.len(),
            namespace
        );

        let request = WarmCacheRequest {
            namespace: namespace.to_string(),
            vector_ids,
        };

        let response: WarmCacheResponse = self
            .send_request("/dakera.VectorService/WarmCache", request)
            .await
            .inspect_err(|_e| {
                let _ = self.stats.try_write().map(|mut s| s.failed_requests += 1);
            })?;

        self.track_success().await;

        Ok(response.warmed_count)
    }
}

impl Clone for GrpcClient {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            channel: self.channel.clone(),
            stats: self.stats.clone(),
        }
    }
}

/// Connection pool for managing multiple gRPC channels
///
/// Provides load balancing across multiple connections for high-throughput scenarios.
pub struct GrpcConnectionPool {
    clients: Vec<GrpcClient>,
    next_idx: Arc<std::sync::atomic::AtomicUsize>,
}

impl GrpcConnectionPool {
    /// Create a new connection pool with the specified number of connections
    pub async fn new(config: GrpcClientConfig, pool_size: usize) -> Result<Self> {
        info!(
            "Creating gRPC connection pool with {} connections",
            pool_size
        );

        let mut clients = Vec::with_capacity(pool_size);
        for i in 0..pool_size {
            let client = GrpcClient::connect(config.clone()).await?;
            debug!("Created pool connection {}/{}", i + 1, pool_size);
            clients.push(client);
        }

        Ok(Self {
            clients,
            next_idx: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        })
    }

    /// Get the next client using round-robin selection
    pub fn get(&self) -> &GrpcClient {
        let idx = self
            .next_idx
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % self.clients.len();
        &self.clients[idx]
    }

    /// Get pool size
    pub fn size(&self) -> usize {
        self.clients.len()
    }

    /// Get aggregate statistics from all connections
    pub async fn aggregate_stats(&self) -> PoolStats {
        let mut total = PoolStats::default();
        for client in &self.clients {
            let stats = client.stats().await;
            total.total_requests += stats.total_requests;
            total.successful_requests += stats.successful_requests;
            total.failed_requests += stats.failed_requests;
            total.reconnects += stats.reconnects;
        }
        total
    }
}

impl Clone for GrpcConnectionPool {
    fn clone(&self) -> Self {
        Self {
            clients: self.clients.clone(),
            next_idx: self.next_idx.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = GrpcClientConfig::default()
            .with_endpoint("http://localhost:9000")
            .with_concurrency_limit(50)
            .with_timeout_ms(10000)
            .with_connect_timeout_ms(3000)
            .with_keep_alive_interval(60)
            .with_keep_alive_timeout(20);

        assert_eq!(config.endpoint, "http://localhost:9000");
        assert_eq!(config.concurrency_limit, 50);
        assert_eq!(config.timeout_ms, 10000);
        assert_eq!(config.connect_timeout_ms, 3000);
        assert_eq!(config.keep_alive_interval_secs, 60);
        assert_eq!(config.keep_alive_timeout_secs, 20);
    }

    #[test]
    fn test_default_config() {
        let config = GrpcClientConfig::default();

        assert_eq!(config.endpoint, "http://localhost:50051");
        assert_eq!(config.concurrency_limit, 100);
        assert_eq!(config.timeout_ms, 30_000);
        assert_eq!(config.connect_timeout_ms, 5_000);
        assert!(config.http2_adaptive_window);
    }
}
