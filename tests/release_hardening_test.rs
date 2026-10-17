//! Next-release support: recall neighbourhood modes and result provenance
//! (Dakera-AI/dakera#917), the admin routes that refuse unknown fields with
//! 422 (#920 F10), the super_admin shutdown (#920 F11), and client hardening
//! (secret redaction, retry wrapper, path escaping, event streams).
//!
//! Wire shapes follow the server source (`common/src/types/mod.rs`
//! `RecallRequest` / `RecallResult` / `RecallResponse` /
//! `SearchMemoriesResponse`, `routes/keys.rs` `CreateKeyRequest`,
//! `routes/namespace_keys.rs`, `routes/admin/types.rs` `CreateBackupRequest`,
//! `routes/admin/mod.rs` `RotateEncryptionKeyRequest`). Each response is also
//! decoded in its older form (fields absent) and with an unknown field.

use std::time::Duration;

use dakera_client::keys::{CreateKeyRequest, CreateNamespaceKeyRequest};
use dakera_client::{
    AdminCreateBackupRequest, ClientError, CreateBackupRequest, DakeraClient, NeighborhoodMode,
    RecallRequest, RecallResponse, ResealEncryptionRequest, RetryConfig,
    RotateEncryptionKeyRequest, RETRIEVAL_NEIGHBORHOOD,
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

fn keys_of(v: &Value) -> Vec<String> {
    let mut keys: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    keys
}

// ============================================================================
// Recall request
// ============================================================================

#[test]
fn recall_request_omits_new_fields_when_unset() {
    let v = serde_json::to_value(RecallRequest::new("a1", "q")).unwrap();
    for key in [
        "neighborhood_mode",
        "include_summaries",
        "rerank_candidates",
        "importance_weighted",
    ] {
        assert!(
            v.get(key).is_none(),
            "{key} must be omitted when unset: {v}"
        );
    }
}

#[test]
fn neighborhood_mode_wire_names() {
    for (mode, wire) in [
        (NeighborhoodMode::Inline, "inline"),
        (NeighborhoodMode::Separate, "separate"),
        (NeighborhoodMode::Off, "off"),
    ] {
        assert_eq!(serde_json::to_value(mode).unwrap(), json!(wire));
        assert_eq!(
            serde_json::from_value::<NeighborhoodMode>(json!(wire)).unwrap(),
            mode
        );
    }
    // A mode a newer server knows is not a decoding error.
    assert_eq!(
        serde_json::from_value::<NeighborhoodMode>(json!("future")).unwrap(),
        NeighborhoodMode::Unknown
    );
}

#[tokio::test]
async fn recall_sends_neighborhood_mode_and_include_summaries() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/memory/recall",
        200,
        r#"{"memories":[]}"#,
    )
    .match_body(Matcher::PartialJson(json!({
        "agent_id": "a1",
        "query": "q",
        "top_k": 3,
        "neighborhood_mode": "separate",
        "include_summaries": false,
        "rerank_candidates": 50
    })))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client
        .recall(
            RecallRequest::new("a1", "q")
                .with_top_k(3)
                .with_neighborhood_mode(NeighborhoodMode::Separate)
                .with_include_summaries(false)
                .with_rerank_candidates(50),
        )
        .await
        .unwrap();
    m.assert_async().await;
}

// ============================================================================
// Recall / search responses
// ============================================================================

const SEPARATE_RESPONSE: &str = r#"{
  "memories": [
    {"memory": {"id": "m1", "content": "Alice moved to Paris in 2021. She likes tea.",
                "agent_id": "a1", "session_id": "s1", "memory_type": "episodic",
                "importance": 0.7, "tags": [], "created_at": 1, "last_accessed_at": 1},
     "score": 0.9, "smart_score": 0.8,
     "matched_text": "Alice moved to Paris in 2021.",
     "matched_ids": ["m1-s0", "m1-s1"]}
  ],
  "context_memories": [
    {"memory": {"id": "m2", "content": "ctx", "agent_id": "a1", "session_id": "s1",
                "created_at": 2, "last_accessed_at": 2},
     "score": 0.31, "retrieval": "neighborhood", "context_of": "m1"}
  ],
  "associated_memories": [
    {"memory": {"id": "m3", "content": "linked", "agent_id": "a1",
                "created_at": 3, "last_accessed_at": 3},
     "score": 0.5, "depth": 2}
  ],
  "query_embedding_time_ms": 4,
  "search_time_ms": 12,
  "rerank": {"applied": true, "candidates": 100},
  "rerank_report": {"applied": true, "candidates": 100},
  "future_field": {"x": 1}
}"#;

