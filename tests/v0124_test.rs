//! Server v0.12.4 wire support: forgetting by description (`about`) and by a
//! user's request (`request`, `POST /v1/memory/forget/detect`), restorable and
//! dry-run forgets, the forget-request reading on store, batch store
//! duplicates, recall `rerank_head` / `instructions` / `degraded` and the new
//! server defaults, session `auto_summarize` / `summary_pending`, audit cursor
//! pagination, strength projection, the import upload and its async job,
//! link labels, extraction and entity-config provenance, memory policy
//! `stored`, the failure lists of dedup / network / compression / autopilot /
//! decay / TTL cleanup, key changes answered `202`, and `503` + `Retry-After`.
//!
//! Wire shapes follow the server source (`common/src/types/mod.rs`,
//! `api/src/routes/memory/mod.rs`, `api/src/routes/transfer.rs`,
//! `api/src/routes/audit.rs`, `api/src/routes/admin/mod.rs`,
//! `api/src/memory_ops/dedup_trash.rs`, `api/src/entity_tagging.rs`).

use dakera_client::memory::{
    BatchStoreMemoryItem, BatchStoreMemoryRequest, ConsolidateRequest, ForgetRequest,
    ImportOptions, SessionEndRequest,
};
use dakera_client::{
    AuditQuery, AutoPilotConfigRequest, AutoPilotStatusResponse, AutoPilotTriggerResponse,
    ClientError, CompressResponse, CrossAgentNetworkResponse, DakeraClient, DecayStatsResponse,
    DeduplicateRequest, DeduplicationKind, DerivationStatus, MemoryPolicy, MemoryType,
    RecallRequest, RecallResponse, StoreMemoryRequest, TtlCleanupResponse, WarmCacheRequest,
    CONSOLIDATION_MODE_REPORT, INSTRUCTION_TAG, RETRIEVAL_INSTRUCTION,
};
use mockito::Matcher;
use serde_json::{json, Value};

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

const MEMORY: &str = r#"{"id":"m1","content":"I love sushi","agent_id":"a","memory_type":"episodic","importance":0.5,"tags":[],"created_at":1,"last_accessed_at":1,"access_count":0}"#;

// ============================================================================
// Forget by description / request, restorable, dry run
// ============================================================================

#[tokio::test]
async fn forget_about_sends_the_v0124_fields_and_reads_forgotten_ids() {
    let mut server = mockito::Server::new_async().await;
    let mock = json_mock(
        &mut server,
        "POST",
        "/v1/memory/forget",
        200,
        r#"{"deleted_count":2,"forgotten_ids":["m2","m1"]}"#,
    )
    .match_body(Matcher::Json(json!({
        "agent_id": "a",
        "about": "I love sushi",
        "until": 1_700_000_000u64,
        "min_score": 0.75,
        "limit": 5,
        "restorable": true
    })))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let resp = client
        .forget(
            ForgetRequest::about("a", "I love sushi")
                .with_until(1_700_000_000)
                .with_min_score(0.75)
                .with_limit(5)
                .restorable(),
        )
        .await
        .unwrap();
    assert_eq!(resp.deleted_count, 2);
    assert_eq!(
        resp.forgotten_ids.as_deref(),
        Some(&["m2".to_string(), "m1".to_string()][..])
    );
    assert!(resp.intent.is_none());
    mock.assert_async().await;
}

#[tokio::test]
async fn forget_by_request_dry_run_reads_the_intent() {
    let mut server = mockito::Server::new_async().await;
    let mock = json_mock(
        &mut server,
        "POST",
        "/v1/memory/forget",
        200,
        r#"{"deleted_count":0,"forgotten_ids":["m1"],
            "intent":{"probability":0.97,"threshold":0.5,"is_request":true,
                      "sentence":"Please forget that I love sushi."}}"#,
    )
    .match_body(Matcher::Json(json!({
        "agent_id": "a",
        "request": "Oublie ce que je t'ai dit sur les sushis",
        "dry_run": true
    })))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let resp = client
        .forget(
            ForgetRequest::by_request("a", "Oublie ce que je t'ai dit sur les sushis").dry_run(),
        )
        .await
        .unwrap();
    let intent = resp.intent.expect("intent");
    assert!(intent.is_request);
    assert_eq!(intent.threshold, 0.5);
    assert_eq!(
        intent.sentence.as_deref(),
        Some("Please forget that I love sushi.")
    );
    assert_eq!(resp.forgotten_ids.unwrap(), vec!["m1".to_string()]);
    mock.assert_async().await;
}

