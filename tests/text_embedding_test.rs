//! Unit tests for text upsert/query endpoints using mockito.
//!
//! Covers: upsert_text with model selection, query_text with model selection,
//! and critically verifies that the new ModernBERT model variants serialize to
//! the correct wire values ("modernbert-embed-base", "gte-modernbert-base") so
//! the server accepts them without a 422 on the model field.

use dakera_client::{BatchQueryTextRequest, QueryTextRequest, UpsertTextRequest};
use dakera_client::{DakeraClient, EmbeddingModel, TextDocument};

// ============================================================================
// upsert_text — ModernBertEmbedBase
// ============================================================================

#[tokio::test]
async fn test_upsert_text_modernbert_embed_base() {
    let mut server = mockito::Server::new_async().await;

    // Response echoes back the model used — verifies round-trip deserialization.
    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/upsert-text")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"upserted_count":1,"tokens_processed":12,"model":"modernbert-embed-base","embedding_time_ms":45}"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let req = UpsertTextRequest::new(vec![TextDocument::new("doc1", "Hello world")])
        .with_model(EmbeddingModel::ModernBertEmbedBase);

    // Verify the request model serializes to the correct wire value before sending.
    assert_eq!(
        serde_json::to_string(&EmbeddingModel::ModernBertEmbedBase).unwrap(),
        r#""modernbert-embed-base""#
    );

    let result = client.upsert_text("test-ns", req).await.unwrap();
    assert_eq!(result.upserted_count, 1);
    assert_eq!(result.tokens_processed, 12);
    assert_eq!(result.model, EmbeddingModel::ModernBertEmbedBase);
    assert_eq!(result.embedding_time_ms, 45);
    mock.assert_async().await;
}

// ============================================================================
// upsert_text — GteModernBertBase
// ============================================================================

#[tokio::test]
async fn test_upsert_text_gte_modernbert_base() {
    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/upsert-text")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"upserted_count":2,"tokens_processed":24,"model":"gte-modernbert-base","embedding_time_ms":60}"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let req = UpsertTextRequest::new(vec![
        TextDocument::new("doc1", "First document"),
        TextDocument::new("doc2", "Second document"),
    ])
    .with_model(EmbeddingModel::GteModernBertBase);

    assert_eq!(
        serde_json::to_string(&EmbeddingModel::GteModernBertBase).unwrap(),
        r#""gte-modernbert-base""#
    );

    let result = client.upsert_text("test-ns", req).await.unwrap();
    assert_eq!(result.upserted_count, 2);
    assert_eq!(result.model, EmbeddingModel::GteModernBertBase);
    mock.assert_async().await;
}

// ============================================================================
// query_text — ModernBertEmbedBase
// ============================================================================

#[tokio::test]
async fn test_query_text_modernbert_embed_base() {
    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/query-text")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"results":[{"id":"doc1","score":0.92}],"model":"modernbert-embed-base","embedding_time_ms":30,"search_time_ms":5}"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let req = QueryTextRequest::new("semantic search query", 10)
        .with_model(EmbeddingModel::ModernBertEmbedBase);

    let result = client.query_text("test-ns", req).await.unwrap();
    assert_eq!(result.results.len(), 1);
    assert_eq!(result.results[0].id, "doc1");
    assert!((result.results[0].score - 0.92).abs() < 1e-4);
    assert_eq!(result.model, EmbeddingModel::ModernBertEmbedBase);
    mock.assert_async().await;
}

// ============================================================================
// query_text — GteModernBertBase
// ============================================================================

#[tokio::test]
async fn test_query_text_gte_modernbert_base() {
    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/query-text")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"results":[],"model":"gte-modernbert-base","embedding_time_ms":28,"search_time_ms":3}"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let req = QueryTextRequest::new("no results expected", 5)
        .with_model(EmbeddingModel::GteModernBertBase);

    let result = client.query_text("test-ns", req).await.unwrap();
    assert!(result.results.is_empty());
    assert_eq!(result.model, EmbeddingModel::GteModernBertBase);
    mock.assert_async().await;
}

// ============================================================================
// batch_query_text — with ModernBertEmbedBase
// ============================================================================

#[tokio::test]
async fn test_batch_query_text_modernbert() {
    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/namespaces/test-ns/batch-query-text")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"results":[[{"id":"doc1","score":0.88}],[{"id":"doc2","score":0.75}]],"model":"modernbert-embed-base","embedding_time_ms":55,"search_time_ms":4}"#,
        )
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let req = BatchQueryTextRequest {
        queries: vec!["query one".to_string(), "query two".to_string()],
        top_k: 5,
        filter: None,
        include_vectors: false,
        model: Some(EmbeddingModel::ModernBertEmbedBase),
    };

    let result = client.batch_query_text("test-ns", req).await.unwrap();
    assert_eq!(result.results.len(), 2);
    assert_eq!(result.model, EmbeddingModel::ModernBertEmbedBase);
    mock.assert_async().await;
}
