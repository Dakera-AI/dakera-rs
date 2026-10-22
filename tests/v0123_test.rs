//! Server v0.12.3 wire support: `event_time` on store and batch store
//! (Dakera-AI/dakera#932), the truncation report (#933), `warnings` on store
//! and recall answers (#938), the deduplication trash and the autopilot dedup
//! settings (#930), `get_memory` without `agent_id` (#938), the capabilities
//! `naming` session-id rules, the `reason` of audit / memory events, the
//! `HOST_NOT_ALLOWED` code and `rerank_report.capped` (#926).
//!
//! Wire shapes follow the server source (`common/src/types/mod.rs`
//! `StoreMemoryRequest` / `EventTime` / `StoreMemoryResponse` /
//! `UpdateMemoryResponse` / `EmbedTruncation` / `TruncatedMemory` /
//! `BatchStore*` / `RecallRequest` / `RerankReport`,
//! `api/src/routes/memory/deduplicated.rs`, `api/src/memory_ops/dedup_trash.rs`,
//! `api/src/routes/admin/mod.rs` autopilot types, `routes/admin/types.rs`
//! runtime config, `routes/capabilities.rs`, `engine/src/graph.rs`
//! `AuditEvent`, `api/src/events.rs` `MemoryEvent`, `api/src/error.rs`).
//! Every request body below is asserted EXACTLY, and each was replayed
//! against a v0.12.3 server, which answered without `warnings` (it knows
//! every field). Each answer is also decoded in its older form.

use dakera_client::memory::{
    BatchStoreMemoryItem, BatchStoreMemoryRequest, FusionStrategy, UpdateMemoryRequest,
};
use dakera_client::{
    AuditEvent, AuditListResponse, AutoPilotConfigRequest, AutoPilotConfigResponse,
    AutoPilotStatusResponse, AutoPilotTriggerResponse, BatchMemoryFilter, BatchRecallRequest,
    BatchRecallResponse, ClientError, DakeraClient, DeduplicationKind, EventTime, MemoryEvent,
    MemoryType, NamingCapability, NeighborhoodMode, RecallRequest, RecallResponse, RoutingMode,
    RuntimeConfig, ServerErrorCode, StoreMemoryRequest, StoreMemoryResponse, DEDUP_MODE_MERGE,
    DEDUP_MODE_REPORT, EVENT_DEDUPLICATED, EVENT_TIME_META_KEY,
};
use mockito::Matcher;
use serde_json::{json, Value};

/// An attachment uploaded to the test agent's namespace (the sha256 of
/// `hello attachment`), so the replayed store bodies are accepted.
const REF: &str = "sha256:7fa36b95d5c98859ed72b4787f3c28b29eaa103970786755c9711cbb19be631c";

fn json_mock(
    server: &mut mockito::ServerGuard,
    method: &str,
    path: &str,
    status: usize,
    body: &str,
) -> mockito::Mock {
    server
        .mock(method, path)
        .with_status(status)
        .with_header("content-type", "application/json")
        .with_body(body)
}

// ============================================================================
// event_time (#932) and valid_from
// ============================================================================

/// Every field a store request can carry, as the SDK sends it.
fn store_all_fields() -> StoreMemoryRequest {
    StoreMemoryRequest::new("wire-agent", "Met Alice in Lisbon for the conference")
        .with_type(MemoryType::Semantic)
        .with_importance(0.8)
        .with_tags(vec!["trip".to_string()])
        .with_session("conv:42")
        .with_metadata(json!({"source": "import"}))
        .with_ttl(86_400)
        .with_expires_at(4_102_444_800)
        .with_valid_from(1_684_540_800)
        .with_lang("en")
        .with_attachment_ref(REF)
        .with_event_time(1_684_549_260i64)
}

fn store_all_fields_wire() -> Value {
    json!({
        "agent_id": "wire-agent",
        "content": "Met Alice in Lisbon for the conference",
        "memory_type": "semantic",
        "importance": 0.8,
        "tags": ["trip"],
        "session_id": "conv:42",
        "metadata": {"source": "import", "valid_from": 1_684_540_800},
        "ttl_seconds": 86_400,
        "expires_at": 4_102_444_800u64,
        "lang": "en",
        "attachment_ref": REF,
        "event_time": 1_684_549_260
    })
}

