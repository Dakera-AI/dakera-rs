//! Types for the Dakera client SDK

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Retry & Timeout Configuration
// ============================================================================

/// Configuration for request retry behavior with exponential backoff.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (default: 3).
    pub max_retries: u32,
    /// Base delay before the first retry (default: 100ms).
    pub base_delay: std::time::Duration,
    /// Maximum delay between retries (default: 60s).
    pub max_delay: std::time::Duration,
    /// Whether to add random jitter to backoff delay (default: true).
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: std::time::Duration::from_millis(100),
            max_delay: std::time::Duration::from_secs(60),
            jitter: true,
        }
    }
}

// ============================================================================
// OPS-1: Rate-Limit Headers
// ============================================================================

/// Rate-limit and quota headers present on every API response (OPS-1).
///
/// Fields are `None` when the server does not include the header (e.g.
/// non-namespaced endpoints where quota does not apply).
#[derive(Debug, Clone, Default)]
pub struct RateLimitHeaders {
    /// `X-RateLimit-Limit` — max requests allowed in the current window.
    pub limit: Option<u64>,
    /// `X-RateLimit-Remaining` — requests left in the current window.
    pub remaining: Option<u64>,
    /// `X-RateLimit-Reset` — Unix timestamp (seconds) when the window resets.
    pub reset: Option<u64>,
    /// `X-Quota-Used` — namespace vectors / storage consumed.
    pub quota_used: Option<u64>,
    /// `X-Quota-Limit` — namespace quota ceiling.
    pub quota_limit: Option<u64>,
}

impl RateLimitHeaders {
    /// Parse rate-limit headers from a `reqwest::Response`.
    pub fn from_response(response: &reqwest::Response) -> Self {
        let headers = response.headers();
        fn parse(h: &reqwest::header::HeaderMap, name: &str) -> Option<u64> {
            h.get(name)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok())
        }
        Self {
            limit: parse(headers, "X-RateLimit-Limit"),
            remaining: parse(headers, "X-RateLimit-Remaining"),
            reset: parse(headers, "X-RateLimit-Reset"),
            quota_used: parse(headers, "X-Quota-Used"),
            quota_limit: parse(headers, "X-Quota-Limit"),
        }
    }
}

// ============================================================================
// Health & Status Types
// ============================================================================

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Overall health status
    pub healthy: bool,
    /// Service version
    pub version: Option<String>,
    /// Uptime in seconds
    pub uptime_seconds: Option<u64>,
    /// Git commit SHA baked into the binary at build time. Present since server v0.11.84.
    pub build_sha: Option<String>,
}

/// Readiness check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessResponse {
    /// Is the service ready to accept requests
    pub ready: bool,
    /// Component status details
    pub components: Option<HashMap<String, bool>>,
}

// ============================================================================
// Namespace Types
// ============================================================================

/// Namespace information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceInfo {
    /// Namespace name
    #[serde(alias = "namespace")]
    pub name: String,
    /// Number of vectors in the namespace
    #[serde(default)]
    pub vector_count: u64,
    /// Vector dimensions
    #[serde(alias = "dimension")]
    pub dimensions: Option<u32>,
    /// Index type used
    pub index_type: Option<String>,
    /// Whether the namespace was newly created (from PUT/configure response)
    #[serde(default)]
    pub created: Option<bool>,
}

/// List namespaces response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListNamespacesResponse {
    /// List of namespace names
    pub namespaces: Vec<String>,
}

// ============================================================================
// Vector Types
// ============================================================================

/// A vector with ID and optional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vector {
    /// Unique vector identifier
    pub id: String,
    /// Vector values (embeddings)
    pub values: Vec<f32>,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl Vector {
    /// Create a new vector with just ID and values
    pub fn new(id: impl Into<String>, values: Vec<f32>) -> Self {
        Self {
            id: id.into(),
            values,
            metadata: None,
        }
    }

    /// Create a new vector with metadata
    pub fn with_metadata(
        id: impl Into<String>,
        values: Vec<f32>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            id: id.into(),
            values,
            metadata: Some(metadata),
        }
    }
}

/// Upsert vectors request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertRequest {
    /// Vectors to upsert
    pub vectors: Vec<Vector>,
}

impl UpsertRequest {
    /// Create a new upsert request with a single vector
    pub fn single(vector: Vector) -> Self {
        Self {
            vectors: vec![vector],
        }
    }

    /// Create a new upsert request with multiple vectors
    pub fn batch(vectors: Vec<Vector>) -> Self {
        Self { vectors }
    }
}

/// Upsert response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertResponse {
    /// Number of vectors upserted
    pub upserted_count: u64,
}

/// Column-based upsert request (Turbopuffer-inspired)
///
/// This format is more efficient for bulk upserts as it avoids repeating
/// field names for each vector. All arrays must have equal length.
///
/// # Example
///
/// ```rust
/// use dakera_client::ColumnUpsertRequest;
/// use std::collections::HashMap;
///
/// let request = ColumnUpsertRequest::new(
///     vec!["id1".to_string(), "id2".to_string()],
///     vec![vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]],
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnUpsertRequest {
    /// Array of vector IDs (required)
    pub ids: Vec<String>,
    /// Array of vectors (required for vector namespaces)
    pub vectors: Vec<Vec<f32>>,
    /// Additional attributes as columns (optional)
    /// Each key is an attribute name, value is array of attribute values
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub attributes: HashMap<String, Vec<serde_json::Value>>,
    /// TTL in seconds for all vectors (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
    /// Expected dimension (optional, for validation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension: Option<usize>,
}

impl ColumnUpsertRequest {
    /// Create a new column upsert request
    pub fn new(ids: Vec<String>, vectors: Vec<Vec<f32>>) -> Self {
        Self {
            ids,
            vectors,
            attributes: HashMap::new(),
            ttl_seconds: None,
            dimension: None,
        }
    }

    /// Add an attribute column
    pub fn with_attribute(
        mut self,
        name: impl Into<String>,
        values: Vec<serde_json::Value>,
    ) -> Self {
        self.attributes.insert(name.into(), values);
        self
    }

    /// Set TTL for all vectors
    pub fn with_ttl(mut self, seconds: u64) -> Self {
        self.ttl_seconds = Some(seconds);
        self
    }

    /// Set expected dimension for validation
    pub fn with_dimension(mut self, dim: usize) -> Self {
        self.dimension = Some(dim);
        self
    }

    /// Get the number of vectors in this request
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Check if the request is empty
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

/// Delete vectors request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRequest {
    /// Vector IDs to delete
    pub ids: Vec<String>,
}

impl DeleteRequest {
    /// Create a delete request for a single ID
    pub fn single(id: impl Into<String>) -> Self {
        Self {
            ids: vec![id.into()],
        }
    }

    /// Create a delete request for multiple IDs
    pub fn batch(ids: Vec<String>) -> Self {
        Self { ids }
    }
}

/// Delete response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResponse {
    /// Number of vectors deleted
    pub deleted_count: u64,
}

// ============================================================================
// Query Types
// ============================================================================

/// Read consistency level for queries (Turbopuffer-inspired)
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReadConsistency {
    /// Always read from primary/leader node - guarantees latest data
    Strong,
    /// Read from any replica - may return slightly stale data but faster
    #[default]
    Eventual,
    /// Read from replicas within staleness bounds
    #[serde(rename = "bounded_staleness")]
    BoundedStaleness,
}

/// Configuration for bounded staleness reads
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StalenessConfig {
    /// Maximum acceptable staleness in milliseconds
    #[serde(default = "default_max_staleness_ms")]
    pub max_staleness_ms: u64,
}

fn default_max_staleness_ms() -> u64 {
    5000 // 5 seconds default
}

impl StalenessConfig {
    /// Create a new staleness config with specified max staleness
    pub fn new(max_staleness_ms: u64) -> Self {
        Self { max_staleness_ms }
    }
}

/// Distance metric for similarity search
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DistanceMetric {
    /// Cosine similarity (default)
    #[default]
    Cosine,
    /// Euclidean distance
    Euclidean,
    /// Dot product
    DotProduct,
}

/// Query request for vector similarity search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    /// Query vector
    pub vector: Vec<f32>,
    /// Number of results to return
    pub top_k: u32,
    /// Distance metric to use
    #[serde(default)]
    pub distance_metric: DistanceMetric,
    /// Optional filter expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
    /// Whether to include metadata in results
    #[serde(default = "default_true")]
    pub include_metadata: bool,
    /// Whether to include vector values in results
    #[serde(default)]
    pub include_vectors: bool,
    /// Read consistency level
    #[serde(default)]
    pub consistency: ReadConsistency,
    /// Staleness configuration for bounded staleness reads
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staleness_config: Option<StalenessConfig>,
}

fn default_true() -> bool {
    true
}

impl QueryRequest {
    /// Create a new query request
    pub fn new(vector: Vec<f32>, top_k: u32) -> Self {
        Self {
            vector,
            top_k,
            distance_metric: DistanceMetric::default(),
            filter: None,
            include_metadata: true,
            include_vectors: false,
            consistency: ReadConsistency::default(),
            staleness_config: None,
        }
    }

    /// Add a filter to the query
    pub fn with_filter(mut self, filter: serde_json::Value) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Set whether to include metadata
    pub fn include_metadata(mut self, include: bool) -> Self {
        self.include_metadata = include;
        self
    }

    /// Set whether to include vector values
    pub fn include_vectors(mut self, include: bool) -> Self {
        self.include_vectors = include;
        self
    }

    /// Set distance metric
    pub fn with_distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Set read consistency level
    pub fn with_consistency(mut self, consistency: ReadConsistency) -> Self {
        self.consistency = consistency;
        self
    }

    /// Set bounded staleness with max staleness in ms
    pub fn with_bounded_staleness(mut self, max_staleness_ms: u64) -> Self {
        self.consistency = ReadConsistency::BoundedStaleness;
        self.staleness_config = Some(StalenessConfig::new(max_staleness_ms));
        self
    }

    /// Use strong consistency (always read from primary)
    pub fn with_strong_consistency(mut self) -> Self {
        self.consistency = ReadConsistency::Strong;
        self
    }
}

/// A match result from a query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Match {
    /// Vector ID
    pub id: String,
    /// Similarity score
    pub score: f32,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Query response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResponse {
    /// Search results
    #[serde(alias = "matches")]
    pub results: Vec<Match>,
}

// ============================================================================
// Full-Text Search Types
// ============================================================================

/// A document for full-text indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Document ID
    pub id: String,
    /// Document text content
    pub text: String,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl Document {
    /// Create a new document
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            metadata: None,
        }
    }

    /// Create a new document with metadata
    pub fn with_metadata(
        id: impl Into<String>,
        text: impl Into<String>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            metadata: Some(metadata),
        }
    }
}

/// Index documents request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDocumentsRequest {
    /// Documents to index
    pub documents: Vec<Document>,
}

/// Index documents response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDocumentsResponse {
    /// Number of documents indexed
    pub indexed_count: u64,
}

/// Full-text search request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullTextSearchRequest {
    /// Search query
    pub query: String,
    /// Maximum number of results
    pub top_k: u32,
    /// Optional filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
}

impl FullTextSearchRequest {
    /// Create a new full-text search request
    pub fn new(query: impl Into<String>, top_k: u32) -> Self {
        Self {
            query: query.into(),
            top_k,
            filter: None,
        }
    }

    /// Add a filter to the search
    pub fn with_filter(mut self, filter: serde_json::Value) -> Self {
        self.filter = Some(filter);
        self
    }
}

