//! Types for the Dakera client SDK

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub vector_count: u64,
    /// Vector dimensions
    #[serde(alias = "dimension")]
    pub dimensions: Option<u32>,
    /// Index type used
    pub index_type: Option<String>,
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
    /// Matched vectors
    pub matches: Vec<Match>,
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
    pub matches: Vec<FullTextMatch>,
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

/// Hybrid search request combining vector and full-text search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchRequest {
    /// Query vector
    pub vector: Vec<f32>,
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
    /// Create a new hybrid search request
    pub fn new(vector: Vec<f32>, text: impl Into<String>, top_k: u32) -> Self {
        Self {
            vector,
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
    pub matches: Vec<Match>,
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
    pub started_at: Option<u64>,
    /// Completion timestamp
    pub completed_at: Option<u64>,
    /// Progress percentage
    pub progress: u8,
    /// Status message
    pub message: Option<String>,
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
    /// MiniLM-L6 — Fast, good quality (384 dimensions)
    #[default]
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
    #[serde(skip_serializing_if = "Option::is_none")]
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
        Self { dimension, distance: None }
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