#[tokio::test]
async fn store_sends_event_time_and_every_optional_field_exactly() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/memory/store",
        200,
        r#"{"memory":{"id":"m1","agent_id":"wire-agent"},"embedding_time_ms":4,"session_state":"active"}"#,
    )
    .match_body(Matcher::Json(store_all_fields_wire()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.store_memory(store_all_fields()).await.unwrap();
    assert_eq!(r.memory_id, "m1");
    // A v0.12.3 answer without a cut and without ignored fields.
    assert!(r.truncation.is_none());
    assert!(r.warnings.is_empty());
    m.assert_async().await;
}

#[test]
fn store_request_is_the_wire_body() {
    let v = serde_json::to_value(store_all_fields()).unwrap();
    assert!(v.get("valid_from").is_none(), "{v}");
    assert_eq!(v["event_time"], json!(1_684_549_260));
    assert_eq!(v["metadata"]["valid_from"], json!(1_684_540_800));
}

#[test]
fn event_time_wire_forms() {
    assert_eq!(
        serde_json::to_value(EventTime::from(1_684_549_260i64)).unwrap(),
        json!(1_684_549_260)
    );
    assert_eq!(
        serde_json::to_value(EventTime::from("2023-05-20T02:21:00Z")).unwrap(),
        json!("2023-05-20T02:21:00Z")
    );
    assert_eq!(
        serde_json::to_value(EventTime::from("2023-05-20T02:21:00+02:00".to_string())).unwrap(),
        json!("2023-05-20T02:21:00+02:00")
    );
    assert_eq!(
        serde_json::from_value::<EventTime>(json!(5)).unwrap(),
        EventTime::Unix(5)
    );
    assert_eq!(
        serde_json::from_value::<EventTime>(json!("2023-05-20T02:21:00Z")).unwrap(),
        EventTime::Rfc3339("2023-05-20T02:21:00Z".into())
    );
    assert_eq!(EVENT_TIME_META_KEY, "_dakera_event_time");
}

#[test]
fn unset_optional_fields_are_omitted() {
    let v = serde_json::to_value(StoreMemoryRequest::new("a", "c")).unwrap();
    for key in [
        "event_time",
        "valid_from",
        "metadata",
        "session_id",
        "ttl_seconds",
        "expires_at",
        "lang",
        "attachment_ref",
    ] {
        assert!(v.get(key).is_none(), "{key} must be omitted: {v}");
    }
    let v = serde_json::to_value(BatchStoreMemoryItem::new("c")).unwrap();
    for key in [
        "event_time",
        "valid_from",
        "metadata",
        "id",
        "attachment_ref",
    ] {
        assert!(v.get(key).is_none(), "{key} must be omitted: {v}");
    }
}

#[test]
fn valid_from_goes_into_metadata_and_never_overrides_it() {
    // No metadata: one is made.
    let v = serde_json::to_value(StoreMemoryRequest::new("a", "c").with_valid_from(7)).unwrap();
    assert_eq!(v["metadata"], json!({"valid_from": 7}));
    // The caller's own metadata.valid_from wins.
    let v = serde_json::to_value(
        StoreMemoryRequest::new("a", "c")
            .with_metadata(json!({"valid_from": 1, "k": "v"}))
            .with_valid_from(7),
    )
    .unwrap();
    assert_eq!(v["metadata"], json!({"valid_from": 1, "k": "v"}));
    // Metadata that is not an object is sent unchanged.
    let v = serde_json::to_value(
        StoreMemoryRequest::new("a", "c")
            .with_metadata(json!("note"))
            .with_valid_from(7),
    )
    .unwrap();
    assert_eq!(v["metadata"], json!("note"));
    assert!(v.get("valid_from").is_none());
    // The same for a batch item.
    let v = serde_json::to_value(BatchStoreMemoryItem::new("c").with_valid_from(7)).unwrap();
    assert_eq!(v["metadata"], json!({"valid_from": 7}));
    assert!(v.get("valid_from").is_none());
}

#[test]
fn a_store_request_still_reads_a_top_level_valid_from() {
    let req: StoreMemoryRequest = serde_json::from_value(json!({
        "agent_id": "a", "content": "c", "valid_from": 9,
        "event_time": "2023-05-20T02:21:00Z"
    }))
    .unwrap();
    assert_eq!(req.valid_from, Some(9));
    assert_eq!(
        req.event_time,
        Some(EventTime::Rfc3339("2023-05-20T02:21:00Z".into()))
    );
}

// ============================================================================
// Truncation report (#933) and warnings (#938) on store / update
// ============================================================================

#[test]
fn store_response_reads_the_flattened_truncation_and_warnings() {
    let r: StoreMemoryResponse = serde_json::from_value(json!({
        "memory": {"id": "m1", "agent_id": "a"},
        "embedding_time_ms": 9,
        "truncated": true,
        "content_tokens": 4718,
        "embedded_tokens": 512,
        "max_tokens": 512,
        "tokens_estimated": true,
        "windows": 3,
        "warnings": ["unknown field 'created_at' ignored"]
    }))
    .unwrap();
    let t = r.truncation.expect("truncation");
    assert!(t.truncated);
    assert_eq!(
        (t.content_tokens, t.embedded_tokens, t.max_tokens, t.windows),
        (4718, 512, 512, 3)
    );
    assert!(t.tokens_estimated);
    assert_eq!(r.warnings, vec!["unknown field 'created_at' ignored"]);

    // tokens_estimated is absent unless true.
    let r: StoreMemoryResponse = serde_json::from_value(json!({
        "memory": {"id": "m1"}, "truncated": true, "content_tokens": 600,
        "embedded_tokens": 512, "max_tokens": 512, "windows": 0
    }))
    .unwrap();
    assert!(!r.truncation.unwrap().tokens_estimated);

    // An older server: neither.
    let r: StoreMemoryResponse =
        serde_json::from_value(json!({"memory": {"id": "m1"}, "embedding_time_ms": 1})).unwrap();
    assert!(r.truncation.is_none());
    assert!(r.warnings.is_empty());
}

#[tokio::test]
async fn update_memory_reads_the_flat_memory_with_truncation() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "PUT",
        "/v1/memory/update/m1",
        200,
        r#"{"id":"m1","agent_id":"a","content":"long","memory_type":"episodic","importance":0.5,"tags":[],"created_at":1,"last_accessed_at":1,"access_count":0,"truncated":true,"content_tokens":700,"embedded_tokens":512,"max_tokens":512,"windows":1}"#,
    )
    .match_query(Matcher::UrlEncoded("agent_id".into(), "a".into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .update_memory(
            "a",
            "m1",
            UpdateMemoryRequest {
                content: Some("long".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(r.memory_id, "m1");
    let t = r.truncation.unwrap();
    assert_eq!(
        (t.content_tokens, t.embedded_tokens, t.windows),
        (700, 512, 1)
    );
    m.assert_async().await;

    // The flat memory of an older server.
    let r: StoreMemoryResponse =
        serde_json::from_value(json!({"id": "m1", "agent_id": "a", "content": "c"})).unwrap();
    assert!(r.truncation.is_none());
}

// ============================================================================
// Batch store (#932, #933, #938)
// ============================================================================

fn batch_all_fields() -> BatchStoreMemoryRequest {
    let mut item = BatchStoreMemoryItem::new("Flew back from Lisbon")
        .with_importance(0.6)
        .with_tags(vec!["trip".to_string()])
        .with_session("conv:42")
        .with_metadata(json!({"source": "import"}))
        .with_valid_from(1_684_540_800)
        .with_id("wire-batch-1")
        .with_attachment_ref(REF)
        .with_event_time("2023-05-21T10:00:00Z");
    item.memory_type = MemoryType::Episodic;
    item.ttl_seconds = Some(86_400);
    item.expires_at = Some(4_102_444_800);
    BatchStoreMemoryRequest::new("wire-agent", vec![item]).with_lang("en")
}

fn batch_all_fields_wire() -> Value {
    json!({
        "agent_id": "wire-agent",
        "lang": "en",
        "memories": [{
            "content": "Flew back from Lisbon",
            "memory_type": "episodic",
            "importance": 0.6,
            "tags": ["trip"],
            "session_id": "conv:42",
            "metadata": {"source": "import", "valid_from": 1_684_540_800},
            "ttl_seconds": 86_400,
            "expires_at": 4_102_444_800u64,
            "id": "wire-batch-1",
            "attachment_ref": REF,
            "event_time": "2023-05-21T10:00:00Z"
        }]
    })
}

#[tokio::test]
async fn batch_store_sends_item_event_time_and_reads_truncated_and_warnings() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/memories/store/batch",
        200,
        r#"{"stored":[{"id":"wire-batch-1","content":"x","agent_id":"wire-agent","created_at":1}],"stored_count":1,"total_embedding_time_ms":2,
            "truncated":[{"id":"wire-batch-1","truncated":true,"content_tokens":900,"embedded_tokens":512,"max_tokens":512,"windows":2}],
            "warnings":["unknown field 'memories[].created_at' ignored (in 1 items)"]}"#,
    )
    .match_body(Matcher::Json(batch_all_fields_wire()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .store_memories_batch(batch_all_fields())
        .await
        .unwrap();
    assert_eq!(r.truncated.len(), 1);
    assert_eq!(r.truncated[0].id, "wire-batch-1");
    assert!(r.truncated[0].truncation.truncated);
    assert_eq!(r.truncated[0].truncation.content_tokens, 900);
    assert_eq!(r.truncated[0].truncation.windows, 2);
    assert!(!r.truncated[0].truncation.tokens_estimated);
    assert_eq!(r.warnings.len(), 1);
    m.assert_async().await;
}

#[test]
fn batch_store_response_of_an_older_server() {
    let r: dakera_client::memory::BatchStoreMemoryResponse = serde_json::from_value(json!({
        "stored": [], "stored_count": 0, "total_embedding_time_ms": 0
    }))
    .unwrap();
    assert!(r.truncated.is_empty());
    assert!(r.warnings.is_empty());
    assert!(r.ended_sessions.is_empty());
}

// ============================================================================
// Recall / batch recall: every field, warnings (#938), rerank_report.capped
// ============================================================================

fn recall_all_fields() -> RecallRequest {
    let mut req = RecallRequest::new("wire-agent", "Where did I meet Alice?")
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
    req.importance_weighted = Some(true);
    req
}

fn recall_all_fields_wire() -> Value {
    json!({
        "agent_id": "wire-agent",
        "query": "Where did I meet Alice?",
        "lang": "en",
        "top_k": 5,
        "memory_type": "semantic",
        "min_importance": 0.25,
        "session_id": "conv:42",
        "tags": ["trip"],
        "include_associated": true,
        "associated_memories_cap": 5,
        "associated_memories_depth": 2,
        "associated_memories_min_weight": 0.25,
        "since": "2023-01-01T00:00:00Z",
        "until": "2030-01-01T00:00:00Z",
        "routing": "hybrid",
        "rerank": true,
        "rerank_candidates": 50,
        "importance_weighted": true,
        "fusion": "minmax",
        "vector_weight": 0.5,
        "iterations": 2,
        "neighborhood": true,
        "neighborhood_mode": "separate",
        "include_summaries": false
    })
}

#[tokio::test]
async fn recall_sends_every_field_exactly_and_reads_warnings_and_capped() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/memory/recall",
        200,
        r#"{"memories":[],"query_embedding_time_ms":1,"search_time_ms":2,
            "rerank":{"applied":true,"candidates":50,"capped":true},
            "rerank_report":{"applied":true,"candidates":50,"capped":true},
            "warnings":["unknown field 'sesion_id' ignored"]}"#,
    )
    .match_body(Matcher::Json(recall_all_fields_wire()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.recall(recall_all_fields()).await.unwrap();
    let report = r.rerank_report.unwrap();
    assert!(report.applied && report.capped);
    assert_eq!(report.candidates, 50);
    assert_eq!(r.warnings, vec!["unknown field 'sesion_id' ignored"]);
    m.assert_async().await;
}

#[test]
fn recall_response_of_an_older_server() {
    let r: RecallResponse = serde_json::from_value(json!({
        "memories": [],
        "rerank_report": {"applied": false, "candidates": 0, "skipped": "bm25_routing"}
    }))
    .unwrap();
    assert!(r.warnings.is_empty());
    let report = r.rerank_report.unwrap();
    assert!(!report.capped);
    assert_eq!(report.skipped.as_deref(), Some("bm25_routing"));
    // capped is serialized only when true.
    assert!(serde_json::to_value(&report)
        .unwrap()
        .get("capped")
        .is_none());
}

fn batch_recall_all_fields() -> BatchRecallRequest {
    let filter = BatchMemoryFilter {
        tags: Some(vec!["trip".to_string()]),
        min_importance: Some(0.25),
        max_importance: Some(1.0),
        created_after: Some(0),
        created_before: Some(4_102_444_800),
        memory_type: Some(MemoryType::Semantic),
        session_id: Some("conv:42".to_string()),
    };
    BatchRecallRequest::new("wire-agent")
        .with_filter(filter)
        .with_limit(10)
}

fn batch_recall_all_fields_wire() -> Value {
    json!({
        "agent_id": "wire-agent",
        "filter": {
            "tags": ["trip"],
            "min_importance": 0.25,
            "max_importance": 1.0,
            "created_after": 0,
            "created_before": 4_102_444_800u64,
            "memory_type": "semantic",
            "session_id": "conv:42"
        },
        "limit": 10
    })
}

#[tokio::test]
async fn batch_recall_sends_every_filter_field_and_reads_warnings() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/memories/recall/batch",
        200,
        r#"{"memories":[],"total":0,"filtered":0,"truncated":false,"warnings":["unknown field 'filter.sesion_id' ignored"]}"#,
    )
    .match_body(Matcher::Json(batch_recall_all_fields_wire()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .batch_recall(batch_recall_all_fields())
        .await
        .unwrap();
    assert_eq!(r.warnings.len(), 1);
    assert!(!r.truncated);
    m.assert_async().await;

    let old: BatchRecallResponse = serde_json::from_value(
        json!({"memories": [], "total": 0, "filtered": 0, "truncated": true}),
    )
    .unwrap();
    assert!(old.warnings.is_empty());
    assert!(old.truncated);
}

#[tokio::test]
async fn recall_of_an_agent_without_memories_is_an_empty_answer() {
    // Server v0.12.3: 200 with no memories (older servers answered 404).
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/memory/recall",
        200,
        r#"{"memories":[],"query_embedding_time_ms":0,"search_time_ms":0,"rerank":{"applied":false,"candidates":0,"skipped":"no_candidates"},"rerank_report":{"applied":false,"candidates":0,"skipped":"no_candidates"}}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .recall(RecallRequest::new("nobody", "anything"))
        .await
        .unwrap();
    assert!(r.memories.is_empty());
    m.assert_async().await;
}

// ============================================================================
// Deduplication trash (#930)
// ============================================================================

#[tokio::test]
async fn list_deduplicated_reads_the_trash() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/memory/deduplicated",
        200,
        r#"{"agent_id":"a&b","memories":[
            {"id":"m2","kind":"deduplication","survivor_id":"m1","session_id":"conv:1","deleted_at":100,"restorable_until":200},
            {"id":"m3","kind":"consolidation","survivor_id":"m9","deleted_at":101,"restorable_until":201},
            {"id":"m4","kind":"future","survivor_id":"m9","deleted_at":102,"restorable_until":202}
        ],"count":3}"#,
    )
    .match_query(Matcher::UrlEncoded("agent_id".into(), "a&b".into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.list_deduplicated("a&b").await.unwrap();
    assert_eq!(r.count, 3);
    assert_eq!(r.memories[0].kind, DeduplicationKind::Deduplication);
    assert_eq!(r.memories[0].survivor_id, "m1");
    assert_eq!(r.memories[0].session_id.as_deref(), Some("conv:1"));
    assert_eq!(
        (r.memories[0].deleted_at, r.memories[0].restorable_until),
        (100, 200)
    );
    assert_eq!(r.memories[1].kind, DeduplicationKind::Consolidation);
    assert!(r.memories[1].session_id.is_none());
    assert_eq!(r.memories[2].kind, DeduplicationKind::Unknown);
    m.assert_async().await;
}

#[tokio::test]
async fn restore_deduplicated_escapes_the_id_and_returns_the_memory() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/memory/deduplicated/m%2F2/restore",
        200,
        r#"{"id":"m/2","agent_id":"a","content":"restored","memory_type":"episodic","importance":0.5,"tags":[],"created_at":1,"last_accessed_at":1,"access_count":0}"#,
    )
    .match_query(Matcher::UrlEncoded("agent_id".into(), "a".into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let mem = client.restore_deduplicated("a", "m/2").await.unwrap();
    assert_eq!(mem.id, "m/2");
    assert_eq!(mem.content, "restored");
    m.assert_async().await;
}

#[tokio::test]
async fn restore_deduplicated_errors() {
    let mut server = mockito::Server::new_async().await;
    let gone = json_mock(
        &mut server,
        "POST",
        "/v1/memory/deduplicated/gone/restore",
        404,
        r#"{"error":"No restorable copy of memory gone: it was not deleted by deduplication or consolidation, was restored already, or its retention ended","code":"VECTOR_NOT_FOUND","status":404,"resource":"memory"}"#,
    )
    .match_query(Matcher::Any)
    .create_async()
    .await;
    let held = json_mock(
        &mut server,
        "POST",
        "/v1/memory/deduplicated/held/restore",
        409,
        r#"{"error":"agent a holds memory held again","code":"CONFLICT","status":409}"#,
    )
    .match_query(Matcher::Any)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let e = client.restore_deduplicated("a", "gone").await.unwrap_err();
    assert!(e.is_not_found(), "{e:?}");
    match client.restore_deduplicated("a", "held").await.unwrap_err() {
        ClientError::Server { status, code, .. } => {
            assert_eq!(status, 409);
            assert_eq!(code, Some(ServerErrorCode::Conflict));
        }
        other => panic!("unexpected {other:?}"),
    }
    gone.assert_async().await;
    held.assert_async().await;
}

// ============================================================================
// get_memory without agent_id (#938)
// ============================================================================

#[tokio::test]
async fn get_memory_by_id_omits_agent_id() {
    let mut server = mockito::Server::new_async().await;
    // The unified path matcher matches only when there is no query string.
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/memory/get/m%2F1",
        200,
        r#"{"id":"m/1","agent_id":"a","content":"c","memory_type":"episodic","importance":0.5,"tags":[],"created_at":1,"last_accessed_at":1,"access_count":0}"#,
    )
    .expect(2)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    assert_eq!(client.get_memory_by_id("m/1").await.unwrap().id, "m/1");
    // An empty agent id omits it too.
    assert_eq!(client.get_memory("", "m/1").await.unwrap().id, "m/1");
    m.assert_async().await;
}

