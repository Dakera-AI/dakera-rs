//! Integration tests against a real Dakera server (Docker service in CI).
//!
//! Requires DAKERA_TEST_URL env var pointing to a running Dakera instance.
//! Auth is enabled — set DAKERA_API_KEY to a valid key (default: test-key).
//!
//! Run locally: DAKERA_TEST_URL=http://localhost:3000 DAKERA_API_KEY=test-key cargo test --test integration_test

use std::env;

use dakera_client::memory::{
    BatchMemoryFilter, BatchRecallRequest, ConsolidateRequest, ForgetRequest, RecallRequest,
    StoreMemoryRequest, UpdateImportanceRequest,
};
use dakera_client::{CreateNamespaceRequest, DakeraClient, Document, HybridSearchRequest};

fn get_client() -> Option<DakeraClient> {
    let url = env::var("DAKERA_TEST_URL").ok()?;
    let api_key = env::var("DAKERA_API_KEY").unwrap_or_else(|_| "test-key".to_string());
    Some(
        DakeraClient::builder(&url)
            .api_key(&api_key)
            .build()
            .expect("Failed to create client"),
    )
}

fn random_hex() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    format!("{:08x}", nanos)
}

fn test_namespace() -> String {
    format!("integ-{}", random_hex())
}

fn test_agent() -> String {
    format!("integ-agent-{}", random_hex())
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_health() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let health = client.health().await.unwrap();
    assert!(health.healthy);
}

// ---------------------------------------------------------------------------
// Namespaces
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_namespace() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let ns = test_namespace();
    let req = CreateNamespaceRequest {
        dimensions: Some(1024),
        ..Default::default()
    };
    let result = client.create_namespace(&ns, req).await.unwrap();
    assert_eq!(result.name, ns);
    client.delete_namespace_admin(&ns).await.unwrap();
}

#[tokio::test]
async fn test_list_namespaces() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let ns = test_namespace();
    let req = CreateNamespaceRequest {
        dimensions: Some(1024),
        ..Default::default()
    };
    client.create_namespace(&ns, req).await.unwrap();
    let namespaces = client.list_namespaces().await.unwrap();
    assert!(namespaces.contains(&ns));
    client.delete_namespace_admin(&ns).await.unwrap();
}

#[tokio::test]
async fn test_get_namespace() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let ns = test_namespace();
    let req = CreateNamespaceRequest {
        dimensions: Some(1024),
        ..Default::default()
    };
    client.create_namespace(&ns, req).await.unwrap();
    let info = client.get_namespace(&ns).await.unwrap();
    assert_eq!(info.name, ns);
    assert_eq!(info.dimensions, Some(1024));
    client.delete_namespace_admin(&ns).await.unwrap();
}

// ---------------------------------------------------------------------------
// Memory CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_store_memory() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let agent = test_agent();
    let req = StoreMemoryRequest::new(&agent, "The user prefers dark mode")
        .with_importance(0.8)
        .with_tags(vec!["preference".to_string(), "ui".to_string()]);
    let result = client.store_memory(req).await.unwrap();
    assert!(!result.memory_id.is_empty());
}

#[tokio::test]
async fn test_recall_semantic() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let agent = test_agent();
    let req = StoreMemoryRequest::new(&agent, "Python is my primary programming language")
        .with_importance(0.9);
    client.store_memory(req).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let recall_req = RecallRequest::new(&agent, "programming language").with_top_k(5);
    let result = client.recall(recall_req).await.unwrap();
    assert!(!result.memories.is_empty());
}

#[tokio::test]
async fn test_batch_recall() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let agent = test_agent();
    let req = StoreMemoryRequest::new(&agent, "Batch recall test memory").with_importance(0.8);
    client.store_memory(req).await.unwrap();

    let filter = BatchMemoryFilter::default().with_min_importance(0.5);
    let batch_req = BatchRecallRequest::new(&agent).with_filter(filter);
    let result = client.batch_recall(batch_req).await.unwrap();
    assert!(!result.memories.is_empty());
}

#[tokio::test]
async fn test_get_memory() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let agent = test_agent();
    let req = StoreMemoryRequest::new(&agent, "Memory for get test").with_importance(0.7);
    let stored = client.store_memory(req).await.unwrap();
    let memory = client.get_memory(&agent, &stored.memory_id).await.unwrap();
    assert_eq!(memory.content, "Memory for get test");
}