/// Full-text search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullTextMatch {
    /// Document ID
    pub id: String,
    /// BM25 score
    pub score: f32,
    /// Document text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Full-text search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullTextSearchResponse {
    /// Matched documents
    #[serde(alias = "matches")]
    pub results: Vec<FullTextMatch>,
}

/// Full-text index statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullTextStats {
    /// Number of documents indexed
    pub document_count: u64,
    /// Number of unique terms
    pub term_count: u64,
}

// ============================================================================
// Hybrid Search Types
// ============================================================================

/// Hybrid search request combining vector and full-text search.
///
/// When `vector` is `None` the server falls back to BM25-only full-text search.
/// When provided, results are blended with vector similarity according to `vector_weight`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchRequest {
    /// Optional query vector. Omit for BM25-only search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector: Option<Vec<f32>>,
    /// Text query
    pub text: String,
    /// Number of results to return
    pub top_k: u32,
    /// Weight for vector search (0.0-1.0)
    #[serde(default = "default_vector_weight")]
    pub vector_weight: f32,
    /// Optional filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
}

fn default_vector_weight() -> f32 {
    0.5
}

impl HybridSearchRequest {
    /// Create a new hybrid search request with a query vector (hybrid mode).
    pub fn new(vector: Vec<f32>, text: impl Into<String>, top_k: u32) -> Self {
        Self {
            vector: Some(vector),
            text: text.into(),
            top_k,
            vector_weight: 0.5,
            filter: None,
        }
    }

    /// Create a BM25-only full-text search request (no vector required).
    pub fn text_only(text: impl Into<String>, top_k: u32) -> Self {
        Self {
            vector: None,
            text: text.into(),
            top_k,
            vector_weight: 0.5,
            filter: None,
        }
    }

    /// Set the vector weight (text weight is 1.0 - vector_weight)
    pub fn with_vector_weight(mut self, weight: f32) -> Self {
        self.vector_weight = weight.clamp(0.0, 1.0);
        self
    }

    /// Add a filter to the search
    pub fn with_filter(mut self, filter: serde_json::Value) -> Self {
        self.filter = Some(filter);
        self
    }
}

/// Hybrid search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchResponse {
    /// Matched results
    #[serde(alias = "matches")]
    pub results: Vec<Match>,
}

// ============================================================================
// Operations Types
// ============================================================================

/// System diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDiagnostics {
    /// System information
    pub system: SystemInfo,
    /// Resource usage
    pub resources: ResourceUsage,
    /// Component health
    pub components: ComponentHealth,
    /// Number of active jobs
    pub active_jobs: u64,
}

/// System information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// Dakera version
    pub version: String,
    /// Rust version
    pub rust_version: String,
    /// Uptime in seconds
    pub uptime_seconds: u64,
    /// Process ID
    pub pid: u32,
}

/// Resource usage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Memory usage in bytes
    pub memory_bytes: u64,
    /// Thread count
    pub thread_count: u64,
    /// Open file descriptors
    pub open_fds: u64,
    /// CPU usage percentage
    pub cpu_percent: Option<f64>,
}

/// Component health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Storage health
    pub storage: HealthStatus,
    /// Search engine health
    pub search_engine: HealthStatus,
    /// Cache health
    pub cache: HealthStatus,
    /// gRPC health
    pub grpc: HealthStatus,
}

/// Health status for a component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Is the component healthy
    pub healthy: bool,
    /// Status message
    pub message: String,
    /// Last check timestamp
    pub last_check: u64,
}

/// Background job information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfo {
    /// Job ID
    pub id: String,
    /// Job type
    pub job_type: String,
    /// Current status
    pub status: String,
    /// Creation timestamp
    pub created_at: u64,
    /// Start timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
    /// Completion timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
    /// Progress percentage
    pub progress: u8,
    /// Status message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Job metadata
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

/// Compaction request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionRequest {
    /// Namespace to compact (None = all)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Force compaction
    #[serde(default)]
    pub force: bool,
}

/// Compaction response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionResponse {
    /// Job ID for tracking
    pub job_id: String,
    /// Status message
    pub message: String,
}

// ============================================================================
// Cache Warming Types (Turbopuffer-inspired)
// ============================================================================

/// Priority level for cache warming operations
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WarmingPriority {
    /// Highest priority - warm immediately, preempt other operations
    Critical,
    /// High priority - warm soon
    High,
    /// Normal priority (default)
    #[default]
    Normal,
    /// Low priority - warm when resources available
    Low,
    /// Background priority - warm during idle time only
    Background,
}

/// Target cache tier for warming
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WarmingTargetTier {
    /// L1 in-memory cache (Moka) - fastest, limited size
    L1,
    /// L2 local disk cache (RocksDB) - larger, persistent
    #[default]
    L2,
    /// Both L1 and L2 caches
    Both,
}

/// Access pattern hint for cache optimization
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccessPatternHint {
    /// Random access pattern
    #[default]
    Random,
    /// Sequential access pattern
    Sequential,
    /// Temporal locality (recently accessed items accessed again)
    Temporal,
    /// Spatial locality (nearby items accessed together)
    Spatial,
}

/// Cache warming request with priority hints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmCacheRequest {
    /// Namespace to warm
    pub namespace: String,
    /// Specific vector IDs to warm (None = all)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_ids: Option<Vec<String>>,
    /// Warming priority level
    #[serde(default)]
    pub priority: WarmingPriority,
    /// Target cache tier
    #[serde(default)]
    pub target_tier: WarmingTargetTier,
    /// Run warming in background (non-blocking)
    #[serde(default)]
    pub background: bool,
    /// TTL hint in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_hint_seconds: Option<u64>,
    /// Access pattern hint for optimization
    #[serde(default)]
    pub access_pattern: AccessPatternHint,
    /// Maximum vectors to warm
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_vectors: Option<usize>,
}

impl WarmCacheRequest {
    /// Create a new cache warming request for a namespace
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            vector_ids: None,
            priority: WarmingPriority::default(),
            target_tier: WarmingTargetTier::default(),
            background: false,
            ttl_hint_seconds: None,
            access_pattern: AccessPatternHint::default(),
            max_vectors: None,
        }
    }

    /// Warm specific vector IDs
    pub fn with_vector_ids(mut self, ids: Vec<String>) -> Self {
        self.vector_ids = Some(ids);
        self
    }

    /// Set warming priority
    pub fn with_priority(mut self, priority: WarmingPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set target cache tier
    pub fn with_target_tier(mut self, tier: WarmingTargetTier) -> Self {
        self.target_tier = tier;
        self
    }

    /// Run warming in background
    pub fn in_background(mut self) -> Self {
        self.background = true;
        self
    }

    /// Set TTL hint
    pub fn with_ttl(mut self, seconds: u64) -> Self {
        self.ttl_hint_seconds = Some(seconds);
        self
    }

    /// Set access pattern hint
    pub fn with_access_pattern(mut self, pattern: AccessPatternHint) -> Self {
        self.access_pattern = pattern;
        self
    }

    /// Limit number of vectors to warm
    pub fn with_max_vectors(mut self, max: usize) -> Self {
        self.max_vectors = Some(max);
        self
    }
}

/// Cache warming response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmCacheResponse {
    /// Operation success
    pub success: bool,
    /// Number of entries warmed
    pub entries_warmed: u64,
    /// Number of entries already warm (skipped)
    pub entries_skipped: u64,
    /// Job ID for tracking background operations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    /// Status message
    pub message: String,
    /// Estimated completion time for background jobs (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_completion: Option<String>,
    /// Target tier that was warmed
    pub target_tier: WarmingTargetTier,
    /// Priority that was used
    pub priority: WarmingPriority,
    /// Bytes warmed (approximate)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_warmed: Option<u64>,
}

// ============================================================================
// Export Types (Turbopuffer-inspired)
// ============================================================================

/// Request to export vectors from a namespace with pagination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRequest {
    /// Maximum number of vectors to return per page (default: 1000, max: 10000)
    #[serde(default = "default_export_top_k")]
    pub top_k: usize,
    /// Cursor for pagination - the last vector ID from previous page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Whether to include vector values in the response (default: true)
    #[serde(default = "default_true")]
    pub include_vectors: bool,
    /// Whether to include metadata in the response (default: true)
    #[serde(default = "default_true")]
    pub include_metadata: bool,
}

fn default_export_top_k() -> usize {
    1000
}

impl Default for ExportRequest {
    fn default() -> Self {
        Self {
            top_k: 1000,
            cursor: None,
            include_vectors: true,
            include_metadata: true,
        }
    }
}

impl ExportRequest {
    /// Create a new export request with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of vectors to return per page
    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k = top_k;
        self
    }

    /// Set the pagination cursor
    pub fn with_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Set whether to include vector values
    pub fn include_vectors(mut self, include: bool) -> Self {
        self.include_vectors = include;
        self
    }

    /// Set whether to include metadata
    pub fn include_metadata(mut self, include: bool) -> Self {
        self.include_metadata = include;
        self
    }
}

/// A single exported vector record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedVector {
    /// Vector ID
    pub id: String,
    /// Vector values (optional based on include_vectors)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<f32>>,
    /// Metadata (optional based on include_metadata)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// TTL in seconds if set
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
}

/// Response from export operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResponse {
    /// Exported vectors for this page
    pub vectors: Vec<ExportedVector>,
    /// Cursor for next page (None if this is the last page)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Total vectors in namespace (for progress tracking)
    pub total_count: usize,
    /// Number of vectors returned in this page
    pub returned_count: usize,
}

// ============================================================================
// Batch Query Types
// ============================================================================

/// A single query within a batch request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchQueryItem {
    /// Unique identifier for this query within the batch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The query vector
    pub vector: Vec<f32>,
    /// Number of results to return
    #[serde(default = "default_batch_top_k")]
    pub top_k: u32,
    /// Optional filter expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
    /// Whether to include metadata in results
    #[serde(default)]
    pub include_metadata: bool,
    /// Read consistency level
    #[serde(default)]
    pub consistency: ReadConsistency,
    /// Staleness configuration for bounded staleness reads
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staleness_config: Option<StalenessConfig>,
}

fn default_batch_top_k() -> u32 {
    10
}

impl BatchQueryItem {
    /// Create a new batch query item
    pub fn new(vector: Vec<f32>, top_k: u32) -> Self {
        Self {
            id: None,
            vector,
            top_k,
            filter: None,
            include_metadata: true,
            consistency: ReadConsistency::default(),
            staleness_config: None,
        }
    }

    /// Set a unique identifier for this query
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Add a filter to the query
    pub fn with_filter(mut self, filter: serde_json::Value) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Set whether to include metadata
    pub fn include_metadata(mut self, include: bool) -> Self {
        self.include_metadata = include;
        self
    }

    /// Set read consistency level
    pub fn with_consistency(mut self, consistency: ReadConsistency) -> Self {
        self.consistency = consistency;
        self
    }

    /// Set bounded staleness with max staleness in ms
    pub fn with_bounded_staleness(mut self, max_staleness_ms: u64) -> Self {
        self.consistency = ReadConsistency::BoundedStaleness;
        self.staleness_config = Some(StalenessConfig::new(max_staleness_ms));
        self
    }
}

/// Batch query request - execute multiple queries in parallel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchQueryRequest {
    /// List of queries to execute
    pub queries: Vec<BatchQueryItem>,
}

impl BatchQueryRequest {
    /// Create a new batch query request
    pub fn new(queries: Vec<BatchQueryItem>) -> Self {
        Self { queries }
    }