#[test]
fn recall_response_separate_mode() {
    let r: RecallResponse = serde_json::from_str(SEPARATE_RESPONSE).unwrap();
    assert_eq!(r.memories.len(), 1);
    let m = &r.memories[0];
    assert_eq!(m.id, "m1");
    assert_eq!(m.session_id.as_deref(), Some("s1"));
    assert_eq!(
        m.matched_text.as_deref(),
        Some("Alice moved to Paris in 2021.")
    );
    assert_eq!(
        m.matched_ids.as_deref(),
        Some(&["m1-s0".to_string(), "m1-s1".to_string()][..])
    );
    assert!(m.retrieval.is_none());
    assert!(!m.is_neighborhood());

    let ctx = r.context_memories.as_ref().expect("context_memories");
    assert_eq!(ctx.len(), 1);
    assert_eq!(ctx[0].id, "m2");
    assert_eq!(ctx[0].context_of.as_deref(), Some("m1"));
    assert_eq!(ctx[0].retrieval.as_deref(), Some(RETRIEVAL_NEIGHBORHOOD));
    assert!(ctx[0].is_neighborhood());

    // `depth` is a field of the result, next to the scores.
    let assoc = r.associated_memories.as_ref().unwrap();
    assert_eq!(assoc[0].depth, Some(2));

    assert_eq!(r.query_embedding_time_ms, Some(4));
    assert_eq!(r.search_time_ms, Some(12));
    let report = r.rerank_report.as_ref().unwrap();
    assert!(report.applied);
    assert_eq!(report.candidates, 100);
}

#[test]
fn recall_response_inline_neighborhood_item() {
    let r: RecallResponse = serde_json::from_str(
        r#"{"memories": [
            {"memory": {"id": "m1", "content": "a"}, "score": 0.9},
            {"memory": {"id": "m2", "content": "b"}, "score": 0.2, "retrieval": "neighborhood"}
        ]}"#,
    )
    .unwrap();
    assert!(!r.memories[0].is_neighborhood());
    assert!(r.memories[1].is_neighborhood());
    assert!(r.context_memories.is_none());
}

#[test]
fn recall_response_from_an_older_server() {
    // v0.12.2 shape: no matched_*, retrieval, context_*, rerank_report.
    let r: RecallResponse = serde_json::from_str(
        r#"{"memories": [{"memory": {"id": "m1", "content": "a"}, "score": 0.9,
                         "vector_score": 0.5}],
            "query_embedding_time_ms": 1, "search_time_ms": 2,
            "rerank": {"applied": false, "candidates": 0, "skipped": "bm25_routing"}}"#,
    )
    .unwrap();
    let m = &r.memories[0];
    assert!(m.matched_text.is_none());
    assert!(m.matched_ids.is_none());
    assert!(m.retrieval.is_none());
    assert!(m.context_of.is_none());
    assert_eq!(m.vector_score, Some(0.5));
    assert!(r.context_memories.is_none());
    assert!(r.rerank_report.is_none());
    // And the oldest shape: memories only.
    let r: RecallResponse = serde_json::from_str(r#"{"memories": []}"#).unwrap();
    assert!(r.memories.is_empty());
}

#[tokio::test]
async fn search_results_carry_matched_fields() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/memory/search",
        200,
        r#"{"memories": [{"memory": {"id": "m1", "content": "src"}, "score": 0.9,
                          "matched_text": "best sentence", "matched_ids": ["m1-s2"]}],
            "total_count": 7,
            "rerank_report": {"applied": false, "candidates": 0, "skipped": "disabled"},
            "effective_top_k": 5,
            "another_future": true}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .search_memories(RecallRequest::new("a1", "q"))
        .await
        .unwrap();
    assert_eq!(r.total_found, 7);
    assert_eq!(r.effective_top_k, Some(5));
    assert_eq!(
        r.rerank_report.as_ref().unwrap().skipped.as_deref(),
        Some("disabled")
    );
    assert_eq!(r.memories[0].matched_text.as_deref(), Some("best sentence"));
    assert_eq!(
        r.memories[0].matched_ids.as_deref(),
        Some(&["m1-s2".to_string()][..])
    );
    m.assert_async().await;
}

// ============================================================================
// Admin requests that refuse unknown fields (422 on the server)
// ============================================================================

