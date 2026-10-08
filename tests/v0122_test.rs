//! Dakera server v0.12.2 support: agents, key edits, rotation grace, whoami,
//! namespace kinds, capabilities v2, session lifecycle, listing previews and
//! `include_derived`, derived-data admin routes, dedup / compress counts and
//! the node-wide `unavailable` lists.
//!
//! Every wire shape here is taken from the server source (`routes/keys.rs`,
//! `routes/agents.rs`, `routes/sessions.rs`, `routes/knowledge.rs`,
//! `derivation/routes.rs`, `common/src/types/{mod,memory}.rs`) and the v0.12.2
//! API contract. Each response is also checked in its pre-v0.12.2 form
//! (fields absent), so the SDK keeps working against v0.12.0 / v0.12.1.

use dakera_client::memory::{
    BatchStoreMemoryItem, BatchStoreMemoryRequest, SessionMemoriesOptions, SessionStartRequest,
    StoreMemoryRequest,
};
use dakera_client::{
    AgentMemoriesOptions, ClientError, CrossAgentNetworkRequest, DakeraClient, DeduplicateRequest,
    FullKnowledgeGraphRequest, KeyInfo, NamespaceKind, ServerCapabilities, UpdateKeyRequest,
    WakeUpOptions,
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

const SESSION_V0122: &str = r#"{"id":"sess_1","agent_id":"mlx-dev","started_at":1791392203,
    "ended_at":1791406603,"memory_count":7,"last_activity_at":1791392803,
    "ended_reason":"idle","idle_since":1791392803,"idle_timeout_secs":7200}"#;

// ============================================================================
// POST /v1/agents
// ============================================================================

#[tokio::test]
async fn create_agent_201_created() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/agents",
        201,
        r#"{"agent_id":"mlx-dev","namespace":"_dakera_agent_mlx-dev","created":true,"dimension":1024,"model":"bge-large"}"#,
    )
    .match_body(Matcher::JsonString(r#"{"agent_id":"mlx-dev"}"#.into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.create_agent("mlx-dev").await.unwrap();
    assert!(r.created);
    assert_eq!(r.namespace, "_dakera_agent_mlx-dev");
    assert_eq!(r.dimension, Some(1024));
    assert_eq!(r.model.as_deref(), Some("bge-large"));
    m.assert_async().await;
}

#[tokio::test]
async fn create_agent_200_existing_with_null_dimension() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/agents",
        200,
        r#"{"agent_id":"mlx-dev","namespace":"_dakera_agent_mlx-dev","created":false,"dimension":null,"model":"bge-large"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.create_agent("mlx-dev").await.unwrap();
    assert!(!r.created);
    assert_eq!(r.dimension, None);
    m.assert_async().await;
}

#[tokio::test]
async fn list_agents_reads_vector_count_and_unavailable() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/agents",
        200,
        r#"[{"agent_id":"a","memory_count":0,"vector_count":0,"session_count":0,"active_sessions":0,"unavailable":"did not answer within 2000 ms"},
            {"agent_id":"b","memory_count":3,"session_count":1,"active_sessions":0}]"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let agents = client.list_agents().await.unwrap();
    assert_eq!(
        agents[0].unavailable.as_deref(),
        Some("did not answer within 2000 ms")
    );
    assert_eq!(agents[1].vector_count, 0);
    assert!(agents[1].unavailable.is_none());
    m.assert_async().await;
}

// ============================================================================
// Keys: PATCH, rotation grace, whoami, KeyInfo
// ============================================================================

const KEY_INFO_V0122: &str = r#"{"key_id":"dk_key_1a2b3c4d","name":"dev","scope":"write",
    "namespaces":["_dakera_agent_mlx-*"],"created_at":1791392203,"expires_at":null,
    "active":true,"grants_version":1,"inert_namespaces":["foo*","_dakera_sessions"]}"#;