    /// Create a batch query request from a single query
    pub fn single(query: BatchQueryItem) -> Self {
        Self {
            queries: vec![query],
        }
    }
}

/// Results for a single query within a batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchQueryResult {
    /// The query identifier (if provided in request)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Query results (empty if an error occurred)
    pub results: Vec<Match>,
    /// Query execution time in milliseconds
    pub latency_ms: f64,
    /// Error message if this individual query failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Batch query response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchQueryResponse {
    /// Results for each query in the batch
    pub results: Vec<BatchQueryResult>,
    /// Total execution time in milliseconds
    pub total_latency_ms: f64,
    /// Number of queries executed
    pub query_count: usize,
}

// ============================================================================
// Multi-Vector Search Types
// ============================================================================

/// Request for multi-vector search with positive and negative vectors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiVectorSearchRequest {
    /// Positive vectors to search towards (required, at least one)
    pub positive_vectors: Vec<Vec<f32>>,
    /// Weights for positive vectors (optional, defaults to equal weights)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub positive_weights: Option<Vec<f32>>,
    /// Negative vectors to search away from (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negative_vectors: Option<Vec<Vec<f32>>>,
    /// Weights for negative vectors (optional, defaults to equal weights)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negative_weights: Option<Vec<f32>>,
    /// Number of results to return
    #[serde(default = "default_multi_vector_top_k")]
    pub top_k: u32,
    /// Distance metric to use
    #[serde(default)]
    pub distance_metric: DistanceMetric,
    /// Minimum score threshold
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score_threshold: Option<f32>,
    /// Enable MMR (Maximal Marginal Relevance) for diversity
    #[serde(default)]
    pub enable_mmr: bool,
    /// Lambda parameter for MMR (0 = max diversity, 1 = max relevance)
    #[serde(default = "default_mmr_lambda")]
    pub mmr_lambda: f32,
    /// Include metadata in results
    #[serde(default = "default_true")]
    pub include_metadata: bool,
    /// Include vectors in results
    #[serde(default)]
    pub include_vectors: bool,
    /// Optional metadata filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
    /// Read consistency level
    #[serde(default)]
    pub consistency: ReadConsistency,
    /// Staleness configuration for bounded staleness reads
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staleness_config: Option<StalenessConfig>,
}

fn default_multi_vector_top_k() -> u32 {
    10
}

fn default_mmr_lambda() -> f32 {
    0.5
}

impl MultiVectorSearchRequest {
    /// Create a new multi-vector search request with positive vectors
    pub fn new(positive_vectors: Vec<Vec<f32>>) -> Self {
        Self {
            positive_vectors,
            positive_weights: None,
            negative_vectors: None,
            negative_weights: None,
            top_k: 10,
            distance_metric: DistanceMetric::default(),
            score_threshold: None,
            enable_mmr: false,
            mmr_lambda: 0.5,
            include_metadata: true,
            include_vectors: false,
            filter: None,
            consistency: ReadConsistency::default(),
            staleness_config: None,
        }
    }

    /// Set the number of results to return
    pub fn with_top_k(mut self, top_k: u32) -> Self {
        self.top_k = top_k;
        self
    }

    /// Add weights for positive vectors
    pub fn with_positive_weights(mut self, weights: Vec<f32>) -> Self {
        self.positive_weights = Some(weights);
        self
    }

    /// Add negative vectors to search away from
    pub fn with_negative_vectors(mut self, vectors: Vec<Vec<f32>>) -> Self {
        self.negative_vectors = Some(vectors);
        self
    }

    /// Add weights for negative vectors
    pub fn with_negative_weights(mut self, weights: Vec<f32>) -> Self {
        self.negative_weights = Some(weights);
        self
    }

    /// Set distance metric
    pub fn with_distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Set minimum score threshold
    pub fn with_score_threshold(mut self, threshold: f32) -> Self {
        self.score_threshold = Some(threshold);
        self
    }

    /// Enable MMR for diversity
    pub fn with_mmr(mut self, lambda: f32) -> Self {
        self.enable_mmr = true;
        self.mmr_lambda = lambda.clamp(0.0, 1.0);
        self
    }

    /// Set whether to include metadata
    pub fn include_metadata(mut self, include: bool) -> Self {
        self.include_metadata = include;
        self
    }

    /// Set whether to include vectors
    pub fn include_vectors(mut self, include: bool) -> Self {
        self.include_vectors = include;
        self
    }

    /// Add a filter
    pub fn with_filter(mut self, filter: serde_json::Value) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Set read consistency level
    pub fn with_consistency(mut self, consistency: ReadConsistency) -> Self {
        self.consistency = consistency;
        self
    }
}

/// Single result from multi-vector search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiVectorSearchResult {
    /// Vector ID
    pub id: String,
    /// Similarity score
    pub score: f32,
    /// MMR score (if MMR enabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mmr_score: Option<f32>,
    /// Original rank before reranking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_rank: Option<usize>,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    /// Optional vector values
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector: Option<Vec<f32>>,
}

/// Response from multi-vector search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiVectorSearchResponse {
    /// Search results
    pub results: Vec<MultiVectorSearchResult>,
    /// The computed query vector (weighted combination of positive - negative)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed_query_vector: Option<Vec<f32>>,
}

// ============================================================================
// Aggregation Types (Turbopuffer-inspired)
// ============================================================================

/// Aggregate function for computing values across documents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AggregateFunction {
    /// Count matching documents
    Count,
    /// Sum numeric attribute values
    Sum { field: String },
    /// Average numeric attribute values
    Avg { field: String },
    /// Minimum numeric attribute value
    Min { field: String },
    /// Maximum numeric attribute value
    Max { field: String },
}

/// Request for aggregation query (Turbopuffer-inspired)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationRequest {
    /// Named aggregations to compute
    /// Example: {"my_count": ["Count"], "total_score": ["Sum", "score"]}
    pub aggregate_by: HashMap<String, serde_json::Value>,
    /// Fields to group results by (optional)
    /// Example: ["category", "status"]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group_by: Vec<String>,
    /// Filter to apply before aggregation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
    /// Maximum number of groups to return (default: 100)
    #[serde(default = "default_agg_limit")]
    pub limit: usize,
}

fn default_agg_limit() -> usize {
    100
}

impl AggregationRequest {
    /// Create a new aggregation request with a single aggregation
    pub fn new() -> Self {
        Self {
            aggregate_by: HashMap::new(),
            group_by: Vec::new(),
            filter: None,
            limit: 100,
        }
    }

    /// Add a count aggregation
    pub fn with_count(mut self, name: impl Into<String>) -> Self {
        self.aggregate_by
            .insert(name.into(), serde_json::json!(["Count"]));
        self
    }

    /// Add a sum aggregation
    pub fn with_sum(mut self, name: impl Into<String>, field: impl Into<String>) -> Self {
        self.aggregate_by
            .insert(name.into(), serde_json::json!(["Sum", field.into()]));
        self
    }

    /// Add an average aggregation
    pub fn with_avg(mut self, name: impl Into<String>, field: impl Into<String>) -> Self {
        self.aggregate_by
            .insert(name.into(), serde_json::json!(["Avg", field.into()]));
        self
    }

    /// Add a min aggregation
    pub fn with_min(mut self, name: impl Into<String>, field: impl Into<String>) -> Self {
        self.aggregate_by
            .insert(name.into(), serde_json::json!(["Min", field.into()]));
        self
    }

    /// Add a max aggregation
    pub fn with_max(mut self, name: impl Into<String>, field: impl Into<String>) -> Self {
        self.aggregate_by
            .insert(name.into(), serde_json::json!(["Max", field.into()]));
        self
    }

    /// Set group by fields
    pub fn group_by(mut self, fields: Vec<String>) -> Self {
        self.group_by = fields;
        self
    }

    /// Add a single group by field
    pub fn with_group_by(mut self, field: impl Into<String>) -> Self {
        self.group_by.push(field.into());
        self
    }

    /// Set filter for aggregation
    pub fn with_filter(mut self, filter: serde_json::Value) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Set maximum number of groups to return
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

impl Default for AggregationRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// Response for aggregation query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationResponse {
    /// Aggregation results (without grouping)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregations: Option<HashMap<String, serde_json::Value>>,
    /// Grouped aggregation results (with group_by)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregation_groups: Option<Vec<AggregationGroup>>,
}

/// Single group in aggregation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationGroup {
    /// Group key values (flattened into object)
    #[serde(flatten)]
    pub group_key: HashMap<String, serde_json::Value>,
    /// Aggregation results for this group
    #[serde(flatten)]
    pub aggregations: HashMap<String, serde_json::Value>,
}

// ============================================================================
// Unified Query Types (Turbopuffer-inspired)
// ============================================================================

/// Vector search method for unified query
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum VectorSearchMethod {
    /// Approximate Nearest Neighbor (fast, default)
    #[default]
    ANN,
    /// Exact k-Nearest Neighbor (exhaustive, requires filters)
    #[serde(rename = "kNN")]
    KNN,
}

/// Sort direction for attribute ordering
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    /// Ascending order
    Asc,
    /// Descending order
    #[default]
    Desc,
}

/// Ranking function for unified query API
/// Supports vector search (ANN/kNN), full-text BM25, and attribute ordering
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RankBy {
    /// Vector search: uses field, method, and query_vector
    VectorSearch {
        field: String,
        method: VectorSearchMethod,
        query_vector: Vec<f32>,
    },
    /// Full-text BM25 search
    FullTextSearch {
        field: String,
        method: String, // Always "BM25"
        query: String,
    },
    /// Attribute ordering
    AttributeOrder {
        field: String,
        direction: SortDirection,
    },
    /// Sum of multiple ranking functions
    Sum(Vec<RankBy>),
    /// Max of multiple ranking functions
    Max(Vec<RankBy>),
    /// Product with weight
    Product { weight: f32, ranking: Box<RankBy> },
}

impl RankBy {
    /// Create a vector search ranking using ANN
    pub fn vector_ann(field: impl Into<String>, query_vector: Vec<f32>) -> Self {
        RankBy::VectorSearch {
            field: field.into(),
            method: VectorSearchMethod::ANN,
            query_vector,
        }
    }

    /// Create a vector search ranking using ANN on the default "vector" field
    pub fn ann(query_vector: Vec<f32>) -> Self {
        Self::vector_ann("vector", query_vector)
    }

    /// Create a vector search ranking using exact kNN
    pub fn vector_knn(field: impl Into<String>, query_vector: Vec<f32>) -> Self {
        RankBy::VectorSearch {
            field: field.into(),
            method: VectorSearchMethod::KNN,
            query_vector,
        }
    }

    /// Create a vector search ranking using kNN on the default "vector" field
    pub fn knn(query_vector: Vec<f32>) -> Self {
        Self::vector_knn("vector", query_vector)
    }

    /// Create a BM25 full-text search ranking
    pub fn bm25(field: impl Into<String>, query: impl Into<String>) -> Self {
        RankBy::FullTextSearch {
            field: field.into(),
            method: "BM25".to_string(),
            query: query.into(),
        }
    }

    /// Create an attribute ordering ranking (ascending)
    pub fn asc(field: impl Into<String>) -> Self {
        RankBy::AttributeOrder {
            field: field.into(),
            direction: SortDirection::Asc,
        }
    }

    /// Create an attribute ordering ranking (descending)
    pub fn desc(field: impl Into<String>) -> Self {
        RankBy::AttributeOrder {
            field: field.into(),
            direction: SortDirection::Desc,
        }
    }