#[tokio::test]
async fn get_memory_with_agent_id_still_sends_it() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/memory/get/m1",
        200,
        r#"{"id":"m1","content":"c"}"#,
    )
    .match_query(Matcher::UrlEncoded("agent_id".into(), "a".into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client.get_memory("a", "m1").await.unwrap();
    m.assert_async().await;
}

#[tokio::test]
async fn get_memory_by_id_held_by_several_agents_is_a_400() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/memory/get/dup",
        400,
        r#"{"error":"memory id dup exists for 2 agents you can read (a, b); pass ?agent_id= to choose one","code":"INVALID_REQUEST","status":400}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    match client.get_memory_by_id("dup").await.unwrap_err() {
        ClientError::Server {
            status,
            message,
            code,
        } => {
            assert_eq!(status, 400);
            assert!(message.contains("pass ?agent_id="), "{message}");
            assert_eq!(code, Some(ServerErrorCode::InvalidRequest));
        }
        other => panic!("unexpected {other:?}"),
    }
    m.assert_async().await;
}

// ============================================================================
// Autopilot dedup settings (#930)
// ============================================================================

#[tokio::test]
async fn autopilot_config_update_sends_the_new_fields_only_when_set() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "PUT",
        "/v1/admin/autopilot/config",
        200,
        r#"{"success":true,"config":{"enabled":true,"dedup_threshold":0.93,"dedup_interval_hours":6,"consolidation_interval_hours":12,"dedup_mode":"merge","dedup_cross_session":true,"consolidate_cross_session":false,"trash_retention_hours":48},"message":"ok"}"#,
    )
    .match_body(Matcher::Json(json!({
        "dedup_mode": "merge",
        "dedup_cross_session": true,
        "consolidate_cross_session": false,
        "trash_retention_hours": 48
    })))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .autopilot_update_config(AutoPilotConfigRequest {
            dedup_mode: Some(DEDUP_MODE_MERGE.to_string()),
            dedup_cross_session: Some(true),
            consolidate_cross_session: Some(false),
            trash_retention_hours: Some(48),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(r.config.dedup_mode, "merge");
    assert!(r.config.dedup_cross_session);
    assert!(!r.config.consolidate_cross_session);
    assert_eq!(r.config.trash_retention_hours, 48);
    m.assert_async().await;

    let v = serde_json::to_value(AutoPilotConfigRequest::default()).unwrap();
    assert_eq!(v, json!({}));
}

#[test]
fn autopilot_answers_new_and_old() {
    let s: AutoPilotStatusResponse = serde_json::from_value(json!({
        "config": {"enabled": true, "dedup_threshold": 0.93, "dedup_interval_hours": 6,
                   "consolidation_interval_hours": 12, "dedup_mode": "report",
                   "dedup_cross_session": false, "consolidate_cross_session": true,
                   "trash_retention_hours": 168},
        "last_dedup_at": 5, "last_consolidation_at": null,
        "last_dedup": {"namespaces_processed": 1, "memories_scanned": 10,
                       "duplicates_removed": 0, "duplicates_found": 3},
        "last_consolidation": null, "total_dedup_removed": 0, "total_consolidated": 0
    }))
    .unwrap();
    assert_eq!(s.config.dedup_mode, DEDUP_MODE_REPORT);
    assert!(s.config.consolidate_cross_session);
    assert_eq!(s.config.trash_retention_hours, 168);
    assert_eq!(s.last_dedup.unwrap().duplicates_found, 3);

    let t: AutoPilotTriggerResponse = serde_json::from_value(json!({
        "success": true, "action": "dedup",
        "dedup": {"namespaces_processed": 1, "memories_scanned": 10,
                  "duplicates_removed": 0, "duplicates_found": 2, "mode": "report"},
        "consolidation": null, "message": "ok"
    }))
    .unwrap();
    let d = t.dedup.unwrap();
    assert_eq!((d.duplicates_found, d.mode.as_str()), (2, "report"));

    // An older server.
    let old: AutoPilotConfigResponse = serde_json::from_value(json!({
        "success": true, "message": "ok",
        "config": {"enabled": true, "dedup_threshold": 0.93, "dedup_interval_hours": 6,
                   "consolidation_interval_hours": 12}
    }))
    .unwrap();
    assert!(old.config.dedup_mode.is_empty());
    assert_eq!(old.config.trash_retention_hours, 0);
    let old: AutoPilotTriggerResponse = serde_json::from_value(json!({
        "success": true, "action": "dedup", "message": "ok",
        "dedup": {"namespaces_processed": 1, "memories_scanned": 10, "duplicates_removed": 4}
    }))
    .unwrap();
    let d = old.dedup.unwrap();
    assert_eq!((d.duplicates_found, d.mode.as_str()), (0, ""));
}

#[test]
fn runtime_config_reads_the_autopilot_overrides() {
    let base = json!({
        "default_index_type": "hnsw", "cache_enabled": true, "cache_max_size_bytes": 1,
        "rate_limit_enabled": false, "rate_limit_rps": 1, "query_timeout_ms": 1
    });
    let mut with = base.clone();
    with["autopilot_dedup_mode"] = json!("merge");
    with["autopilot_dedup_cross_session"] = json!(true);
    with["autopilot_consolidate_cross_session"] = json!(false);
    with["autopilot_trash_retention_hours"] = json!(24);
    let cfg: RuntimeConfig = serde_json::from_value(with).unwrap();
    assert_eq!(cfg.autopilot_dedup_mode.as_deref(), Some("merge"));
    assert_eq!(cfg.autopilot_dedup_cross_session, Some(true));
    assert_eq!(cfg.autopilot_consolidate_cross_session, Some(false));
    assert_eq!(cfg.autopilot_trash_retention_hours, Some(24));
    let cfg: RuntimeConfig = serde_json::from_value(base).unwrap();
    assert!(cfg.autopilot_dedup_mode.is_none());
    assert!(cfg.autopilot_trash_retention_hours.is_none());
}

// ============================================================================
// Capabilities naming, audit / memory events, error codes
// ============================================================================

#[test]
fn naming_reads_the_session_id_rules() {
    let n: NamingCapability = serde_json::from_value(json!({
        "agent_id_pattern": "^[a-zA-Z0-9][a-zA-Z0-9_\\-.]*$",
        "agent_id_max_bytes": 241,
        "session_id_pattern": "^[a-zA-Z0-9][a-zA-Z0-9_\\-:.]*$",
        "session_id_max_bytes": 256
    }))
    .unwrap();
    assert_eq!(n.session_id_pattern, "^[a-zA-Z0-9][a-zA-Z0-9_\\-:.]*$");
    assert_eq!(n.session_id_max_bytes, 256);
    assert!(!n.extra.contains_key("session_id_pattern"));
    let old: NamingCapability = serde_json::from_value(json!({"agent_id_max_bytes": 241})).unwrap();
    assert!(old.session_id_pattern.is_empty());
    assert_eq!(old.session_id_max_bytes, 0);
}

#[tokio::test]
async fn audit_events_read_reason_and_the_server_list_shape() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/audit",
        200,
        r#"{"events":[
            {"id":7,"event_type":"deduplicated","agent_id":"a","memory_id":"m2","timestamp":1700000000123,"reason":"absorbed into m1"},
            {"id":6,"event_type":"stored","agent_id":"a","memory_id":"m1","importance":0.5,"timestamp":1700000000000}
        ],"count":2}"#,
    )
    .match_query(Matcher::AllOf(vec![
        Matcher::UrlEncoded("agent_id".into(), "a".into()),
        Matcher::UrlEncoded("event_type".into(), "deduplicated".into()),
        Matcher::UrlEncoded("from".into(), "1700000000000".into()),
        Matcher::UrlEncoded("to".into(), "1700000001000".into()),
        Matcher::UrlEncoded("limit".into(), "10".into()),
    ]))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r: AuditListResponse = client
        .list_audit_events(dakera_client::AuditQuery {
            agent_id: Some("a".into()),
            event_type: Some(EVENT_DEDUPLICATED.into()),
            from: Some(1_700_000_000_000),
            to: Some(1_700_000_001_000),
            limit: Some(10),
            cursor: None,
        })
        .await
        .unwrap();
    assert_eq!(r.total, 2);
    assert!(r.cursor.is_none());
    assert_eq!(r.events[0].id, "7");
    assert_eq!(r.events[0].timestamp, 1_700_000_000_123);
    assert_eq!(r.events[0].reason.as_deref(), Some("absorbed into m1"));
    assert!(r.events[1].reason.is_none());
    m.assert_async().await;

    // A string id (older SDK-written fixtures) still decodes.
    let e: AuditEvent = serde_json::from_value(
        json!({"id": "x1", "event_type": "restored", "agent_id": "a", "timestamp": 1}),
    )
    .unwrap();
    assert_eq!(e.id, "x1");
}