#[test]
fn key_info_reads_v0122_and_v0121_shapes() {
    let k: KeyInfo = serde_json::from_str(KEY_INFO_V0122).unwrap();
    assert_eq!(k.grants_version, Some(1));
    assert_eq!(k.inert_namespaces, vec!["foo*", "_dakera_sessions"]);
    let old: KeyInfo = serde_json::from_str(
        r#"{"key_id":"k","name":"n","scope":"read","created_at":1,"active":true}"#,
    )
    .unwrap();
    assert_eq!(old.grants_version, None);
    assert!(old.inert_namespaces.is_empty());
    // Not echoed back when empty.
    assert!(!serde_json::to_string(&old)
        .unwrap()
        .contains("inert_namespaces"));
}

#[test]
fn update_key_request_expresses_absent_null_and_list() {
    let absent = UpdateKeyRequest::new().with_name("renamed");
    assert_eq!(
        serde_json::to_value(&absent).unwrap(),
        json!({"name": "renamed"})
    );
    let null = UpdateKeyRequest::new().with_all_namespaces();
    assert_eq!(
        serde_json::to_value(&null).unwrap(),
        json!({"namespaces": null})
    );
    let empty = UpdateKeyRequest::new().with_namespaces(vec![]);
    assert_eq!(
        serde_json::to_value(&empty).unwrap(),
        json!({"namespaces": []})
    );
    let both = UpdateKeyRequest::new()
        .with_name("r")
        .with_namespaces(vec!["team-*".into(), "docs".into()]);
    assert_eq!(
        serde_json::to_value(&both).unwrap(),
        json!({"name": "r", "namespaces": ["team-*", "docs"]})
    );
    // Round trip keeps the three cases apart.
    let back: UpdateKeyRequest = serde_json::from_str(r#"{"namespaces":null}"#).unwrap();
    assert_eq!(back.namespaces, Some(None));
    let back: UpdateKeyRequest = serde_json::from_str(r#"{"name":"x"}"#).unwrap();
    assert_eq!(back.namespaces, None);
}

#[tokio::test]
async fn update_key_patches_admin_route() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "PATCH",
        "/admin/keys/dk_key_1a2b3c4d",
        200,
        KEY_INFO_V0122,
    )
    .match_body(Matcher::JsonString(r#"{"namespaces":null}"#.into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let k = client
        .update_key(
            "dk_key_1a2b3c4d",
            &UpdateKeyRequest::new().with_all_namespaces(),
        )
        .await
        .unwrap();
    assert_eq!(k.grants_version, Some(1));
    m.assert_async().await;
}

#[tokio::test]
async fn update_namespace_key_patches_namespace_route() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "PATCH",
        "/v1/namespaces/team-a/keys/dk_key_1",
        200,
        KEY_INFO_V0122,
    )
    .match_body(Matcher::JsonString(
        r#"{"name":"ci","namespaces":["team-a*"]}"#.into(),
    ))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client
        .update_namespace_key(
            "team-a",
            "dk_key_1",
            &UpdateKeyRequest::new()
                .with_name("ci")
                .with_namespaces(vec!["team-a*".into()]),
        )
        .await
        .unwrap();
    m.assert_async().await;
}

#[tokio::test]
async fn empty_key_update_is_refused_before_sending() {
    let client = DakeraClient::new("http://127.0.0.1:1").unwrap();
    let err = client
        .update_key("dk_key_1", &UpdateKeyRequest::new())
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::InvalidRequest(_)));
    let err = client
        .update_namespace_key("ns", "dk_key_1", &UpdateKeyRequest::default())
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::InvalidRequest(_)));
}