#[tokio::test]
async fn create_key_sends_only_server_fields() {
    let req = CreateKeyRequest {
        name: "k".into(),
        scope: "read".into(),
        namespaces: None,
        expires_in_days: None,
    };
    assert_eq!(
        serde_json::to_value(&req).unwrap(),
        json!({"name": "k", "scope": "read"})
    );

    let full = CreateKeyRequest {
        name: "k".into(),
        scope: "read".into(),
        namespaces: Some(vec!["a".into(), "b*".into()]),
        expires_in_days: Some(30),
    };
    assert_eq!(
        keys_of(&serde_json::to_value(&full).unwrap()),
        ["expires_in_days", "name", "namespaces", "scope"]
    );

    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/admin/keys",
        201,
        r#"{"key_id":"dk_key_1","key":"dk_live_x","name":"k","scope":"read",
            "namespaces":null,"created_at":1,"expires_at":null,"warning":"save it"}"#,
    )
    .match_body(Matcher::Json(json!({"name": "k", "scope": "read"})))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client.create_key(req).await.unwrap();
    m.assert_async().await;
}

#[tokio::test]
async fn create_namespace_key_sends_only_server_fields() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/namespaces/ns-1/keys",
        201,
        r#"{"key_id":"k1","key":"dk_x","name":"n","namespace":"ns-1","created_at":1,
            "warning":"w","scope":"read","namespaces":["ns-1"]}"#,
    )
    .match_body(Matcher::Json(json!({"name": "n", "scope": "read"})))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client
        .create_namespace_key("ns-1", CreateNamespaceKeyRequest::new("n", "read"))
        .await
        .unwrap();
    m.assert_async().await;

    let mut full = CreateNamespaceKeyRequest::new("n", "write");
    full.extra_namespaces = Some(vec!["ns-2".into()]);
    full.expires_in_days = Some(7);
    assert_eq!(
        keys_of(&serde_json::to_value(&full).unwrap()),
        ["expires_in_days", "extra_namespaces", "name", "scope"]
    );
}

#[tokio::test]
async fn create_backup_requests_send_only_server_fields() {
    let legacy = CreateBackupRequest {
        name: "n".into(),
        backup_type: None,
        namespaces: None,
        encrypt: None,
        compression: None,
    };
    assert_eq!(serde_json::to_value(&legacy).unwrap(), json!({"name": "n"}));

    let full = AdminCreateBackupRequest {
        name: "n".into(),
        backup_type: Some("snapshot".into()),
        namespaces: Some(vec!["a".into()]),
        encrypt: Some(true),
        compression: Some("zstd".into()),
    };
    assert_eq!(
        keys_of(&serde_json::to_value(&full).unwrap()),
        [
            "backup_type",
            "compression",
            "encrypt",
            "name",
            "namespaces"
        ]
    );

    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/admin/backups",
        200,
        r#"{"backup":{"backup_id":"bk-1","name":"n","backup_type":"full","status":"completed",
            "namespaces":[],"vector_count":0,"size_bytes":0,"created_at":10,"encrypted":false}}"#,
    )
    .match_body(Matcher::Json(json!({"name": "n"})))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .admin_create_backup(AdminCreateBackupRequest {
            name: "n".into(),
            backup_type: None,
            namespaces: None,
            encrypt: None,
            compression: None,
        })
        .await
        .unwrap();
    assert_eq!(r.backup.backup_id, "bk-1");
    m.assert_async().await;
}

#[tokio::test]
async fn rotate_encryption_key_sends_only_set_fields() {
    let mut server = mockito::Server::new_async().await;
    let answer = r#"{"key_id":"k2","previous_key_id":"k1","scope":"global","reseal":"completed",
        "rotated":3,"skipped":0,"namespaces":["a"],"fulltext_indices_rotated":1,
        "status_url":"/admin/encryption/status"}"#;
    let empty = json_mock(
        &mut server,
        "POST",
        "/v1/admin/encryption/rotate-key",
        200,
        answer,
    )
    .match_body(Matcher::Json(json!({})))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.rotate_encryption_key("", None).await.unwrap();
    empty.assert_async().await;
    assert_eq!(r.key_id.as_deref(), Some("k2"));
    assert_eq!(r.previous_key_id.as_deref(), Some("k1"));
    assert_eq!(r.reseal.as_deref(), Some("completed"));
    assert_eq!(r.rotated, 3);
    assert_eq!(r.fulltext_indices_rotated, Some(1));

    let full = json_mock(
        &mut server,
        "POST",
        "/v1/admin/encryption/rotate-key",
        200,
        answer,
    )
    .match_body(Matcher::Json(json!({
        "new_key": "correct horse battery staple",
        "namespace": "ns",
        "wait_secs": 0
    })))
    .create_async()
    .await;
    client
        .rotate_encryption_key_with(RotateEncryptionKeyRequest {
            new_key: "correct horse battery staple".into(),
            namespace: Some("ns".into()),
            wait_secs: Some(0),
        })
        .await
        .unwrap();
    full.assert_async().await;

    // An older server's answer still decodes.
    let old: dakera_client::RotateEncryptionKeyResponse =
        serde_json::from_str(r#"{"rotated":1,"skipped":2,"namespaces":[]}"#).unwrap();
    assert_eq!(old.skipped, 2);
    assert!(old.key_id.is_none());
}

