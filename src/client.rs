//! Dakera client implementation

use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION},
    Client, StatusCode,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, instrument};

use serde::Deserialize;

use crate::error::{ClientError, Result, ServerErrorCode};
use crate::types::*;

/// Default timeout for requests
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Dakera client for interacting with the vector database
#[derive(Debug, Clone)]
pub struct DakeraClient {
    /// HTTP client
    pub(crate) client: Client,
    /// Base URL of the Dakera server
    pub(crate) base_url: String,
    /// ODE-2: Base URL of the dakera-ode sidecar (optional)
    pub(crate) ode_url: Option<String>,
    /// Retry configuration (wired into API call sites in a follow-up; suppressed until then)
    #[allow(dead_code)]
    pub(crate) retry_config: RetryConfig,
    /// OPS-1: last seen rate-limit headers (shared across clones)
    pub(crate) last_rate_limit: Arc<Mutex<Option<RateLimitHeaders>>>,
}

impl DakeraClient {
    /// Create a new client with the given base URL
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dakera_client::DakeraClient;
    ///
    /// let client = DakeraClient::new("http://localhost:3000").unwrap();
    /// ```
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        DakeraClientBuilder::new(base_url).build()
    }

    /// Create a new client builder for more configuration options
    pub fn builder(base_url: impl Into<String>) -> DakeraClientBuilder {
        DakeraClientBuilder::new(base_url)
    }

    // ========================================================================
    // Health & Status
    // ========================================================================

    /// Check server health
    #[instrument(skip(self))]
    pub async fn health(&self) -> Result<HealthResponse> {
        let url = format!("{}/health", self.base_url);
        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            let json: serde_json::Value = response.json().await?;
            // Server returns {"service":"dakera","status":"healthy","version":"..."}.
            // Accept both `healthy: bool` (legacy) and `status: "healthy"` (current).
            let healthy = json
                .get("healthy")
                .and_then(|v| v.as_bool())
                .unwrap_or_else(|| json.get("status").and_then(|v| v.as_str()) == Some("healthy"));
            let version = json
                .get("version")
                .and_then(|v| v.as_str())
                .map(String::from);
            let uptime_seconds = json.get("uptime_seconds").and_then(|v| v.as_u64());
            Ok(HealthResponse {
                healthy,
                version,
                uptime_seconds,
            })
        } else {
            // Health endpoint might return simple OK
            Ok(HealthResponse {
                healthy: true,
                version: None,
                uptime_seconds: None,
            })
        }
    }

    /// Check if server is ready
    #[instrument(skip(self))]
    pub async fn ready(&self) -> Result<ReadinessResponse> {
        let url = format!("{}/health/ready", self.base_url);
        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Ok(ReadinessResponse {
                ready: false,
                components: None,
            })
        }
    }

    /// Check if server is live
    #[instrument(skip(self))]
    pub async fn live(&self) -> Result<bool> {
        let url = format!("{}/health/live", self.base_url);
        let response = self.client.get(&url).send().await?;
        Ok(response.status().is_success())
    }

    // ========================================================================
    // Namespace Operations
    // ========================================================================

    /// List all namespaces
    #[instrument(skip(self))]
    pub async fn list_namespaces(&self) -> Result<Vec<String>> {
        let url = format!("{}/v1/namespaces", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response::<ListNamespacesResponse>(response)
            .await
            .map(|r| r.namespaces)
    }

    /// Get namespace information
    #[instrument(skip(self))]
    pub async fn get_namespace(&self, namespace: &str) -> Result<NamespaceInfo> {
        let url = format!("{}/v1/namespaces/{}", self.base_url, namespace);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Create a new namespace
    #[instrument(skip(self, request))]
    pub async fn create_namespace(
        &self,
        namespace: &str,
        request: CreateNamespaceRequest,
    ) -> Result<NamespaceInfo> {
        let url = format!("{}/v1/namespaces/{}", self.base_url, namespace);
        let response = self.client.put(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Create or update a namespace configuration (upsert semantics — v0.6.0).
    ///
    /// Creates the namespace if it does not exist, or updates its distance-metric
    /// configuration if it already exists.  Dimension changes are rejected to
    /// prevent silent data corruption.  Requires `Scope::Write`.
    #[instrument(skip(self, request), fields(namespace = %namespace))]
    pub async fn configure_namespace(
        &self,
        namespace: &str,
        request: ConfigureNamespaceRequest,
    ) -> Result<ConfigureNamespaceResponse> {
        let url = format!("{}/v1/namespaces/{}", self.base_url, namespace);
        let response = self.client.put(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Delete a namespace and all its data.
    #[instrument(skip(self))]
    pub async fn delete_namespace(&self, namespace: &str) -> Result<()> {
        let url = format!("{}/v1/namespaces/{}", self.base_url, namespace);
        let response = self.client.delete(&url).send().await?;
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            Err(ClientError::Server {
                status,
                message: text,
                code: None,
            })
        }
    }

    /// Flush pending writes for a namespace.
    #[instrument(skip(self))]
    pub async fn flush(&self, namespace: &str) -> Result<serde_json::Value> {
        let url = format!("{}/v1/namespaces/{}/flush", self.base_url, namespace);
        let response = self.client.post(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get index statistics for a specific namespace.
    #[instrument(skip(self))]
    pub async fn get_namespace_stats(&self, namespace: &str) -> Result<serde_json::Value> {
        let url = format!("{}/v1/namespaces/{}/stats", self.base_url, namespace);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Alias for [`get_namespace_stats`](Self::get_namespace_stats) matching Python/JS naming.
    #[instrument(skip(self))]
    pub async fn get_index_stats(&self, namespace: &str) -> Result<serde_json::Value> {
        self.get_namespace_stats(namespace).await
    }

    // ========================================================================
    // Vector Operations
    // ========================================================================

    /// Upsert vectors into a namespace
    #[instrument(skip(self, request), fields(vector_count = request.vectors.len()))]
    pub async fn upsert(&self, namespace: &str, request: UpsertRequest) -> Result<UpsertResponse> {
        let url = format!("{}/v1/namespaces/{}/vectors", self.base_url, namespace);
        debug!(
            "Upserting {} vectors to {}",
            request.vectors.len(),
            namespace
        );

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Upsert a single vector (convenience method)
    #[instrument(skip(self, vector))]
    pub async fn upsert_one(&self, namespace: &str, vector: Vector) -> Result<UpsertResponse> {
        self.upsert(namespace, UpsertRequest::single(vector)).await
    }

    /// Upsert vectors in column format (Turbopuffer-inspired)
    ///
    /// This format is more efficient for bulk upserts as it avoids repeating
    /// field names for each vector. All arrays must have equal length.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dakera_client::{DakeraClient, ColumnUpsertRequest};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = DakeraClient::new("http://localhost:3000")?;
    ///
    /// let request = ColumnUpsertRequest::new(
    ///     vec!["id1".to_string(), "id2".to_string(), "id3".to_string()],
    ///     vec![
    ///         vec![0.1, 0.2, 0.3],
    ///         vec![0.4, 0.5, 0.6],
    ///         vec![0.7, 0.8, 0.9],
    ///     ],
    /// )
    /// .with_attribute("category", vec![
    ///     serde_json::json!("A"),
    ///     serde_json::json!("B"),
    ///     serde_json::json!("A"),
    /// ]);
    ///
    /// let response = client.upsert_columns("my-namespace", request).await?;
    /// println!("Upserted {} vectors", response.upserted_count);
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self, request), fields(namespace = %namespace, count = request.ids.len()))]
    pub async fn upsert_columns(
        &self,
        namespace: &str,
        request: ColumnUpsertRequest,
    ) -> Result<UpsertResponse> {
        let url = format!(
            "{}/v1/namespaces/{}/upsert-columns",
            self.base_url, namespace
        );
        debug!(
            "Upserting {} vectors in column format to {}",
            request.ids.len(),
            namespace
        );

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Query for similar vectors
    #[instrument(skip(self, request), fields(top_k = request.top_k))]
    pub async fn query(&self, namespace: &str, request: QueryRequest) -> Result<QueryResponse> {
        let url = format!("{}/v1/namespaces/{}/query", self.base_url, namespace);
        debug!(
            "Querying namespace {} for top {} results",
            namespace, request.top_k
        );

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Simple query with just a vector and top_k (convenience method)
    #[instrument(skip(self, vector))]
    pub async fn query_simple(
        &self,
        namespace: &str,
        vector: Vec<f32>,
        top_k: u32,
    ) -> Result<QueryResponse> {
        self.query(namespace, QueryRequest::new(vector, top_k))
            .await
    }

    /// Execute multiple queries in a single request
    ///
    /// This allows executing multiple vector similarity queries in parallel,
    /// which is more efficient than making separate requests.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dakera_client::{DakeraClient, BatchQueryRequest, BatchQueryItem};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = DakeraClient::new("http://localhost:3000")?;
    ///
    /// let request = BatchQueryRequest::new(vec![
    ///     BatchQueryItem::new(vec![0.1, 0.2, 0.3], 5).with_id("query1"),
    ///     BatchQueryItem::new(vec![0.4, 0.5, 0.6], 10).with_id("query2"),
    /// ]);
    ///
    /// let response = client.batch_query("my-namespace", request).await?;
    /// println!("Executed {} queries in {}ms", response.query_count, response.total_latency_ms);
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self, request), fields(namespace = %namespace, query_count = request.queries.len()))]
    pub async fn batch_query(
        &self,
        namespace: &str,
        request: BatchQueryRequest,
    ) -> Result<BatchQueryResponse> {
        let url = format!("{}/v1/namespaces/{}/batch-query", self.base_url, namespace);
        debug!(
            "Batch querying namespace {} with {} queries",
            namespace,
            request.queries.len()
        );

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Delete vectors by ID
    #[instrument(skip(self, request), fields(id_count = request.ids.len()))]
    pub async fn delete(&self, namespace: &str, request: DeleteRequest) -> Result<DeleteResponse> {
        let url = format!(
            "{}/v1/namespaces/{}/vectors/delete",
            self.base_url, namespace
        );
        debug!("Deleting {} vectors from {}", request.ids.len(), namespace);

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Delete a single vector by ID (convenience method)
    #[instrument(skip(self))]
    pub async fn delete_one(&self, namespace: &str, id: &str) -> Result<DeleteResponse> {
        self.delete(namespace, DeleteRequest::single(id)).await
    }

    // ========================================================================
    // Full-Text Search Operations
    // ========================================================================

    /// Index documents for full-text search
    #[instrument(skip(self, request), fields(doc_count = request.documents.len()))]
    pub async fn index_documents(
        &self,
        namespace: &str,
        request: IndexDocumentsRequest,
    ) -> Result<IndexDocumentsResponse> {
        let url = format!(
            "{}/v1/namespaces/{}/fulltext/index",
            self.base_url, namespace
        );
        debug!(
            "Indexing {} documents in {}",
            request.documents.len(),
            namespace
        );

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Index a single document (convenience method)
    #[instrument(skip(self, document))]
    pub async fn index_document(
        &self,
        namespace: &str,
        document: Document,
    ) -> Result<IndexDocumentsResponse> {
        self.index_documents(
            namespace,
            IndexDocumentsRequest {
                documents: vec![document],
            },
        )
        .await
    }

    /// Perform full-text search
    #[instrument(skip(self, request))]
    pub async fn fulltext_search(
        &self,
        namespace: &str,
        request: FullTextSearchRequest,
    ) -> Result<FullTextSearchResponse> {
        let url = format!(
            "{}/v1/namespaces/{}/fulltext/search",
            self.base_url, namespace
        );
        debug!("Full-text search in {} for: {}", namespace, request.query);

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Simple full-text search (convenience method)
    #[instrument(skip(self))]
    pub async fn search_text(
        &self,
        namespace: &str,
        query: &str,
        top_k: u32,
    ) -> Result<FullTextSearchResponse> {
        self.fulltext_search(namespace, FullTextSearchRequest::new(query, top_k))
            .await
    }

    /// Get full-text index statistics
    #[instrument(skip(self))]
    pub async fn fulltext_stats(&self, namespace: &str) -> Result<FullTextStats> {
        let url = format!(
            "{}/v1/namespaces/{}/fulltext/stats",
            self.base_url, namespace
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Delete documents from full-text index
    #[instrument(skip(self, request))]
    pub async fn fulltext_delete(
        &self,
        namespace: &str,
        request: DeleteRequest,
    ) -> Result<DeleteResponse> {
        let url = format!(
            "{}/v1/namespaces/{}/fulltext/delete",
            self.base_url, namespace
        );
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // Hybrid Search Operations
    // ========================================================================

    /// Perform hybrid search (vector + full-text)
    #[instrument(skip(self, request), fields(top_k = request.top_k))]
    pub async fn hybrid_search(
        &self,
        namespace: &str,
        request: HybridSearchRequest,
    ) -> Result<HybridSearchResponse> {
        let url = format!("{}/v1/namespaces/{}/hybrid", self.base_url, namespace);
        debug!(
            "Hybrid search in {} with vector_weight={}",
            namespace, request.vector_weight
        );

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // Multi-Vector Search Operations
    // ========================================================================

    /// Multi-vector search with positive/negative vectors and MMR
    ///
    /// This performs semantic search using multiple positive vectors (to search towards)
    /// and optional negative vectors (to search away from). Supports MMR (Maximal Marginal
    /// Relevance) for result diversity.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dakera_client::{DakeraClient, MultiVectorSearchRequest};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = DakeraClient::new("http://localhost:3000")?;
    ///
    /// // Search towards multiple concepts, away from others
    /// let request = MultiVectorSearchRequest::new(vec![
    ///     vec![0.1, 0.2, 0.3],  // positive vector 1
    ///     vec![0.4, 0.5, 0.6],  // positive vector 2
    /// ])
    /// .with_negative_vectors(vec![
    ///     vec![0.7, 0.8, 0.9],  // negative vector
    /// ])
    /// .with_top_k(10)
    /// .with_mmr(0.7);  // Enable MMR with lambda=0.7
    ///
    /// let response = client.multi_vector_search("my-namespace", request).await?;
    /// for result in response.results {
    ///     println!("ID: {}, Score: {}", result.id, result.score);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self, request), fields(namespace = %namespace))]
    pub async fn multi_vector_search(
        &self,
        namespace: &str,
        request: MultiVectorSearchRequest,
    ) -> Result<MultiVectorSearchResponse> {
        let url = format!("{}/v1/namespaces/{}/multi-vector", self.base_url, namespace);
        debug!(
            "Multi-vector search in {} with {} positive vectors",
            namespace,
            request.positive_vectors.len()
        );

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // Aggregation Operations
    // ========================================================================

    /// Aggregate vectors with grouping (Turbopuffer-inspired)
    ///
    /// This performs aggregation queries on vector metadata, supporting
    /// count, sum, avg, min, and max operations with optional grouping.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dakera_client::{DakeraClient, AggregationRequest};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = DakeraClient::new("http://localhost:3000")?;
    ///
    /// // Count all vectors and sum scores, grouped by category
    /// let request = AggregationRequest::new()
    ///     .with_count("total_count")
    ///     .with_sum("total_score", "score")
    ///     .with_avg("avg_score", "score")
    ///     .with_group_by("category");
    ///
    /// let response = client.aggregate("my-namespace", request).await?;
    /// if let Some(groups) = response.aggregation_groups {
    ///     for group in groups {
    ///         println!("Group: {:?}", group.group_key);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self, request), fields(namespace = %namespace))]
    pub async fn aggregate(
        &self,
        namespace: &str,
        request: AggregationRequest,
    ) -> Result<AggregationResponse> {
        let url = format!("{}/v1/namespaces/{}/aggregate", self.base_url, namespace);
        debug!(
            "Aggregating in namespace {} with {} aggregations",
            namespace,
            request.aggregate_by.len()
        );

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // Unified Query Operations
    // ========================================================================

    /// Unified query with flexible ranking options (Turbopuffer-inspired)
    ///
    /// This provides a unified API for vector search (ANN/kNN), full-text search (BM25),
    /// and attribute ordering. Supports combining multiple ranking functions with
    /// Sum, Max, and Product operators.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dakera_client::{DakeraClient, UnifiedQueryRequest, SortDirection};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = DakeraClient::new("http://localhost:3000")?;
    ///
    /// // Vector ANN search
    /// let request = UnifiedQueryRequest::vector_search(vec![0.1, 0.2, 0.3], 10);
    /// let response = client.unified_query("my-namespace", request).await?;
    ///
    /// // Full-text BM25 search
    /// let request = UnifiedQueryRequest::fulltext_search("content", "hello world", 10);
    /// let response = client.unified_query("my-namespace", request).await?;
    ///
    /// // Attribute ordering with filter
    /// let request = UnifiedQueryRequest::attribute_order("timestamp", SortDirection::Desc, 10)
    ///     .with_filter(serde_json::json!({"category": {"$eq": "science"}}));
    /// let response = client.unified_query("my-namespace", request).await?;
    ///
    /// for result in response.results {
    ///     println!("ID: {}, Score: {:?}", result.id, result.dist);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self, request), fields(namespace = %namespace))]
    pub async fn unified_query(
        &self,
        namespace: &str,
        request: UnifiedQueryRequest,
    ) -> Result<UnifiedQueryResponse> {
        let url = format!(
            "{}/v1/namespaces/{}/unified-query",
            self.base_url, namespace
        );
        debug!(
            "Unified query in namespace {} with top_k={}",
            namespace, request.top_k
        );

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Simple vector search using the unified query API (convenience method)
    ///
    /// This is a shortcut for `unified_query` with a vector ANN search.
    #[instrument(skip(self, vector))]
    pub async fn unified_vector_search(
        &self,
        namespace: &str,
        vector: Vec<f32>,
        top_k: usize,
    ) -> Result<UnifiedQueryResponse> {
        self.unified_query(namespace, UnifiedQueryRequest::vector_search(vector, top_k))
            .await
    }

    /// Simple full-text search using the unified query API (convenience method)
    ///
    /// This is a shortcut for `unified_query` with a BM25 full-text search.
    #[instrument(skip(self))]
    pub async fn unified_text_search(
        &self,
        namespace: &str,
        field: &str,
        query: &str,
        top_k: usize,
    ) -> Result<UnifiedQueryResponse> {
        self.unified_query(
            namespace,
            UnifiedQueryRequest::fulltext_search(field, query, top_k),
        )
        .await
    }

    // ========================================================================
    // Query Explain Operations
    // ========================================================================

    /// Explain query execution plan (similar to SQL EXPLAIN)
    ///
    /// This provides detailed information about how a query will be executed,
    /// including index selection, execution stages, cost estimates, and
    /// performance recommendations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dakera_client::{DakeraClient, QueryExplainRequest};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = DakeraClient::new("http://localhost:3000")?;
    ///
    /// // Explain a vector search query
    /// let request = QueryExplainRequest::vector_search(vec![0.1, 0.2, 0.3], 10)
    ///     .with_verbose();
    /// let plan = client.explain_query("my-namespace", request).await?;
    ///
    /// println!("Query plan: {}", plan.summary);
    /// println!("Estimated time: {}ms", plan.cost_estimate.estimated_time_ms);
    ///
    /// for stage in &plan.stages {
    ///     println!("Stage {}: {} - {}", stage.order, stage.name, stage.description);
    /// }
    ///
    /// for rec in &plan.recommendations {
    ///     println!("Recommendation ({}): {}", rec.priority, rec.description);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self, request), fields(namespace = %namespace))]
    pub async fn explain_query(
        &self,
        namespace: &str,
        request: QueryExplainRequest,
    ) -> Result<QueryExplainResponse> {
        let url = format!("{}/v1/namespaces/{}/explain", self.base_url, namespace);
        debug!(
            "Explaining query in namespace {} (query_type={:?}, top_k={})",
            namespace, request.query_type, request.top_k
        );

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // Cache Warming Operations
    // ========================================================================

    /// Warm cache for vectors in a namespace
    ///
    /// This pre-loads vectors into cache tiers for faster subsequent access.
    /// Supports priority levels and can run in the background.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dakera_client::{DakeraClient, WarmCacheRequest, WarmingPriority};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = DakeraClient::new("http://localhost:3000")?;
    ///
    /// // Warm entire namespace with high priority
    /// let response = client.warm_cache(
    ///     WarmCacheRequest::new("my-namespace")
    ///         .with_priority(WarmingPriority::High)
    /// ).await?;
    ///
    /// println!("Warmed {} entries", response.entries_warmed);
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self, request), fields(namespace = %request.namespace, priority = ?request.priority))]
    pub async fn warm_cache(&self, request: WarmCacheRequest) -> Result<WarmCacheResponse> {
        let url = format!(
            "{}/v1/namespaces/{}/cache/warm",
            self.base_url, request.namespace
        );
        debug!(
            "Warming cache for namespace {} with priority {:?}",
            request.namespace, request.priority
        );

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Warm specific vectors by ID (convenience method)
    #[instrument(skip(self, vector_ids))]
    pub async fn warm_vectors(
        &self,
        namespace: &str,
        vector_ids: Vec<String>,
    ) -> Result<WarmCacheResponse> {
        self.warm_cache(WarmCacheRequest::new(namespace).with_vector_ids(vector_ids))
            .await
    }

    // ========================================================================
    // Export Operations
    // ========================================================================

    /// Export vectors from a namespace with pagination
    ///
    /// This exports all vectors from a namespace, supporting pagination for
    /// large datasets. Use the `next_cursor` from the response to fetch
    /// subsequent pages.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dakera_client::{DakeraClient, ExportRequest};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = DakeraClient::new("http://localhost:3000")?;
    ///
    /// // Export first page of vectors
    /// let mut request = ExportRequest::new().with_top_k(1000);
    /// let response = client.export("my-namespace", request).await?;
    ///
    /// println!("Exported {} vectors", response.returned_count);
    ///
    /// // Fetch next page if available
    /// if let Some(cursor) = response.next_cursor {
    ///     let next_request = ExportRequest::new().with_cursor(cursor);
    ///     let next_response = client.export("my-namespace", next_request).await?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self, request), fields(namespace = %namespace))]
    pub async fn export(&self, namespace: &str, request: ExportRequest) -> Result<ExportResponse> {
        let url = format!("{}/v1/namespaces/{}/export", self.base_url, namespace);
        debug!(
            "Exporting vectors from namespace {} (top_k={}, cursor={:?})",
            namespace, request.top_k, request.cursor
        );

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Export all vectors from a namespace (convenience method)
    ///
    /// This is a simple wrapper that exports with default settings.
    #[instrument(skip(self))]
    pub async fn export_all(&self, namespace: &str) -> Result<ExportResponse> {
        self.export(namespace, ExportRequest::new()).await
    }

    /// Alias for [`export`](Self::export) matching Python/JS/Go SDK naming.
    #[instrument(skip(self, request), fields(namespace = %namespace))]
    pub async fn export_vectors(
        &self,
        namespace: &str,
        request: ExportRequest,
    ) -> Result<ExportResponse> {
        self.export(namespace, request).await
    }

    // ========================================================================
    // Operations
    // ========================================================================

    /// Get system diagnostics
    #[instrument(skip(self))]
    pub async fn diagnostics(&self) -> Result<SystemDiagnostics> {
        let url = format!("{}/ops/diagnostics", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// List background jobs
    #[instrument(skip(self))]
    pub async fn list_jobs(&self) -> Result<Vec<JobInfo>> {
        let url = format!("{}/ops/jobs", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get a specific job status
    #[instrument(skip(self))]
    pub async fn get_job(&self, job_id: &str) -> Result<Option<JobInfo>> {
        let url = format!("{}/ops/jobs/{}", self.base_url, job_id);
        let response = self.client.get(&url).send().await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        self.handle_response(response).await.map(Some)
    }

    /// Trigger index compaction
    #[instrument(skip(self, request))]
    pub async fn compact(&self, request: CompactionRequest) -> Result<CompactionResponse> {
        let url = format!("{}/ops/compact", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Request graceful shutdown
    #[instrument(skip(self))]
    pub async fn shutdown(&self) -> Result<()> {
        let url = format!("{}/ops/shutdown", self.base_url);
        let response = self.client.post(&url).send().await?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            Err(ClientError::Server {
                status,
                message: text,
                code: None,
            })
        }
    }

    // ========================================================================
    // Fetch by ID
    // ========================================================================

    /// Fetch vectors by their IDs
    #[instrument(skip(self, request), fields(id_count = request.ids.len()))]
    pub async fn fetch(&self, namespace: &str, request: FetchRequest) -> Result<FetchResponse> {
        let url = format!("{}/v1/namespaces/{}/fetch", self.base_url, namespace);
        debug!("Fetching {} vectors from {}", request.ids.len(), namespace);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Fetch vectors by IDs (convenience method)
    #[instrument(skip(self))]
    pub async fn fetch_by_ids(&self, namespace: &str, ids: &[&str]) -> Result<Vec<Vector>> {
        let request = FetchRequest::new(ids.iter().map(|s| s.to_string()).collect());
        self.fetch(namespace, request).await.map(|r| r.vectors)
    }

    // ========================================================================
    // Text Auto-Embedding Operations
    // ========================================================================

    /// Upsert text documents with automatic server-side embedding generation
    #[instrument(skip(self, request), fields(doc_count = request.documents.len()))]
    pub async fn upsert_text(
        &self,
        namespace: &str,
        request: UpsertTextRequest,
    ) -> Result<TextUpsertResponse> {
        let url = format!("{}/v1/namespaces/{}/upsert-text", self.base_url, namespace);
        debug!(
            "Upserting {} text documents to {}",
            request.documents.len(),
            namespace
        );
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Query using natural language text with automatic server-side embedding
    #[instrument(skip(self, request), fields(top_k = request.top_k))]
    pub async fn query_text(
        &self,
        namespace: &str,
        request: QueryTextRequest,
    ) -> Result<TextQueryResponse> {
        let url = format!("{}/v1/namespaces/{}/query-text", self.base_url, namespace);
        debug!("Text query in {} for: {}", namespace, request.text);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Query text (convenience method)
    #[instrument(skip(self))]
    pub async fn query_text_simple(
        &self,
        namespace: &str,
        text: &str,
        top_k: u32,
    ) -> Result<TextQueryResponse> {
        self.query_text(namespace, QueryTextRequest::new(text, top_k))
            .await
    }

    /// Execute multiple text queries with automatic embedding in a single request
    #[instrument(skip(self, request), fields(query_count = request.queries.len()))]
    pub async fn batch_query_text(
        &self,
        namespace: &str,
        request: BatchQueryTextRequest,
    ) -> Result<BatchQueryTextResponse> {
        let url = format!(
            "{}/v1/namespaces/{}/batch-query-text",
            self.base_url, namespace
        );
        debug!(
            "Batch text query in {} with {} queries",
            namespace,
            request.queries.len()
        );
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // CE-4: GLiNER Entity Extraction
    // ========================================================================

    /// Configure namespace-level entity extraction settings (CE-4).
    ///
    /// Sends `PATCH /v1/namespaces/{namespace}/config` with the provided
    /// [`NamespaceNerConfig`].
    #[instrument(skip(self, config))]
    pub async fn configure_namespace_ner(
        &self,
        namespace: &str,
        config: NamespaceNerConfig,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/v1/namespaces/{}/config", self.base_url, namespace);
        let response = self.client.patch(&url).json(&config).send().await?;
        self.handle_response(response).await
    }

    /// Extract entities from arbitrary text using the GLiNER pipeline (CE-4).
    ///
    /// Sends `POST /v1/memories/extract` with the supplied text and optional
    /// entity type list.
    #[instrument(skip(self, text, entity_types))]
    pub async fn extract_entities(
        &self,
        text: &str,
        entity_types: Option<Vec<String>>,
    ) -> Result<EntityExtractionResponse> {
        let url = format!("{}/v1/memories/extract", self.base_url);
        let body = serde_json::json!({
            "content": text,
            "entity_types": entity_types,
        });
        let response = self.client.post(&url).json(&body).send().await?;
        self.handle_response(response).await
    }

    /// Retrieve entity tags associated with a stored memory (CE-4).
    ///
    /// Sends `GET /v1/memory/entities/{memory_id}`.
    #[instrument(skip(self))]
    pub async fn memory_entities(&self, memory_id: &str) -> Result<MemoryEntitiesResponse> {
        let url = format!("{}/v1/memory/entities/{}", self.base_url, memory_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // Private Helpers
    // ========================================================================

    /// Rate-limit headers from the most recent API response (OPS-1).
    ///
    /// Returns `None` until the first successful request has been made.
    pub fn last_rate_limit_headers(&self) -> Option<RateLimitHeaders> {
        self.last_rate_limit.lock().ok()?.clone()
    }

    /// Handle response and deserialize JSON
    pub(crate) async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T> {
        let status = response.status();

        // OPS-1: capture rate-limit headers before consuming the response body
        if let Ok(mut guard) = self.last_rate_limit.lock() {
            *guard = Some(RateLimitHeaders::from_response(&response));
        }

        if status.is_success() {
            Ok(response.json().await?)
        } else {
            let status_code = status.as_u16();
            // Extract Retry-After before consuming response
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());
            let text = response.text().await.unwrap_or_default();

            if status_code == 429 {
                return Err(ClientError::RateLimitExceeded { retry_after });
            }

            #[derive(Deserialize)]
            struct ErrorBody {
                error: Option<String>,
                code: Option<ServerErrorCode>,
            }

            let (message, code) = if let Ok(body) = serde_json::from_str::<ErrorBody>(&text) {
                (body.error.unwrap_or_else(|| text.clone()), body.code)
            } else {
                (text, None)
            };

            match status_code {
                401 => Err(ClientError::Server {
                    status: 401,
                    message,
                    code,
                }),
                403 => Err(ClientError::Authorization {
                    status: 403,
                    message,
                    code,
                }),
                404 => match &code {
                    Some(ServerErrorCode::NamespaceNotFound) => {
                        Err(ClientError::NamespaceNotFound(message))
                    }
                    Some(ServerErrorCode::VectorNotFound) => {
                        Err(ClientError::VectorNotFound(message))
                    }
                    _ => Err(ClientError::Server {
                        status: 404,
                        message,
                        code,
                    }),
                },
                _ => Err(ClientError::Server {
                    status: status_code,
                    message,
                    code,
                }),
            }
        }
    }

    /// Handle response and return raw text body (for non-JSON endpoints like /v1/ops/metrics).
    pub(crate) async fn handle_text_response(&self, response: reqwest::Response) -> Result<String> {
        let status = response.status();

        // OPS-1: capture rate-limit headers before consuming the response body
        if let Ok(mut guard) = self.last_rate_limit.lock() {
            *guard = Some(RateLimitHeaders::from_response(&response));
        }

        let retry_after = response
            .headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        let text = response.text().await.unwrap_or_default();

        if status.is_success() {
            return Ok(text);
        }

        let status_code = status.as_u16();

        if status_code == 429 {
            return Err(ClientError::RateLimitExceeded { retry_after });
        }

        #[derive(Deserialize)]
        struct ErrorBody {
            error: Option<String>,
            code: Option<ServerErrorCode>,
        }

        let (message, code) = if let Ok(body) = serde_json::from_str::<ErrorBody>(&text) {
            (body.error.unwrap_or_else(|| text.clone()), body.code)
        } else {
            (text, None)
        };

        match status_code {
            401 => Err(ClientError::Server {
                status: 401,
                message,
                code,
            }),
            403 => Err(ClientError::Authorization {
                status: 403,
                message,
                code,
            }),
            _ => Err(ClientError::Server {
                status: status_code,
                message,
                code,
            }),
        }
    }

    /// Execute a fallible async operation with retry logic and exponential backoff.
    ///
    /// Retries on transient errors (5xx, rate-limit, connection/timeout).
    /// Respects the `Retry-After` header when the server returns HTTP 429.
    /// Does NOT retry on 4xx client errors (except 429).
    ///
    /// NOTE: API call-site wiring is deferred to a follow-up (infrastructure PR).
    #[allow(dead_code)]
    pub(crate) async fn execute_with_retry<F, Fut, T>(&self, f: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let rc = &self.retry_config;

        for attempt in 0..rc.max_retries {
            match f().await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    let is_last = attempt == rc.max_retries - 1;
                    if is_last || !e.is_retryable() {
                        return Err(e);
                    }

                    let wait = match &e {
                        ClientError::RateLimitExceeded {
                            retry_after: Some(secs),
                        } => Duration::from_secs(*secs),
                        _ => {
                            let base_ms = rc.base_delay.as_millis() as f64;
                            let backoff_ms = base_ms * 2f64.powi(attempt as i32);
                            let capped_ms = backoff_ms.min(rc.max_delay.as_millis() as f64);
                            let final_ms = if rc.jitter {
                                // Simple deterministic jitter: vary between 50% and 150%
                                let seed = (attempt as u64).wrapping_mul(6364136223846793005);
                                let factor = 0.5 + (seed % 1000) as f64 / 1000.0;
                                capped_ms * factor
                            } else {
                                capped_ms
                            };
                            Duration::from_millis(final_ms as u64)
                        }
                    };

                    tokio::time::sleep(wait).await;
                }
            }
        }

        // Unreachable: the loop always returns on the last attempt
        Err(ClientError::Config("retry loop exhausted".to_string()))
    }
}

// ============================================================================
// ODE-2: GLiNER Entity Extraction (dakera-ode sidecar)
// ============================================================================

impl DakeraClient {
    /// Extract named entities from text using the GLiNER sidecar (ODE-2).
    ///
    /// Calls `POST /ode/extract` on the dakera-ode sidecar. Requires
    /// [`ode_url`][DakeraClientBuilder::ode_url] to be set on the builder.
    ///
    /// Unlike the CE-4 server-side NER, this method calls the dedicated GLiNER
    /// sidecar and returns character offsets, model name, and processing time.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Config`] if `ode_url` is not configured.
    pub async fn ode_extract_entities(
        &self,
        req: ExtractEntitiesRequest,
    ) -> Result<ExtractEntitiesResponse> {
        let ode_url = self.ode_url.as_deref().ok_or_else(|| {
            ClientError::Config(
                "ode_url must be configured to use extract_entities(). \
                 Call .ode_url(\"http://localhost:8080\") on the builder."
                    .to_string(),
            )
        })?;
        let url = format!("{}/ode/extract", ode_url);
        let response = self.client.post(&url).json(&req).send().await?;
        if response.status().is_success() {
            Ok(response.json::<ExtractEntitiesResponse>().await?)
        } else {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            Err(ClientError::Server {
                status,
                message: format!("ODE sidecar error: {}", body),
                code: None,
            })
        }
    }

    // ========================================================================
    // COG-1: Per-namespace Memory Lifecycle Policy
    // ========================================================================

    /// Return the memory lifecycle policy for a namespace (COG-1).
    ///
    /// Sends `GET /v1/namespaces/{namespace}/memory_policy`.
    ///
    /// When no explicit policy has been configured the server returns the COG-1
    /// defaults: working=4 h, episodic=30 d, semantic=365 d, procedural=730 d;
    /// exponential/power_law/logarithmic/flat decay curves; SR factor 1.0.
    #[instrument(skip(self))]
    pub async fn get_memory_policy(&self, namespace: &str) -> Result<MemoryPolicy> {
        let url = format!(
            "{}/v1/namespaces/{}/memory_policy",
            self.base_url,
            urlencoding::encode(namespace)
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Set the memory lifecycle policy for a namespace (COG-1).
    ///
    /// Sends `PUT /v1/namespaces/{namespace}/memory_policy`.
    ///
    /// The policy is persisted and applied immediately to the decay engine.
    /// Only populate the fields you want to override — all have safe defaults.
    #[instrument(skip(self, policy))]
    pub async fn set_memory_policy(
        &self,
        namespace: &str,
        policy: MemoryPolicy,
    ) -> Result<MemoryPolicy> {
        let url = format!(
            "{}/v1/namespaces/{}/memory_policy",
            self.base_url,
            urlencoding::encode(namespace)
        );
        let response = self.client.put(&url).json(&policy).send().await?;
        self.handle_response(response).await
    }
}

/// Builder for DakeraClient
#[derive(Debug)]
pub struct DakeraClientBuilder {
    base_url: String,
    api_key: Option<String>,
    ode_url: Option<String>,
    timeout: Duration,
    connect_timeout: Option<Duration>,
    retry_config: RetryConfig,
    user_agent: Option<String>,
}

impl DakeraClientBuilder {
    /// Create a new builder
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: None,
            ode_url: None,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            connect_timeout: None,
            retry_config: RetryConfig::default(),
            user_agent: None,
        }
    }

    /// Set the API key for Bearer authentication.
    ///
    /// If not set explicitly, the builder will try to read `DAKERA_API_KEY`
    /// from the environment at build time.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the base URL of the dakera-ode sidecar (ODE-2).
    ///
    /// Required to call [`DakeraClient::extract_entities`].
    pub fn ode_url(mut self, ode_url: impl Into<String>) -> Self {
        self.ode_url = Some(ode_url.into().trim_end_matches('/').to_string());
        self
    }

    /// Set the request timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the request timeout in seconds
    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout = Duration::from_secs(secs);
        self
    }

    /// Set the connection establishment timeout (defaults to `timeout`).
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    /// Set fine-grained retry configuration.
    pub fn retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// Set the maximum number of retry attempts.
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.retry_config.max_retries = max_retries;
        self
    }

    /// Set a custom user agent
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Build the client
    pub fn build(self) -> Result<DakeraClient> {
        // Normalize base URL (remove trailing slash)
        let base_url = self.base_url.trim_end_matches('/').to_string();

        // Validate URL
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            return Err(ClientError::InvalidUrl(
                "URL must start with http:// or https://".to_string(),
            ));
        }

        let user_agent = self
            .user_agent
            .unwrap_or_else(|| format!("dakera-client/{}", env!("CARGO_PKG_VERSION")));

        let connect_timeout = self.connect_timeout.unwrap_or(self.timeout);

        // Resolve API key: explicit > DAKERA_API_KEY env var
        let api_key = self
            .api_key
            .or_else(|| std::env::var("DAKERA_API_KEY").ok());

        let mut default_headers = HeaderMap::new();
        if let Some(key) = &api_key {
            let bearer = format!("Bearer {key}");
            let mut value = HeaderValue::from_str(&bearer)
                .map_err(|_| ClientError::Config("invalid API key".into()))?;
            value.set_sensitive(true);
            default_headers.insert(AUTHORIZATION, value);
        }

        let client = Client::builder()
            .timeout(self.timeout)
            .connect_timeout(connect_timeout)
            .user_agent(user_agent)
            .default_headers(default_headers)
            .build()
            .map_err(|e| ClientError::Config(e.to_string()))?;

        Ok(DakeraClient {
            client,
            base_url,
            ode_url: self.ode_url,
            retry_config: self.retry_config,
            last_rate_limit: Arc::new(Mutex::new(None)),
        })
    }
}

// ============================================================================
// SSE Streaming (CE-1)
// ============================================================================

impl DakeraClient {
    /// Subscribe to namespace-scoped SSE events.
    ///
    /// Opens a long-lived connection to `GET /v1/namespaces/{namespace}/events`
    /// and returns a [`tokio::sync::mpsc::Receiver`] that yields
    /// [`DakeraEvent`] results as they arrive.  The background task exits when
    /// the server closes the stream or the receiver is dropped.
    ///
    /// Requires a Read-scoped API key.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dakera_client::DakeraClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = DakeraClient::new("http://localhost:3000")?;
    ///     let mut rx = client.stream_namespace_events("my-ns").await?;
    ///     while let Some(result) = rx.recv().await {
    ///         println!("{:?}", result?);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn stream_namespace_events(
        &self,
        namespace: &str,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<crate::events::DakeraEvent>>> {
        let url = format!(
            "{}/v1/namespaces/{}/events",
            self.base_url,
            urlencoding::encode(namespace)
        );
        self.stream_sse(url).await
    }

    /// Subscribe to the global SSE event stream (all namespaces).
    ///
    /// Opens a long-lived connection to `GET /ops/events` and returns a
    /// [`tokio::sync::mpsc::Receiver`] that yields [`DakeraEvent`] results.
    ///
    /// Requires an Admin-scoped API key.
    pub async fn stream_global_events(
        &self,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<crate::events::DakeraEvent>>> {
        let url = format!("{}/ops/events", self.base_url);
        self.stream_sse(url).await
    }

    /// Subscribe to the memory lifecycle SSE event stream (DASH-B).
    ///
    /// Opens a long-lived connection to `GET /v1/events/stream` and returns a
    /// [`tokio::sync::mpsc::Receiver`] that yields [`MemoryEvent`] results as
    /// they arrive.  The background task exits when the server closes the stream
    /// or the receiver is dropped.
    ///
    /// Requires a Read-scoped API key.
    pub async fn stream_memory_events(
        &self,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<crate::events::MemoryEvent>>> {
        let url = format!("{}/v1/events/stream", self.base_url);
        self.stream_sse(url).await
    }

    /// Low-level generic SSE streaming helper.
    pub(crate) async fn stream_sse<T>(
        &self,
        url: String,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<T>>>
    where
        T: serde::de::DeserializeOwned + Send + 'static,
    {
        use futures_util::StreamExt;

        let response = self
            .client
            .get(&url)
            .header("Accept", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ClientError::Server {
                status,
                message: body,
                code: None,
            });
        }

        let (tx, rx) = tokio::sync::mpsc::channel(64);

        tokio::spawn(async move {
            let mut byte_stream = response.bytes_stream();
            let mut remaining = String::new();
            let mut data_lines: Vec<String> = Vec::new();

            while let Some(chunk) = byte_stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        remaining.push_str(&String::from_utf8_lossy(&bytes));
                        while let Some(pos) = remaining.find('\n') {
                            let raw = &remaining[..pos];
                            let line = raw.trim_end_matches('\r').to_string();
                            remaining = remaining[pos + 1..].to_string();

                            if line.starts_with(':') {
                                // SSE comment / heartbeat — skip
                            } else if let Some(data) = line.strip_prefix("data:") {
                                data_lines.push(data.trim_start().to_string());
                            } else if line.is_empty() {
                                if !data_lines.is_empty() {
                                    let payload = data_lines.join("\n");
                                    data_lines.clear();
                                    let result = serde_json::from_str::<T>(&payload)
                                        .map_err(ClientError::Json);
                                    if tx.send(result).await.is_err() {
                                        return; // receiver dropped
                                    }
                                }
                            } else {
                                // Unrecognised field (e.g. "event:") — ignore
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(ClientError::Http(e))).await;
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_builder() {
        let client = DakeraClient::new("http://localhost:3000");
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_builder_with_options() {
        let client = DakeraClient::builder("http://localhost:3000")
            .timeout_secs(60)
            .user_agent("test-client/1.0")
            .build();
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_builder_invalid_url() {
        let client = DakeraClient::new("invalid-url");
        assert!(client.is_err());
    }

    #[test]
    fn test_client_builder_trailing_slash() {
        let client = DakeraClient::new("http://localhost:3000/").unwrap();
        assert!(!client.base_url.ends_with('/'));
    }

    #[test]
    fn test_vector_creation() {
        let v = Vector::new("test", vec![0.1, 0.2, 0.3]);
        assert_eq!(v.id, "test");
        assert_eq!(v.values.len(), 3);
        assert!(v.metadata.is_none());
    }

    #[test]
    fn test_query_request_builder() {
        let req = QueryRequest::new(vec![0.1, 0.2], 10)
            .with_filter(serde_json::json!({"category": "test"}))
            .include_metadata(false);

        assert_eq!(req.top_k, 10);
        assert!(req.filter.is_some());
        assert!(!req.include_metadata);
    }

    #[test]
    fn test_hybrid_search_request() {
        let req = HybridSearchRequest::new(vec![0.1], "test query", 5).with_vector_weight(0.7);

        assert_eq!(req.vector_weight, 0.7);
        assert_eq!(req.text, "test query");
        assert!(req.vector.is_some());
    }

    #[test]
    fn test_hybrid_search_weight_clamping() {
        let req = HybridSearchRequest::new(vec![0.1], "test", 5).with_vector_weight(1.5); // Should be clamped to 1.0

        assert_eq!(req.vector_weight, 1.0);
    }

    #[test]
    fn test_hybrid_search_text_only() {
        let req = HybridSearchRequest::text_only("bm25 query", 10);

        assert!(req.vector.is_none());
        assert_eq!(req.text, "bm25 query");
        assert_eq!(req.top_k, 10);
        // Verify vector is not serialised
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("vector").is_none());
    }

    #[test]
    fn test_text_document_builder() {
        let doc = TextDocument::new("doc1", "Hello world").with_ttl(3600);

        assert_eq!(doc.id, "doc1");
        assert_eq!(doc.text, "Hello world");
        assert_eq!(doc.ttl_seconds, Some(3600));
        assert!(doc.metadata.is_none());
    }

    #[test]
    fn test_upsert_text_request_builder() {
        let docs = vec![
            TextDocument::new("doc1", "Hello"),
            TextDocument::new("doc2", "World"),
        ];
        let req = UpsertTextRequest::new(docs).with_model(EmbeddingModel::BgeSmall);

        assert_eq!(req.documents.len(), 2);
        assert_eq!(req.model, Some(EmbeddingModel::BgeSmall));
    }

    #[test]
    fn test_query_text_request_builder() {
        let req = QueryTextRequest::new("semantic search query", 5)
            .with_filter(serde_json::json!({"category": "docs"}))
            .include_vectors(true)
            .with_model(EmbeddingModel::E5Small);

        assert_eq!(req.text, "semantic search query");
        assert_eq!(req.top_k, 5);
        assert!(req.filter.is_some());
        assert!(req.include_vectors);
        assert_eq!(req.model, Some(EmbeddingModel::E5Small));
    }

    #[test]
    fn test_fetch_request_builder() {
        let req = FetchRequest::new(vec!["id1".to_string(), "id2".to_string()]);

        assert_eq!(req.ids.len(), 2);
        assert!(req.include_values);
        assert!(req.include_metadata);
    }

    #[test]
    fn test_create_namespace_request_builder() {
        let req = CreateNamespaceRequest::new()
            .with_dimensions(384)
            .with_index_type("hnsw");

        assert_eq!(req.dimensions, Some(384));
        assert_eq!(req.index_type.as_deref(), Some("hnsw"));
    }

    #[test]
    fn test_batch_query_text_request() {
        let req =
            BatchQueryTextRequest::new(vec!["query one".to_string(), "query two".to_string()], 10);

        assert_eq!(req.queries.len(), 2);
        assert_eq!(req.top_k, 10);
        assert!(!req.include_vectors);
        assert!(req.model.is_none());
    }

    // =========================================================================
    // RetryConfig tests
    // =========================================================================

    #[test]
    fn test_retry_config_defaults() {
        let rc = RetryConfig::default();
        assert_eq!(rc.max_retries, 3);
        assert_eq!(rc.base_delay, Duration::from_millis(100));
        assert_eq!(rc.max_delay, Duration::from_secs(60));
        assert!(rc.jitter);
    }

    #[test]
    fn test_builder_connect_timeout() {
        let client = DakeraClient::builder("http://localhost:3000")
            .connect_timeout(Duration::from_secs(5))
            .timeout_secs(30)
            .build()
            .unwrap();
        // Client was built successfully with separate connect timeout
        assert!(client.base_url.starts_with("http"));
    }

    #[test]
    fn test_builder_max_retries() {
        let client = DakeraClient::builder("http://localhost:3000")
            .max_retries(5)
            .build()
            .unwrap();
        assert_eq!(client.retry_config.max_retries, 5);
    }

    #[test]
    fn test_builder_retry_config() {
        let rc = RetryConfig {
            max_retries: 7,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(30),
            jitter: false,
        };
        let client = DakeraClient::builder("http://localhost:3000")
            .retry_config(rc)
            .build()
            .unwrap();
        assert_eq!(client.retry_config.max_retries, 7);
        assert!(!client.retry_config.jitter);
    }

    #[test]
    fn test_rate_limit_error_retryable() {
        let e = ClientError::RateLimitExceeded { retry_after: None };
        assert!(e.is_retryable());
    }

    #[test]
    fn test_server_408_retryable() {
        let e = ClientError::Server {
            status: 408,
            message: String::new(),
            code: None,
        };
        assert!(e.is_retryable());
    }

    #[test]
    fn test_server_400_not_retryable() {
        let e = ClientError::Server {
            status: 400,
            message: String::new(),
            code: None,
        };
        assert!(!e.is_retryable());
    }

    #[test]
    fn test_rate_limit_error_with_retry_after_zero() {
        // retry_after: Some(0) should still be Some, not treated as missing
        let e = ClientError::RateLimitExceeded {
            retry_after: Some(0),
        };
        assert!(e.is_retryable());
        if let ClientError::RateLimitExceeded {
            retry_after: Some(secs),
        } = &e
        {
            assert_eq!(*secs, 0u64);
        } else {
            panic!("unexpected variant");
        }
    }

    #[tokio::test]
    async fn test_execute_with_retry_succeeds_immediately() {
        let client = DakeraClient::builder("http://localhost:3000")
            .max_retries(3)
            .build()
            .unwrap();

        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let cc = call_count.clone();
        let result = client
            .execute_with_retry(|| {
                let cc = cc.clone();
                async move {
                    cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok::<u32, ClientError>(42)
                }
            })
            .await;
        assert_eq!(result.unwrap(), 42);
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_execute_with_retry_no_retry_on_4xx() {
        let client = DakeraClient::builder("http://localhost:3000")
            .max_retries(3)
            .build()
            .unwrap();

        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let cc = call_count.clone();
        let result = client
            .execute_with_retry(|| {
                let cc = cc.clone();
                async move {
                    cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Err::<u32, ClientError>(ClientError::Server {
                        status: 400,
                        message: "bad request".to_string(),
                        code: None,
                    })
                }
            })
            .await;
        assert!(result.is_err());
        // Should not retry on 4xx
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_execute_with_retry_retries_on_5xx() {
        let client = DakeraClient::builder("http://localhost:3000")
            .retry_config(RetryConfig {
                max_retries: 3,
                base_delay: Duration::from_millis(0),
                max_delay: Duration::from_millis(0),
                jitter: false,
            })
            .build()
            .unwrap();

        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let cc = call_count.clone();
        let result = client
            .execute_with_retry(|| {
                let cc = cc.clone();
                async move {
                    let n = cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if n < 2 {
                        Err::<u32, ClientError>(ClientError::Server {
                            status: 503,
                            message: "unavailable".to_string(),
                            code: None,
                        })
                    } else {
                        Ok(99)
                    }
                }
            })
            .await;
        assert_eq!(result.unwrap(), 99);
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    // =========================================================================
    // CE-2: Batch Recall / Forget (v0.7.0)
    // =========================================================================

    #[test]
    fn test_batch_recall_request_new() {
        use crate::memory::BatchRecallRequest;
        let req = BatchRecallRequest::new("agent-1");
        assert_eq!(req.agent_id, "agent-1");
        assert_eq!(req.limit, 100);
    }

    #[test]
    fn test_batch_recall_request_builder() {
        use crate::memory::{BatchMemoryFilter, BatchRecallRequest};
        let filter = BatchMemoryFilter::default()
            .with_tags(vec!["qa".to_string()])
            .with_min_importance(0.7);
        let req = BatchRecallRequest::new("agent-1")
            .with_filter(filter)
            .with_limit(50);
        assert_eq!(req.agent_id, "agent-1");
        assert_eq!(req.limit, 50);
        assert_eq!(
            req.filter.tags.as_deref(),
            Some(["qa".to_string()].as_slice())
        );
        assert_eq!(req.filter.min_importance, Some(0.7));
    }

    #[test]
    fn test_batch_recall_request_serialization() {
        use crate::memory::{BatchMemoryFilter, BatchRecallRequest};
        let filter = BatchMemoryFilter::default().with_min_importance(0.5);
        let req = BatchRecallRequest::new("agent-1")
            .with_filter(filter)
            .with_limit(25);
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["agent_id"], "agent-1");
        assert_eq!(json["limit"], 25);
        assert_eq!(json["filter"]["min_importance"], 0.5);
    }

    #[test]
    fn test_batch_forget_request_new() {
        use crate::memory::{BatchForgetRequest, BatchMemoryFilter};
        let filter = BatchMemoryFilter::default().with_min_importance(0.1);
        let req = BatchForgetRequest::new("agent-1", filter);
        assert_eq!(req.agent_id, "agent-1");
        assert_eq!(req.filter.min_importance, Some(0.1));
    }

    #[test]
    fn test_batch_forget_request_serialization() {
        use crate::memory::{BatchForgetRequest, BatchMemoryFilter};
        let filter = BatchMemoryFilter {
            created_before: Some(1_700_000_000),
            ..Default::default()
        };
        let req = BatchForgetRequest::new("agent-1", filter);
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["agent_id"], "agent-1");
        assert_eq!(json["filter"]["created_before"], 1_700_000_000u64);
    }

    #[test]
    fn test_batch_recall_response_deserialization() {
        use crate::memory::BatchRecallResponse;
        let json = serde_json::json!({
            "memories": [],
            "total": 42,
            "filtered": 7
        });
        let resp: BatchRecallResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.total, 42);
        assert_eq!(resp.filtered, 7);
        assert!(resp.memories.is_empty());
    }

    #[test]
    fn test_batch_forget_response_deserialization() {
        use crate::memory::BatchForgetResponse;
        let json = serde_json::json!({ "deleted_count": 13 });
        let resp: BatchForgetResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.deleted_count, 13);
    }

    // =========================================================================
    // OPS-1: RateLimitHeaders (v0.7.0)
    // =========================================================================

    #[test]
    fn test_rate_limit_headers_default_all_none() {
        use crate::types::RateLimitHeaders;
        let rl = RateLimitHeaders {
            limit: None,
            remaining: None,
            reset: None,
            quota_used: None,
            quota_limit: None,
        };
        assert!(rl.limit.is_none());
        assert!(rl.remaining.is_none());
        assert!(rl.reset.is_none());
        assert!(rl.quota_used.is_none());
        assert!(rl.quota_limit.is_none());
    }

    #[test]
    fn test_rate_limit_headers_populated() {
        use crate::types::RateLimitHeaders;
        let rl = RateLimitHeaders {
            limit: Some(1000),
            remaining: Some(750),
            reset: Some(1_700_000_060),
            quota_used: Some(500),
            quota_limit: Some(10_000),
        };
        assert_eq!(rl.limit, Some(1000));
        assert_eq!(rl.remaining, Some(750));
        assert_eq!(rl.reset, Some(1_700_000_060));
        assert_eq!(rl.quota_used, Some(500));
        assert_eq!(rl.quota_limit, Some(10_000));
    }

    #[test]
    fn test_last_rate_limit_headers_initially_none() {
        let client = DakeraClient::new("http://localhost:3000").unwrap();
        assert!(client.last_rate_limit_headers().is_none());
    }

    // =========================================================================
    // CE-4: GLiNER Entity Extraction
    // =========================================================================

    #[test]
    fn test_namespace_ner_config_default() {
        use crate::types::NamespaceNerConfig;
        let cfg = NamespaceNerConfig::default();
        assert!(!cfg.extract_entities);
        assert!(cfg.entity_types.is_none());
    }

    #[test]
    fn test_namespace_ner_config_serialization_skip_none() {
        use crate::types::NamespaceNerConfig;
        let cfg = NamespaceNerConfig {
            extract_entities: true,
            entity_types: None,
        };
        let json = serde_json::to_value(&cfg).unwrap();
        assert_eq!(json["extract_entities"], true);
        // entity_types should be omitted when None
        assert!(json.get("entity_types").is_none());
    }

    #[test]
    fn test_namespace_ner_config_serialization_with_types() {
        use crate::types::NamespaceNerConfig;
        let cfg = NamespaceNerConfig {
            extract_entities: true,
            entity_types: Some(vec!["PERSON".to_string(), "ORG".to_string()]),
        };
        let json = serde_json::to_value(&cfg).unwrap();
        assert_eq!(json["extract_entities"], true);
        assert_eq!(json["entity_types"][0], "PERSON");
        assert_eq!(json["entity_types"][1], "ORG");
    }

    #[test]
    fn test_extracted_entity_deserialization() {
        use crate::types::ExtractedEntity;
        let json = serde_json::json!({
            "entity_type": "PERSON",
            "value": "Alice",
            "score": 0.95
        });
        let entity: ExtractedEntity = serde_json::from_value(json).unwrap();
        assert_eq!(entity.entity_type, "PERSON");
        assert_eq!(entity.value, "Alice");
        assert!((entity.score - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn test_entity_extraction_response_deserialization() {
        use crate::types::EntityExtractionResponse;
        let json = serde_json::json!({
            "entities": [
                { "entity_type": "PERSON", "value": "Bob", "score": 0.9 },
                { "entity_type": "ORG",    "value": "Acme", "score": 0.87 }
            ]
        });
        let resp: EntityExtractionResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.entities.len(), 2);
        assert_eq!(resp.entities[0].entity_type, "PERSON");
        assert_eq!(resp.entities[1].value, "Acme");
    }

    #[test]
    fn test_memory_entities_response_deserialization() {
        use crate::types::MemoryEntitiesResponse;
        let json = serde_json::json!({
            "memory_id": "mem-abc-123",
            "entities": [
                { "entity_type": "LOC", "value": "London", "score": 0.88 }
            ]
        });
        let resp: MemoryEntitiesResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.memory_id, "mem-abc-123");
        assert_eq!(resp.entities.len(), 1);
        assert_eq!(resp.entities[0].entity_type, "LOC");
        assert_eq!(resp.entities[0].value, "London");
    }

    #[test]
    fn test_configure_namespace_ner_url_pattern() {
        // Verify the client is constructable and base_url is correct
        let client = DakeraClient::new("http://localhost:3000").unwrap();
        let expected = "http://localhost:3000/v1/namespaces/my-ns/config";
        let actual = format!("{}/v1/namespaces/{}/config", client.base_url, "my-ns");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_extract_entities_url_pattern() {
        let client = DakeraClient::new("http://localhost:3000").unwrap();
        let expected = "http://localhost:3000/v1/memories/extract";
        let actual = format!("{}/v1/memories/extract", client.base_url);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_memory_entities_url_pattern() {
        let client = DakeraClient::new("http://localhost:3000").unwrap();
        let memory_id = "mem-xyz-789";
        let expected = "http://localhost:3000/v1/memory/entities/mem-xyz-789";
        let actual = format!("{}/v1/memory/entities/{}", client.base_url, memory_id);
        assert_eq!(actual, expected);
    }

    // ========================================================================
    // INT-1 Memory Feedback Loop tests
    // ========================================================================

    #[test]
    fn test_feedback_signal_serialization() {
        use crate::types::FeedbackSignal;
        let upvote = serde_json::to_value(FeedbackSignal::Upvote).unwrap();
        assert_eq!(upvote, serde_json::json!("upvote"));
        let downvote = serde_json::to_value(FeedbackSignal::Downvote).unwrap();
        assert_eq!(downvote, serde_json::json!("downvote"));
        let flag = serde_json::to_value(FeedbackSignal::Flag).unwrap();
        assert_eq!(flag, serde_json::json!("flag"));
    }

    #[test]
    fn test_feedback_signal_deserialization() {
        use crate::types::FeedbackSignal;
        let signal: FeedbackSignal = serde_json::from_str("\"upvote\"").unwrap();
        assert_eq!(signal, FeedbackSignal::Upvote);
        let signal: FeedbackSignal = serde_json::from_str("\"positive\"").unwrap();
        assert_eq!(signal, FeedbackSignal::Positive);
    }

    #[test]
    fn test_feedback_response_deserialization() {
        use crate::types::{FeedbackResponse, FeedbackSignal};
        let json = serde_json::json!({
            "memory_id": "mem-abc",
            "new_importance": 0.92,
            "signal": "upvote"
        });
        let resp: FeedbackResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.memory_id, "mem-abc");
        assert!((resp.new_importance - 0.92).abs() < f32::EPSILON);
        assert_eq!(resp.signal, FeedbackSignal::Upvote);
    }

    #[test]
    fn test_feedback_history_response_deserialization() {
        use crate::types::{FeedbackHistoryResponse, FeedbackSignal};
        let json = serde_json::json!({
            "memory_id": "mem-abc",
            "entries": [
                {"signal": "upvote", "timestamp": 1774000000_u64, "old_importance": 0.5, "new_importance": 0.575},
                {"signal": "downvote", "timestamp": 1774001000_u64, "old_importance": 0.575, "new_importance": 0.489}
            ]
        });
        let resp: FeedbackHistoryResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.memory_id, "mem-abc");
        assert_eq!(resp.entries.len(), 2);
        assert_eq!(resp.entries[0].signal, FeedbackSignal::Upvote);
        assert_eq!(resp.entries[1].signal, FeedbackSignal::Downvote);
    }

    #[test]
    fn test_agent_feedback_summary_deserialization() {
        use crate::types::AgentFeedbackSummary;
        let json = serde_json::json!({
            "agent_id": "agent-1",
            "upvotes": 42_u64,
            "downvotes": 7_u64,
            "flags": 2_u64,
            "total_feedback": 51_u64,
            "health_score": 0.78
        });
        let summary: AgentFeedbackSummary = serde_json::from_value(json).unwrap();
        assert_eq!(summary.agent_id, "agent-1");
        assert_eq!(summary.upvotes, 42);
        assert_eq!(summary.total_feedback, 51);
        assert!((summary.health_score - 0.78).abs() < f32::EPSILON);
    }

    #[test]
    fn test_feedback_health_response_deserialization() {
        use crate::types::FeedbackHealthResponse;
        let json = serde_json::json!({
            "agent_id": "agent-1",
            "health_score": 0.78,
            "memory_count": 120_usize,
            "avg_importance": 0.72
        });
        let health: FeedbackHealthResponse = serde_json::from_value(json).unwrap();
        assert_eq!(health.agent_id, "agent-1");
        assert!((health.health_score - 0.78).abs() < f32::EPSILON);
        assert_eq!(health.memory_count, 120);
    }

    #[test]
    fn test_memory_feedback_body_serialization() {
        use crate::types::{FeedbackSignal, MemoryFeedbackBody};
        let body = MemoryFeedbackBody {
            agent_id: "agent-1".to_string(),
            signal: FeedbackSignal::Flag,
        };
        let json = serde_json::to_value(body).unwrap();
        assert_eq!(json["agent_id"], "agent-1");
        assert_eq!(json["signal"], "flag");
    }

    #[test]
    fn test_feedback_memory_url_pattern() {
        let client = DakeraClient::new("http://localhost:3000").unwrap();
        let memory_id = "mem-abc";
        let expected_post = "http://localhost:3000/v1/memories/mem-abc/feedback";
        let actual_post = format!("{}/v1/memories/{}/feedback", client.base_url, memory_id);
        assert_eq!(actual_post, expected_post);

        let expected_patch = "http://localhost:3000/v1/memories/mem-abc/importance";
        let actual_patch = format!("{}/v1/memories/{}/importance", client.base_url, memory_id);
        assert_eq!(actual_patch, expected_patch);
    }

    #[test]
    fn test_feedback_health_url_pattern() {
        let client = DakeraClient::new("http://localhost:3000").unwrap();
        let agent_id = "agent-1";
        let expected = "http://localhost:3000/v1/feedback/health?agent_id=agent-1";
        let actual = format!(
            "{}/v1/feedback/health?agent_id={}",
            client.base_url, agent_id
        );
        assert_eq!(actual, expected);
    }

    // ODE-2 tests
    #[test]
    fn test_ode_extract_entities_requires_ode_url() {
        // Client without ode_url should return Config error.
        let client = DakeraClient::new("http://localhost:3000").unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(client.ode_extract_entities(ExtractEntitiesRequest {
            content: "Alice lives in Paris.".to_string(),
            agent_id: "agent-1".to_string(),
            memory_id: None,
            entity_types: None,
        }));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ClientError::Config(_)));
    }

    #[test]
    fn test_ode_extract_entities_url_built_from_ode_url() {
        // Verify the ODE URL is used, not base_url.
        let client = DakeraClient::builder("http://localhost:3000")
            .ode_url("http://localhost:8080")
            .build()
            .unwrap();
        assert_eq!(client.ode_url.as_deref(), Some("http://localhost:8080"));
        let expected = "http://localhost:8080/ode/extract";
        let actual = format!("{}/ode/extract", client.ode_url.as_deref().unwrap());
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_extract_entities_request_serialization() {
        let req = ExtractEntitiesRequest {
            content: "Alice in Wonderland".to_string(),
            agent_id: "agent-42".to_string(),
            memory_id: Some("mem-001".to_string()),
            entity_types: Some(vec!["person".to_string(), "location".to_string()]),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"content\":\"Alice in Wonderland\""));
        assert!(json.contains("\"agent_id\":\"agent-42\""));
        assert!(json.contains("\"memory_id\":\"mem-001\""));
        assert!(json.contains("\"person\""));
    }

    #[test]
    fn test_extract_entities_request_omits_none_fields() {
        let req = ExtractEntitiesRequest {
            content: "hello".to_string(),
            agent_id: "a".to_string(),
            memory_id: None,
            entity_types: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("memory_id"));
        assert!(!json.contains("entity_types"));
    }

    #[test]
    fn test_ode_entity_deserialization() {
        let json = r#"{"text":"Alice","label":"person","start":0,"end":5,"score":0.97}"#;
        let entity: OdeEntity = serde_json::from_str(json).unwrap();
        assert_eq!(entity.text, "Alice");
        assert_eq!(entity.label, "person");
        assert_eq!(entity.start, 0);
        assert_eq!(entity.end, 5);
        assert!((entity.score - 0.97).abs() < 1e-4);
    }

    #[test]
    fn test_extract_entities_response_deserialization() {
        let json = r#"{
            "entities": [
                {"text":"Alice","label":"person","start":0,"end":5,"score":0.97},
                {"text":"Paris","label":"location","start":16,"end":21,"score":0.92}
            ],
            "model": "gliner-multi-v2.1",
            "processing_time_ms": 34
        }"#;
        let resp: ExtractEntitiesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.entities.len(), 2);
        assert_eq!(resp.entities[0].text, "Alice");
        assert_eq!(resp.model, "gliner-multi-v2.1");
        assert_eq!(resp.processing_time_ms, 34);
    }
}
