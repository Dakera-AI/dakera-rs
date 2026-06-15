//! Unit tests for memory and session methods using mockito.
//!
//! Covers methods not already tested in integration_test.rs:
//! search_memories, consolidate, compress_agent, consolidate_agent,
//! session lifecycle (start_session, get_session, end_session, list_sessions,
//! session_memories, start_session_with_metadata), batch_forget,
//! bulk_update_vectors, bulk_delete_vectors, count_vectors,
//! multi_vector_search, unified_query, aggregate, export_vectors,
//! explain_query, upsert_columns, warm_cache.

use dakera_client::DakeraClient;

// ============================================================================
// Memory: search_memories
// ============================================================================

#[tokio::test]
async fn test_search_memories() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/memory/search")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "memories": [
                    {"id": "m1", "content": "User likes Python", "score": 0.92, "importance": 0.8, "tags": ["lang"], "memory_type": "semantic", "created_at": "2024-01-01T00:00:00Z"}
                ]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::memory::RecallRequest::new("agent-1", "programming language");
    let result = client.search_memories(request).await.unwrap();
    assert_eq!(result.memories.len(), 1);
    assert_eq!(result.memories[0].id, "m1");
    mock.assert_async().await;
}

// ============================================================================
// Memory: consolidate
// ============================================================================

#[tokio::test]
async fn test_consolidate() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/memory/consolidate")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "memories_removed": 3,
                "source_memory_ids": ["m1", "m2", "m3"],
                "consolidated_memory": {"id": "m-new", "content": "consolidated"}
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::memory::ConsolidateRequest {
        threshold: Some(0.9),
        dry_run: false,
        ..Default::default()
    };
    let result = client.consolidate("agent-1", request).await.unwrap();
    assert_eq!(result.consolidated_count, 3);
    assert_eq!(result.removed_count, 3);
    assert_eq!(result.new_memories.len(), 3);
    mock.assert_async().await;
}

// ============================================================================
// Agent: compress_agent
// ============================================================================

#[tokio::test]
async fn test_compress_agent() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/agents/agent-1/compress")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "agent_id": "agent-1",
                "memories_scanned": 100,
                "originals_deprecated": 15
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.compress_agent("agent-1").await.unwrap();
    assert_eq!(result.agent_id, "agent-1");
    assert_eq!(result.memories_scanned, 100);
    assert_eq!(result.originals_deprecated, 15);
    mock.assert_async().await;
}

// ============================================================================
// Agent: consolidate_agent
// ============================================================================

#[tokio::test]
async fn test_consolidate_agent() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/agents/agent-1/consolidate")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "agent_id": "agent-1",
                "memories_scanned": 200,
                "clusters_found": 5,
                "memories_deprecated": 12,
                "anchor_ids": ["a1", "a2"],
                "deprecated_ids": ["d1", "d2", "d3"]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.consolidate_agent("agent-1").await.unwrap();
    assert_eq!(result.agent_id, "agent-1");
    assert_eq!(result.memories_scanned, 200);
    assert_eq!(result.clusters_found, 5);
    assert_eq!(result.memories_deprecated, 12);
    mock.assert_async().await;
}

// ============================================================================
// Session: start_session
// ============================================================================

#[tokio::test]
async fn test_start_session() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/sessions/start")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "session": {
                    "id": "sess-abc",
                    "agent_id": "agent-1",
                    "started_at": 1700000000,
                    "memory_count": 0
                }
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let session = client.start_session("agent-1").await.unwrap();
    assert_eq!(session.id, "sess-abc");
    assert_eq!(session.agent_id, "agent-1");
    mock.assert_async().await;
}

// ============================================================================
// Session: start_session_with_metadata
// ============================================================================

#[tokio::test]
async fn test_start_session_with_metadata() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/sessions/start")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "session": {
                    "id": "sess-meta",
                    "agent_id": "agent-1",
                    "started_at": 1700000000,
                    "metadata": {"source": "test"},
                    "memory_count": 0
                }
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let metadata = serde_json::json!({"source": "test"});
    let session = client
        .start_session_with_metadata("agent-1", metadata)
        .await
        .unwrap();
    assert_eq!(session.id, "sess-meta");
    assert_eq!(session.metadata.unwrap()["source"], "test");
    mock.assert_async().await;
}

// ============================================================================
// Session: get_session
// ============================================================================