#[tokio::test]
async fn rotate_with_grace_sends_grace_and_reads_old_key() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/admin/keys/dk_key_old/rotate",
        200,
        r#"{"new_key":"dk_new","key_id":"dk_key_new","old_key_id":"dk_key_old","old_key_expires_at":1791395803,"warning":"Save this new key now!"}"#,
    )
    .match_body(Matcher::JsonString(r#"{"grace_secs":3600}"#.into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .rotate_key_with_grace("dk_key_old", 3600)
        .await
        .unwrap();
    assert_eq!(r.key_id, "dk_key_new");
    assert_eq!(r.old_key_id.as_deref(), Some("dk_key_old"));
    assert_eq!(r.old_key_expires_at, Some(1791395803));
    m.assert_async().await;
}

#[tokio::test]
async fn rotate_without_grace_sends_no_body_and_reads_v0121_answer() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/admin/keys/dk_key_old/rotate",
        200,
        r#"{"new_key":"dk_new","key_id":"dk_key_new","warning":"w"}"#,
    )
    .match_body(Matcher::Exact(String::new()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.rotate_key("dk_key_old").await.unwrap();
    assert!(r.old_key_id.is_none());
    assert!(r.old_key_expires_at.is_none());
    m.assert_async().await;
}

#[tokio::test]
async fn rotate_grace_over_seven_days_is_refused() {
    let client = DakeraClient::new("http://127.0.0.1:1").unwrap();
    let err = client
        .rotate_key_with_grace("k", 604_801)
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::InvalidRequest(_)));
}

#[tokio::test]
async fn whoami_reads_key_and_auth_disabled_identity() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/auth/whoami",
        200,
        r#"{"key_id":"dk_key_1a2b3c4d","name":"dev","scope":"write","namespaces":["foo*","docs"],
            "unrestricted":false,"expires_at":null,"grants_version":0,"inert_namespaces":["foo*"],"auth_enabled":true}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let w = client.whoami().await.unwrap();
    assert_eq!(w.scope, "write");
    assert_eq!(w.grants_version, 0);
    assert_eq!(w.inert_namespaces, vec!["foo*"]);
    assert!(!w.unrestricted);
    assert!(w.auth_enabled);
    m.assert_async().await;

    let off: dakera_client::WhoamiResponse = serde_json::from_str(
        r#"{"key_id":"auth-disabled","name":"Auth Disabled","scope":"super_admin","namespaces":null,"unrestricted":true,"expires_at":null,"grants_version":1,"inert_namespaces":[],"auth_enabled":false}"#,
    )
    .unwrap();
    assert!(off.namespaces.is_none());
    assert!(!off.auth_enabled);
}

#[tokio::test]
async fn namespace_key_list_reads_server_key_info_shape() {
    // The server answers the namespace-key listing with its KeyInfo shape
    // (no `namespace` echo).
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/namespaces/team-a/keys",
        200,
        &format!(r#"{{"keys":[{KEY_INFO_V0122}],"total":1}}"#),
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.list_namespace_keys("team-a").await.unwrap();
    assert_eq!(r.total, 1);
    assert_eq!(r.keys[0].scope, "write");
    assert_eq!(r.keys[0].grants_version, Some(1));
    m.assert_async().await;
}

// ============================================================================
// Namespace kinds, capabilities v2
// ============================================================================

#[tokio::test]
async fn namespace_kinds_are_read() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/namespaces",
        200,
        r#"{"namespaces":["docs","_dakera_agent_a"],"kinds":{"docs":"data","_dakera_agent_a":"agent"}}"#,
    )
    .expect(2)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.list_namespaces_with_kinds().await.unwrap();
    assert_eq!(r.kinds["docs"], NamespaceKind::Data);
    assert_eq!(r.kinds["_dakera_agent_a"], NamespaceKind::Agent);
    assert_eq!(client.list_namespaces().await.unwrap().len(), 2);
    m.assert_async().await;

    let info: dakera_client::NamespaceInfo =
        serde_json::from_str(r#"{"name":"x","vector_count":1,"kind":"quantum"}"#).unwrap();
    assert_eq!(info.kind, Some(NamespaceKind::Unknown));
}

