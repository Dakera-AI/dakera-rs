//! Calls that used to go to routes the server does not serve (or send/parse
//! shapes it does not use), now matched against the v0.12.0 server source:
//! quotas, memory update, memory feedback, audit export / list, namespace
//! stats.

use dakera_client::memory::{AuditQuery, FeedbackRequest, UpdateMemoryRequest};
use dakera_client::{
    ClientError, DakeraClient, QuotaConfig, SetDefaultQuotaRequest, SetQuotaRequest,
};
use mockito::Matcher;
use serde_json::json;

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

// ---------------------------------------------------------------------------
// Quotas: PUT /v1/admin/quotas/{namespace} and /v1/admin/quotas/default
// ---------------------------------------------------------------------------

const QUOTA_STATUS: &str = r#"{"namespace":"ns","config":{"max_vectors":10,"max_dimensions":4,"enforcement":"hard"},
  "usage":{"vector_count":7,"storage_bytes":512,"avg_dimensions":4,"last_updated":1},
  "vector_usage_percent":70.0,"is_exceeded":false}"#;

#[tokio::test]
async fn set_quota_puts_the_server_config_shape() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "PUT",
        "/v1/admin/quotas/ns",
        200,
        r#"{"success":true,"namespace":"ns","config":{"max_vectors":10,"enforcement":"hard"},"message":"ok"}"#,
    )
    .match_body(Matcher::Json(
        json!({"config": {"max_vectors": 10, "enforcement": "hard"}}),
    ))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let cfg = QuotaConfig {
        max_vectors: Some(10),
        enforcement: Some("hard".into()),
        ..Default::default()
    };
    client.set_quota("ns", cfg.clone()).await.unwrap();
    m.assert_async().await;

    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "PUT",
        "/v1/admin/quotas/ns",
        200,
        r#"{"success":true,"namespace":"ns","config":{"max_vectors":10,"enforcement":"hard"},"message":"ok"}"#,
    )
    .match_body(Matcher::Json(
        json!({"config": {"max_vectors": 10, "enforcement": "hard"}}),
    ))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let resp = client
        .admin_set_quota("ns", SetQuotaRequest { config: cfg })
        .await
        .unwrap();
    assert!(resp.success);
    assert_eq!(resp.config.enforcement.as_deref(), Some("hard"));
    m.assert_async().await;
}

#[tokio::test]
async fn default_quota_is_put_to_quotas_default() {
    let mut server = mockito::Server::new_async().await;
    let put = json_mock(
        &mut server,
        "PUT",
        "/v1/admin/quotas/default",
        200,
        r#"{"success":true,"namespace":"_default","config":{"max_vectors":5},"message":"Default quota configuration updated"}"#,
    )
    .match_body(Matcher::Json(json!({"config": {"max_vectors": 5}})))
    .expect(2)
    .create_async()
    .await;
    let get = json_mock(
        &mut server,
        "GET",
        "/v1/admin/quotas/default",
        200,
        r#"{"config":{"max_vectors":5,"enforcement":"hard"}}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let cfg = QuotaConfig {
        max_vectors: Some(5),
        ..Default::default()
    };
    client.update_quotas(Some(cfg.clone())).await.unwrap();
    let resp = client
        .admin_set_default_quota(SetDefaultQuotaRequest { config: Some(cfg) })
        .await
        .unwrap();
    assert_eq!(resp.namespace, "_default");
    let def = client.admin_get_default_quota().await.unwrap();
    assert_eq!(def.config.unwrap().max_vectors, Some(5));
    put.assert_async().await;
    get.assert_async().await;
}

