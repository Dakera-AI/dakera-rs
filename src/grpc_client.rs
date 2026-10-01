//! gRPC Client for Dakera with Connection Pooling
//!
//! Provides a high-performance gRPC client with:
//! - Connection pooling via HTTP/2 multiplexing
//! - Configurable concurrency limits
//! - Timeout support
//! - Automatic reconnection
//! - API-key authentication (`x-api-key` call metadata; required by the
//!   server from v0.12 whenever authentication is on)
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
//!         .with_timeout_ms(30000)
//!         // v0.12 servers refuse unauthenticated gRPC calls (except Health).
//!         // Without this the DAKERA_API_KEY environment variable is used.
//!         .with_api_key("dk-...");
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

/// The `x-api-key` metadata value for `key` (marked sensitive so it never
/// shows up in debug output of the request).
fn api_key_header(key: &str) -> Result<http::HeaderValue> {
    let mut value = http::HeaderValue::from_str(key)
        .map_err(|_| ClientError::Config("invalid API key".to_string()))?;
    value.set_sensitive(true);
    Ok(value)
}

/// Build the unary gRPC request for `path` (`/dakera.v1.VectorService/<Rpc>`),
/// with the API key as `x-api-key` metadata when there is one.
fn build_http_request(
    path: &str,
    body_bytes: Vec<u8>,
    api_key: Option<&str>,
) -> Result<http::Request<tonic::body::Body>> {
    let mut builder = http::Request::builder()
        .method(http::Method::POST)
        .uri(path)
        .header("content-type", "application/grpc")
        .header("te", "trailers");
    if let Some(key) = api_key {
        builder = builder.header("x-api-key", api_key_header(key)?);
    }
    builder
        .body(tonic::body::Body::new(
            http_body_util::Full::new(bytes::Bytes::from(body_bytes))
                .map_err(|_: std::convert::Infallible| tonic::Status::internal("body error")),
        ))
        .map_err(|e| ClientError::Grpc(format!("Failed to build request: {}", e)))
}

/// Map a non-OK `grpc-status` (headers or trailers) to an error; `None` when
/// the status is absent or OK.  `UNAUTHENTICATED` / `PERMISSION_DENIED` map to
/// the auth errors the HTTP client uses, `UNAVAILABLE` to `ServiceUnavailable`.
fn grpc_status_error(headers: &http::HeaderMap) -> Option<ClientError> {
    let code: i32 = headers.get("grpc-status")?.to_str().ok()?.parse().ok()?;
    if code == 0 {
        return None;
    }
    let message = headers
        .get("grpc-message")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let name = match code {
        1 => "CANCELLED",
        2 => "UNKNOWN",
        3 => "INVALID_ARGUMENT",
        4 => "DEADLINE_EXCEEDED",
        5 => "NOT_FOUND",
        6 => "ALREADY_EXISTS",
        7 => "PERMISSION_DENIED",
        8 => "RESOURCE_EXHAUSTED",
        9 => "FAILED_PRECONDITION",
        10 => "ABORTED",
        11 => "OUT_OF_RANGE",
        12 => "UNIMPLEMENTED",
        13 => "INTERNAL",
        14 => "UNAVAILABLE",
        15 => "DATA_LOSS",
        16 => "UNAUTHENTICATED",
        _ => "UNKNOWN",
    };
    Some(match code {
        16 => ClientError::Server {
            status: 401,
            message: format!("gRPC {name}: {message}"),
            code: None,
        },
        7 => ClientError::Authorization {
            status: 403,
            message: format!("gRPC {name}: {message}"),
            code: None,
        },
        14 => ClientError::ServiceUnavailable {
            message: format!("gRPC {name}: {message}"),
            details: None,
            retry_after: None,
        },
        _ => ClientError::Grpc(format!("{name}: {message}")),
    })
}

/// Configuration for the gRPC client
#[derive(Clone)]
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
    /// API key sent as `x-api-key` call metadata on every RPC.  Server v0.12
    /// requires one for everything but `Health` when authentication is on
    /// (v0.11 servers ignore it).  `None` falls back to the `DAKERA_API_KEY`
    /// environment variable at connect time, like the HTTP client.
    pub api_key: Option<String>,
}