#[test]
fn capabilities_v2_blocks_and_v1_document() {
    let caps: ServerCapabilities = serde_json::from_value(json!({
        "capabilities_version": 2,
        "auth": {"prefix_patterns": true, "sessions_by_agent": true, "key_update": true,
                 "rotation_grace_max_secs": 604800, "max_grants": 100, "max_grant_len": 255},
        "naming": {"agent_id_pattern": "^[a-zA-Z0-9][a-zA-Z0-9_\\-.]*$", "agent_id_max_bytes": 241,
                   "agent_namespace_prefix": "_dakera_agent_", "agent_namespace_max_bytes": 255,
                   "namespace_pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]*$", "namespace_max_bytes": 128,
                   "reserved_prefixes": ["_"], "internal_namespaces": ["_dakera_sessions"],
                   "internal_prefixes": ["_dakera_reembed_staging_"], "future": 1},
        "sessions": {"idle_timeout_secs": 14400, "max_idle_timeout_secs": 2592000,
                     "touch": true, "ended_reason": true}
    }))
    .unwrap();
    assert_eq!(caps.capabilities_version, 2);
    assert!(caps.supports_key_update());
    assert!(caps.supports_session_touch());
    assert_eq!(caps.auth.rotation_grace_max_secs, 604_800);
    assert_eq!(caps.naming.agent_id_max_bytes, 241);
    assert!(caps.naming.extra.contains_key("future"));
    assert_eq!(caps.sessions.idle_timeout_secs, 14_400);
    assert!(!caps.extra.contains_key("auth"));

    let v1: ServerCapabilities = serde_json::from_str(r#"{"capabilities_version":1}"#).unwrap();
    assert!(!v1.supports_key_update());
    assert!(!v1.supports_session_touch());
}

// ============================================================================
// Sessions
// ============================================================================

#[tokio::test]
async fn start_session_with_idle_timeout() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/sessions/start",
        200,
        r#"{"session":{"id":"sess_1","agent_id":"mlx-dev","started_at":10,"memory_count":0,"last_activity_at":10,"idle_timeout_secs":0}}"#,
    )
    .match_body(Matcher::JsonString(
        r#"{"agent_id":"mlx-dev","idle_timeout_secs":0}"#.into(),
    ))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let s = client
        .start_session_with(&SessionStartRequest::new("mlx-dev").with_idle_timeout_secs(0))
        .await
        .unwrap();
    assert_eq!(s.idle_timeout_secs, Some(0));
    assert_eq!(s.last_activity_at, Some(10));
    assert!(!s.is_ended());
    m.assert_async().await;
}

#[tokio::test]
async fn start_session_without_options_sends_no_new_fields() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/sessions/start",
        200,
        r#"{"session":{"id":"s","agent_id":"a","started_at":1}}"#,
    )
    .match_body(Matcher::JsonString(r#"{"agent_id":"a"}"#.into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let s = client.start_session("a").await.unwrap();
    // A v0.12.1 session has none of the lifecycle fields.
    assert!(s.last_activity_at.is_none());
    assert!(s.ended_reason.is_none());
    m.assert_async().await;
}

#[tokio::test]
async fn start_session_timeout_over_30_days_is_refused() {
    let client = DakeraClient::new("http://127.0.0.1:1").unwrap();
    let err = client
        .start_session_with(&SessionStartRequest::new("a").with_idle_timeout_secs(2_592_001))
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::InvalidRequest(_)));
}