#[tokio::test]
async fn quota_status_and_list_parse_the_server_usage_fields() {
    let mut server = mockito::Server::new_async().await;
    let one = json_mock(&mut server, "GET", "/v1/admin/quotas/ns", 200, QUOTA_STATUS)
        .create_async()
        .await;
    let list = json_mock(
        &mut server,
        "GET",
        "/v1/admin/quotas",
        200,
        &format!(
            r#"{{"quotas":[{QUOTA_STATUS}],"total":1,"default_config":{{"max_vectors":100}}}}"#
        ),
    )
    .create_async()
    .await;
    let del = json_mock(
        &mut server,
        "DELETE",
        "/v1/admin/quotas/ns",
        200,
        r#"{"success":true}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let st = client.get_quota("ns").await.unwrap();
    // These were silently 0 when the SDK read current_vectors / current_storage_bytes.
    assert_eq!(st.usage.vector_count, 7);
    assert_eq!(st.usage.storage_bytes, 512);
    assert_eq!(st.vector_usage_percent, Some(70.0));
    let all = client.get_quotas().await.unwrap();
    assert_eq!(all.total, 1);
    assert_eq!(all.quotas[0].usage.vector_count, 7);
    assert_eq!(all.default_config.unwrap().max_vectors, Some(100));
    client.delete_quota("ns").await.unwrap();
    one.assert_async().await;
    list.assert_async().await;
    del.assert_async().await;
}

#[tokio::test]
async fn a_hard_quota_refusal_is_quota_exceeded() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/namespaces/ns/vectors",
        413,
        r#"{"error":"Quota exceeded","code":"QUOTA_EXCEEDED","details":"namespace: ns, reason: max_vectors"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let e = client
        .upsert(
            "ns",
            dakera_client::UpsertRequest {
                vectors: vec![dakera_client::Vector::new("v", vec![0.5])],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(e, ClientError::QuotaExceeded { .. }));
    m.assert_async().await;
}

// ---------------------------------------------------------------------------
// Memory update: PUT /v1/memory/update/{id}?agent_id=...  -> flat Memory
// ---------------------------------------------------------------------------

#[tokio::test]
async fn update_memory_uses_memory_update_with_agent_id_in_the_query() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "PUT",
        "/v1/memory/update/m1",
        200,
        r#"{"id":"m1","agent_id":"a","content":"new","memory_type":"episodic","importance":0.9}"#,
    )
    .match_query(Matcher::UrlEncoded("agent_id".into(), "a".into()))
    .match_body(Matcher::Json(
        json!({"content": "new", "importance": 0.9, "tags": ["x"]}),
    ))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .update_memory(
            "a",
            "m1",
            UpdateMemoryRequest {
                content: Some("new".into()),
                importance: Some(0.9),
                tags: Some(vec!["x".into()]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    // The server answers with the flat memory object, not {"memory": ...}.
    assert_eq!(r.memory_id, "m1");
    assert_eq!(r.agent_id, "a");
    m.assert_async().await;
}

// ---------------------------------------------------------------------------
// Feedback: POST /v1/memory/feedback {agent_id, memory_id, signal}
// ---------------------------------------------------------------------------

#[tokio::test]
async fn memory_feedback_posts_agent_memory_and_signal() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/memory/feedback",
        200,
        r#"{"memory_id":"m1","new_importance":0.575,"signal":"upvote"}"#,
    )
    .match_body(Matcher::Json(
        json!({"agent_id": "a", "memory_id": "m1", "signal": "upvote"}),
    ))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .memory_feedback(
            "a",
            FeedbackRequest {
                memory_id: "m1".into(),
                feedback: "upvote".into(),
                relevance_score: Some(0.9),
            },
        )
        .await
        .unwrap();
    assert_eq!(r.status, "ok");
    assert_eq!(r.updated_importance, Some(0.575));
    assert_eq!(r.memory_id.as_deref(), Some("m1"));
    assert_eq!(r.signal.as_deref(), Some("upvote"));
    m.assert_async().await;
}

// ---------------------------------------------------------------------------
// Audit: GET /v1/audit -> {events:[{id: <int>, ...}], count}; GET export
// ---------------------------------------------------------------------------

#[tokio::test]
async fn audit_list_parses_integer_ids_and_count() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/audit",
        200,
        r#"{"events":[{"id":42,"event_type":"store","agent_id":"a","memory_id":"m1","importance":0.5,"timestamp":1700000000000}],"count":1}"#,
    )
    .match_query(Matcher::Any)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .list_audit_events(AuditQuery::default())
        .await
        .unwrap();
    assert_eq!(r.total, 1);
    assert_eq!(r.events[0].id, "42");
    assert_eq!(r.events[0].memory_id.as_deref(), Some("m1"));
    assert_eq!(r.events[0].importance, Some(0.5));
    m.assert_async().await;
}

#[tokio::test]
async fn audit_export_is_a_get_with_query_parameters() {
    let mut server = mockito::Server::new_async().await;
    let csv = server
        .mock("GET", "/v1/audit/export")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("format".into(), "csv".into()),
            Matcher::UrlEncoded("agent_id".into(), "a".into()),
            Matcher::UrlEncoded("from".into(), "10".into()),
            Matcher::UrlEncoded("to".into(), "20".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "text/csv; charset=utf-8")
        .with_body("id,event_type,agent_id,memory_id,session_id,importance,timestamp\n1,store,a,m1,,0.5,15\n2,recall,a,m2,,,16\n")
        .create_async()
        .await;
    let json = json_mock(
        &mut server,
        "GET",
        "/v1/audit/export",
        200,
        r#"{"events":[],"count":0}"#,
    )
    .match_query(Matcher::UrlEncoded("format".into(), "json".into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .export_audit("csv", Some("a"), None, Some(10), Some(20))
        .await
        .unwrap();
    assert_eq!(r.format, "csv");
    assert_eq!(r.count, 2);
    assert!(r.data.starts_with("id,event_type"));
    let r = client
        .export_audit("json", None, None, None, None)
        .await
        .unwrap();
    assert_eq!(r.count, 0);
    assert!(r.data.contains("\"events\""));
    csv.assert_async().await;
    json.assert_async().await;
}

// ---------------------------------------------------------------------------
// Namespace stats: GET /v1/admin/indexes/stats (the server has no per-namespace route)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn namespace_stats_reads_the_admin_index_stats() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/admin/indexes/stats",
        200,
        r#"{"namespaces":{"a":{"indexed_vectors":3,"size_bytes":10},"b":{"indexed_vectors":4}},"total_indexed_vectors":7,"total_size_bytes":10}"#,
    )
    .expect(3)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let a = client.get_namespace_stats("a").await.unwrap();
    assert_eq!(a["indexed_vectors"], 3);
    let a = client.get_index_stats("a").await.unwrap();
    assert_eq!(a["size_bytes"], 10);
    let e = client.get_namespace_stats("zzz").await.unwrap_err();
    assert!(matches!(e, ClientError::NamespaceNotFound(_)));
    m.assert_async().await;
}