#[tokio::test]
async fn test_get_session() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/sessions/sess-123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "id": "sess-123",
                "agent_id": "agent-1",
                "started_at": 1700000000,
                "ended_at": 1700003600,
                "memory_count": 5,
                "summary": "Session about coffee preferences"
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let session = client.get_session("sess-123").await.unwrap();
    assert_eq!(session.id, "sess-123");
    assert_eq!(session.ended_at, Some(1700003600));
    assert_eq!(session.memory_count, 5);
    mock.assert_async().await;
}

// ============================================================================
// Session: end_session
// ============================================================================

#[tokio::test]
async fn test_end_session() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/sessions/sess-123/end")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "session": {
                    "id": "sess-123",
                    "agent_id": "agent-1",
                    "started_at": 1700000000,
                    "ended_at": 1700003600,
                    "summary": "User discussed preferences",
                    "memory_count": 8
                },
                "memory_count": 8
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client
        .end_session("sess-123", Some("User discussed preferences".to_string()))
        .await
        .unwrap();
    assert_eq!(result.session.id, "sess-123");
    assert_eq!(result.memory_count, 8);
    mock.assert_async().await;
}

// ============================================================================
// Session: list_sessions
// ============================================================================

#[tokio::test]
async fn test_list_sessions() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/sessions?agent_id=agent-1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "sessions": [
                    {"id": "s1", "agent_id": "agent-1", "started_at": 1700000000, "memory_count": 3},
                    {"id": "s2", "agent_id": "agent-1", "started_at": 1700010000, "memory_count": 7}
                ],
                "total": 2
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let sessions = client.list_sessions("agent-1").await.unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].id, "s1");
    assert_eq!(sessions[1].memory_count, 7);
    mock.assert_async().await;
}

// ============================================================================
// Session: session_memories
// ============================================================================

#[tokio::test]
async fn test_session_memories() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/sessions/sess-123/memories")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "memories": [
                    {"id": "m1", "content": "session memory 1", "score": 1.0, "importance": 0.7, "tags": [], "memory_type": "episodic", "created_at": "2024-01-01T00:00:00Z"},
                    {"id": "m2", "content": "session memory 2", "score": 1.0, "importance": 0.8, "tags": [], "memory_type": "episodic", "created_at": "2024-01-01T00:01:00Z"}
                ]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.session_memories("sess-123").await.unwrap();
    assert_eq!(result.memories.len(), 2);
    assert_eq!(result.memories[0].content, "session memory 1");
    mock.assert_async().await;
}

// ============================================================================
// Batch: batch_forget
// ============================================================================

#[tokio::test]
async fn test_batch_forget() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("DELETE", "/v1/memories/forget/batch")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deleted_count": 5, "agent_id": "agent-1"}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let filter = dakera_client::memory::BatchMemoryFilter::default().with_min_importance(0.0);
    let request = dakera_client::memory::BatchForgetRequest::new("agent-1", filter);
    let result = client.batch_forget(request).await.unwrap();
    assert_eq!(result.deleted_count, 5);
    mock.assert_async().await;
}

// ============================================================================
// Vector bulk: bulk_update_vectors
// ============================================================================

#[tokio::test]
async fn test_bulk_update_vectors() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/vectors/bulk-update")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"updated": 10, "failed": 0, "errors": []}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::BulkUpdateRequest {
        filter: serde_json::json!({"category": {"$eq": "old"}}),
        update: serde_json::json!({"category": "archived"}),
    };
    let result = client
        .bulk_update_vectors("test-ns", request)
        .await
        .unwrap();
    assert_eq!(result.updated, 10);
    assert_eq!(result.failed, 0);
    mock.assert_async().await;
}

// ============================================================================
// Vector bulk: bulk_delete_vectors
// ============================================================================

#[tokio::test]
async fn test_bulk_delete_vectors() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/vectors/bulk-delete")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deleted": 25, "failed": 1, "errors": ["vec-999: not found"]}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::BulkDeleteRequest {
        filter: serde_json::json!({"status": {"$eq": "expired"}}),
    };
    let result = client
        .bulk_delete_vectors("test-ns", request)
        .await
        .unwrap();
    assert_eq!(result.deleted, 25);
    assert_eq!(result.failed, 1);
    assert_eq!(result.errors.len(), 1);
    mock.assert_async().await;
}

// ============================================================================
// Vector bulk: count_vectors
// ============================================================================