#[tokio::test]
async fn touch_session_active_and_ended() {
    let mut server = mockito::Server::new_async().await;
    let active = json_mock(
        &mut server,
        "POST",
        "/v1/sessions/sess_a/touch",
        200,
        r#"{"session":{"id":"sess_a","agent_id":"a","started_at":1,"memory_count":0,"last_activity_at":1791393000},"session_state":"active","idle_deadline_at":1791407400}"#,
    )
    .create_async()
    .await;
    let ended = json_mock(
        &mut server,
        "POST",
        "/v1/sessions/sess_1/touch",
        200,
        &format!(r#"{{"session":{SESSION_V0122},"session_state":"ended"}}"#),
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let t = client.touch_session("sess_a").await.unwrap();
    assert_eq!(t.session_state, "active");
    assert_eq!(t.idle_deadline_at, Some(1791407400));
    let t = client.touch_session("sess_1").await.unwrap();
    assert_eq!(t.session_state, "ended");
    assert!(t.idle_deadline_at.is_none());
    assert_eq!(t.session.ended_reason.as_deref(), Some("idle"));
    assert_eq!(t.session.idle_since, Some(1791392803));
    assert!(t.session.is_ended());
    active.assert_async().await;
    ended.assert_async().await;
}

#[tokio::test]
async fn store_reports_session_state_and_batch_ended_sessions() {
    let mut server = mockito::Server::new_async().await;
    let store = json_mock(
        &mut server,
        "POST",
        "/v1/memory/store",
        200,
        r#"{"memory":{"id":"m1","agent_id":"a","content":"x"},"embedding_time_ms":12,"session_state":"ended"}"#,
    )
    .create_async()
    .await;
    let batch = json_mock(
        &mut server,
        "POST",
        "/v1/memories/store/batch",
        200,
        r#"{"stored":[{"id":"m2","content":"y","agent_id":"a","created_at":1}],"stored_count":1,"total_embedding_time_ms":40,"ended_sessions":["sess_a"]}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .store_memory(StoreMemoryRequest::new("a", "x").with_session("sess_a"))
        .await
        .unwrap();
    assert_eq!(r.session_state.as_deref(), Some("ended"));
    let b = client
        .store_memories_batch(BatchStoreMemoryRequest::new(
            "a",
            vec![BatchStoreMemoryItem::new("y")],
        ))
        .await
        .unwrap();
    assert_eq!(b.ended_sessions, vec!["sess_a"]);
    store.assert_async().await;
    batch.assert_async().await;
}

// ============================================================================
// Listings: include_derived, content_preview_chars
// ============================================================================

#[tokio::test]
async fn agent_memories_with_sends_options_and_reads_preview_fields() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/agents/a/memories",
        200,
        r#"[{"id":"m1","agent_id":"a","content":"Hello wor","memory_type":"semantic","importance":0.5,
             "created_at":1,"last_accessed_at":1,"access_count":0,"content_len":42,"content_truncated":true},
            {"id":"m2","agent_id":"a","content":"short","memory_type":"semantic","importance":0.5,
             "created_at":1,"last_accessed_at":1,"access_count":0,"content_len":5,"content_truncated":false}]"#,
    )
    .match_query(Matcher::AllOf(vec![
        Matcher::UrlEncoded("limit".into(), "10".into()),
        Matcher::UrlEncoded("offset".into(), "20".into()),
        Matcher::UrlEncoded("include_derived".into(), "true".into()),
        Matcher::UrlEncoded("content_preview_chars".into(), "9".into()),
    ]))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let opts = AgentMemoriesOptions {
        limit: Some(10),
        offset: Some(20),
        include_derived: Some(true),
        content_preview_chars: Some(9),
        ..Default::default()
    };
    let mems = client.agent_memories_with("a", &opts).await.unwrap();
    assert_eq!(mems[0].content_len, Some(42));
    assert_eq!(mems[0].content_truncated, Some(true));
    assert_eq!(mems[1].content_truncated, Some(false));
    m.assert_async().await;
}