#[test]
fn memory_events_read_the_dedup_reason() {
    let e: MemoryEvent = serde_json::from_value(json!({
        "event_type": "deduplicated", "agent_id": "a", "memory_id": "m2",
        "timestamp": 1, "reason": "absorbed into m1"
    }))
    .unwrap();
    assert_eq!(e.event_type, "deduplicated");
    assert_eq!(e.reason.as_deref(), Some("absorbed into m1"));
    let e: MemoryEvent = serde_json::from_value(json!({
        "event_type": "restored", "agent_id": "a", "memory_id": "m2", "timestamp": 2
    }))
    .unwrap();
    assert!(e.reason.is_none());
}

#[tokio::test]
async fn host_not_allowed_keeps_the_server_text_and_code() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/namespaces",
        421,
        r#"{"error":"authentication is disabled, so requests are served only for localhost ... (DNS rebinding protection)","code":"HOST_NOT_ALLOWED","status":421}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    match client.list_namespaces().await.unwrap_err() {
        ClientError::Server {
            status,
            message,
            code,
        } => {
            assert_eq!(status, 421);
            assert!(message.contains("DNS rebinding"), "{message}");
            assert_eq!(code, Some(ServerErrorCode::HostNotAllowed));
        }
        other => panic!("unexpected {other:?}"),
    }
    m.assert_async().await;
}

#[test]
fn error_codes_of_this_release() {
    for (wire, code) in [
        ("HOST_NOT_ALLOWED", ServerErrorCode::HostNotAllowed),
        ("RATE_LIMIT_EXCEEDED", ServerErrorCode::RateLimitExceeded),
        (
            "UNPROCESSABLE_CONTENT",
            ServerErrorCode::UnprocessableContent,
        ),
        ("EXTRACTION_TIMEOUT", ServerErrorCode::ExtractionTimeout),
    ] {
        assert_eq!(
            serde_json::from_value::<ServerErrorCode>(json!(wire)).unwrap(),
            code
        );
    }
}

#[tokio::test]
async fn a_422_keeps_the_server_text_and_code() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/namespaces",
        422,
        r#"{"error":"the document has no text","code":"UNPROCESSABLE_CONTENT","status":422}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    match client.list_namespaces().await.unwrap_err() {
        ClientError::Server {
            status,
            message,
            code,
        } => {
            assert_eq!(status, 422);
            assert_eq!(message, "the document has no text");
            assert_eq!(code, Some(ServerErrorCode::UnprocessableContent));
        }
        other => panic!("unexpected {other:?}"),
    }
    m.assert_async().await;
}