    /// Sum multiple ranking functions together
    pub fn sum(rankings: Vec<RankBy>) -> Self {
        RankBy::Sum(rankings)
    }

    /// Take the max of multiple ranking functions
    pub fn max(rankings: Vec<RankBy>) -> Self {
        RankBy::Max(rankings)
    }

    /// Apply a weight multiplier to a ranking function
    pub fn product(weight: f32, ranking: RankBy) -> Self {
        RankBy::Product {
            weight,
            ranking: Box::new(ranking),
        }
    }
}

/// Unified query request with flexible ranking options (Turbopuffer-inspired)
///
/// # Example
///
/// ```rust
/// use dakera_client::UnifiedQueryRequest;
///
/// // Vector ANN search
/// let request = UnifiedQueryRequest::vector_search(vec![0.1, 0.2, 0.3], 10);
///
/// // Full-text BM25 search
/// let request = UnifiedQueryRequest::fulltext_search("content", "hello world", 10);
///
/// // Custom rank_by with filters
/// let request = UnifiedQueryRequest::vector_search(vec![0.1, 0.2, 0.3], 10)
///     .with_filter(serde_json::json!({"category": {"$eq": "science"}}));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedQueryRequest {
    /// How to rank documents (required)
    pub rank_by: serde_json::Value,
    /// Number of results to return
    #[serde(default = "default_unified_top_k")]
    pub top_k: usize,
    /// Optional metadata filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
    /// Include metadata in results
    #[serde(default = "default_true")]
    pub include_metadata: bool,
    /// Include vectors in results
    #[serde(default)]
    pub include_vectors: bool,
    /// Distance metric for vector search (default: cosine)
    #[serde(default)]
    pub distance_metric: DistanceMetric,
}

fn default_unified_top_k() -> usize {
    10
}

impl UnifiedQueryRequest {
    /// Create a new unified query request with vector ANN search
    pub fn vector_search(query_vector: Vec<f32>, top_k: usize) -> Self {
        Self {
            rank_by: serde_json::json!(["ANN", query_vector]),
            top_k,
            filter: None,
            include_metadata: true,
            include_vectors: false,
            distance_metric: DistanceMetric::default(),
        }
    }

    /// Create a new unified query request with vector kNN search
    pub fn vector_knn_search(query_vector: Vec<f32>, top_k: usize) -> Self {
        Self {
            rank_by: serde_json::json!(["kNN", query_vector]),
            top_k,
            filter: None,
            include_metadata: true,
            include_vectors: false,
            distance_metric: DistanceMetric::default(),
        }
    }

    /// Create a new unified query request with full-text BM25 search
    pub fn fulltext_search(
        field: impl Into<String>,
        query: impl Into<String>,
        top_k: usize,
    ) -> Self {
        Self {
            rank_by: serde_json::json!([field.into(), "BM25", query.into()]),
            top_k,
            filter: None,
            include_metadata: true,
            include_vectors: false,
            distance_metric: DistanceMetric::default(),
        }
    }

    /// Create a new unified query request with attribute ordering
    pub fn attribute_order(
        field: impl Into<String>,
        direction: SortDirection,
        top_k: usize,
    ) -> Self {
        let dir = match direction {
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        };
        Self {
            rank_by: serde_json::json!([field.into(), dir]),
            top_k,
            filter: None,
            include_metadata: true,
            include_vectors: false,
            distance_metric: DistanceMetric::default(),
        }
    }

    /// Create a unified query with a raw rank_by JSON value
    pub fn with_rank_by(rank_by: serde_json::Value, top_k: usize) -> Self {
        Self {
            rank_by,
            top_k,
            filter: None,
            include_metadata: true,
            include_vectors: false,
            distance_metric: DistanceMetric::default(),
        }
    }

    /// Add a filter to the query
    pub fn with_filter(mut self, filter: serde_json::Value) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Set whether to include metadata
    pub fn include_metadata(mut self, include: bool) -> Self {
        self.include_metadata = include;
        self
    }

    /// Set whether to include vector values
    pub fn include_vectors(mut self, include: bool) -> Self {
        self.include_vectors = include;
        self
    }

    /// Set the distance metric
    pub fn with_distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Set the number of results to return
    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k = top_k;
        self
    }
}

/// Single result from unified query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedSearchResult {
    /// Vector/document ID
    pub id: String,
    /// Ranking score (distance for vector search, BM25 score for text)
    /// Named $dist for Turbopuffer compatibility
    #[serde(rename = "$dist", skip_serializing_if = "Option::is_none")]
    pub dist: Option<f32>,
    /// Metadata if requested
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Vector values if requested
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector: Option<Vec<f32>>,
}

/// Unified query response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedQueryResponse {
    /// Search results ordered by rank_by score
    pub results: Vec<UnifiedSearchResult>,
    /// Cursor for pagination (if more results available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

// ============================================================================
// Query Explain Types
// ============================================================================

fn default_explain_top_k() -> usize {
    10
}

/// Query type for explain
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ExplainQueryType {
    /// Vector similarity search
    #[default]
    VectorSearch,
    /// Full-text search
    FullTextSearch,
    /// Hybrid search combining vector and text
    HybridSearch,
    /// Multi-vector search with positive/negative vectors
    MultiVector,
    /// Batch query execution
    BatchQuery,
}

/// Query explain request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryExplainRequest {
    /// Type of query to explain
    #[serde(default)]
    pub query_type: ExplainQueryType,
    /// Query vector (for vector searches)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector: Option<Vec<f32>>,
    /// Number of results to return
    #[serde(default = "default_explain_top_k")]
    pub top_k: usize,
    /// Optional metadata filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
    /// Optional text query for hybrid/fulltext search
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_query: Option<String>,
    /// Distance metric
    #[serde(default = "default_distance_metric")]
    pub distance_metric: String,
    /// Whether to actually execute the query for actual stats
    #[serde(default)]
    pub execute: bool,
    /// Include verbose output
    #[serde(default)]
    pub verbose: bool,
}

fn default_distance_metric() -> String {
    "cosine".to_string()
}

impl QueryExplainRequest {
    /// Create a new explain request for a vector search
    pub fn vector_search(vector: Vec<f32>, top_k: usize) -> Self {
        Self {
            query_type: ExplainQueryType::VectorSearch,
            vector: Some(vector),
            top_k,
            filter: None,
            text_query: None,
            distance_metric: "cosine".to_string(),
            execute: false,
            verbose: false,
        }
    }

    /// Create a new explain request for a full-text search
    pub fn fulltext_search(text_query: impl Into<String>, top_k: usize) -> Self {
        Self {
            query_type: ExplainQueryType::FullTextSearch,
            vector: None,
            top_k,
            filter: None,
            text_query: Some(text_query.into()),
            distance_metric: "bm25".to_string(),
            execute: false,
            verbose: false,
        }
    }

    /// Create a new explain request for a hybrid search
    pub fn hybrid_search(vector: Vec<f32>, text_query: impl Into<String>, top_k: usize) -> Self {
        Self {
            query_type: ExplainQueryType::HybridSearch,
            vector: Some(vector),
            top_k,
            filter: None,
            text_query: Some(text_query.into()),
            distance_metric: "hybrid".to_string(),
            execute: false,
            verbose: false,
        }
    }

    /// Add a filter to the explain request
    pub fn with_filter(mut self, filter: serde_json::Value) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Set the distance metric
    pub fn with_distance_metric(mut self, metric: impl Into<String>) -> Self {
        self.distance_metric = metric.into();
        self
    }

    /// Execute the query to get actual stats
    pub fn with_execution(mut self) -> Self {
        self.execute = true;
        self
    }

    /// Enable verbose output
    pub fn with_verbose(mut self) -> Self {
        self.verbose = true;
        self
    }
}

/// A stage in query execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStage {
    /// Stage name
    pub name: String,
    /// Stage description
    pub description: String,
    /// Stage order (1-based)
    pub order: u32,
    /// Estimated input rows
    pub estimated_input: u64,
    /// Estimated output rows
    pub estimated_output: u64,
    /// Estimated cost for this stage
    pub estimated_cost: f64,
    /// Stage-specific details
    #[serde(default)]
    pub details: HashMap<String, serde_json::Value>,
}

/// Cost estimation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    /// Total estimated cost (abstract units)
    pub total_cost: f64,
    /// Estimated execution time in milliseconds
    pub estimated_time_ms: u64,
    /// Estimated memory usage in bytes
    pub estimated_memory_bytes: u64,
    /// Estimated I/O operations
    pub estimated_io_ops: u64,
    /// Cost breakdown by component
    #[serde(default)]
    pub cost_breakdown: HashMap<String, f64>,
    /// Confidence level (0.0-1.0)
    pub confidence: f64,
}

/// Actual execution statistics (when execute=true)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActualStats {
    /// Actual execution time in milliseconds
    pub execution_time_ms: u64,
    /// Actual results returned
    pub results_returned: usize,
    /// Vectors scanned
    pub vectors_scanned: u64,
    /// Vectors after filter
    pub vectors_after_filter: u64,
    /// Index lookups performed
    pub index_lookups: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// Memory used in bytes
    pub memory_used_bytes: u64,
}

/// Performance recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    /// Recommendation type
    pub recommendation_type: String,
    /// Priority (high, medium, low)
    pub priority: String,
    /// Recommendation description
    pub description: String,
    /// Expected improvement
    pub expected_improvement: String,
    /// How to implement
    pub implementation: String,
}

/// Index selection details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexSelection {
    /// Index type that will be used
    pub index_type: String,
    /// Why this index was selected
    pub selection_reason: String,
    /// Alternative indexes considered
    #[serde(default)]
    pub alternatives_considered: Vec<IndexAlternative>,
    /// Index configuration
    #[serde(default)]
    pub index_config: HashMap<String, serde_json::Value>,
    /// Index statistics
    pub index_stats: IndexStatistics,
}

/// Alternative index that was considered
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexAlternative {
    /// Index type
    pub index_type: String,
    /// Why it wasn't selected
    pub rejection_reason: String,
    /// Estimated cost if this index was used
    pub estimated_cost: f64,
}

/// Index statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatistics {
    /// Total vectors in index
    pub vector_count: u64,
    /// Vector dimension
    pub dimension: usize,
    /// Index memory usage (estimated)
    pub memory_bytes: u64,
    /// Index build time (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_time_ms: Option<u64>,
    /// Last updated timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<u64>,
}

/// Query parameters for reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryParams {
    /// Number of results requested
    pub top_k: usize,
    /// Whether a filter was applied
    pub has_filter: bool,
    /// Filter complexity level
    pub filter_complexity: String,
    /// Vector dimension (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_dimension: Option<usize>,
    /// Distance metric used
    pub distance_metric: String,
    /// Text query length (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_query_length: Option<usize>,
}

/// Query explain response - detailed execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryExplainResponse {
    /// Query type being explained
    pub query_type: ExplainQueryType,
    /// Namespace being queried
    pub namespace: String,
    /// Index selection information
    pub index_selection: IndexSelection,
    /// Query execution stages
    pub stages: Vec<ExecutionStage>,
    /// Cost estimates
    pub cost_estimate: CostEstimate,
    /// Actual execution stats (if execute=true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_stats: Option<ActualStats>,
    /// Performance recommendations
    #[serde(default)]
    pub recommendations: Vec<Recommendation>,
    /// Query plan summary
    pub summary: String,
    /// Raw query parameters
    pub query_params: QueryParams,
}