#[tokio::test]
async fn agent_memories_without_options_sends_no_new_parameters() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/agents/a/memories",
        200,
        r#"[{"id":"m1","content":"full text","memory_type":"semantic","importance":0.5,"created_at":1,"last_accessed_at":1,"access_count":0}]"#,
    )
    .match_query(Matcher::UrlEncoded("limit".into(), "5".into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let mems = client.agent_memories("a", None, Some(5)).await.unwrap();
    assert!(mems[0].content_len.is_none());
    assert!(mems[0].content_truncated.is_none());
    m.assert_async().await;
}

#[tokio::test]
async fn content_preview_out_of_range_is_refused() {
    let client = DakeraClient::new("http://127.0.0.1:1").unwrap();
    for n in [0, 10_001] {
        let opts = AgentMemoriesOptions {
            content_preview_chars: Some(n),
            ..Default::default()
        };
        assert!(matches!(
            client.agent_memories_with("a", &opts).await.unwrap_err(),
            ClientError::InvalidRequest(_)
        ));
        let sopts = SessionMemoriesOptions {
            content_preview_chars: Some(n),
            ..Default::default()
        };
        assert!(matches!(
            client.session_memories_with("s", &sopts).await.unwrap_err(),
            ClientError::InvalidRequest(_)
        ));
        let req = FullKnowledgeGraphRequest {
            agent_id: "a".into(),
            content_preview_chars: Some(n),
            ..Default::default()
        };
        assert!(matches!(
            client.full_knowledge_graph(req).await.unwrap_err(),
            ClientError::InvalidRequest(_)
        ));
    }
}

#[tokio::test]
async fn session_memories_with_reads_session_total_and_previews() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/sessions/sess_1/memories",
        200,
        &format!(
            r#"{{"session":{SESSION_V0122},"memories":[{{"id":"m1","agent_id":"mlx-dev","content":"abc","memory_type":"episodic","importance":0.5,"created_at":1,"last_accessed_at":1,"access_count":0,"content_len":300,"content_truncated":true}}],"total":7}}"#
        ),
    )
    .match_query(Matcher::AllOf(vec![
        Matcher::UrlEncoded("limit".into(), "1".into()),
        Matcher::UrlEncoded("content_preview_chars".into(), "3".into()),
    ]))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .session_memories_with(
            "sess_1",
            &SessionMemoriesOptions {
                limit: Some(1),
                content_preview_chars: Some(3),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(r.total, Some(7));
    assert_eq!(
        r.session.as_ref().unwrap().ended_reason.as_deref(),
        Some("idle")
    );
    assert_eq!(r.memories[0].content_len, Some(300));
    m.assert_async().await;
}

#[tokio::test]
async fn wake_up_with_include_derived() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/agents/a/wake-up",
        200,
        r#"{"agent_id":"a","memories":[],"total_available":3}"#,
    )
    .match_query(Matcher::AllOf(vec![
        Matcher::UrlEncoded("top_n".into(), "5".into()),
        Matcher::UrlEncoded("include_derived".into(), "true".into()),
    ]))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .wake_up_with(
            "a",
            &WakeUpOptions {
                top_n: Some(5),
                include_derived: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(r.total_available, 3);
    m.assert_async().await;
}

// ============================================================================
// Knowledge graph / network previews, dedup, compress
// ============================================================================

#[tokio::test]
async fn full_graph_sends_preview_and_reads_node_lengths() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/knowledge/graph/full",
        200,
        r#"{"nodes":[{"id":"m1","content":"ab","content_len":90000,"content_truncated":true,"memory_type":"Semantic",
             "importance":0.9,"tags":[],"created_at":"1700000000","cluster_id":0,"centrality":1.0}],
           "edges":[],"clusters":[{"id":0,"node_count":1,"top_tags":[],"avg_importance":0.9}],
           "stats":{"total_memories":4,"included_memories":1,"total_edges":0,"cluster_count":1,"density":0.0,"hub_memory_id":null}}"#,
    )
    .match_body(Matcher::PartialJsonString(
        r#"{"agent_id":"a","content_preview_chars":2}"#.into(),
    ))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let g = client
        .full_knowledge_graph(FullKnowledgeGraphRequest {
            agent_id: "a".into(),
            content_preview_chars: Some(2),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(g.nodes[0].content_len, Some(90000));
    assert_eq!(g.nodes[0].content_truncated, Some(true));
    m.assert_async().await;
}

#[tokio::test]
async fn cross_agent_network_sends_preview_and_reads_node_lengths() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/knowledge/network/cross-agent",
        200,
        r#"{"agents":[{"agent_id":"a","memory_count":1,"avg_importance":0.7}],
           "nodes":[{"id":"m1","agent_id":"a","content":"x","content_len":1234,"content_truncated":true,
                     "importance":0.7,"tags":[],"memory_type":"Semantic","created_at":1700000000}],
           "edges":[],"stats":{"total_agents":1,"total_nodes":1,"total_cross_edges":0,"density":0.0}}"#,
    )
    .match_body(Matcher::PartialJsonString(
        r#"{"content_preview_chars":1}"#.into(),
    ))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .cross_agent_network(CrossAgentNetworkRequest {
            content_preview_chars: Some(1),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(r.nodes[0].content_len, Some(1234));
    m.assert_async().await;
    // Unset: not sent.
    let body = serde_json::to_value(CrossAgentNetworkRequest::default()).unwrap();
    assert!(body.get("content_preview_chars").is_none());
}

#[tokio::test]
async fn deduplicate_reads_duplicates_skipped_changed() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/knowledge/deduplicate",
        200,
        r#"{"groups":[],"duplicates_found":3,"duplicates_merged":2,"duplicates_skipped_changed":1}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .deduplicate(DeduplicateRequest {
            agent_id: "a".into(),
            threshold: None,
            memory_type: None,
            dry_run: false,
        })
        .await
        .unwrap();
    assert_eq!(r.removed_count, 2);
    assert_eq!(r.duplicates_skipped_changed, 1);
    m.assert_async().await;
}