#[test]
fn forget_filters_do_not_send_empty_lists() {
    // An empty `memory_ids` list is a filter that matches nothing on the
    // server, so `by_session` / `by_tags` must leave it out.
    let by_session = serde_json::to_value(ForgetRequest::by_session("a", "s1")).unwrap();
    assert_eq!(by_session, json!({"agent_id": "a", "session_id": "s1"}));
    let by_tags = serde_json::to_value(ForgetRequest::by_tags("a", vec!["t".into()])).unwrap();
    assert_eq!(by_tags, json!({"agent_id": "a", "tags": ["t"]}));
    let by_ids = serde_json::to_value(ForgetRequest::by_ids("a", vec!["m1".into()])).unwrap();
    assert_eq!(by_ids, json!({"agent_id": "a", "memory_ids": ["m1"]}));
    let typed =
        serde_json::to_value(ForgetRequest::about("a", "x").with_type(MemoryType::Semantic))
            .unwrap();
    assert_eq!(
        typed,
        json!({"agent_id": "a", "about": "x", "memory_type": "semantic"})
    );
}

#[tokio::test]
async fn forget_refuses_invalid_v0124_fields_before_sending() {
    // No mock: a request that reached the server would fail differently.
    let client = DakeraClient::new("http://127.0.0.1:9").unwrap();
    let mut both = ForgetRequest::about("a", "x");
    both.request = Some("forget x".into());
    let too_long = ForgetRequest::about("a", "x".repeat(8_001));
    for req in [
        both,
        too_long,
        ForgetRequest::about("a", "  "),
        ForgetRequest::about("a", "x").with_limit(0),
        ForgetRequest::about("a", "x").with_limit(51),
        ForgetRequest::about("a", "x").with_min_score(1.5),
    ] {
        match client.forget(req).await {
            Err(ClientError::InvalidRequest(_)) => {}
            other => panic!("expected InvalidRequest, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn forget_without_reranker_is_a_retryable_503() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("POST", "/v1/memory/forget")
        .with_status(503)
        .with_header("content-type", "application/json")
        .with_header("retry-after", "5")
        .with_body(r#"{"error":"the re-ranker is not available","code":"SERVICE_UNAVAILABLE"}"#)
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let err = client
        .forget(ForgetRequest::about("a", "x"))
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::ServiceUnavailable { .. }));
    assert!(err.is_retryable());
    assert_eq!(err.retry_after(), Some(std::time::Duration::from_secs(5)));
}

#[tokio::test]
async fn detect_forget_requests_round_trip_and_bounds() {
    let mut server = mockito::Server::new_async().await;
    let mock = json_mock(
        &mut server,
        "POST",
        "/v1/memory/forget/detect",
        200,
        r#"{"model":"multilingual-minilm","threshold":0.5,"sentence_threshold":0.8,
            "results":[{"probability":0.98,"is_request":true},
                       {"probability":0.02,"is_request":false}]}"#,
    )
    .match_body(Matcher::Json(json!({
        "texts": ["Please forget my address", "I always forget my keys"]
    })))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let resp = client
        .detect_forget_requests(&["Please forget my address", "I always forget my keys"])
        .await
        .unwrap();
    assert_eq!(resp.model, "multilingual-minilm");
    assert_eq!(resp.sentence_threshold, 0.8);
    assert_eq!(resp.results.len(), 2);
    assert!(resp.results[0].is_request);
    assert!(!resp.results[1].is_request);
    assert!(resp.results[1].sentence.is_none());
    mock.assert_async().await;

    let none: [&str; 0] = [];
    assert!(matches!(
        client.detect_forget_requests(&none).await,
        Err(ClientError::InvalidRequest(_))
    ));
    let many = vec!["x".to_string(); 65];
    assert!(matches!(
        client.detect_forget_requests(&many).await,
        Err(ClientError::InvalidRequest(_))
    ));
    let long = vec!["x".repeat(8_001)];
    assert!(matches!(
        client.detect_forget_requests(&long).await,
        Err(ClientError::InvalidRequest(_))
    ));
}

// ============================================================================
// Store / batch store answers
// ============================================================================

#[tokio::test]
async fn store_reads_forget_request() {
    let mut server = mockito::Server::new_async().await;
    let _m = json_mock(
        &mut server,
        "POST",
        "/v1/memory/store",
        200,
        &format!(
            r#"{{"memory":{MEMORY},"embedding_time_ms":3,
                "forget_request":{{"probability":0.91,"threshold":0.5,"is_request":true}}}}"#
        ),
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let resp = client
        .store_memory(StoreMemoryRequest::new("a", "Please forget my address"))
        .await
        .unwrap();
    let intent = resp.forget_request.expect("forget_request");
    assert!(intent.is_request);
    assert!((intent.probability - 0.91).abs() < 1e-6);

    // An older answer has none.
    let old: dakera_client::StoreMemoryResponse =
        serde_json::from_str(&format!(r#"{{"memory":{MEMORY}}}"#)).unwrap();
    assert!(old.forget_request.is_none());
}

#[tokio::test]
async fn batch_store_reads_forget_requests_and_duplicates() {
    let mut server = mockito::Server::new_async().await;
    let _m = json_mock(
        &mut server,
        "POST",
        "/v1/memories/store/batch",
        200,
        r#"{"stored":[],"stored_count":1,"total_embedding_time_ms":4,
            "forget_requests":[{"id":"m1","probability":0.9,"threshold":0.5,"is_request":true,
                                "sentence":"forget it"}],
            "duplicates":[{"index":1,"existing_id":"m0"}]}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let resp = client
        .store_memories_batch(BatchStoreMemoryRequest::new(
            "a",
            vec![
                BatchStoreMemoryItem::new("forget it"),
                BatchStoreMemoryItem::new("dup"),
            ],
        ))
        .await
        .unwrap();
    assert_eq!(resp.forget_requests.len(), 1);
    assert_eq!(resp.forget_requests[0].id, "m1");
    assert!(resp.forget_requests[0].intent.is_request);
    assert_eq!(
        resp.forget_requests[0].intent.sentence.as_deref(),
        Some("forget it")
    );
    assert_eq!(resp.duplicates.len(), 1);
    assert_eq!(resp.duplicates[0].index, 1);
    assert_eq!(resp.duplicates[0].existing_id, "m0");
}

// ============================================================================
// Recall: server defaults, rerank_head, instructions, degraded
// ============================================================================

#[test]
fn recall_leaves_the_v0124_defaults_to_the_server() {
    let wire = serde_json::to_value(RecallRequest::new("a", "q")).unwrap();
    let obj = wire.as_object().unwrap();
    assert!(!obj.contains_key("include_associated"));
    assert!(!obj.contains_key("importance_weighted"));
    assert!(!obj.contains_key("rerank_head"));

    let wire = serde_json::to_value(
        RecallRequest::new("a", "q")
            .with_include_associated(false)
            .with_importance_weighted(false)
            .with_rerank_head(0),
    )
    .unwrap();
    assert_eq!(wire["include_associated"], json!(false));
    assert_eq!(wire["importance_weighted"], json!(false));
    assert_eq!(wire["rerank_head"], json!(0));
}

#[test]
fn recall_response_reads_instructions_and_degraded() {
    let resp: RecallResponse = serde_json::from_value(json!({
        "memories": [],
        "instructions": [{
            "id": "i1", "content": "From now on, answer in French", "memory_type": "procedural",
            "importance": 0.7, "score": 0.4, "tags": [INSTRUCTION_TAG],
            "retrieval": RETRIEVAL_INSTRUCTION
        }],
        "degraded": ["graph: the graph store could not be read"]
    }))
    .unwrap();
    let instructions = resp.instructions.expect("instructions");
    assert_eq!(instructions[0].id, "i1");
    assert_eq!(
        instructions[0].retrieval.as_deref(),
        Some(RETRIEVAL_INSTRUCTION)
    );
    assert_eq!(resp.degraded.len(), 1);

    let old: RecallResponse = serde_json::from_value(json!({"memories": []})).unwrap();
    assert!(old.instructions.is_none());
    assert!(old.degraded.is_empty());
}

// ============================================================================
// Sessions
// ============================================================================

#[tokio::test]
async fn end_session_with_auto_summarize_reads_summary_pending() {
    let mut server = mockito::Server::new_async().await;
    let session = r#"{"id":"s1","agent_id":"a","started_at":1,"ended_at":2,"memory_count":4}"#;
    let mock = json_mock(
        &mut server,
        "POST",
        "/v1/sessions/s1/end",
        200,
        &format!(
            r#"{{"session":{session},"memory_count":4,
                "summary_pending":"the store refused the summary; it is stored once it recovers"}}"#
        ),
    )
    .match_body(Matcher::Json(json!({"auto_summarize": true})))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let resp = client
        .end_session_with(
            "s1",
            &SessionEndRequest::default().with_auto_summarize(true),
        )
        .await
        .unwrap();
    assert!(resp.summary_pending.is_some());
    mock.assert_async().await;

    // end_session without a summary still sends an empty body.
    let mock = json_mock(
        &mut server,
        "POST",
        "/v1/sessions/s2/end",
        200,
        &format!(r#"{{"session":{session},"memory_count":4}}"#),
    )
    .match_body(Matcher::Json(json!({})))
    .create_async()
    .await;
    let resp = client.end_session("s2", None).await.unwrap();
    assert!(resp.summary_pending.is_none());
    mock.assert_async().await;
}

// ============================================================================
// Audit cursor
// ============================================================================

#[tokio::test]
async fn audit_pages_by_cursor() {
    let mut server = mockito::Server::new_async().await;
    let mock = json_mock(
        &mut server,
        "GET",
        "/v1/audit",
        200,
        r#"{"events":[{"id":7,"event_type":"stored","timestamp":5}],"count":1,
            "next_cursor":"5:7"}"#,
    )
    .match_query(Matcher::AllOf(vec![
        Matcher::UrlEncoded("agent_id".into(), "a".into()),
        Matcher::UrlEncoded("limit".into(), "1".into()),
        Matcher::UrlEncoded("cursor".into(), "9:12".into()),
    ]))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let page = client
        .list_audit_events(AuditQuery {
            agent_id: Some("a".into()),
            limit: Some(1),
            cursor: Some("9:12".into()),
            ..AuditQuery::default()
        })
        .await
        .unwrap();
    assert_eq!(page.next_cursor.as_deref(), Some("5:7"));
    assert_eq!(page.events[0].id, "7");
    mock.assert_async().await;
}