// ============================================================================
// Text Auto-Embedding Types
// ============================================================================

/// Supported embedding models for text-based operations.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum EmbeddingModel {
    /// BGE-large — Best quality, server default (1024 dimensions)
    #[default]
    BgeLarge,
    /// MiniLM-L6 — Fast, good quality (384 dimensions)
    Minilm,
    /// BGE-small — Balanced performance (384 dimensions)
    BgeSmall,
    /// E5-small — High quality (384 dimensions)
    E5Small,
}

/// A text document to upsert with automatic embedding generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDocument {
    /// Unique identifier for the document.
    pub id: String,
    /// Raw text content to be embedded.
    pub text: String,
    /// Optional metadata for the document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    /// Optional TTL in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
}

impl TextDocument {
    /// Create a new text document with the given ID and text.
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            metadata: None,
            ttl_seconds: None,
        }
    }

    /// Add metadata to this document.
    pub fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set a TTL on this document.
    pub fn with_ttl(mut self, ttl_seconds: u64) -> Self {
        self.ttl_seconds = Some(ttl_seconds);
        self
    }
}

/// Request to upsert text documents with automatic embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertTextRequest {
    /// Documents to upsert.
    pub documents: Vec<TextDocument>,
    /// Embedding model to use (default: minilm).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<EmbeddingModel>,
}

impl UpsertTextRequest {
    /// Create a new upsert-text request.
    pub fn new(documents: Vec<TextDocument>) -> Self {
        Self {
            documents,
            model: None,
        }
    }

    /// Set the embedding model.
    pub fn with_model(mut self, model: EmbeddingModel) -> Self {
        self.model = Some(model);
        self
    }
}

/// Response from a text upsert operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextUpsertResponse {
    /// Number of documents upserted.
    pub upserted_count: u64,
    /// Approximate number of tokens processed.
    pub tokens_processed: u64,
    /// Embedding model used.
    pub model: EmbeddingModel,
    /// Time spent generating embeddings in milliseconds.
    pub embedding_time_ms: u64,
}

/// A single text search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSearchResult {
    /// Document ID.
    pub id: String,
    /// Similarity score.
    pub score: f32,
    /// Original text (if `include_text` was true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Document metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    /// Vector values (if `include_vectors` was true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector: Option<Vec<f32>>,
}

/// Request to query using natural language text with automatic embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryTextRequest {
    /// Query text.
    pub text: String,
    /// Number of results to return.
    pub top_k: u32,
    /// Optional metadata filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
    /// Whether to include the original text in results.
    pub include_text: bool,
    /// Whether to include vectors in results.
    pub include_vectors: bool,
    /// Embedding model to use (default: minilm).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<EmbeddingModel>,
}

impl QueryTextRequest {
    /// Create a new text query request.
    pub fn new(text: impl Into<String>, top_k: u32) -> Self {
        Self {
            text: text.into(),
            top_k,
            filter: None,
            include_text: true,
            include_vectors: false,
            model: None,
        }
    }

    /// Add a metadata filter.
    pub fn with_filter(mut self, filter: serde_json::Value) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Set whether to include the original text in results.
    pub fn include_text(mut self, include: bool) -> Self {
        self.include_text = include;
        self
    }

    /// Set whether to include vectors in results.
    pub fn include_vectors(mut self, include: bool) -> Self {
        self.include_vectors = include;
        self
    }

    /// Set the embedding model.
    pub fn with_model(mut self, model: EmbeddingModel) -> Self {
        self.model = Some(model);
        self
    }
}

/// Response from a text query operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextQueryResponse {
    /// Search results.
    pub results: Vec<TextSearchResult>,
    /// Embedding model used.
    pub model: EmbeddingModel,
    /// Time spent generating the query embedding in milliseconds.
    pub embedding_time_ms: u64,
    /// Time spent searching in milliseconds.
    pub search_time_ms: u64,
}

/// Request to execute multiple text queries with automatic embedding in a single call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchQueryTextRequest {
    /// Text queries.
    pub queries: Vec<String>,
    /// Number of results per query.
    pub top_k: u32,
    /// Optional metadata filter applied to all queries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
    /// Whether to include vectors in results.
    pub include_vectors: bool,
    /// Embedding model to use (default: minilm).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<EmbeddingModel>,
}

impl BatchQueryTextRequest {
    /// Create a new batch text query request.
    pub fn new(queries: Vec<String>, top_k: u32) -> Self {
        Self {
            queries,
            top_k,
            filter: None,
            include_vectors: false,
            model: None,
        }
    }
}

/// Response from a batch text query operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchQueryTextResponse {
    /// Results for each query (in the same order as the request).
    pub results: Vec<Vec<TextSearchResult>>,
    /// Embedding model used.
    pub model: EmbeddingModel,
    /// Time spent generating all embeddings in milliseconds.
    pub embedding_time_ms: u64,
    /// Time spent on all searches in milliseconds.
    pub search_time_ms: u64,
}

// ============================================================================
// Fetch by ID Types
// ============================================================================

/// Request to fetch vectors by their IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchRequest {
    /// IDs of vectors to fetch.
    pub ids: Vec<String>,
    /// Whether to include vector values.
    pub include_values: bool,
    /// Whether to include metadata.
    pub include_metadata: bool,
}

impl FetchRequest {
    /// Create a new fetch request.
    pub fn new(ids: Vec<String>) -> Self {
        Self {
            ids,
            include_values: true,
            include_metadata: true,
        }
    }
}

/// Response from a fetch-by-ID operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResponse {
    /// Fetched vectors.
    pub vectors: Vec<Vector>,
}

// ============================================================================
// Namespace Management Types
// ============================================================================

/// Request to create a new namespace.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateNamespaceRequest {
    /// Vector dimensions (inferred from first upsert if omitted).
    #[serde(rename = "dimension", skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    /// Index type (e.g. "hnsw", "flat").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_type: Option<String>,
    /// Arbitrary namespace metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl CreateNamespaceRequest {
    /// Create a minimal request (server picks sensible defaults).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the vector dimensions.
    pub fn with_dimensions(mut self, dimensions: u32) -> Self {
        self.dimensions = Some(dimensions);
        self
    }

    /// Set the index type.
    pub fn with_index_type(mut self, index_type: impl Into<String>) -> Self {
        self.index_type = Some(index_type.into());
        self
    }
}

/// Request body for `PUT /v1/namespaces/:namespace` — upsert semantics (v0.6.0).
///
/// Creates the namespace if it does not exist, or updates its configuration
/// if it already exists.  Requires `Scope::Write`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigureNamespaceRequest {
    /// Vector dimension.  Required on first creation; must match on subsequent calls.
    pub dimension: usize,
    /// Distance metric (defaults to cosine when omitted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<DistanceMetric>,
}

impl ConfigureNamespaceRequest {
    /// Create a new configure-namespace request with the given dimension.
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            distance: None,
        }
    }

    /// Set the distance metric.
    pub fn with_distance(mut self, distance: DistanceMetric) -> Self {
        self.distance = Some(distance);
        self
    }
}

/// Response from `PUT /v1/namespaces/:namespace`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigureNamespaceResponse {
    /// Namespace name.
    pub namespace: String,
    /// Vector dimension.
    pub dimension: usize,
    /// Distance metric in use.
    pub distance: DistanceMetric,
    /// `true` if the namespace was newly created; `false` if it already existed.
    pub created: bool,
}

// ============================================================================
// Memory Knowledge Graph Types (CE-5 / SDK-9)
// ============================================================================

/// Edge type for memory knowledge graph relationships (CE-5).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    /// Cosine similarity ≥ 0.85 — two memories are semantically similar.
    RelatedTo,
    /// Both memories reference the same named entity (CE-4 tags).
    SharesEntity,
    /// Temporal ordering — source was created before target.
    Precedes,
    /// Explicit user/agent-created link.
    #[default]
    LinkedBy,
}

/// A directed edge in the memory knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Unique edge identifier.
    pub id: String,
    /// Source memory ID.
    pub source_id: String,
    /// Target memory ID.
    pub target_id: String,
    /// Relationship type between the two memories.
    pub edge_type: EdgeType,
    /// Edge weight (0.0–1.0). For `RelatedTo` this is the cosine similarity score.
    pub weight: f64,
    /// Unix timestamp of edge creation.
    pub created_at: i64,
}

/// A node (memory) in the knowledge graph traversal result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    /// Memory identifier.
    pub memory_id: String,
    /// First 200 characters of memory content.
    pub content_preview: String,
    /// Memory importance score.
    pub importance: f64,
    /// Traversal depth from the root node (root = 0).
    pub depth: u32,
}

/// Graph traversal result from `GET /v1/memories/{id}/graph`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryGraph {
    /// The root memory ID from which traversal started.
    pub root_id: String,
    /// Maximum traversal depth used.
    pub depth: u32,
    /// All memory nodes reachable within the requested depth.
    pub nodes: Vec<GraphNode>,
    /// All edges connecting the returned nodes.
    pub edges: Vec<GraphEdge>,
}

/// Shortest path between two memories from `GET /v1/memories/{id}/path`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPath {
    /// Starting memory ID.
    pub source_id: String,
    /// Destination memory ID.
    pub target_id: String,
    /// Ordered list of memory IDs from source to target (inclusive).
    pub path: Vec<String>,
    /// Number of edges traversed (`path.len() - 1`). `-1` if no path exists.
    pub hops: i32,
    /// Edges along the path, in traversal order.
    pub edges: Vec<GraphEdge>,
}

/// Request body for `POST /v1/memories/{id}/links`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphLinkRequest {
    /// Target memory ID to link to.
    pub target_id: String,
    /// Edge type — must be `LinkedBy` for explicit links.
    pub edge_type: EdgeType,
}

/// Response from `POST /v1/memories/{id}/links`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphLinkResponse {
    /// The newly created edge.
    pub edge: GraphEdge,
}

/// Agent graph export from `GET /v1/agents/{id}/graph/export`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphExport {
    /// Agent whose graph was exported.
    pub agent_id: String,
    /// Export format: `json`, `graphml`, or `csv`.
    pub format: String,
    /// Serialised graph in the requested format.
    pub data: String,
    /// Total number of memory nodes in the export.
    pub node_count: u64,
    /// Total number of edges in the export.
    pub edge_count: u64,
}

/// Options for [`DakeraClient::memory_graph`].
#[derive(Debug, Clone, Default)]
pub struct GraphOptions {
    /// Maximum traversal depth (default: 1, max: 3).
    pub depth: Option<u32>,
    /// Filter by edge types. `None` returns all types.
    pub types: Option<Vec<EdgeType>>,
}

impl GraphOptions {
    /// Create default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set traversal depth.
    pub fn depth(mut self, depth: u32) -> Self {
        self.depth = Some(depth);
        self
    }

    /// Filter by edge types.
    pub fn types(mut self, types: Vec<EdgeType>) -> Self {
        self.types = Some(types);
        self
    }
}

// ============================================================================
// CE-4: GLiNER Entity Extraction Types
// ============================================================================

/// Configuration for namespace-level entity extraction (CE-4).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NamespaceNerConfig {
    pub extract_entities: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_types: Option<Vec<String>>,
}

/// A single extracted entity from GLiNER or rule-based pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEntity {
    pub entity_type: String,
    pub value: String,
    pub score: f64,
}

/// Response from POST /v1/memories/extract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityExtractionResponse {
    pub entities: Vec<ExtractedEntity>,
}

