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
// StoreMemoryRequest — bi-temporal valid_from serialization (DAK-7424)
// ---------------------------------------------------------------------------

#[test]
fn test_store_memory_request_valid_from_serialized_when_set() {
    // The server reads valid_from from the memory's metadata; a top-level
    // valid_from is not a store field (it would come back as a warning).
    let req = StoreMemoryRequest::new("agent-1", "temporal memory").with_valid_from(1_700_000_000);
    let json = serde_json::to_value(&req).unwrap();
    assert!(json.get("valid_from").is_none(), "{json}");
    assert_eq!(json["metadata"]["valid_from"], 1_700_000_000i64);
}

#[test]
fn test_store_memory_request_valid_from_omitted_when_none() {
    let req = StoreMemoryRequest::new("agent-1", "non-temporal memory");
    let json = serde_json::to_value(&req).unwrap();
    assert!(
        json.get("valid_from").is_none(),
        "valid_from must be absent from JSON when not set"
    );
}

#[test]
fn test_health_response_build_sha_present() {
    let json = r#"{"healthy":true,"version":"0.11.84","build_sha":"abc1234def5678"}"#;
    let h: dakera_client::HealthResponse = serde_json::from_str(json).unwrap();
    assert!(h.healthy);
    assert_eq!(h.build_sha.as_deref(), Some("abc1234def5678"));
}