// ============================================================================
// Strength projection
// ============================================================================

#[tokio::test]
async fn get_memory_with_projection_reads_the_projection() {
    let mut server = mockito::Server::new_async().await;
    let mut body: Value = serde_json::from_str(MEMORY).unwrap();
    body["strength_projection"] = json!([
        {"day": 0, "importance": 0.5},
        {"day": 1, "importance": 0.48}
    ]);
    let mock = json_mock(
        &mut server,
        "GET",
        "/v1/memory/get/m1",
        200,
        &body.to_string(),
    )
    .match_query(Matcher::AllOf(vec![
        Matcher::UrlEncoded("projection_days".into(), "1".into()),
        Matcher::UrlEncoded("agent_id".into(), "a".into()),
    ]))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let got = client
        .get_memory_with_projection("a", "m1", 1)
        .await
        .unwrap();
    assert_eq!(got.memory.id, "m1");
    assert_eq!(got.strength_projection.len(), 2);
    assert_eq!(got.strength_projection[1].day, 1);
    mock.assert_async().await;

    for days in [0, 366] {
        assert!(matches!(
            client.get_memory_with_projection("a", "m1", days).await,
            Err(ClientError::InvalidRequest(_))
        ));
    }
}

// ============================================================================
// Trash: content and restorable deletions
// ============================================================================