/// Response from GET /v1/memory/entities/:id
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntitiesResponse {
    pub memory_id: String,
    pub entities: Vec<ExtractedEntity>,
}

// ============================================================================
// Memory Feedback Loop (INT-1)
// ============================================================================

/// Feedback signal for memory active learning (INT-1).
///
/// - `upvote`: Boost importance ×1.15, capped at 1.0.
/// - `downvote`: Penalise importance ×0.85, floor 0.0.
/// - `flag`: Mark as irrelevant — sets `decay_flag=true`, no immediate importance change.
/// - `positive`: Backward-compatible alias for `upvote`.
/// - `negative`: Backward-compatible alias for `downvote`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeedbackSignal {
    Upvote,
    Downvote,
    Flag,
    Positive,
    Negative,
}

/// A single recorded feedback event stored in memory metadata (INT-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackHistoryEntry {
    pub signal: FeedbackSignal,
    /// Unix timestamp (seconds) when feedback was submitted.
    pub timestamp: u64,
    pub old_importance: f32,
    pub new_importance: f32,
}

/// Request body for `POST /v1/memories/:id/feedback` (INT-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFeedbackBody {
    pub agent_id: String,
    pub signal: FeedbackSignal,
}

/// Request body for `PATCH /v1/memories/:id/importance` (INT-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryImportancePatch {
    pub agent_id: String,
    pub importance: f32,
}

/// Response from `POST /v1/memories/:id/feedback` and `PATCH /v1/memories/:id/importance` (INT-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackResponse {
    pub memory_id: String,
    /// New importance score after the feedback was applied (0.0–1.0).
    pub new_importance: f32,
    pub signal: FeedbackSignal,
}

/// Response from `GET /v1/memories/:id/feedback` (INT-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackHistoryResponse {
    pub memory_id: String,
    /// Ordered list of feedback events (oldest first, capped at 100).
    pub entries: Vec<FeedbackHistoryEntry>,
}

/// Response from `GET /v1/agents/:id/feedback/summary` (INT-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFeedbackSummary {
    pub agent_id: String,
    pub upvotes: u64,
    pub downvotes: u64,
    pub flags: u64,
    pub total_feedback: u64,
    /// Weighted-average importance across all non-expired memories (0.0–1.0).
    pub health_score: f32,
}

/// Response from `GET /v1/feedback/health` (INT-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackHealthResponse {
    pub agent_id: String,
    /// Mean importance of all non-expired memories (0.0–1.0). Higher = healthier.
    pub health_score: f32,
    pub memory_count: usize,
    pub avg_importance: f32,
}

// ============================================================================
// T-I-F Reliability Scoring (Phase 3 T-I-F RFC)
// ============================================================================

/// Reliability classification label from a [`TifScore`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TifClassification {
    /// Majority of feedback is negative — the memory likely contains incorrect information.
    SurfaceContradiction,
    /// Majority of feedback is uncertain — ask the user for clarification before reusing.
    AskClarification,
    /// Strong positive feedback signal — safe to reuse without additional verification.
    ConfidentReuse,
    /// Mixed or weak signals — verify the memory before acting on it.
    VerifyBeforeUse,
}

impl TifClassification {
    /// Stable string label matching the Python/JS/Go SDK classification strings.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SurfaceContradiction => "surface_contradiction",
            Self::AskClarification => "ask_clarification",
            Self::ConfidentReuse => "confident_reuse",
            Self::VerifyBeforeUse => "verify_before_use",
        }
    }
}

/// Truth-Indeterminacy-Falsity reliability score for a memory (T-I-F RFC Phase 3).
///
/// All three proportions (`truth`, `indeterminacy`, `falsity`) sum to 1.0.
/// Build via [`TifScore::from_feedback_history`] or [`TifScore::from_metadata`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TifScore {
    /// Proportion of positive feedback signals (`upvote` / `positive`).
    pub truth: f64,
    /// Proportion of uncertainty signals (`flag`).
    pub indeterminacy: f64,
    /// Proportion of negative feedback signals (`downvote` / `negative`).
    pub falsity: f64,
    /// Total feedback events used to compute this score.
    pub feedback_count: u64,
    /// Human-readable reliability classification.
    pub classification: TifClassification,
}

fn classify_tif(truth: f64, indeterminacy: f64, falsity: f64) -> TifClassification {
    if falsity >= 0.5 {
        TifClassification::SurfaceContradiction
    } else if indeterminacy >= 0.5 {
        TifClassification::AskClarification
    } else if truth >= 0.7 {
        TifClassification::ConfidentReuse
    } else {
        TifClassification::VerifyBeforeUse
    }
}

impl TifScore {
    /// Compute a [`TifScore`] from a memory's [`FeedbackHistoryResponse`].
    ///
    /// Signals are bucketed as:
    /// - [`FeedbackSignal::Upvote`] / [`FeedbackSignal::Positive`] → truth
    /// - [`FeedbackSignal::Downvote`] / [`FeedbackSignal::Negative`] → falsity
    /// - [`FeedbackSignal::Flag`] → indeterminacy
    ///
    /// With no feedback the score is `{ truth: 0.0, indeterminacy: 1.0, falsity: 0.0, feedback_count: 0 }`.
    pub fn from_feedback_history(history: &FeedbackHistoryResponse) -> Self {
        let mut upvotes: u64 = 0;
        let mut downvotes: u64 = 0;
        let mut flags: u64 = 0;
        for entry in &history.entries {
            match entry.signal {
                FeedbackSignal::Upvote | FeedbackSignal::Positive => upvotes += 1,
                FeedbackSignal::Downvote | FeedbackSignal::Negative => downvotes += 1,
                FeedbackSignal::Flag => flags += 1,
            }
        }
        let total = upvotes + downvotes + flags;
        if total == 0 {
            return Self {
                truth: 0.0,
                indeterminacy: 1.0,
                falsity: 0.0,
                feedback_count: 0,
                classification: TifClassification::AskClarification,
            };
        }
        let total_f = total as f64;
        let truth = upvotes as f64 / total_f;
        let indeterminacy = flags as f64 / total_f;
        let falsity = downvotes as f64 / total_f;
        Self {
            truth,
            indeterminacy,
            falsity,
            feedback_count: total,
            classification: classify_tif(truth, indeterminacy, falsity),
        }
    }

    /// Deserialise a [`TifScore`] from a `metadata["reliability"]` map.
    ///
    /// Expected keys: `truth`, `indeterminacy`, `falsity`, `feedback_count` (snake_case).
    pub fn from_metadata(data: &serde_json::Value) -> Option<Self> {
        let truth = data["truth"].as_f64()?;
        let indeterminacy = data["indeterminacy"].as_f64()?;
        let falsity = data["falsity"].as_f64()?;
        let feedback_count = data["feedback_count"].as_u64().unwrap_or(0);
        Some(Self {
            truth,
            indeterminacy,
            falsity,
            feedback_count,
            classification: classify_tif(truth, indeterminacy, falsity),
        })
    }
}

#[cfg(test)]
mod tif_tests {
    use super::*;

    fn make_history(signals: &[&str]) -> FeedbackHistoryResponse {
        FeedbackHistoryResponse {
            memory_id: "test-mem".to_string(),
            entries: signals
                .iter()
                .map(|s| {
                    let signal = match *s {
                        "upvote" => FeedbackSignal::Upvote,
                        "downvote" => FeedbackSignal::Downvote,
                        "flag" => FeedbackSignal::Flag,
                        "positive" => FeedbackSignal::Positive,
                        "negative" => FeedbackSignal::Negative,
                        other => panic!("unknown signal: {other}"),
                    };
                    FeedbackHistoryEntry { signal, timestamp: 0, old_importance: 0.5, new_importance: 0.5 }
                })
                .collect(),
        }
    }

    #[test]
    fn no_feedback_max_indeterminacy() {
        let score = TifScore::from_feedback_history(&make_history(&[]));
        assert_eq!(score.truth, 0.0);
        assert_eq!(score.indeterminacy, 1.0);
        assert_eq!(score.falsity, 0.0);
        assert_eq!(score.feedback_count, 0);
        assert_eq!(score.classification, TifClassification::AskClarification);
    }

    #[test]
    fn all_upvotes() {
        let score = TifScore::from_feedback_history(&make_history(&["upvote", "upvote", "upvote"]));
        assert!((score.truth - 1.0).abs() < 1e-9);
        assert_eq!(score.feedback_count, 3);
        assert_eq!(score.classification, TifClassification::ConfidentReuse);
    }

    #[test]
    fn all_downvotes() {
        let score = TifScore::from_feedback_history(&make_history(&["downvote", "downvote"]));
        assert!((score.falsity - 1.0).abs() < 1e-9);
        assert_eq!(score.classification, TifClassification::SurfaceContradiction);
    }

    #[test]
    fn all_flags() {
        let score = TifScore::from_feedback_history(&make_history(&["flag", "flag"]));
        assert!((score.indeterminacy - 1.0).abs() < 1e-9);
        assert_eq!(score.classification, TifClassification::AskClarification);
    }

    #[test]
    fn mixed_signals() {
        let score = TifScore::from_feedback_history(&make_history(&[
            "upvote", "upvote", "upvote", "upvote", "downvote", "downvote", "flag", "flag", "flag", "flag",
        ]));
        assert!((score.truth - 0.4).abs() < 1e-9);
        assert!((score.falsity - 0.2).abs() < 1e-9);
        assert!((score.indeterminacy - 0.4).abs() < 1e-9);
        assert_eq!(score.feedback_count, 10);
    }