#[test]
fn rotate_encryption_key_request_debug_redacts_the_key() {
    let req = RotateEncryptionKeyRequest {
        new_key: "s3cr3t-passphrase".into(),
        namespace: Some("ns".into()),
        wait_secs: None,
    };
    let out = format!("{req:?} {req:#?}");
    assert!(!out.contains("s3cr3t"), "{out}");
    assert!(out.contains("<redacted>"), "{out}");
    assert!(out.contains("ns"), "{out}");
}

#[tokio::test]
async fn encryption_admin_routes() {
    let mut server = mockito::Server::new_async().await;
    let status = json_mock(
        &mut server,
        "GET",
        "/v1/admin/encryption/status",
        200,
        r#"{"enabled":true}"#,
    )
    .create_async()
    .await;
    let reseal = json_mock(
        &mut server,
        "POST",
        "/v1/admin/encryption/reseal",
        202,
        r#"{"reseal":"running","status_url":"/admin/encryption/status"}"#,
    )
    .match_body(Matcher::Json(json!({"namespace": "ns"})))
    .create_async()
    .await;
    let revoke = json_mock(
        &mut server,
        "POST",
        "/v1/admin/encryption/keys/key%201/revoke-master",
        200,
        r#"{"key_id":"key 1"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client.encryption_status().await.unwrap();
    client
        .reseal_encryption(ResealEncryptionRequest {
            namespace: Some("ns".into()),
            wait_secs: None,
        })
        .await
        .unwrap();
    client.revoke_encryption_master("key 1").await.unwrap();
    status.assert_async().await;
    reseal.assert_async().await;
    revoke.assert_async().await;
}

#[tokio::test]
async fn ops_shutdown_with_an_admin_key_is_a_403() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/ops/shutdown",
        403,
        r#"{"error":"requires super_admin scope","code":"INSUFFICIENT_SCOPE"}"#,
    )
    .expect(1)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let err = client.ops_shutdown().await.unwrap_err();
    assert!(
        matches!(err, ClientError::Authorization { status: 403, .. }),
        "{err:?}"
    );
    assert!(!err.is_retryable());
    m.assert_async().await;
}

// ============================================================================
// Secret redaction
// ============================================================================

#[test]
fn builder_and_client_debug_never_print_the_api_key() {
    const SECRET: &str = "dk_live_TOPSECRET";
    let builder = DakeraClient::builder("http://localhost:3000")
        .api_key(SECRET)
        .header("X-Other", "hdr-secret");
    let out = format!("{builder:?} {builder:#?}");
    assert!(!out.contains(SECRET), "{out}");
    assert!(!out.contains("hdr-secret"), "{out}");

    let client = builder.build().unwrap();
    let out = format!("{client:?} {client:#?}");
    assert!(!out.contains(SECRET), "{out}");
    assert!(!out.contains("hdr-secret"), "{out}");
}