#[tokio::test]
async fn deduplicated_lists_content_and_deletions() {
    let mut server = mockito::Server::new_async().await;
    let _m = json_mock(
        &mut server,
        "GET",
        "/v1/memory/deduplicated",
        200,
        r#"{"agent_id":"a","count":1,"memories":[{"id":"m1","kind":"deletion","survivor_id":"",
            "deleted_at":10,"restorable_until":20,"content":"I love sushi"}]}"#,
    )
    .match_query(Matcher::UrlEncoded("agent_id".into(), "a".into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let list = client.list_deduplicated("a").await.unwrap();
    assert_eq!(list.memories[0].kind, DeduplicationKind::Deletion);
    assert_eq!(list.memories[0].content.as_deref(), Some("I love sushi"));
}

// ============================================================================
// Import upload and async jobs
// ============================================================================

#[tokio::test]
async fn import_memories_uploads_a_multipart_file() {
    let mut server = mockito::Server::new_async().await;
    let mock = json_mock(
        &mut server,
        "POST",
        "/v1/import",
        200,
        r#"{"job_id":"j1","status":"done","format":"jsonl","total":2,"imported":2,"skipped":0,
            "errors":[],"started_at":1,"finished_at":2}"#,
    )
    .match_query(Matcher::AllOf(vec![
        Matcher::UrlEncoded("agent_id".into(), "a".into()),
        Matcher::UrlEncoded("format".into(), "jsonl".into()),
    ]))
    .match_header(
        "content-type",
        Matcher::Regex("^multipart/form-data; boundary=".into()),
    )
    .match_body(Matcher::AllOf(vec![
        Matcher::Regex(r#"filename="import\.jsonl""#.into()),
        Matcher::Regex("\\{\"content\":\"one\"\\}\n\\{\"content\":\"two\"\\}\n".into()),
    ]))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let resp = client
        .import_memories(
            json!([{"content": "one"}, {"content": "two"}]),
            "jsonl",
            Some("a"),
            None,
        )
        .await
        .unwrap();
    assert_eq!(resp.imported_count, 2);
    assert_eq!(resp.skipped_count, 0);
    mock.assert_async().await;

    assert!(matches!(
        client.import_memories(json!([]), "jsonl", None, None).await,
        Err(ClientError::InvalidRequest(_))
    ));
}

#[tokio::test]
async fn import_file_async_answers_the_running_job() {
    let mut server = mockito::Server::new_async().await;
    let mock = json_mock(
        &mut server,
        "POST",
        "/v1/import",
        202,
        r#"{"job_id":"j2","status":"running","format":"csv","total":10,"imported":0,
            "skipped":0,"errors":[],"started_at":1,"finished_at":null}"#,
    )
    .match_query(Matcher::AllOf(vec![
        Matcher::UrlEncoded("agent_id".into(), "a".into()),
        Matcher::UrlEncoded("async".into(), "true".into()),
    ]))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let job = client
        .import_memories_file(
            "a",
            "memories.csv",
            b"content\nhello\n".to_vec(),
            &ImportOptions {
                format: None,
                run_async: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(job.status, "running");
    assert_eq!(job.total, 10);
    assert!(job.finished_at.is_none());
    mock.assert_async().await;
}

// ============================================================================
// Consolidate: the fields the server reads
// ============================================================================

#[tokio::test]
async fn consolidate_sends_memory_ids_and_target_type_and_refuses_dry_run() {
    let mut server = mockito::Server::new_async().await;
    let mock = json_mock(
        &mut server,
        "POST",
        "/v1/memory/consolidate",
        200,
        &format!(
            r#"{{"consolidated_memory":{MEMORY},"source_memory_ids":["m2","m3"],
                "memories_removed":2}}"#
        ),
    )
    .match_body(Matcher::Json(json!({
        "agent_id": "a",
        "threshold": 0.75,
        "memory_ids": ["m2", "m3"],
        "target_type": "semantic"
    })))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let resp = client
        .consolidate(
            "a",
            ConsolidateRequest {
                threshold: Some(0.75),
                memory_type: Some("episodic".into()),
                memory_ids: Some(vec!["m2".into(), "m3".into()]),
                target_type: Some(MemoryType::Semantic),
                ..ConsolidateRequest::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.memories_removed, 2);
    assert_eq!(resp.source_memory_ids, vec!["m2", "m3"]);
    assert_eq!(resp.consolidated_memory.unwrap().id, "m1");
    mock.assert_async().await;

    let dry = ConsolidateRequest {
        dry_run: true,
        ..ConsolidateRequest::default()
    };
    assert!(matches!(
        client.consolidate("a", dry).await,
        Err(ClientError::InvalidRequest(_))
    ));
}

// ============================================================================
// Graph links, extraction, entity config, memory policy
// ============================================================================

#[tokio::test]
async fn link_label_is_read_back() {
    let mut server = mockito::Server::new_async().await;
    let mock = json_mock(
        &mut server,
        "POST",
        "/v1/memories/m1/links",
        200,
        r#"{"from_id":"m1","to_id":"m2","edge_type":"linked_by","label":"caused"}"#,
    )
    .match_body(Matcher::PartialJson(
        json!({"label": "caused", "target_id": "m2"}),
    ))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let link = client
        .memory_link("a", "m1", "m2", Some("caused"))
        .await
        .unwrap();
    assert_eq!(link.label.as_deref(), Some("caused"));
    assert_eq!(link.edge.label.as_deref(), Some("caused"));
    mock.assert_async().await;
}

#[test]
fn extraction_and_entity_config_read_their_provenance() {
    let ex: dakera_client::EntityExtractionResponse = serde_json::from_value(json!({
        "entities": [], "count": 0, "extractor": "rule_based",
        "warnings": ["the neural pass failed; rule-based entities only"]
    }))
    .unwrap();
    assert_eq!(ex.extractor.as_deref(), Some("rule_based"));
    assert_eq!(ex.warnings.len(), 1);

    let cfg: dakera_client::NamespaceEntityConfig = serde_json::from_value(json!({
        "namespace": "ns", "extract_entities": true,
        "entity_types": ["person", "organization", "location"], "source": "server_default"
    }))
    .unwrap();
    assert_eq!(
        cfg.source.as_deref(),
        Some(dakera_client::ENTITY_CONFIG_SOURCE_SERVER_DEFAULT)
    );
}

#[test]
fn memory_policy_reads_stored_and_never_sends_it() {
    let policy: MemoryPolicy = serde_json::from_value(json!({
        "working_ttl_seconds": null, "working_decay": "exponential", "stored": false
    }))
    .unwrap();
    assert_eq!(policy.stored, Some(false));
    assert!(policy.working_ttl_seconds.is_none());
    let wire = serde_json::to_value(&policy).unwrap();
    assert!(wire.get("stored").is_none());

    // The defaults match the server's: no TTL by type.
    let default = serde_json::to_value(MemoryPolicy::default()).unwrap();
    for ttl in [
        "working_ttl_seconds",
        "episodic_ttl_seconds",
        "semantic_ttl_seconds",
        "procedural_ttl_seconds",
    ] {
        assert!(default.get(ttl).is_none(), "{ttl} sent by default");
    }
    assert_eq!(MemoryPolicy::default().dedup_threshold, Some(0.95));
}

// ============================================================================
// Failure lists: dedup, network, compression, autopilot, decay, TTL
// ============================================================================

#[tokio::test]
async fn deduplicate_reads_groups_failed() {
    let mut server = mockito::Server::new_async().await;
    let _m = json_mock(
        &mut server,
        "POST",
        "/v1/knowledge/deduplicate",
        200,
        r#"{"groups":[],"duplicates_found":2,"duplicates_merged":0,
            "groups_failed":[{"canonical_id":"m1","duplicate_ids":["m2"],"reason":"write failed"}]}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let resp = client
        .deduplicate(DeduplicateRequest {
            agent_id: "a".into(),
            threshold: None,
            memory_type: None,
            dry_run: false,
        })
        .await
        .unwrap();
    assert_eq!(resp.groups_failed.len(), 1);
    assert_eq!(resp.groups_failed[0].duplicate_ids, vec!["m2"]);
}

#[test]
fn network_and_compression_read_what_they_could_not_do() {
    let net: CrossAgentNetworkResponse = serde_json::from_value(json!({
        "agents": [], "nodes": [], "edges": [],
        "stats": {"total_agents": 0, "total_nodes": 0, "total_cross_edges": 0, "density": 0.0},
        "unavailable": [{"agent_id": "b", "reason": "storage error"}]
    }))
    .unwrap();
    assert_eq!(net.unavailable[0].agent_id, "b");

    let c: CompressResponse = serde_json::from_value(json!({
        "agent_id": "a", "deprecated_ids": ["m1"], "deprecation_skipped": ["m2"],
        "deprecation_error": "write refused"
    }))
    .unwrap();
    assert_eq!(c.deprecation_skipped, vec!["m2"]);
    assert_eq!(c.deprecation_error.as_deref(), Some("write refused"));
}

#[test]
fn autopilot_decay_ttl_and_derivations_read_v0124_fields() {
    let status: AutoPilotStatusResponse = serde_json::from_value(json!({
        "config": {"enabled": true, "dedup_threshold": 0.93, "dedup_interval_hours": 6,
                   "consolidation_interval_hours": 12, "dedup_mode": "report",
                   "consolidation_mode": "report", "dedup_cross_session": false,
                   "consolidate_cross_session": false, "trash_retention_hours": 168},
        "last_dedup": {"namespaces_processed": 2, "memories_scanned": 9,
                       "duplicates_removed": 0, "duplicates_found": 1,
                       "failures": ["ns1: could not read"]},
        "last_consolidation": {"namespaces_processed": 2, "memories_scanned": 9,
                               "clusters_merged": 0, "memories_consolidated": 0,
                               "clusters_found": 1, "memories_in_clusters": 3},
        "total_dedup_removed": 0, "total_consolidated": 0
    }))
    .unwrap();
    assert_eq!(status.config.consolidation_mode, CONSOLIDATION_MODE_REPORT);
    assert_eq!(status.last_dedup.unwrap().failures.len(), 1);
    let cons = status.last_consolidation.unwrap();
    assert_eq!((cons.clusters_found, cons.memories_in_clusters), (1, 3));

    let req = AutoPilotConfigRequest {
        consolidation_mode: Some("merge".into()),
        ..AutoPilotConfigRequest::default()
    };
    assert_eq!(
        serde_json::to_value(req).unwrap(),
        json!({"consolidation_mode": "merge"})
    );

    let trig: AutoPilotTriggerResponse = serde_json::from_value(json!({
        "success": false, "action": "all", "message": "partial",
        "dedup": {"namespaces_processed": 1, "memories_scanned": 1, "duplicates_removed": 0,
                  "duplicates_found": 0, "mode": "report", "failures": ["ns: search failed"]},
        "consolidation": {"namespaces_processed": 1, "memories_scanned": 1, "clusters_merged": 0,
                          "memories_consolidated": 0, "clusters_found": 0,
                          "memories_in_clusters": 0, "mode": "report"}
    }))
    .unwrap();
    assert_eq!(trig.dedup.unwrap().failures.len(), 1);
    assert_eq!(trig.consolidation.unwrap().mode, "report");

    let decay: DecayStatsResponse = serde_json::from_value(json!({
        "total_decayed": 1, "total_deleted": 0, "cycles_run": 3, "cycles_failed": 1,
        "last_cycle": {"namespaces_processed": 1, "memories_processed": 2,
                       "memories_decayed": 1, "memories_deleted": 0,
                       "failures": ["ns: page read failed"]}
    }))
    .unwrap();
    assert_eq!(decay.cycles_failed, 1);
    assert_eq!(decay.last_cycle.unwrap().failures.len(), 1);

    let ttl: TtlCleanupResponse = serde_json::from_value(json!({
        "success": false, "vectors_removed": 0, "namespaces_cleaned": [],
        "message": "1 namespace failed", "failed": [{"namespace": "ns", "reason": "listing failed"}]
    }))
    .unwrap();
    assert!(!ttl.success);
    assert_eq!(ttl.failed[0].namespace, "ns");

    let deriv: DerivationStatus = serde_json::from_value(json!({
        "settled": false,
        "entity_tagging": {"mode": "background", "pending": 4, "sweep_namespaces": [],
                           "tagged": 10, "already_tagged": 0, "superseded": 0, "retried": 1,
                           "dropped_to_sweep": 0, "failed": 0, "extractor_gave_up": 0}
    }))
    .unwrap();
    let tagging = deriv.entity_tagging.expect("entity_tagging");
    assert_eq!((tagging.pending, tagging.tagged), (4, 10));
    assert!(!deriv.extra.contains_key("entity_tagging"));
}

#[test]
fn warm_cache_sends_the_v0124_selectors() {
    let req = WarmCacheRequest::new("ns")
        .with_query_vector(vec![0.5, 0.25])
        .with_min_score_threshold(0.5)
        .with_recently_accessed_within(3600);
    let wire = serde_json::to_value(req).unwrap();
    assert_eq!(wire["query_vector"], json!([0.5, 0.25]));
    assert_eq!(wire["min_score_threshold"], json!(0.5));
    assert_eq!(wire["recently_accessed_within"], json!(3600));
    let plain = serde_json::to_value(WarmCacheRequest::new("ns")).unwrap();
    assert!(plain.get("query_vector").is_none());
}

#[tokio::test]
async fn analytics_overview_echoes_period_and_namespace() {
    let mut server = mockito::Server::new_async().await;
    let _m = json_mock(
        &mut server,
        "GET",
        "/v1/analytics/overview",
        200,
        r#"{"period":"1h","namespace":"ns","total_queries":1,"avg_latency_ms":1.0,
            "p95_latency_ms":1.0,"p99_latency_ms":1.0,"queries_per_second":0.1,"error_rate":0.0,
            "cache_hit_rate":0.0,"storage_used_bytes":1,"total_vectors":1,"total_namespaces":1,
            "uptime_seconds":5}"#,
    )
    .match_query(Matcher::AllOf(vec![
        Matcher::UrlEncoded("period".into(), "1h".into()),
        Matcher::UrlEncoded("namespace".into(), "ns".into()),
    ]))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let o = client
        .analytics_overview(Some("1h"), Some("ns"))
        .await
        .unwrap();
    assert_eq!(o.period.as_deref(), Some("1h"));
    assert_eq!(o.namespace.as_deref(), Some("ns"));
}