    #[test]
    fn positive_alias() {
        let score = TifScore::from_feedback_history(&make_history(&["positive", "positive", "downvote"]));
        assert!((score.truth - 2.0 / 3.0).abs() < 1e-9);
        assert!((score.falsity - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn negative_alias() {
        let score = TifScore::from_feedback_history(&make_history(&["upvote", "negative", "negative"]));
        assert!((score.falsity - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn proportions_sum_to_one() {
        let score = TifScore::from_feedback_history(&make_history(&["upvote", "downvote", "flag"]));
        assert!((score.truth + score.indeterminacy + score.falsity - 1.0).abs() < 1e-9);
    }

    #[test]
    fn classification_surface_contradiction() {
        let score = TifScore::from_feedback_history(&make_history(&[
            "downvote", "downvote", "downvote", "upvote", "upvote",
        ]));
        assert_eq!(
            score.classification,
            TifClassification::SurfaceContradiction
        );
    }

    #[test]
    fn classification_verify_before_use() {
        // 2 upvotes, 2 downvotes, 3 flags → no dominant signal
        let score = TifScore::from_feedback_history(&make_history(&[
            "upvote", "upvote", "downvote", "downvote", "flag", "flag", "flag",
        ]));
        assert_eq!(score.classification, TifClassification::VerifyBeforeUse);
    }

    #[test]
    fn falsity_priority_over_indeterminacy() {
        // 3 downvotes + 3 flags: both >= 0.5, falsity wins
        let score = TifScore::from_feedback_history(&make_history(&[
            "downvote", "downvote", "downvote", "flag", "flag", "flag",
        ]));
        assert_eq!(
            score.classification,
            TifClassification::SurfaceContradiction
        );
    }

    #[test]
    fn from_metadata_round_trip() {
        use serde_json::json;
        let data =
            json!({ "truth": 0.75, "indeterminacy": 0.15, "falsity": 0.10, "feedback_count": 20 });
        let score = TifScore::from_metadata(&data).unwrap();
        assert!((score.truth - 0.75).abs() < 1e-9);
        assert_eq!(score.feedback_count, 20);
        assert_eq!(score.classification, TifClassification::ConfidentReuse);
    }

    #[test]
    fn from_metadata_missing_feedback_count() {
        use serde_json::json;
        let data = json!({ "truth": 0.8, "indeterminacy": 0.1, "falsity": 0.1 });
        let score = TifScore::from_metadata(&data).unwrap();
        assert_eq!(score.feedback_count, 0);
    }

    #[test]
    fn classification_as_str() {
        assert_eq!(
            TifClassification::SurfaceContradiction.as_str(),
            "surface_contradiction"
        );
        assert_eq!(
            TifClassification::AskClarification.as_str(),
            "ask_clarification"
        );
        assert_eq!(
            TifClassification::ConfidentReuse.as_str(),
            "confident_reuse"
        );
        assert_eq!(
            TifClassification::VerifyBeforeUse.as_str(),
            "verify_before_use"
        );
    }
}

// ============================================================================
// ODE-2: GLiNER Entity Extraction (dakera-ode sidecar)
// ============================================================================

/// A single entity extracted by the GLiNER model (ODE-2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OdeEntity {
    /// Span text as it appears in the input.
    pub text: String,
    /// Entity type label (e.g. `"person"`, `"organization"`).
    pub label: String,
    /// Start character offset (inclusive) within the input text.
    pub start: usize,
    /// End character offset (exclusive) within the input text.
    pub end: usize,
    /// Confidence score in the range [0, 1].
    pub score: f32,
}

/// Request body for `POST /ode/extract` (ODE-2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractEntitiesRequest {
    /// The text to extract entities from.
    pub content: String,
    /// Agent context for the extraction.
    pub agent_id: String,
    /// Optional memory ID to associate with the extraction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_id: Option<String>,
    /// Optional list of entity type labels to extract.
    /// When omitted the ODE sidecar uses its default set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_types: Option<Vec<String>>,
}

/// Response from `POST /ode/extract` on the ODE sidecar (ODE-2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractEntitiesResponse {
    /// Extracted entities ordered by their start offset.
    pub entities: Vec<OdeEntity>,
    /// GLiNER model variant used for extraction.
    pub model: String,
    /// Wall-clock time taken by the ODE sidecar in milliseconds.
    pub processing_time_ms: u64,
}

// ============================================================================
// KG-2: Graph Query & Export — response types
// ============================================================================

/// Response from `GET /v1/knowledge/query` (KG-2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KgQueryResponse {
    /// Agent whose graph was queried.
    pub agent_id: String,
    /// Number of unique memory node IDs referenced by the returned edges.
    pub node_count: usize,
    /// Number of edges returned.
    pub edge_count: usize,
    /// Matching edges, up to `limit`.
    pub edges: Vec<GraphEdge>,
}

/// Response from `GET /v1/knowledge/path` (KG-2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KgPathResponse {
    /// Agent whose graph was traversed.
    pub agent_id: String,
    /// Source memory ID.
    pub from_id: String,
    /// Target memory ID.
    pub to_id: String,
    /// Number of edges in the shortest path (0 if source == target).
    pub hop_count: usize,
    /// Ordered list of memory IDs from source to target (inclusive).
    pub path: Vec<String>,
}

/// Response from `GET /v1/knowledge/export` with `format=json` (KG-2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KgExportResponse {
    /// Agent whose graph was exported.
    pub agent_id: String,
    /// Export format used (`"json"` when this struct is deserialized).
    pub format: String,
    /// Total number of unique memory node IDs in the export.
    pub node_count: usize,
    /// Total number of edges in the export.
    pub edge_count: usize,
    /// All graph edges for the agent.
    pub edges: Vec<GraphEdge>,
}

// ============================================================================
// COG-1: Cognitive Memory Lifecycle — per-namespace memory policy
// ============================================================================

/// Per-namespace memory lifecycle policy (COG-1).
///
/// Controls type-specific TTLs, decay curves, and spaced repetition behaviour.
/// All fields have sensible defaults; only override what you need.
///
/// Used by [`DakeraClient::get_memory_policy`] and
/// [`DakeraClient::set_memory_policy`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPolicy {
    // Differential TTLs ------------------------------------------------------
    /// Default TTL for `working` memories in seconds (default: 14 400 = 4 h).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_ttl_seconds: Option<u64>,
    /// Default TTL for `episodic` memories in seconds (default: 2 592 000 = 30 d).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episodic_ttl_seconds: Option<u64>,
    /// Default TTL for `semantic` memories in seconds (default: 31 536 000 = 365 d).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_ttl_seconds: Option<u64>,
    /// Default TTL for `procedural` memories in seconds (default: 63 072 000 = 730 d).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub procedural_ttl_seconds: Option<u64>,

    // Decay curves ------------------------------------------------------------
    /// Decay strategy for `working` memories (default: `"exponential"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_decay: Option<String>,
    /// Decay strategy for `episodic` memories (default: `"power_law"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episodic_decay: Option<String>,
    /// Decay strategy for `semantic` memories (default: `"logarithmic"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_decay: Option<String>,
    /// Decay strategy for `procedural` memories (default: `"flat"` — no decay).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub procedural_decay: Option<String>,

    // Spaced repetition -------------------------------------------------------
    /// TTL extension multiplier per recall hit (default: 1.0; set to 0.0 to disable).
    /// Extension = `access_count × sr_factor × sr_base_interval_seconds`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spaced_repetition_factor: Option<f64>,
    /// Base interval in seconds for spaced repetition TTL extension (default: 86 400 = 1 d).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spaced_repetition_base_interval_seconds: Option<u64>,

    // Proactive consolidation (COG-3) -----------------------------------------
    /// Enable background DBSCAN deduplication for this namespace (default: `false`).
    /// When `true` the server merges semantically near-duplicate memories every
    /// [`consolidation_interval_hours`](Self::consolidation_interval_hours) hours.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consolidation_enabled: Option<bool>,
    /// DBSCAN epsilon — cosine-similarity threshold to consider two memories
    /// duplicates (default: `0.92`; higher = only merge very close neighbours).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consolidation_threshold: Option<f32>,
    /// How often (in hours) the background consolidation job runs (default: `24`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consolidation_interval_hours: Option<u32>,
    /// **Read-only.** Lifetime count of memories merged by the consolidation engine.
    /// The server manages this field; any value sent via [`set_memory_policy`] is ignored.
    ///
    /// [`set_memory_policy`]: crate::DakeraClient::set_memory_policy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consolidated_count: Option<u64>,

    // Per-namespace rate limiting (SEC-5) -----------------------------------------
    /// Enable per-namespace store/recall rate limiting (default: `false`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_enabled: Option<bool>,
    /// Max store operations per minute for this namespace. `None` = unlimited (default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_stores_per_minute: Option<u32>,
    /// Max recall operations per minute for this namespace. `None` = unlimited (default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_recalls_per_minute: Option<u32>,

    // Store-time deduplication (CE-10) -----------------------------------------
    /// Deduplicate against existing memories at store time (CE-10, default: `false`).
    ///
    /// When `true` the server computes a similarity check before persisting a new
    /// memory and drops it if a near-duplicate already exists (threshold controlled
    /// by [`dedup_threshold`](Self::dedup_threshold)).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dedup_on_store: Option<bool>,
    /// Cosine-similarity threshold for store-time deduplication (default: `0.92`).
    ///
    /// Memories with similarity ≥ this value are considered duplicates and the
    /// incoming memory is dropped. Only active when `dedup_on_store` is `true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dedup_threshold: Option<f32>,
}

impl Default for MemoryPolicy {
    fn default() -> Self {
        Self {
            working_ttl_seconds: Some(14_400),
            episodic_ttl_seconds: Some(2_592_000),
            semantic_ttl_seconds: Some(31_536_000),
            procedural_ttl_seconds: Some(63_072_000),
            working_decay: Some("exponential".to_string()),
            episodic_decay: Some("power_law".to_string()),
            semantic_decay: Some("logarithmic".to_string()),
            procedural_decay: Some("flat".to_string()),
            spaced_repetition_factor: Some(1.0),
            spaced_repetition_base_interval_seconds: Some(86_400),
            consolidation_enabled: Some(false),
            consolidation_threshold: Some(0.92),
            consolidation_interval_hours: Some(24),
            consolidated_count: Some(0),
            rate_limit_enabled: Some(false),
            rate_limit_stores_per_minute: None,
            rate_limit_recalls_per_minute: None,
            dedup_on_store: Some(false),
            dedup_threshold: Some(0.92),
        }
    }
}

// =============================================================================
// Engine Parity — Vector Bulk Ops, Agent Consolidation, Namespace Config
// =============================================================================

/// Request for `POST /v1/namespaces/{ns}/vectors/bulk-update`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkUpdateRequest {
    pub filter: serde_json::Value,
    pub update: serde_json::Value,
}

/// Response from `POST /v1/namespaces/{ns}/vectors/bulk-update`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkUpdateResponse {
    pub updated: u64,
    pub failed: u64,
    pub errors: Vec<String>,
}

/// Request for `POST /v1/namespaces/{ns}/vectors/bulk-delete`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkDeleteRequest {
    pub filter: serde_json::Value,
}

/// Response from `POST /v1/namespaces/{ns}/vectors/bulk-delete`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkDeleteResponse {
    pub deleted: u64,
    pub failed: u64,
    pub errors: Vec<String>,
}

/// Request for `POST /v1/namespaces/{ns}/vectors/count`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountVectorsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
}

/// Response from `POST /v1/namespaces/{ns}/vectors/count`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountVectorsResponse {
    pub count: u64,
    pub namespace: String,
}

/// Response from `POST /v1/agents/{agent_id}/consolidate`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConsolidateResponse {
    pub agent_id: String,
    pub memories_scanned: u64,
    pub clusters_found: u64,
    pub memories_deprecated: u64,
    pub anchor_ids: Vec<String>,
    pub deprecated_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// One entry in the agent consolidation log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConsolidationLogEntry {
    pub timestamp: u64,
    pub clusters_found: u64,
    pub memories_deprecated: u64,
    pub anchor_ids: Vec<String>,
    pub deprecated_ids: Vec<String>,
}

/// Request for `PATCH /v1/agents/{agent_id}/consolidation/config`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConsolidationConfigPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epsilon: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_samples: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_deprecation_days: Option<u32>,
}

/// Response from consolidation config endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConsolidationConfig {
    pub enabled: bool,
    pub epsilon: f64,
    pub min_samples: u32,
    pub soft_deprecation_days: u32,
}

/// Response from `GET /v1/namespaces/{ns}/config`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceEntityConfig {
    pub namespace: String,
    pub extract_entities: bool,
    pub entity_types: Vec<String>,
}

/// Response from `GET /v1/namespaces/{ns}/extractor`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceExtractorConfig {
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

// ============================================================================
// Phase 2 Types — Cluster, Quotas, Backups, Ops
// ============================================================================

/// Per-node replication lag entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeReplicationLag {
    pub node_id: String,
    pub lag_ms: u64,
    pub status: String,
}

/// Response from `GET /admin/cluster/replication`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationStatus {
    pub replication_factor: u32,
    pub healthy_replicas: u32,
    pub total_nodes: u32,
    #[serde(default)]
    pub replication_lag: Vec<NodeReplicationLag>,
}

/// Shard information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardInfo {
    pub shard_id: String,
    pub namespace: String,
    pub primary_node: String,
    #[serde(default)]
    pub replica_nodes: Vec<String>,
    pub state: String,
    pub vector_count: u64,
    pub size_bytes: u64,
}