impl std::fmt::Debug for GrpcClientConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GrpcClientConfig")
            .field("endpoint", &self.endpoint)
            .field("concurrency_limit", &self.concurrency_limit)
            .field("timeout_ms", &self.timeout_ms)
            .field("connect_timeout_ms", &self.connect_timeout_ms)
            .field("keep_alive_interval_secs", &self.keep_alive_interval_secs)
            .field("keep_alive_timeout_secs", &self.keep_alive_timeout_secs)
            .field("http2_adaptive_window", &self.http2_adaptive_window)
            .field(
                "initial_connection_window_size",
                &self.initial_connection_window_size,
            )
            .field(
                "initial_stream_window_size",
                &self.initial_stream_window_size,
            )
            // Never print the key.
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .finish()
    }
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
            api_key: None,
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

    /// Set the API key sent as `x-api-key` metadata on every call
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
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
    pub async fn connect(mut config: GrpcClientConfig) -> Result<Self> {
        info!("Connecting to gRPC server at {}", config.endpoint);

        // Resolve the API key: explicit > DAKERA_API_KEY (same as the HTTP client).
        if config.api_key.is_none() {
            config.api_key = std::env::var("DAKERA_API_KEY")
                .ok()
                .filter(|k| !k.is_empty());
        }
        // Fail at connect, not on the first call, if the key cannot be a header value.
        if let Some(key) = &config.api_key {
            api_key_header(key)?;
        }

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
        let http_request = build_http_request(path, body_bytes, self.config.api_key.as_deref())?;

        // Call the service
        let response = client
            .call(http_request)
            .await
            .map_err(|e| ClientError::Grpc(format!("gRPC call failed: {}", e)))?;

        // A trailers-only answer (an error before any message, e.g.
        // UNAUTHENTICATED) carries grpc-status in the headers.
        if let Some(e) = grpc_status_error(response.headers()) {
            return Err(e);
        }

        // Extract the body
        let body = response.into_body();
        let collected = body
            .collect()
            .await
            .map_err(|e| ClientError::Grpc(format!("Failed to collect body: {}", e)))?;

        // A non-OK status of a normal answer arrives in the trailers.
        if let Some(e) = collected.trailers().and_then(grpc_status_error) {
            return Err(e);
        }

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
            .send_request("/dakera.v1.VectorService/Health", request)
            .await
            .inspect_err(|_e| {
                let _ = self.stats.try_write().map(|mut s| s.failed_requests += 1);
            })?;

        self.track_success().await;

        Ok(ClientHealthResponse {
            healthy: response.status == "healthy",
            status: Some(response.status.clone()),
            version: Some(response.version),
            uptime_seconds: None, // gRPC health doesn't include uptime
            build_sha: None,      // gRPC health proto doesn't carry build_sha
        })
    }

    /// Get namespace information
    pub async fn get_namespace(&self, namespace: &str) -> Result<ClientNamespaceInfo> {
        debug!("Getting namespace info: {}", namespace);

        let request = GetNamespaceRequest {
            namespace: namespace.to_string(),
        };

        let response: NamespaceInfo = self
            .send_request("/dakera.v1.VectorService/GetNamespace", request)
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
            .send_request("/dakera.v1.VectorService/DeleteNamespace", request)
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
            .send_request("/dakera.v1.VectorService/Upsert", request)
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
            .send_request("/dakera.v1.VectorService/Query", request)
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
            .send_request("/dakera.v1.VectorService/DeleteVectors", request)
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
            .send_request("/dakera.v1.VectorService/WarmCache", request)
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
    fn test_request_carries_api_key_metadata() {
        let path = "/dakera.v1.VectorService/Query";
        let req = build_http_request(path, vec![0; 5], Some("dk-secret")).unwrap();
        assert_eq!(req.uri().path(), "/dakera.v1.VectorService/Query");
        assert_eq!(req.headers()["x-api-key"], "dk-secret");
        assert!(req.headers()["x-api-key"].is_sensitive());
        assert_eq!(req.headers()["content-type"], "application/grpc");
    }

    #[test]
    fn test_request_without_api_key_has_no_metadata() {
        let req = build_http_request("/dakera.v1.VectorService/Health", vec![0; 5], None).unwrap();
        assert!(req.headers().get("x-api-key").is_none());
    }

    #[test]
    fn test_invalid_api_key_is_a_config_error() {
        let err = build_http_request("/x", vec![], Some("bad\nkey")).unwrap_err();
        assert!(matches!(err, ClientError::Config(_)));
    }

    #[test]
    fn test_api_key_is_redacted_in_debug() {
        let config = GrpcClientConfig::default().with_api_key("dk-secret");
        assert_eq!(config.api_key.as_deref(), Some("dk-secret"));
        let shown = format!("{config:?}");
        assert!(!shown.contains("dk-secret"));
        assert!(shown.contains("<redacted>"));
    }

    #[test]
    fn test_grpc_status_mapping() {
        let mut h = http::HeaderMap::new();
        assert!(grpc_status_error(&h).is_none());
        h.insert("grpc-status", "0".parse().unwrap());
        assert!(grpc_status_error(&h).is_none());

        h.insert("grpc-status", "16".parse().unwrap());
        h.insert("grpc-message", "API key required".parse().unwrap());
        let e = grpc_status_error(&h).unwrap();
        assert!(e.is_auth_error());
        assert!(e.to_string().contains("UNAUTHENTICATED"));

        h.insert("grpc-status", "7".parse().unwrap());
        assert!(grpc_status_error(&h).unwrap().is_auth_error());

        h.insert("grpc-status", "14".parse().unwrap());
        assert!(grpc_status_error(&h).unwrap().is_retryable());

        h.insert("grpc-status", "3".parse().unwrap());
        h.insert("grpc-message", "top_k must be > 0".parse().unwrap());
        match grpc_status_error(&h).unwrap() {
            ClientError::Grpc(m) => assert!(m.contains("INVALID_ARGUMENT") && m.contains("top_k")),
            other => panic!("unexpected {other:?}"),
        }
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