#[tokio::test]
async fn test_count_vectors() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/vectors/count")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"count": 42000, "namespace": "test-ns"}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::CountVectorsRequest { filter: None };
    let result = client.count_vectors("test-ns", request).await.unwrap();
    assert_eq!(result.count, 42000);
    assert_eq!(result.namespace, "test-ns");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_count_vectors_with_filter() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/vectors/count")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"count": 150, "namespace": "test-ns"}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::CountVectorsRequest {
        filter: Some(serde_json::json!({"category": {"$eq": "active"}})),
    };
    let result = client.count_vectors("test-ns", request).await.unwrap();
    assert_eq!(result.count, 150);
    mock.assert_async().await;
}

// ============================================================================
// Vector: multi_vector_search
// ============================================================================

#[tokio::test]
async fn test_multi_vector_search() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/multi-vector")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "results": [
                    {"id": "v1", "score": 0.95},
                    {"id": "v2", "score": 0.88}
                ]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::MultiVectorSearchRequest::new(vec![
        vec![0.1, 0.2, 0.3],
        vec![0.4, 0.5, 0.6],
    ])
    .with_top_k(10);
    let result = client
        .multi_vector_search("test-ns", request)
        .await
        .unwrap();
    assert_eq!(result.results.len(), 2);
    assert_eq!(result.results[0].id, "v1");
    mock.assert_async().await;
}

// ============================================================================
// Vector: unified_query
// ============================================================================

#[tokio::test]
async fn test_unified_query_vector_search() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/unified-query")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "results": [
                    {"id": "u1", "$dist": 0.12},
                    {"id": "u2", "$dist": 0.25}
                ]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::UnifiedQueryRequest::vector_search(vec![0.1, 0.2, 0.3], 5);
    let result = client.unified_query("test-ns", request).await.unwrap();
    assert_eq!(result.results.len(), 2);
    assert_eq!(result.results[0].id, "u1");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_unified_query_fulltext_search() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/unified-query")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "results": [
                    {"id": "ft1", "$dist": 0.05}
                ]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::UnifiedQueryRequest::fulltext_search("content", "hello world", 10);
    let result = client.unified_query("test-ns", request).await.unwrap();
    assert_eq!(result.results.len(), 1);
    assert_eq!(result.results[0].id, "ft1");
    mock.assert_async().await;
}

// ============================================================================
// Vector: aggregate
// ============================================================================

#[tokio::test]
async fn test_aggregate() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/aggregate")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "aggregations": {"total_count": 500},
                "aggregation_groups": [
                    {"category": "science", "total_count": 200},
                    {"category": "art", "total_count": 300}
                ]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::AggregationRequest::new()
        .with_count("total_count")
        .with_group_by("category");
    let result = client.aggregate("test-ns", request).await.unwrap();
    assert!(result.aggregations.is_some());
    assert_eq!(result.aggregations.unwrap()["total_count"], 500);
    assert!(result.aggregation_groups.is_some());
    assert_eq!(result.aggregation_groups.unwrap().len(), 2);
    mock.assert_async().await;
}

// ============================================================================
// Vector: export_vectors
// ============================================================================

#[tokio::test]
async fn test_export_vectors() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/export")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "vectors": [
                    {"id": "v1", "values": [0.1, 0.2, 0.3]},
                    {"id": "v2", "values": [0.4, 0.5, 0.6]}
                ],
                "total_count": 5000,
                "returned_count": 2,
                "next_cursor": "cursor-abc"
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::ExportRequest::new().with_top_k(1000);
    let result = client.export_vectors("test-ns", request).await.unwrap();
    assert_eq!(result.returned_count, 2);
    assert_eq!(result.next_cursor, Some("cursor-abc".to_string()));
    mock.assert_async().await;
}

// ============================================================================
// Vector: explain_query
// ============================================================================