#[tokio::test]
async fn errors_do_not_carry_the_api_key() {
    const SECRET: &str = "dk_live_TOPSECRET";
    let mut server = mockito::Server::new_async().await;
    let _m = json_mock(
        &mut server,
        "POST",
        "/v1/memory/recall",
        401,
        r#"{"error":"invalid api key","code":"INVALID_API_KEY"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::builder(server.url())
        .api_key(SECRET)
        .build()
        .unwrap();
    let err = client
        .recall(RecallRequest::new("a", "q"))
        .await
        .unwrap_err();
    assert!(!format!("{err} {err:?}").contains(SECRET));
}

// ============================================================================
// Retry wrapper
// ============================================================================

#[tokio::test]
async fn execute_with_retry_with_zero_retries_still_runs_once() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(&mut server, "GET", "/health", 200, r#"{"healthy":true}"#)
        .expect(1)
        .create_async()
        .await;
    let client = DakeraClient::builder(server.url())
        .retry_config(RetryConfig {
            max_retries: 0,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
            jitter: false,
        })
        .build()
        .unwrap();
    client.execute_with_retry(|| client.health()).await.unwrap();
    m.assert_async().await;
}

#[tokio::test]
async fn an_undecodable_answer_is_not_retryable() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(&mut server, "POST", "/v1/memory/recall", 200, "not json")
        .expect(1)
        .create_async()
        .await;
    let client = DakeraClient::builder(server.url())
        .retry_config(RetryConfig {
            max_retries: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
            jitter: false,
        })
        .build()
        .unwrap();
    let err = client
        .execute_with_retry(|| client.recall(RecallRequest::new("a", "q")))
        .await
        .unwrap_err();
    assert!(!err.is_retryable(), "{err:?}");
    m.assert_async().await;
}

// ============================================================================
// Path escaping
// ============================================================================

#[tokio::test]
async fn path_and_query_parameters_are_escaped() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/memory/get/..%2F..%2Fadmin%2Fkeys%3Fy",
        200,
        r#"{"id":"x","content":"c"}"#,
    )
    .match_query(Matcher::UrlEncoded("agent_id".into(), "agent&x=1".into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client
        .get_memory("agent&x=1", "../../admin/keys?y")
        .await
        .unwrap();
    m.assert_async().await;
}

#[tokio::test]
async fn attachment_references_keep_their_colon() {
    let mut server = mockito::Server::new_async().await;
    let m = server
        .mock("DELETE", "/v1/namespaces/ns/attachments/sha256:abcd")
        .with_status(204)
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client.delete_attachment("ns", "sha256:abcd").await.unwrap();
    m.assert_async().await;
}

// ============================================================================
// Event streams
// ============================================================================

#[tokio::test]
async fn event_stream_outlives_the_request_timeout() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/v1/namespaces/n/events")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_chunked_body(|w| {
            w.write_all(b": heartbeat\n\n")?;
            w.flush()?;
            // Longer than the client's request timeout below.
            std::thread::sleep(Duration::from_millis(600));
            w.write_all(b"data: {\"type\":\"namespace_deleted\",\"namespace\":\"n\"}\n\n")
        })
        .create_async()
        .await;
    let client = DakeraClient::builder(server.url())
        .timeout(Duration::from_millis(200))
        .build()
        .unwrap();
    let mut rx = client.stream_namespace_events("n").await.unwrap();
    let first = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("an event")
        .expect("stream open");
    match first.unwrap() {
        dakera_client::DakeraEvent::NamespaceDeleted { namespace } => assert_eq!(namespace, "n"),
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn event_stream_errors_are_typed() {
    let mut server = mockito::Server::new_async().await;
    let _m = json_mock(
        &mut server,
        "GET",
        "/ops/events",
        403,
        r#"{"error":"admin required","code":"INSUFFICIENT_SCOPE"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let err = client.stream_global_events().await.unwrap_err();
    assert!(
        matches!(err, ClientError::Authorization { status: 403, .. }),
        "{err:?}"
    );
}

#[tokio::test]
async fn audit_stream_yields_memory_events() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/v1/audit/stream")
        .match_query(Matcher::UrlEncoded("agent_id".into(), "a 1".into()))
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(
            "event: connected\r\ndata: {\"type\":\"connected\",\"timestamp\":1}\r\n\r\n\
             event: stored\r\ndata: {\"event_type\":\"stored\",\"agent_id\":\"a 1\",\"timestamp\":2,\
             \"memory_id\":\"m1\",\"content\":\"caf\u{e9} \u{1F600}\"}\r\n\r\n",
        )
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let mut rx = client
        .stream_audit_memory_events(Some("a 1"), None)
        .await
        .unwrap();
    let connected = rx.recv().await.unwrap().unwrap();
    assert_eq!(connected.event_type, "connected");
    let stored = rx.recv().await.unwrap().unwrap();
    assert_eq!(stored.event_type, "stored");
    assert_eq!(stored.memory_id.as_deref(), Some("m1"));
    assert_eq!(stored.content.as_deref(), Some("caf\u{e9} \u{1F600}"));
}