/// Response from `GET /admin/cluster/shards`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardListResponse {
    pub shards: Vec<ShardInfo>,
    pub total: u32,
}

/// Request for `POST /admin/cluster/shards/rebalance`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShardRebalanceRequest {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shard_ids: Vec<String>,
    #[serde(default)]
    pub dry_run: bool,
}

/// A planned shard move.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardMove {
    pub shard_id: String,
    pub from_node: String,
    pub to_node: String,
}

/// Response from `POST /admin/cluster/shards/rebalance`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardRebalanceResponse {
    pub initiated: bool,
    pub operation_id: String,
    pub shards_affected: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_seconds: Option<u64>,
    #[serde(default)]
    pub planned_moves: Vec<ShardMove>,
}

/// Response from `GET /admin/cluster/maintenance`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceStatus {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_end: Option<u64>,
    #[serde(default)]
    pub nodes_in_maintenance: Vec<String>,
    pub rejecting_requests: bool,
}

/// Request for `POST /admin/cluster/maintenance/enable`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnableMaintenanceRequest {
    pub reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_ids: Vec<String>,
    #[serde(default)]
    pub reject_requests: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_minutes: Option<u32>,
}

/// Request for `POST /admin/cluster/maintenance/disable`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DisableMaintenanceRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,
}

/// Quota configuration for a namespace.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuotaConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_vectors: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_storage_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_dimensions: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_metadata_bytes: Option<usize>,
    #[serde(default)]
    pub enforcement: String,
}

/// Quota usage for a namespace.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuotaUsage {
    pub vector_count: u64,
    pub storage_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_dimensions: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_metadata_bytes: Option<usize>,
    pub last_updated: u64,
}

/// Combined quota status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaStatus {
    pub namespace: String,
    pub config: QuotaConfig,
    pub usage: QuotaUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_usage_percent: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_usage_percent: Option<f32>,
    pub is_exceeded: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exceeded_quotas: Vec<String>,
}

/// Response from `GET /admin/quotas`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaListResponse {
    pub quotas: Vec<QuotaStatus>,
    pub total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_config: Option<QuotaConfig>,
}

/// Response from `GET /admin/quotas/default`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultQuotaResponse {
    pub config: Option<QuotaConfig>,
}

/// Request for `PUT /admin/quotas/default`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SetDefaultQuotaRequest {
    pub config: Option<QuotaConfig>,
}

/// Request for `PUT /admin/quotas/{namespace}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetQuotaRequest {
    pub config: QuotaConfig,
}

/// Response from `PUT /admin/quotas`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetQuotaResponse {
    pub success: bool,
    pub namespace: String,
    pub config: QuotaConfig,
    pub message: String,
}

/// Request for `POST /admin/quotas/{namespace}/check`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaCheckRequest {
    pub vector_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_bytes: Option<usize>,
}

/// Response from `POST /admin/quotas/{namespace}/check`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaCheckResult {
    pub allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub usage: QuotaUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exceeded_quota: Option<String>,
}

/// Backup information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminBackupInfo {
    pub backup_id: String,
    pub name: String,
    pub backup_type: String,
    pub status: String,
    #[serde(default)]
    pub namespaces: Vec<String>,
    pub vector_count: u64,
    pub size_bytes: u64,
    pub created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub encrypted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>,
}

/// Response from `GET /admin/backups`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupListResponse {
    pub backups: Vec<AdminBackupInfo>,
    pub total: u64,
}

/// Request for `POST /admin/backups`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBackupRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespaces: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypt: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>,
}

/// Response from `POST /admin/backups`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBackupResponse {
    pub backup: AdminBackupInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_completion: Option<u64>,
}

/// Request for `POST /admin/backups/restore`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreBackupRequest {
    pub backup_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_namespaces: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overwrite: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point_in_time: Option<u64>,
}

/// Response from `POST /admin/backups/restore`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreBackupResponse {
    pub restore_id: String,
    pub status: String,
    pub backup_id: String,
    #[serde(default)]
    pub namespaces: Vec<String>,
    pub started_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_completion: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_percent: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vectors_restored: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Backup schedule configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSchedule {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    pub backup_type: String,
    pub retention_days: u32,
    pub max_backups: u32,
    #[serde(default)]
    pub namespaces: Vec<String>,
    pub encrypt: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_backup_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_backup_at: Option<u64>,
}

/// Request for `POST /admin/backups/schedule`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateBackupScheduleRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_backups: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespaces: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypt: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>,
}

// ============================================================================
// Route Query (POST /v1/route)
// ============================================================================

fn default_route_top_k() -> usize {
    3
}

fn default_route_min_similarity() -> f32 {
    0.3
}

/// Request for `POST /v1/route`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRequest {
    /// The query string to route.
    pub query: String,
    /// Maximum number of matching routes to return.
    #[serde(default = "default_route_top_k")]
    pub top_k: usize,
    /// Minimum similarity threshold for route matches.
    #[serde(default = "default_route_min_similarity")]
    pub min_similarity: f32,
    /// Optional embedding model override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// A single route match returned by `POST /v1/route`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMatch {
    /// Matched namespace name.
    pub namespace: String,
    /// Cosine similarity score.
    pub similarity: f64,
    /// Optional namespace description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Response from `POST /v1/route`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResponse {
    /// Ordered list of route matches.
    pub routes: Vec<RouteMatch>,
    /// Embedding model used.
    pub model: String,
    /// Time spent computing embeddings (milliseconds).
    pub embedding_time_ms: u64,
}

// ============================================================================
// Import Job Status (GET /v1/import/{job_id}/status)
// ============================================================================

/// Status of an import job returned by `GET /v1/import/{job_id}/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportJobStatus {
    /// Unique job identifier.
    pub job_id: String,
    /// Current job status (e.g. "running", "completed", "failed").
    pub status: String,
    /// Import file format (e.g. "jsonl", "csv").
    pub format: String,
    /// Total records in the import.
    pub total: usize,
    /// Records successfully imported.
    pub imported: usize,
    /// Records skipped (duplicates, invalid).
    pub skipped: usize,
    /// Per-record error messages, if any.
    #[serde(default)]
    pub errors: Vec<String>,
    /// Unix timestamp when the job started.
    pub started_at: u64,
    /// Unix timestamp when the job finished, if completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<u64>,
}

// ============================================================================
// Storage Tier Overview (GET /admin/storage/tiers)
// ============================================================================

/// Information about a single storage tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierInfo {
    /// Tier name (e.g. "hot", "warm", "cold").
    pub name: String,
    /// Tier classification.
    pub tier_type: String,
    /// Underlying storage technology.
    pub technology: String,
    /// Human-readable tier description.
    pub description: String,
    /// Target access latency.
    pub target_latency: String,
    /// Optional capacity limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity: Option<String>,
    /// Current tier status.
    pub status: String,
    /// Number of items currently in this tier.
    pub current_count: u64,
    /// Number of cache/access hits.
    pub hit_count: u64,
    /// Hit rate as a fraction (0.0–1.0).
    pub hit_rate: f64,
}

/// Tiered storage configuration parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfig {
    /// Maximum number of items in the hot tier.
    pub hot_tier_capacity: usize,
    /// Seconds of inactivity before promoting from hot to warm.
    pub hot_to_warm_threshold_secs: u64,
    /// Seconds of inactivity before demoting from warm to cold.
    pub warm_to_cold_threshold_secs: u64,
    /// Whether automatic tiering is enabled.
    pub auto_tier_enabled: bool,
    /// Interval between tier-check cycles (seconds).
    pub tier_check_interval_secs: u64,
}

/// Tier movement activity counters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierActivity {
    /// Total promotions across all tiers.
    pub promotions: u64,
    /// Total demotions across all tiers.
    pub demotions: u64,
    /// Overall cache hit rate.
    pub cache_hit_rate: f64,
    /// Active storage backend name.
    pub storage_backend: String,
    /// Promotions specifically to the hot tier.
    pub promotions_to_hot: u64,
    /// Demotions specifically to warm.
    pub demotions_to_warm: u64,
    /// Demotions specifically to cold.
    pub demotions_to_cold: u64,
}

/// Overview of the tiered storage system from `GET /admin/storage/tiers`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageTierOverview {
    /// Whether tiered storage is enabled.
    pub tiers_enabled: bool,
    /// Description of each tier.
    pub architecture: Vec<TierInfo>,
    /// Current tier configuration.
    pub config: TierConfig,
    /// Tier movement activity.
    pub activity: TierActivity,
}

// ============================================================================
// Memory Type Stats (GET /admin/memory-type-stats)
// ============================================================================

/// Per-type memory statistics from `GET /admin/memory-type-stats`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryTypeStatsResponse {
    /// Total number of memories.
    pub total: u64,
    /// Working memory count.
    pub working: u64,
    /// Episodic memory count.
    pub episodic: u64,
    /// Semantic memory count.
    pub semantic: u64,
    /// Procedural memory count.
    pub procedural: u64,
    /// Number of distinct agent namespaces.
    pub agent_namespaces: u64,
}

// ============================================================================
// Migrate Namespace Dimensions (POST /admin/namespaces/migrate-dimensions)
// ============================================================================

fn default_target_dimension() -> usize {
    1024
}

/// Request for `POST /admin/namespaces/migrate-dimensions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrateNamespaceDimensionsRequest {
    /// Namespaces to migrate (empty = all).
    #[serde(default)]
    pub namespaces: Vec<String>,
    /// Target embedding dimension.
    #[serde(default = "default_target_dimension")]
    pub target_dimension: usize,
}

/// Per-namespace migration result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceMigrationResult {
    /// Namespace that was migrated.
    pub namespace: String,
    /// Original embedding dimension.
    pub original_dimension: usize,
    /// Vectors successfully re-embedded.
    pub vectors_migrated: usize,
    /// Vectors skipped (already at target dimension).
    pub vectors_skipped: usize,
    /// Migration status for this namespace.
    pub status: String,
    /// Error message if migration failed for this namespace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Response from `POST /admin/namespaces/migrate-dimensions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrateDimensionsResponse {
    /// Number of namespaces successfully migrated.
    pub migrated: usize,
    /// Number of namespaces that failed migration.
    pub failed: usize,
    /// Namespaces already at the target dimension.
    pub already_current: usize,
    /// Per-namespace results.
    pub results: Vec<NamespaceMigrationResult>,
}

/// Request body for `POST /admin/reembed/drain` (v0.11.82+). All fields optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DrainReembedRequest {
    /// Hard wall-clock cap in seconds (default 600).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
    /// Candidates upgraded per cycle (default 10000).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<usize>,
    /// Minimum importance to upgrade (default 0.0 — upgrade all statics).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_importance: Option<f32>,
}

/// Response from `POST /admin/reembed/drain` (v0.11.82+).
///
/// A [`remaining`][DrainReembedResponse::remaining] of `0` means all
/// `_embedding_kind=static` vectors have been upgraded to full ONNX quality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrainReembedResponse {
    /// Total vectors upgraded across all cycles in this drain.
    pub processed: usize,
    /// Static candidates still remaining (0 on a full drain).
    pub remaining: usize,
    /// Wall-clock duration of the drain in milliseconds.
    pub elapsed_ms: u128,
    /// Number of upgrade cycles executed.
    pub cycles: usize,
    /// `true` if the drain stopped on the timeout rather than reaching zero.
    pub timed_out: bool,
}