// ============================================================================
// Key changes answered as stored, and the fenced-namespace 503
// ============================================================================

#[tokio::test]
async fn key_revocation_reports_pending_on_202_and_errors_on_409_503() {
    let mut server = mockito::Server::new_async().await;
    let _a = json_mock(
        &mut server,
        "DELETE",
        "/admin/keys/k-logged",
        202,
        r#"{"success":true,"message":"revoked on every node; storing it is retried"}"#,
    )
    .create_async()
    .await;
    let _b = json_mock(
        &mut server,
        "DELETE",
        "/admin/keys/k-stored",
        200,
        r#"{"success":true,"message":"revoked"}"#,
    )
    .create_async()
    .await;
    let _c = json_mock(
        &mut server,
        "POST",
        "/admin/keys/k-late/deactivate",
        409,
        r#"{"error":"a later change took over","code":"CONFLICT"}"#,
    )
    .create_async()
    .await;
    let _d = server
        .mock("DELETE", "/admin/keys/k-unlogged")
        .with_status(503)
        .with_header("content-type", "application/json")
        .with_header("retry-after", "5")
        .with_body(r#"{"error":"could not be stored or logged","code":"SERVICE_UNAVAILABLE"}"#)
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();

    let logged = client.delete_key("k-logged").await.unwrap();
    assert!(logged.success && logged.pending);
    let stored = client.delete_key("k-stored").await.unwrap();
    assert!(stored.success && !stored.pending);
    match client.deactivate_key("k-late").await {
        Err(ClientError::Server { status: 409, .. }) => {}
        other => panic!("expected a 409, got {other:?}"),
    }
    let err = client.delete_key("k-unlogged").await.unwrap_err();
    assert!(matches!(err, ClientError::ServiceUnavailable { .. }));
    assert_eq!(err.retry_after(), Some(std::time::Duration::from_secs(5)));
}

#[tokio::test]
async fn a_fenced_namespace_is_a_retryable_503() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("POST", "/v1/memory/store")
        .with_status(503)
        .with_header("content-type", "application/json")
        .with_header("retry-after", "5")
        .with_body(
            r#"{"error":"namespace is being torn down; retry","code":"SERVICE_UNAVAILABLE"}"#,
        )
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let err = client
        .store_memory(StoreMemoryRequest::new("a", "x"))
        .await
        .unwrap_err();
    assert!(err.is_retryable());
    assert_eq!(err.retry_after(), Some(std::time::Duration::from_secs(5)));
}