#[tokio::test]
async fn test_explain_query() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/explain")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "query_type": "vector_search",
                "namespace": "test-ns",
                "index_selection": {
                    "index_type": "hnsw",
                    "selection_reason": "vector search query",
                    "alternatives_considered": [],
                    "index_config": {},
                    "index_stats": {
                        "vector_count": 10000,
                        "dimension": 3,
                        "memory_bytes": 1048576
                    }
                },
                "stages": [
                    {"name": "index_lookup", "description": "HNSW graph traversal", "order": 1, "estimated_input": 10000, "estimated_output": 10, "estimated_cost": 1.0}
                ],
                "cost_estimate": {
                    "total_cost": 1.0,
                    "estimated_time_ms": 4,
                    "estimated_memory_bytes": 4096,
                    "estimated_io_ops": 50,
                    "confidence": 0.9
                },
                "recommendations": [],
                "summary": "ANN vector search using HNSW index",
                "query_params": {
                    "top_k": 10,
                    "has_filter": false,
                    "filter_complexity": "none",
                    "vector_dimension": 3,
                    "distance_metric": "cosine"
                }
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::QueryExplainRequest::vector_search(vec![0.1, 0.2, 0.3], 10);
    let result = client.explain_query("test-ns", request).await.unwrap();
    assert_eq!(result.summary, "ANN vector search using HNSW index");
    assert_eq!(result.stages.len(), 1);
    assert_eq!(result.stages[0].name, "index_lookup");
    mock.assert_async().await;
}

// ============================================================================
// Vector: upsert_columns
// ============================================================================

#[tokio::test]
async fn test_upsert_columns() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/upsert-columns")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"upserted_count": 3}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::ColumnUpsertRequest::new(
        vec!["id1".to_string(), "id2".to_string(), "id3".to_string()],
        vec![
            vec![0.1, 0.2, 0.3],
            vec![0.4, 0.5, 0.6],
            vec![0.7, 0.8, 0.9],
        ],
    );
    let result = client.upsert_columns("test-ns", request).await.unwrap();
    assert_eq!(result.upserted_count, 3);
    mock.assert_async().await;
}

// ============================================================================
// Vector: warm_cache
// ============================================================================

#[tokio::test]
async fn test_warm_cache() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/cache/warm")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "success": true,
                "entries_warmed": 500,
                "entries_skipped": 50,
                "message": "Cache warmed successfully",
                "target_tier": "l2",
                "priority": "normal"
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let request = dakera_client::WarmCacheRequest::new("test-ns");
    let result = client.warm_cache(request).await.unwrap();
    assert!(result.success);
    assert_eq!(result.entries_warmed, 500);
    assert_eq!(result.entries_skipped, 50);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_warm_vectors_by_ids() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/my-ns/cache/warm")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "success": true,
                "entries_warmed": 3,
                "entries_skipped": 0,
                "message": "Vectors warmed",
                "target_tier": "l2",
                "priority": "normal"
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client
        .warm_vectors(
            "my-ns",
            vec!["v1".to_string(), "v2".to_string(), "v3".to_string()],
        )
        .await
        .unwrap();
    assert!(result.success);
    assert_eq!(result.entries_warmed, 3);
    mock.assert_async().await;
}

// ============================================================================
// T-I-F: evaluate_tif (v0.11.91)
// ============================================================================

#[tokio::test]
async fn test_evaluate_tif_confident_reuse() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/memories/mem-abc/feedback")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "memory_id": "mem-abc",
                "entries": [
                    {"signal": "Upvote", "timestamp": 1000, "old_importance": 0.7, "new_importance": 0.8},
                    {"signal": "Upvote", "timestamp": 1001, "old_importance": 0.8, "new_importance": 0.85},
                    {"signal": "Upvote", "timestamp": 1002, "old_importance": 0.85, "new_importance": 0.9}
                ]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let score = client.evaluate_tif("mem-abc").await.unwrap();
    // 3 upvotes → truth=1.0, indeterminacy=0.0, falsity=0.0
    assert_eq!(score.feedback_count, 3);
    assert!(score.truth > 0.9);
    assert!(score.falsity < 0.1);
    assert!(
        matches!(
            score.classification,
            dakera_client::TifClassification::ConfidentReuse
        ),
        "expected ConfidentReuse, got {:?}",
        score.classification
    );
    mock.assert_async().await;
}

#[tokio::test]
async fn test_evaluate_tif_empty_history() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/memories/mem-xyz/feedback")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"memory_id": "mem-xyz", "entries": []}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let score = client.evaluate_tif("mem-xyz").await.unwrap();
    assert_eq!(score.feedback_count, 0);
    // No feedback → AskClarification (indeterminate)
    assert!(
        matches!(
            score.classification,
            dakera_client::TifClassification::AskClarification
        ),
        "expected AskClarification on empty history, got {:?}",
        score.classification
    );
    mock.assert_async().await;
}