#[test]
fn test_health_response_build_sha_absent() {
    let json = r#"{"healthy":true,"version":"0.11.83"}"#;
    let h: dakera_client::HealthResponse = serde_json::from_str(json).unwrap();
    assert!(h.healthy);
    assert!(h.build_sha.is_none());
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

#[tokio::test]
async fn test_graph_contract_round_trip() {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let agent = test_agent();
    let a = client
        .store_memory(StoreMemoryRequest::new(
            &agent,
            "Anna moved to Berlin in May",
        ))
        .await
        .unwrap();
    let b = client
        .store_memory(StoreMemoryRequest::new(&agent, "Anna now works at Siemens"))
        .await
        .unwrap();

    // Link: the server needs agent_id and answers {from_id, to_id, edge_type}.
    let link = client
        .memory_link(&agent, &a.memory_id, &b.memory_id, Some("career"))
        .await
        .unwrap();
    assert_eq!(link.from_id, a.memory_id);
    assert_eq!(link.to_id, b.memory_id);
    assert_eq!(link.edge_type, dakera_client::EdgeType::LinkedBy);

    // Traversal: nodes carry their edges (from_id / to_id, no edge id).
    let graph = client
        .memory_graph(&a.memory_id, dakera_client::GraphOptions::new().depth(2))
        .await
        .unwrap();
    assert_eq!(graph.root_id, a.memory_id);
    assert!(graph.nodes.iter().any(|n| n.memory_id == b.memory_id));
    assert!(graph
        .edges
        .iter()
        .any(|e| e.source_id == a.memory_id && e.target_id == b.memory_id));

    // Shortest path: `to` query parameter, `hop_count` answer.
    let path = client
        .memory_path(&a.memory_id, &b.memory_id)
        .await
        .unwrap();
    assert_eq!(path.hops, 1);
    assert_eq!(path.path, vec![a.memory_id.clone(), b.memory_id.clone()]);

    // Export and KG query return the same edge.
    let export = client.agent_graph_export(&agent, "json").await.unwrap();
    assert_eq!(export.namespace, format!("_dakera_agent_{agent}"));
    assert!(export.edge_count >= 1);
    assert!(export.edges.iter().any(|e| e.source_id == a.memory_id));
    let query = client
        .knowledge_query(&agent, None, Some("linked_by"), None, None, None)
        .await
        .unwrap();
    assert!(query
        .edges
        .iter()
        .any(|e| e.source_id == a.memory_id && e.target_id == b.memory_id));

    // Entities: {entities, count}; the client fills memory_id.
    let ents = client.memory_entities(&a.memory_id).await.unwrap();
    assert_eq!(ents.memory_id, a.memory_id);
    assert_eq!(ents.count, ents.entities.len());

    // Update reads agent_id from the query string.
    let updated = client
        .update_memory(
            &agent,
            &a.memory_id,
            dakera_client::memory::UpdateMemoryRequest {
                content: Some("Anna moved to Berlin in June".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.memory_id, a.memory_id);
    let got = client.get_memory(&agent, &a.memory_id).await.unwrap();
    assert_eq!(got.content, "Anna moved to Berlin in June");
}

// Every call 0.12.1 fixed (decode or request mismatches against v0.12.0), live.
#[tokio::test]
async fn test_decode_contract_round_trip() {
    use dakera_client::knowledge::{
        DeduplicateRequest, FullKnowledgeGraphRequest, KnowledgeGraphRequest, SummarizeRequest,
    };
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return;
    };
    let agent = test_agent();
    let ns = format!("_dakera_agent_{agent}");
    let a = client
        .store_memory(StoreMemoryRequest::new(
            &agent,
            "Standup: Bob fixed the login bug",
        ))
        .await
        .unwrap();
    let b = client
        .store_memory(StoreMemoryRequest::new(
            &agent,
            "Standup: Bob shipped the login fix",
        ))
        .await
        .unwrap();
    let ids = vec![a.memory_id.clone(), b.memory_id.clone()];

    // Knowledge: seed graph, full graph, deduplicate, summarize.
    let kg = client
        .knowledge_graph(KnowledgeGraphRequest {
            agent_id: agent.clone(),
            memory_id: Some(a.memory_id.clone()),
            depth: None,
            min_similarity: None,
        })
        .await
        .unwrap();
    assert!(kg.nodes.iter().any(|n| n.id == a.memory_id));
    let no_seed = client
        .knowledge_graph(KnowledgeGraphRequest {
            agent_id: agent.clone(),
            memory_id: None,
            depth: None,
            min_similarity: None,
        })
        .await;
    assert!(no_seed.is_err(), "the server needs a seed memory_id");
    let full = client
        .full_knowledge_graph(FullKnowledgeGraphRequest {
            agent_id: agent.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(full.nodes.len(), 2);
    client
        .deduplicate(DeduplicateRequest {
            agent_id: agent.clone(),
            threshold: None,
            memory_type: None,
            dry_run: true,
        })
        .await
        .unwrap();
    let one = client
        .summarize(SummarizeRequest {
            agent_id: agent.clone(),
            memory_ids: Some(vec![a.memory_id.clone()]),
            target_type: None,
            dry_run: false,
        })
        .await;
    assert!(one.is_err(), "the server needs at least two memory_ids");
    let summary = client
        .summarize(SummarizeRequest {
            agent_id: agent.clone(),
            memory_ids: Some(ids.clone()),
            target_type: None,
            dry_run: false,
        })
        .await
        .unwrap();
    assert_eq!(summary.source_count, 2);
    assert!(summary.new_memory_id.is_some());
    assert!(!summary.summary.is_empty());

    // Agents, memory importance, wake-up, export, feedback history / TIF.
    let stats = client.agent_stats(&agent).await.unwrap();
    assert!(stats.total_memories >= 2);
    client.agent_sessions(&agent, None, None).await.unwrap();
    let imp = client
        .patch_memory_importance(&a.memory_id, &agent, 0.75)
        .await
        .unwrap();
    assert_eq!(imp.memory_id, a.memory_id);
    assert!((imp.new_importance - 0.75).abs() < 1e-6);
    client.wake_up(&agent, Some(5), None).await.unwrap();
    let export = client
        .export_memories("jsonl", Some(&agent), None, None)
        .await
        .unwrap();
    assert!(export.count >= 2);
    client
        .get_memory_feedback_history(&a.memory_id, &agent)
        .await
        .unwrap();
    client.evaluate_tif(&a.memory_id, &agent).await.unwrap();

    // Full-text stats, namespace creation, admin and analytics answers.
    client.fulltext_stats(&ns).await.unwrap();
    let new_ns = test_namespace();
    client
        .create_namespace(
            &new_ns,
            CreateNamespaceRequest {
                dimensions: Some(8),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    client.get_kpis().await.unwrap();
    client.admin_cluster_replication().await.unwrap();
    client
        .admin_list_slow_queries(None, None, Some(5))
        .await
        .unwrap();
    client.analytics_latency(None, None).await.unwrap();
    client.analytics_throughput(None, None).await.unwrap();
    client.analytics_storage(None).await.unwrap();
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
    // list_namespaces succeeded — the unwrap() above is the real assertion
    let _ = client.list_namespaces().await.unwrap();
}

// ---------------------------------------------------------------------------
// Server v0.12.3 behaviour (skipped on older servers: CI pins 0.12.0)
// ---------------------------------------------------------------------------

/// The client when the server is at least v0.12.3 (its `server_version` in
/// `GET /v1/capabilities`), else `None` with a note.
async fn client_at_least_0123() -> Option<DakeraClient> {
    let Some(client) = get_client() else {
        eprintln!("DAKERA_TEST_URL not set — skipping");
        return None;
    };
    let version = match client.capabilities().await {
        Ok(caps) => caps.server_version.clone(),
        Err(e) => {
            eprintln!("capabilities unavailable ({e}) — skipping a v0.12.3 test");
            return None;
        }
    };
    let mut parts = version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .map(|p| p.parse::<u64>().unwrap_or(0));
    let v = (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    );
    if v < (0, 12, 3) {
        eprintln!("server {version} is older than 0.12.3 — skipping");
        return None;
    }
    Some(client)
}

fn agent_0123(tag: &str) -> String {
    format!("integ-0123-{tag}-{}", random_hex())
}

#[tokio::test]
async fn test_0123_empty_agent_answers_empty() {
    let Some(client) = client_at_least_0123().await else {
        return;
    };
    let agent = agent_0123("empty");
    let recall = client
        .recall(RecallRequest::new(&agent, "anything"))
        .await
        .unwrap();
    assert!(recall.memories.is_empty());
    let search = client
        .search_memories(RecallRequest::new(&agent, "anything"))
        .await
        .unwrap();
    assert!(search.memories.is_empty());
    let batch = client
        .batch_recall(BatchRecallRequest::new(&agent))
        .await
        .unwrap();
    assert!(batch.memories.is_empty());
}

#[tokio::test]
async fn test_0123_event_time_is_recorded() {
    let Some(client) = client_at_least_0123().await else {
        return;
    };
    let agent = agent_0123("event");
    let stored = client
        .store_memory(
            StoreMemoryRequest::new(&agent, "Met Alice in Lisbon")
                .with_event_time(1_684_549_260i64),
        )
        .await
        .unwrap();
    let memory = client.get_memory(&agent, &stored.memory_id).await.unwrap();
    let meta = memory.metadata.expect("metadata");
    assert_eq!(
        meta[dakera_client::EVENT_TIME_META_KEY],
        serde_json::json!(1_684_549_260i64)
    );

    let resp = client
        .store_memories_batch(dakera_client::memory::BatchStoreMemoryRequest::new(
            &agent,
            vec![
                dakera_client::memory::BatchStoreMemoryItem::new("Flew back")
                    .with_event_time("2023-05-21T10:00:00Z"),
            ],
        ))
        .await
        .unwrap();
    let memory = client.get_memory(&agent, &resp.stored[0].id).await.unwrap();
    assert_eq!(
        memory.metadata.expect("metadata")[dakera_client::EVENT_TIME_META_KEY],
        serde_json::json!(1_684_663_200i64)
    );
}

#[tokio::test]
async fn test_0123_every_sdk_field_is_known_to_the_server() {
    use dakera_client::memory::{BatchStoreMemoryItem, BatchStoreMemoryRequest, FusionStrategy};
    use dakera_client::{MemoryType, NeighborhoodMode, RoutingMode};

    let Some(client) = client_at_least_0123().await else {
        return;
    };
    let agent = agent_0123("fields");
    let store = client
        .store_memory(
            StoreMemoryRequest::new(&agent, "Met Alice in Lisbon for the conference")
                .with_type(MemoryType::Semantic)
                .with_importance(0.8)
                .with_tags(vec!["trip".to_string()])
                .with_session("conv:42")
                .with_metadata(serde_json::json!({"source": "import"}))
                .with_ttl(86_400)
                .with_expires_at(4_102_444_800)
                .with_valid_from(1_684_540_800)
                .with_lang("en")
                .with_event_time(1_684_549_260i64),
        )
        .await
        .unwrap();
    assert!(store.warnings.is_empty(), "{:?}", store.warnings);

    let mut item = BatchStoreMemoryItem::new("Flew back from Lisbon")
        .with_importance(0.6)
        .with_tags(vec!["trip".to_string()])
        .with_session("conv:42")
        .with_metadata(serde_json::json!({"source": "import"}))
        .with_valid_from(1_684_540_800)
        .with_id(format!("{agent}-b1"))
        .with_event_time("2023-05-21T10:00:00Z");
    item.memory_type = MemoryType::Episodic;
    item.ttl_seconds = Some(86_400);
    item.expires_at = Some(4_102_444_800);
    let batch = client
        .store_memories_batch(BatchStoreMemoryRequest::new(&agent, vec![item]).with_lang("en"))
        .await
        .unwrap();
    assert!(batch.warnings.is_empty(), "{:?}", batch.warnings);

    let mut recall = RecallRequest::new(&agent, "Where did I meet Alice?")
        .with_lang("en")
        .with_top_k(5)
        .with_type(MemoryType::Semantic)
        .with_min_importance(0.25)
        .with_session("conv:42")
        .with_tags(vec!["trip".to_string()])
        .with_associated_cap(5)
        .with_associated_depth(2)
        .with_associated_min_weight(0.25)
        .with_since("2023-01-01T00:00:00Z")
        .with_until("2030-01-01T00:00:00Z")
        .with_routing(RoutingMode::Hybrid)
        .with_rerank(true)
        .with_rerank_candidates(50)
        .with_fusion(FusionStrategy::MinMax)
        .with_vector_weight(0.5)
        .with_iterations(2)
        .with_neighborhood(true)
        .with_neighborhood_mode(NeighborhoodMode::Separate)
        .with_include_summaries(false);
    recall.importance_weighted = Some(true);
    let recalled = client.recall(recall).await.unwrap();
    assert!(recalled.warnings.is_empty(), "{:?}", recalled.warnings);

    let filter = BatchMemoryFilter {
        tags: Some(vec!["trip".to_string()]),
        min_importance: Some(0.25),
        max_importance: Some(1.0),
        created_after: Some(0),
        created_before: Some(4_102_444_800),
        memory_type: Some(MemoryType::Semantic),
        session_id: Some("conv:42".to_string()),
    };
    let batch_recall = client
        .batch_recall(
            BatchRecallRequest::new(&agent)
                .with_filter(filter)
                .with_limit(10),
        )
        .await
        .unwrap();
    assert!(
        batch_recall.warnings.is_empty(),
        "{:?}",
        batch_recall.warnings
    );
}

#[tokio::test]
async fn test_0123_deduplication_trash() {
    let Some(client) = client_at_least_0123().await else {
        return;
    };
    let agent = agent_0123("trash");
    let list = client.list_deduplicated(&agent).await.unwrap();
    assert_eq!(list.count, 0);
    assert!(list.memories.is_empty());
    let err = client
        .restore_deduplicated(&agent, "no-such-memory")
        .await
        .unwrap_err();
    assert!(err.is_not_found(), "{err:?}");
}

#[tokio::test]
async fn test_0123_get_memory_without_agent_id() {
    let Some(client) = client_at_least_0123().await else {
        return;
    };
    let agent = agent_0123("get");
    let stored = client
        .store_memory(StoreMemoryRequest::new(&agent, "Find me by id alone"))
        .await
        .unwrap();
    let memory = client.get_memory_by_id(&stored.memory_id).await.unwrap();
    assert_eq!(memory.content, "Find me by id alone");
}
