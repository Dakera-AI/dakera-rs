//! Dakera client implementation

use reqwest::{Client, StatusCode};
use std::time::Duration;
use tracing::{debug, instrument};

use crate::error::{ClientError, Result};
use crate::types::*;

/// Default timeout for requests
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Dakeraclient for interacting with the vector database
#[derive(Debug, Clone)]
pub struct DakeraClient {
    /// HTTP client
    pub(crate) client: Client,
    /// Base URL of the Dakera server
    pub(crate) base_url: String,
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
            Ok(response.json().await?)
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
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
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
    // Private Helpers
    // ========================================================================

    /// Handle response and deserialize JSON
    pub(crate) async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T> {
        let status = response.status();

        if status.is_success() {
            Ok(response.json().await?)
        } else {
            let status_code = status.as_u16();
            let text = response.text().await.unwrap_or_default();

            // Try to parse error message from JSON
            let message = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                json.get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or(&text)
                    .to_string()
            } else {
                text
            };

            Err(ClientError::Server {
                status: status_code,
                message,
            })
        }
    }
}

/// Builder for DakeraClient
#[derive(Debug)]
pub struct DakeraClientBuilder {
    base_url: String,
    timeout: Duration,
    user_agent: Option<String>,
}

impl DakeraClientBuilder {
    /// Create a new builder
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            user_agent: None,
        }
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

        let client = Client::builder()
            .timeout(self.timeout)
            .user_agent(user_agent)
            .build()
            .map_err(|e| ClientError::Config(e.to_string()))?;

        Ok(DakeraClient { client, base_url })
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
    async fn stream_sse<T>(
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
    }

    #[test]
    fn test_hybrid_search_weight_clamping() {
        let req = HybridSearchRequest::new(vec![0.1], "test", 5).with_vector_weight(1.5); // Should be clamped to 1.0

        assert_eq!(req.vector_weight, 1.0);
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
}