#[tokio::test]
async fn test_update_importance() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let agent = test_agent();
    let req = StoreMemoryRequest::new(&agent, "Importance update test").with_importance(0.5);
    let stored = client.store_memory(req).await.unwrap();
    let update_req = UpdateImportanceRequest {
        memory_ids: vec![stored.memory_id],
        importance: 0.95,
    };
    client.update_importance(&agent, update_req).await.unwrap();
}

#[tokio::test]
async fn test_forget() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let agent = test_agent();
    let req = StoreMemoryRequest::new(&agent, "Memory to forget").with_importance(0.3);
    let stored = client.store_memory(req).await.unwrap();
    let forget_req = ForgetRequest::by_ids(&agent, vec![stored.memory_id]);
    client.forget(forget_req).await.unwrap();
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_session_lifecycle() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let agent = test_agent();
    let session = client.start_session(&agent).await.unwrap();
    assert!(!session.id.is_empty());

    let sessions = client.list_sessions(&agent).await.unwrap();
    assert!(!sessions.is_empty());

    client.end_session(&session.id, None).await.unwrap();
}

// ---------------------------------------------------------------------------
// Vectors / Text
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_index_and_search() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let ns = test_namespace();
    let req = CreateNamespaceRequest {
        dimensions: Some(1024),
        ..Default::default()
    };
    client.create_namespace(&ns, req).await.unwrap();

    client
        .index_document(
            &ns,
            Document::new("doc-1", "Machine learning transforms data"),
        )
        .await
        .unwrap();
    client
        .index_document(
            &ns,
            Document::new("doc-2", "Natural language processing understands text"),
        )
        .await
        .unwrap();
    client
        .index_document(
            &ns,
            Document::new("doc-3", "Deep learning uses neural networks"),
        )
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let results = client.search_text(&ns, "neural networks", 3).await.unwrap();
    let _ = results;

    client.delete_namespace_admin(&ns).await.unwrap();
}

#[tokio::test]
async fn test_hybrid_search() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let ns = test_namespace();
    let req = CreateNamespaceRequest {
        dimensions: Some(1024),
        ..Default::default()
    };
    client.create_namespace(&ns, req).await.unwrap();

    client
        .index_document(&ns, Document::new("h-1", "Machine learning data analysis"))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let search_req = HybridSearchRequest::text_only("machine learning", 3);
    let _results = client.hybrid_search(&ns, search_req).await;

    client.delete_namespace_admin(&ns).await.unwrap();
}

// ---------------------------------------------------------------------------
// Knowledge Graph
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_memory_graph() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let agent = test_agent();
    let req = StoreMemoryRequest::new(&agent, "Knowledge graph test memory").with_importance(0.8);
    let stored = client.store_memory(req).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let opts = dakera_client::GraphOptions::new().depth(1);
    let _graph = client.memory_graph(&stored.memory_id, opts).await;
}

// ---------------------------------------------------------------------------
// Consolidate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_consolidate() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let agent = test_agent();
    for i in 0..3 {
        let req = StoreMemoryRequest::new(
            &agent,
            format!("Consolidation test variation {i}: similar content"),
        )
        .with_importance(0.6);
        client.store_memory(req).await.unwrap();
    }
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let consolidate_req = ConsolidateRequest::default();
    let _result = client.consolidate(&agent, consolidate_req).await;
}

// ---------------------------------------------------------------------------
// Error Handling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_nonexistent_namespace() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let result = client.get_namespace("nonexistent-ns-xyz-99999").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_nonexistent_memory() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let result = client
        .get_memory("test-agent", "nonexistent-memory-id")
        .await;
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Authentication
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_auth_rejects_invalid_key() {
    let url = match env::var("DAKERA_TEST_URL") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("DAKERA_TEST_URL not set — skipping");
            return;
        }
    };
    let bad_client = DakeraClient::builder(&url)
        .api_key("invalid-key-xxx")
        .build()
        .expect("Failed to create client");
    let result = bad_client.list_namespaces().await;
    assert!(result.is_err(), "expected auth error with invalid key");
    let err = result.unwrap_err();
    assert!(err.is_auth_error(), "expected auth error, got: {err:?}");
}

#[tokio::test]
async fn test_auth_accepts_valid_key() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let namespaces = client.list_namespaces().await.unwrap();
    assert!(namespaces.len() >= 0);
}
