//! Tests for hybrid search — DAK-679 optional vector (BM25-only fallback).
//!
//! Verifies that `HybridSearchRequest::text_only()` serialises without a
//! `vector` field and that the client sends / parses the request correctly.

use dakera_client::{DakeraClient, HybridSearchRequest};

// ============================================================================
// Hybrid search with vector (standard path)
// ============================================================================

#[tokio::test]
async fn test_hybrid_search_with_vector_hits_correct_endpoint() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/hybrid")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "matches": [
                    {"id": "mem_001", "score": 0.95}
                ]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let req = HybridSearchRequest::new(vec![0.1, 0.2, 0.3], "search query", 5);
    let result = client.hybrid_search("test-ns", req).await.unwrap();

    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].id, "mem_001");
    assert!((result.matches[0].score - 0.95).abs() < 1e-5);
    mock.assert_async().await;
}

// ============================================================================
// BM25-only hybrid search (DAK-679 — vector omitted / None)
// ============================================================================

#[tokio::test]
async fn test_hybrid_search_bm25_only_hits_correct_endpoint() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/hybrid")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "matches": [
                    {"id": "mem_002", "score": 0.80},
                    {"id": "mem_003", "score": 0.72}
                ]
            }"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let req = HybridSearchRequest::text_only("hello world", 10);
    let result = client.hybrid_search("test-ns", req).await.unwrap();

    assert_eq!(result.matches.len(), 2);
    assert_eq!(result.matches[0].id, "mem_002");
    assert_eq!(result.matches[1].id, "mem_003");
    mock.assert_async().await;
}

/// Ensure `text_only()` serialises without a `vector` key — the server
/// must not see the field at all (not even `"vector": null`).
#[tokio::test]
async fn test_hybrid_search_bm25_only_omits_vector_field() {
    let req = HybridSearchRequest::text_only("omit vector", 3);
    let json = serde_json::to_value(&req).unwrap();

    assert!(
        json.get("vector").is_none(),
        "vector field must be absent when using text_only()"
    );
    assert_eq!(json["text"].as_str().unwrap(), "omit vector");
    assert_eq!(json["top_k"].as_u64().unwrap(), 3);
}