#[test]
fn compress_reads_summaries_skipped() {
    let r: dakera_client::CompressResponse = serde_json::from_str(
        r#"{"agent_id":"a","memories_scanned":40,"clusters_found":3,"summaries_created":2,"originals_deprecated":9,
            "summary_ids":["mem_compress_1"],"deprecated_ids":["m1"],
            "summaries_skipped":[{"summary_id":"mem_compress_2","reason":"content: too many bytes"}]}"#,
    )
    .unwrap();
    assert_eq!(r.summary_ids, vec!["mem_compress_1"]);
    assert_eq!(r.summaries_skipped[0].summary_id, "mem_compress_2");
    let old: dakera_client::CompressResponse = serde_json::from_str(r#"{"agent_id":"a"}"#).unwrap();
    assert!(old.summaries_skipped.is_empty());
}

// ============================================================================
// Admin: derivations, config, unavailable
// ============================================================================

const DERIVATION_STATUS: &str = r#"{"settled":false,"pending_sentences":4,"pending_parents":2,
    "unmarked_parents":0,"stale_children":1,"orphan_children":0,"remeta_children":0,
    "duplicate_children":0,"legacy_children":0,"bm25_missing":0,"graph_owed":0,"in_flight":1,
    "graph_queue_owed":0,"dirty_namespaces":["_dakera_agent_x"],"namespaces":3,
    "unreadable_namespaces":[],"heal":{"version":1,"complete":true,"namespace":null,"cursor":null,
    "parents_healed":12,"graph_adopted":40,"started_at":1760000000,"completed_at":1760000010},
    "reconciler":{"state":"sleeping","last_tick_at":1760000100,"ticks":7,"next_namespace":"_dakera_agent_x"},
    "counters":{"derived":5,"adopted":0,"rewritten":0,"deleted_stale":1,"deleted_orphans":0,
    "recheck_deleted":0,"retries":0,"deferred":0,"stamped":0,"superseded":0,"bm25_restored":0,
    "graph_rebuilds":0,"graph_adopted":0}}"#;

#[tokio::test]
async fn derivations_status_and_drain() {
    let mut server = mockito::Server::new_async().await;
    let status = json_mock(
        &mut server,
        "GET",
        "/v1/admin/derivations/status",
        200,
        DERIVATION_STATUS,
    )
    .create_async()
    .await;
    let drain = json_mock(
        &mut server,
        "POST",
        "/v1/admin/derivations/drain",
        200,
        &format!(
            r#"{{"settled":true,"timed_out":false,"rounds":1,"elapsed_ms":1234,"parents_run":17,"pending_left":0,"deleted":3,"bm25_restored":0,"graph_queued":1,"status":{DERIVATION_STATUS}}}"#
        ),
    )
    .match_body(Matcher::JsonString(r#"{"timeout_secs":60}"#.into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let s = client.derivations_status().await.unwrap();
    assert!(!s.settled);
    assert_eq!(s.pending_sentences, 4);
    assert_eq!(s.heal.as_ref().unwrap().parents_healed, 12);
    assert_eq!(s.reconciler.state, "sleeping");
    assert_eq!(s.counters["derived"], 5);
    let d = client.drain_derivations(Some(60)).await.unwrap();
    assert!(d.settled);
    assert_eq!(d.parents_run, 17);
    assert_eq!(d.status.namespaces, 3);
    status.assert_async().await;
    drain.assert_async().await;
}

#[tokio::test]
async fn drain_without_timeout_sends_empty_object() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/admin/derivations/drain",
        200,
        r#"{"settled":true,"timed_out":false,"rounds":0,"elapsed_ms":1,"parents_run":0,"pending_left":0,"deleted":0,"bm25_restored":0,"graph_queued":0,"status":{"settled":true,"heal":null}}"#,
    )
    .match_body(Matcher::JsonString("{}".into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let d = client.drain_derivations(None).await.unwrap();
    assert!(d.status.heal.is_none());
    m.assert_async().await;
}

#[tokio::test]
async fn config_session_idle_timeout_get_and_set() {
    let config = r#"{"default_index_type":"hnsw","cache_enabled":true,"cache_max_size_bytes":1,
        "rate_limit_enabled":false,"rate_limit_rps":0,"query_timeout_ms":1000,"session_idle_timeout_secs":7200}"#;
    let mut server = mockito::Server::new_async().await;
    let get = json_mock(&mut server, "GET", "/v1/admin/config", 200, config)
        .create_async()
        .await;
    let put = json_mock(
        &mut server,
        "PUT",
        "/v1/admin/config",
        200,
        &format!(r#"{{"success":true,"config":{config},"message":"ok"}}"#),
    )
    .match_body(Matcher::JsonString(
        r#"{"session_idle_timeout_secs":7200}"#.into(),
    ))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    assert_eq!(
        client.get_config().await.unwrap().session_idle_timeout_secs,
        Some(7200)
    );
    let r = client.set_session_idle_timeout(7200).await.unwrap();
    assert_eq!(r.config.session_idle_timeout_secs, Some(7200));
    assert!(matches!(
        client
            .set_session_idle_timeout(2_592_001)
            .await
            .unwrap_err(),
        ClientError::InvalidRequest(_)
    ));
    get.assert_async().await;
    put.assert_async().await;
}

#[tokio::test]
async fn ops_stats_and_admin_namespaces_read_unavailable() {
    let mut server = mockito::Server::new_async().await;
    let stats = json_mock(
        &mut server,
        "GET",
        "/v1/ops/stats",
        200,
        r#"{"version":"0.12.2","total_vectors":10,"namespace_count":3,"uptime_seconds":5,"timestamp":1,"state":"healthy",
            "unavailable":[{"namespace":"_dakera_agent_y","reason":"did not answer within 2000 ms"}]}"#,
    )
    .create_async()
    .await;
    let ns = json_mock(
        &mut server,
        "GET",
        "/v1/admin/namespaces",
        200,
        r#"{"namespaces":[{"name":"_dakera_agent_a","vector_count":1,"index_type":"hnsw","storage_bytes":0,"document_count":0,
              "index_stats":{"index_type":"hnsw","is_built":true,"size_bytes":0,"indexed_vectors":1},"kind":"agent"}],
            "total":1,"total_vectors":1}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let s = client.ops_stats().await.unwrap();
    assert_eq!(s.unavailable[0].namespace, "_dakera_agent_y");
    let n = client.list_namespaces_admin().await.unwrap();
    assert_eq!(n.namespaces[0].kind, Some(NamespaceKind::Agent));
    assert!(n.unavailable.is_empty());
    stats.assert_async().await;
    ns.assert_async().await;
}

#[test]
fn session_ended_event_carries_reason() {
    let e: dakera_client::MemoryEvent = serde_json::from_str(
        r#"{"event_type":"session_ended","agent_id":"a","session_id":"s","timestamp":1791406603000,"reason":"idle"}"#,
    )
    .unwrap();
    assert_eq!(e.reason.as_deref(), Some("idle"));
}
