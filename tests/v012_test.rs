//! Dakera server v0.12.0 support: health, errors, retry, attachments, records,
//! per-request `lang`, namespace config PUT, capabilities.
//!
//! Every wire shape here is taken from the server source (`routes/attachments.rs`,
//! `routes/records.rs`, `routes/health.rs`, `error.rs`, `startup.rs`,
//! `common/src/types/{attachment,record,mod}.rs`).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dakera_client::memory::{
    BatchStoreMemoryItem, BatchStoreMemoryRequest, RecallRequest, StoreMemoryRequest,
    UpdateMemoryRequest,
};
use dakera_client::{
    BlockDType, ClientError, DakeraClient, EdgeType, EmbeddingModel, IndexImageRequest,
    NamespaceNerConfig, RecordInput, RepresentationInput, RepresentationKind, ServerCapabilities,
    ServerErrorCode, TranscribeRequest,
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

// ============================================================================
// Health: ready / live / health (TRACKER K14)
// ============================================================================

#[tokio::test]
async fn health_503_starting_is_not_healthy() {
    // v0.12 binds its port while models load: 503 {"status":"starting"} + Retry-After.
    // The v0.11 Rust SDK reported this as healthy: true.
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/health",
        503,
        r#"{"service":"dakera","status":"starting","version":"0.12.0","reason":"loading models","downloads":[]}"#,
    )
    .with_header("Retry-After", "5")
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let h = client.health().await.unwrap();
    assert!(!h.healthy, "a 503 must never be reported healthy");
    assert_eq!(h.status.as_deref(), Some("starting"));
    assert_eq!(h.version.as_deref(), Some("0.12.0"));
    m.assert_async().await;
}

#[tokio::test]
async fn health_503_with_non_json_body_is_not_healthy() {
    let mut server = mockito::Server::new_async().await;
    let m = server
        .mock("GET", "/health")
        .with_status(503)
        .with_body("upstream unavailable")
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    assert!(!client.health().await.unwrap().healthy);
    m.assert_async().await;
}

#[tokio::test]
async fn health_200_healthy_and_degraded() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/health",
        200,
        r#"{"service":"dakera","status":"healthy","version":"0.12.0","build_sha":"abc"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let h = client.health().await.unwrap();
    assert!(h.healthy);
    assert_eq!(h.build_sha.as_deref(), Some("abc"));
    m.assert_async().await;

    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/health",
        200,
        r#"{"service":"dakera","status":"degraded","version":"0.12.0","advice":"x"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let h = client.health().await.unwrap();
    assert!(!h.healthy);
    assert_eq!(h.status.as_deref(), Some("degraded"));
    m.assert_async().await;
}

#[tokio::test]
async fn ready_200_parses_v012_checks() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/health/ready",
        200,
        r#"{"ready":true,"version":"0.12.0","checks":{"storage":{"status":"ok","message":null},
            "embedding_engine":{"status":"ok","message":null},
            "tiered_engine":{"status":"disabled","message":null}}}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.ready().await.unwrap();
    assert!(r.ready);
    assert!(!r.starting);
    assert_eq!(r.version.as_deref(), Some("0.12.0"));
    let checks = r.checks.unwrap();
    assert_eq!(checks["storage"]["status"], "ok");
    assert_eq!(checks["tiered_engine"]["status"], "disabled");
    assert_eq!(r.retry_after, None);
    m.assert_async().await;
}

#[tokio::test]
async fn ready_accepts_the_legacy_v011_body() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/health/ready",
        200,
        r#"{"ready":true,"components":{"storage":true}}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.ready().await.unwrap();
    assert!(r.ready);
    assert!(r.components.unwrap()["storage"]);
    m.assert_async().await;
}

#[tokio::test]
async fn ready_503_starting_is_not_ready_and_carries_retry_after() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/health/ready",
        503,
        r#"{"ready":false,"version":"0.12.0","starting":true,"reason":"loading the embedding model","downloads":[{"file":"model.onnx"}]}"#,
    )
    .with_header("Retry-After", "5")
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.ready().await.unwrap();
    assert!(!r.ready);
    assert!(r.starting);
    assert_eq!(r.reason.as_deref(), Some("loading the embedding model"));
    assert_eq!(r.retry_after, Some(5));
    assert!(r.downloads.is_some());
    m.assert_async().await;
}

#[tokio::test]
async fn ready_503_failed_component_is_not_ready() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/health/ready",
        503,
        r#"{"ready":false,"version":"0.12.0","checks":{"storage":{"status":"error","message":"storage unreachable (details in the server log)"},
            "embedding_engine":{"status":"ok","message":null},"tiered_engine":{"status":"disabled","message":null}}}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.ready().await.unwrap();
    assert!(!r.ready);
    assert_eq!(r.checks.unwrap()["storage"]["status"], "error");
    m.assert_async().await;
}

#[tokio::test]
async fn ready_503_with_garbage_body_is_still_not_ready() {
    let mut server = mockito::Server::new_async().await;
    let m = server
        .mock("GET", "/health/ready")
        .with_status(503)
        .with_header("Retry-After", "2")
        .with_body("<html>bad gateway</html>")
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.ready().await.unwrap();
    assert!(!r.ready);
    assert_eq!(r.retry_after, Some(2));
    m.assert_async().await;
}

#[tokio::test]
async fn live_is_true_while_starting_and_false_on_error() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/health/live",
        200,
        r#"{"alive":true,"version":"0.12.0","uptime_seconds":3,"starting":true}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    assert!(client.live().await.unwrap());
    m.assert_async().await;

    let mut server = mockito::Server::new_async().await;
    let m = server
        .mock("GET", "/health/live")
        .with_status(500)
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    assert!(!client.live().await.unwrap());
    m.assert_async().await;
}

#[tokio::test]
async fn wait_until_ready_returns_when_ready() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/health/ready",
        200,
        r#"{"ready":true,"version":"0.12.0"}"#,
    )
    .expect(1)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .wait_until_ready(Duration::from_secs(5))
        .await
        .unwrap();
    assert!(r.ready);
    m.assert_async().await;
}

#[tokio::test]
async fn wait_until_ready_times_out_on_a_server_that_stays_503() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/health/ready",
        503,
        r#"{"ready":false,"starting":true,"reason":"loading"}"#,
    )
    .with_header("Retry-After", "1")
    .expect_at_least(1)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let started = Instant::now();
    let err = client
        .wait_until_ready(Duration::from_millis(1500))
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::Timeout), "got {err:?}");
    assert!(started.elapsed() < Duration::from_secs(4));
    m.assert_async().await;
}

// ============================================================================
// Errors: JSON bodies, 413, 501, 503 + Retry-After
// ============================================================================

#[tokio::test]
async fn http_503_maps_to_service_unavailable_with_retry_after() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/namespaces",
        503,
        r#"{"error":"Service unavailable","code":"SERVICE_UNAVAILABLE","details":"the server is starting (loading models); GET /health/ready says when it is ready"}"#,
    )
    .with_header("Retry-After", "5")
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let err = client.list_namespaces().await.unwrap_err();
    match &err {
        ClientError::ServiceUnavailable {
            message,
            details,
            retry_after,
        } => {
            assert!(message.contains("starting"), "{message}");
            assert!(details.is_some());
            assert_eq!(*retry_after, Some(5));
        }
        other => panic!("unexpected {other:?}"),
    }
    assert!(err.is_retryable());
    assert_eq!(err.retry_after(), Some(Duration::from_secs(5)));
    m.assert_async().await;
}

#[tokio::test]
async fn http_413_quota_vs_payload_too_large() {
    let mut server = mockito::Server::new_async().await;
    let quota = json_mock(
        &mut server,
        "POST",
        "/v1/namespaces/q/vectors",
        413,
        r#"{"error":"Quota exceeded for namespace 'q'","code":"QUOTA_EXCEEDED","details":"namespace: q, reason: hard limit of 10 vectors"}"#,
    )
    .create_async()
    .await;
    let big = json_mock(
        &mut server,
        "POST",
        "/v1/namespaces/b/vectors",
        413,
        r#"{"error":"records[0] (id 'r1'): too many vectors","code":"PAYLOAD_TOO_LARGE"}"#,
    )
    .create_async()
    .await;
    let proxy = server
        .mock("POST", "/v1/namespaces/p/vectors")
        .with_status(413)
        .with_body("Request Entity Too Large")
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let req = || dakera_client::UpsertRequest {
        vectors: vec![dakera_client::Vector::new("v", vec![0.5, 0.5])],
    };

    let e = client.upsert("q", req()).await.unwrap_err();
    match &e {
        ClientError::QuotaExceeded { message, details } => {
            assert!(message.contains("Quota exceeded"));
            assert!(details.as_deref().unwrap().contains("hard limit"));
        }
        other => panic!("unexpected {other:?}"),
    }
    assert!(e.is_payload_too_large());
    assert!(!e.is_retryable());

    let e = client.upsert("b", req()).await.unwrap_err();
    assert!(matches!(e, ClientError::PayloadTooLarge { .. }), "{e:?}");
    assert!(e.is_payload_too_large());
    assert!(!e.is_retryable());

    let e = client.upsert("p", req()).await.unwrap_err();
    match e {
        ClientError::PayloadTooLarge { message, .. } => {
            assert_eq!(message, "Request Entity Too Large")
        }
        other => panic!("unexpected {other:?}"),
    }
    quota.assert_async().await;
    big.assert_async().await;
    proxy.assert_async().await;
}

#[tokio::test]
async fn http_501_feature_disabled_and_not_implemented() {
    let mut server = mockito::Server::new_async().await;
    let off = json_mock(
        &mut server,
        "POST",
        "/v1/namespaces/docs/records",
        501,
        r#"{"error":"The records API is not enabled on this server","code":"FEATURE_DISABLED","details":"set DAKERA_RECORDS to enable it"}"#,
    )
    .create_async()
    .await;
    let nope = json_mock(
        &mut server,
        "POST",
        "/ops/compact",
        501,
        r#"{"error":"compaction is not available on this backend","code":"NOT_IMPLEMENTED","details":"use the RocksDB backend"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();

    let e = client
        .upsert_records("docs", vec![RecordInput::new("r1", vec![0.5, 0.5])])
        .await
        .unwrap_err();
    match &e {
        ClientError::FeatureDisabled { message, details } => {
            assert!(message.contains("DAKERA_RECORDS"), "{message}");
            assert_eq!(details.as_deref(), Some("set DAKERA_RECORDS to enable it"));
        }
        other => panic!("unexpected {other:?}"),
    }
    assert!(e.is_feature_disabled());
    assert!(!e.is_retryable(), "a 501 must not be retried");

    let e = client
        .compact(dakera_client::CompactionRequest {
            namespace: None,
            force: false,
        })
        .await
        .unwrap_err();
    assert!(matches!(e, ClientError::NotImplemented { .. }), "{e:?}");
    assert!(!e.is_retryable());
    off.assert_async().await;
    nope.assert_async().await;
}

#[tokio::test]
async fn http_429_keeps_retry_after() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/namespaces",
        429,
        r#"{"error":"Rate limit exceeded","code":"RATE_LIMIT_EXCEEDED"}"#,
    )
    .with_header("Retry-After", "17")
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let e = client.list_namespaces().await.unwrap_err();
    assert!(matches!(
        e,
        ClientError::RateLimitExceeded {
            retry_after: Some(17)
        }
    ));
    assert_eq!(e.retry_after(), Some(Duration::from_secs(17)));
    m.assert_async().await;
}

#[tokio::test]
async fn http_404_job_not_found_and_resource_codes() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/namespaces/up/attachments/sha256:abc/transcribe/job_1_0",
        404,
        r#"{"error":"transcription job 'job_1_0' for attachment 'sha256:abc' is unknown: the server restarted","code":"JOB_NOT_FOUND","status":404,"resource":"job"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let e = client
        .transcription_status("up", "sha256:abc", "job_1_0")
        .await
        .unwrap_err();
    match &e {
        ClientError::Server {
            status,
            code,
            message,
        } => {
            assert_eq!(*status, 404);
            assert_eq!(*code, Some(ServerErrorCode::JobNotFound));
            assert!(message.contains("restarted"));
        }
        other => panic!("unexpected {other:?}"),
    }
    assert!(e.is_not_found());
    m.assert_async().await;
}

#[tokio::test]
async fn http_409_conflict_code_is_typed() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "DELETE",
        "/v1/namespaces/up/attachments/sha256:abc",
        409,
        r#"{"error":"attachment is referenced by a memory","code":"CONFLICT","details":"forget the memory instead"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let e = client
        .delete_attachment("up", "sha256:abc")
        .await
        .unwrap_err();
    assert!(matches!(
        e,
        ClientError::Server {
            status: 409,
            code: Some(ServerErrorCode::Conflict),
            ..
        }
    ));
    m.assert_async().await;
}

#[tokio::test]
async fn delete_namespace_error_is_mapped_from_the_json_body() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "DELETE",
        "/v1/namespaces/ns",
        503,
        r#"{"error":"Service unavailable","code":"SERVICE_UNAVAILABLE","details":"namespace: ns"}"#,
    )
    .with_header("Retry-After", "3")
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let e = client.delete_namespace("ns").await.unwrap_err();
    assert_eq!(e.retry_after(), Some(Duration::from_secs(3)));
    m.assert_async().await;
}

#[test]
fn every_v012_error_code_deserialises() {
    for (wire, want) in [
        ("PAYLOAD_TOO_LARGE", ServerErrorCode::PayloadTooLarge),
        ("FEATURE_DISABLED", ServerErrorCode::FeatureDisabled),
        ("NOT_IMPLEMENTED", ServerErrorCode::NotImplemented),
        ("CONFLICT", ServerErrorCode::Conflict),
        (
            "CROSS_ORIGIN_REQUEST_REFUSED",
            ServerErrorCode::CrossOriginRequestRefused,
        ),
        ("RATE_LIMIT_EXCEEDED", ServerErrorCode::RateLimitExceeded),
        ("QUERY_TIMEOUT", ServerErrorCode::QueryTimeout),
        ("ROUTE_NOT_FOUND", ServerErrorCode::RouteNotFound),
        ("METHOD_NOT_ALLOWED", ServerErrorCode::MethodNotAllowed),
        (
            "UNSUPPORTED_MEDIA_TYPE",
            ServerErrorCode::UnsupportedMediaType,
        ),
        ("REQUEST_TIMEOUT", ServerErrorCode::RequestTimeout),
        ("API_KEY_NOT_FOUND", ServerErrorCode::ApiKeyNotFound),
        ("JOB_NOT_FOUND", ServerErrorCode::JobNotFound),
        ("QUOTA_EXCEEDED", ServerErrorCode::QuotaExceeded),
        ("SERVICE_UNAVAILABLE", ServerErrorCode::ServiceUnavailable),
    ] {
        let got: ServerErrorCode = serde_json::from_value(json!(wire)).unwrap();
        assert_eq!(got, want, "{wire}");
    }
    let future: ServerErrorCode = serde_json::from_value(json!("SOMETHING_NEW")).unwrap();
    assert_eq!(future, ServerErrorCode::Unknown);
}

// ============================================================================
// Retry-After is honoured by the retry logic
// ============================================================================

#[tokio::test]
async fn execute_with_retry_waits_the_retry_after_of_a_503() {
    let client = DakeraClient::builder("http://localhost:1")
        .retry_config(dakera_client::RetryConfig {
            max_retries: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_secs(30),
            jitter: false,
        })
        .build()
        .unwrap();
    let calls = Arc::new(AtomicU32::new(0));
    let counter = calls.clone();
    let started = Instant::now();
    let out = client
        .execute_with_retry(|| {
            let counter = counter.clone();
            async move {
                if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(ClientError::ServiceUnavailable {
                        message: "starting".into(),
                        details: None,
                        retry_after: Some(1),
                    })
                } else {
                    Ok(42u32)
                }
            }
        })
        .await
        .unwrap();
    assert_eq!(out, 42);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(
        started.elapsed() >= Duration::from_millis(950),
        "must wait the server's Retry-After (1 s), not the 1 ms backoff: {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn execute_with_retry_caps_retry_after_at_max_delay() {
    let client = DakeraClient::builder("http://localhost:1")
        .retry_config(dakera_client::RetryConfig {
            max_retries: 2,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(50),
            jitter: false,
        })
        .build()
        .unwrap();
    let calls = Arc::new(AtomicU32::new(0));
    let counter = calls.clone();
    let started = Instant::now();
    client
        .execute_with_retry(|| {
            let counter = counter.clone();
            async move {
                if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(ClientError::RateLimitExceeded {
                        retry_after: Some(3600),
                    })
                } else {
                    Ok(())
                }
            }
        })
        .await
        .unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn execute_with_retry_does_not_retry_413_or_501() {
    let client = DakeraClient::builder("http://localhost:1")
        .max_retries(3)
        .build()
        .unwrap();
    let makers: [fn() -> ClientError; 4] = [
        || ClientError::PayloadTooLarge {
            message: "big".into(),
            details: None,
        },
        || ClientError::QuotaExceeded {
            message: "full".into(),
            details: None,
        },
        || ClientError::FeatureDisabled {
            message: "off".into(),
            details: None,
        },
        || ClientError::NotImplemented {
            message: "no".into(),
            details: None,
        },
    ];
    for make in makers {
        let calls = Arc::new(AtomicU32::new(0));
        let counter = calls.clone();
        let err = client
            .execute_with_retry(|| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Err::<(), _>(make())
                }
            })
            .await
            .unwrap_err();
        assert_eq!(calls.load(Ordering::SeqCst), 1, "{err:?} must not retry");
    }
}

// ============================================================================
// Attachments
// ============================================================================

const REF: &str = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[tokio::test]
async fn upload_attachment_sends_a_raw_body_with_its_content_type() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/namespaces/uploads/attachments",
        201,
        &format!(
            r#"{{"attachment_ref":"{REF}","content_type":"audio/wav","size_bytes":8,"created":true}}"#
        ),
    )
    .match_header("content-type", "audio/wav")
    .match_body(Matcher::Exact("RIFFwave".to_string()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let up = client
        .upload_attachment("uploads", b"RIFFwave".to_vec(), "audio/wav")
        .await
        .unwrap();
    assert_eq!(up.attachment_ref, REF);
    assert_eq!(up.content_type, "audio/wav");
    assert_eq!(up.size_bytes, 8);
    assert!(up.created);
    m.assert_async().await;
}

#[tokio::test]
async fn upload_attachment_200_means_the_bytes_were_already_there() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/namespaces/uploads/attachments",
        200,
        &format!(
            r#"{{"attachment_ref":"{REF}","content_type":"image/png","size_bytes":4,"created":false}}"#
        ),
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let up = client
        .upload_attachment("uploads", vec![1u8, 2, 3, 4], "image/png")
        .await
        .unwrap();
    assert!(!up.created);
    m.assert_async().await;
}

#[tokio::test]
async fn upload_attachment_over_the_limit_is_payload_too_large() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/namespaces/uploads/attachments",
        413,
        r#"{"error":"body over DAKERA_ATTACHMENT_MAX_BYTES (26214400 bytes)","code":"PAYLOAD_TOO_LARGE"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let e = client
        .upload_attachment("uploads", vec![0u8; 16], "application/octet-stream")
        .await
        .unwrap_err();
    match e {
        ClientError::PayloadTooLarge { message, .. } => {
            assert!(message.contains("DAKERA_ATTACHMENT_MAX_BYTES"))
        }
        other => panic!("unexpected {other:?}"),
    }
    m.assert_async().await;
}

#[tokio::test]
async fn attachments_disabled_server_is_feature_disabled() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/namespaces/uploads/attachments",
        501,
        r#"{"error":"The attachments API is not enabled on this server","code":"FEATURE_DISABLED","details":"set DAKERA_ATTACHMENTS to enable it"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let e = client.list_attachments("uploads").await.unwrap_err();
    assert!(e.is_feature_disabled());
    assert!(e.to_string().contains("DAKERA_ATTACHMENTS"));
    m.assert_async().await;
}

#[tokio::test]
async fn list_attachments_parses_entries() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/namespaces/uploads/attachments",
        200,
        &format!(
            r#"{{"attachments":[{{"attachment_ref":"{REF}","content_type":"audio/wav","size_bytes":8,"future":1}}]}}"#
        ),
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let list = client.list_attachments("uploads").await.unwrap();
    assert_eq!(list.attachments.len(), 1);
    assert_eq!(list.attachments[0].attachment_ref, REF);
    assert_eq!(list.attachments[0].size_bytes, 8);
    m.assert_async().await;
}

#[tokio::test]
async fn download_attachment_returns_bytes_type_and_etag() {
    let mut server = mockito::Server::new_async().await;
    let m = server
        .mock(
            "GET",
            format!("/v1/namespaces/uploads/attachments/{REF}").as_str(),
        )
        .with_status(200)
        .with_header("content-type", "audio/wav")
        .with_header("etag", &format!("\"{REF}\""))
        .with_body(vec![0u8, 1, 2, 255])
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let dl = client.download_attachment("uploads", REF).await.unwrap();
    assert_eq!(dl.data, vec![0u8, 1, 2, 255]);
    assert_eq!(dl.content_type, "audio/wav");
    assert_eq!(dl.etag.as_deref(), Some(REF));
    m.assert_async().await;
}

#[tokio::test]
async fn download_attachment_404_is_an_error() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        format!("/v1/namespaces/uploads/attachments/{REF}").as_str(),
        404,
        r#"{"error":"Attachment not found","code":"VECTOR_NOT_FOUND","resource":"attachment"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let e = client
        .download_attachment("uploads", REF)
        .await
        .unwrap_err();
    assert!(e.is_not_found());
    m.assert_async().await;
}

#[tokio::test]
async fn delete_attachment_204() {
    let mut server = mockito::Server::new_async().await;
    let m = server
        .mock(
            "DELETE",
            format!("/v1/namespaces/uploads/attachments/{REF}").as_str(),
        )
        .with_status(204)
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client.delete_attachment("uploads", REF).await.unwrap();
    m.assert_async().await;
}

#[tokio::test]
async fn transcribe_attachment_sends_the_memory_fields_and_parses_the_202() {
    let mut server = mockito::Server::new_async().await;
    let status_url = format!("/v1/namespaces/uploads/attachments/{REF}/transcribe/job_1a2b3c4d_0");
    let m = json_mock(
        &mut server,
        "POST",
        format!("/v1/namespaces/uploads/attachments/{REF}/transcribe").as_str(),
        202,
        &format!(
            r#"{{"job_id":"job_1a2b3c4d_0","attachment_ref":"{REF}","agent_id":"my-agent","memory_id":"mem_17f3","model":"whisper-tiny.en","status_url":"{status_url}"}}"#
        ),
    )
    .match_body(Matcher::Json(json!({
        "agent_id": "my-agent",
        "importance": 0.75,
        "tags": ["voice"],
        "lang": "en"
    })))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let req = TranscribeRequest::new("my-agent")
        .with_importance(0.75)
        .with_tags(vec!["voice".into()])
        .with_lang("en");
    let job = client
        .transcribe_attachment("uploads", REF, req)
        .await
        .unwrap();
    assert_eq!(job.job_id, "job_1a2b3c4d_0");
    assert_eq!(job.memory_id, "mem_17f3");
    assert_eq!(job.model, "whisper-tiny.en");
    assert_eq!(job.status_url, status_url);
    m.assert_async().await;
}

#[tokio::test]
async fn transcription_status_running_completed_and_failed() {
    let mut server = mockito::Server::new_async().await;
    let path = format!("/v1/namespaces/uploads/attachments/{REF}/transcribe/job_1_0");
    let m = json_mock(
        &mut server,
        "GET",
        path.as_str(),
        200,
        r#"{"id":"job_1_0","job_type":"attachment_transcription","status":"Running","created_at":1,"started_at":2,"completed_at":null,"progress":42,"message":"transcribing","metadata":{"namespace":"uploads"}}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let job = client
        .transcription_status("uploads", REF, "job_1_0")
        .await
        .unwrap();
    assert_eq!(job.progress, 42);
    assert!(!job.is_finished());
    assert!(job.error.is_none());
    m.assert_async().await;

    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        path.as_str(),
        200,
        r#"{"id":"job_1_0","job_type":"attachment_transcription","status":"Failed","created_at":1,"started_at":2,"completed_at":3,"progress":5,"message":"the audio holds no speech","metadata":{},"error":{"status":400,"code":"INVALID_REQUEST"}}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let job = client
        .transcription_status("uploads", REF, "job_1_0")
        .await
        .unwrap();
    assert!(job.is_finished() && job.is_failed() && !job.is_completed());
    let err = job.error.unwrap();
    assert_eq!(err.status, 400);
    assert_eq!(err.code, ServerErrorCode::InvalidRequest);
    m.assert_async().await;
}

#[tokio::test]
async fn index_image_attachment_and_status() {
    let mut server = mockito::Server::new_async().await;
    let status_url = format!("/v1/namespaces/pages/attachments/{REF}/index/job_9_3");
    let post = json_mock(
        &mut server,
        "POST",
        format!("/v1/namespaces/pages/attachments/{REF}/index").as_str(),
        202,
        &format!(
            r#"{{"job_id":"job_9_3","attachment_ref":"{REF}","agent_id":"a","memory_id":"mem_1","model":"colmodernvbert","status_url":"{status_url}"}}"#
        ),
    )
    .match_body(Matcher::Json(json!({
        "agent_id": "a",
        "content": "page 1",
        "id": "mem_1"
    })))
    .create_async()
    .await;
    let get = json_mock(
        &mut server,
        "GET",
        status_url.as_str(),
        200,
        r#"{"id":"job_9_3","job_type":"attachment_image_index","status":"Completed","created_at":1,"started_at":2,"completed_at":3,"progress":100,"message":"memory mem_1 stored","metadata":{}}"#,
    )
    .expect(2)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let job = client
        .index_image_attachment(
            "pages",
            REF,
            IndexImageRequest::new("a")
                .with_content("page 1")
                .with_id("mem_1"),
        )
        .await
        .unwrap();
    assert_eq!(job.model, "colmodernvbert");
    let done = client
        .index_image_status("pages", REF, &job.job_id)
        .await
        .unwrap();
    assert!(done.is_completed());
    // The status_url the 202 returned is directly pollable, and so is wait_for.
    let waited = client
        .wait_for_attachment_job(&job.status_url, Duration::from_secs(5))
        .await
        .unwrap();
    assert!(waited.is_completed());
    post.assert_async().await;
    get.assert_async().await;
}

#[tokio::test]
async fn wait_for_attachment_job_returns_a_failed_job_as_ok() {
    let mut server = mockito::Server::new_async().await;
    let url = "/v1/namespaces/uploads/attachments/sha256:x/transcribe/job_2_0";
    let m = json_mock(
        &mut server,
        "GET",
        url,
        200,
        r#"{"id":"job_2_0","job_type":"attachment_transcription","status":"Failed","created_at":1,"progress":2,"message":"cannot load model","metadata":{},"error":{"status":503,"code":"SERVICE_UNAVAILABLE"}}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let job = client
        .wait_for_attachment_job(url, Duration::from_secs(5))
        .await
        .unwrap();
    assert!(job.is_failed());
    assert_eq!(job.error.unwrap().status, 503);
    m.assert_async().await;
}

#[tokio::test]
async fn wait_for_attachment_job_times_out_and_surfaces_job_not_found() {
    let mut server = mockito::Server::new_async().await;
    let running = "/v1/namespaces/u/attachments/sha256:r/transcribe/job_3_0";
    let m = json_mock(
        &mut server,
        "GET",
        running,
        200,
        r#"{"id":"job_3_0","job_type":"t","status":"Running","created_at":1,"progress":10,"metadata":{}}"#,
    )
    .expect_at_least(1)
    .create_async()
    .await;
    let gone = "/v1/namespaces/u/attachments/sha256:g/transcribe/job_3_1";
    let g = json_mock(
        &mut server,
        "GET",
        gone,
        404,
        r#"{"error":"unknown job: the server restarted","code":"JOB_NOT_FOUND","resource":"job"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let e = client
        .wait_for_attachment_job(running, Duration::from_millis(1200))
        .await
        .unwrap_err();
    assert!(matches!(e, ClientError::Timeout), "{e:?}");
    let e = client
        .wait_for_attachment_job(gone, Duration::from_secs(5))
        .await
        .unwrap_err();
    assert!(e.is_not_found());
    m.assert_async().await;
    g.assert_async().await;
}

// ============================================================================
// Records with named representations
// ============================================================================

#[tokio::test]
async fn upsert_records_sends_named_representations() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/namespaces/docs/records",
        200,
        r#"{"upserted_count":1}"#,
    )
    .match_body(Matcher::Json(json!({"records": [{
        "id": "r1",
        "values": [0.5, 0.25, 0.125, 1.0],
        "representations": [{
            "name": "tokens",
            "kind": "token_multivector",
            "vectors": [[0.5, 0.25], [0.125, 1.0]],
            "store_as": "f16"
        }],
        "metadata": {"source": "demo"}
    }]})))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let record = RecordInput::new("r1", vec![0.5, 0.25, 0.125, 1.0])
        .with_representation(
            RepresentationInput::token_multivector(
                "tokens",
                vec![vec![0.5, 0.25], vec![0.125, 1.0]],
            )
            .with_store_as(BlockDType::F16),
        )
        .with_metadata(json!({"source": "demo"}));
    let resp = client.upsert_records("docs", vec![record]).await.unwrap();
    assert_eq!(resp.upserted_count, 1);
    m.assert_async().await;
}

#[test]
fn a_plain_record_serialises_like_a_vector_and_a_model_is_optional() {
    let plain = serde_json::to_value(RecordInput::new("r", vec![0.5])).unwrap();
    assert_eq!(plain, json!({"id": "r", "values": [0.5]}));

    let rep = RepresentationInput::patch_multivector("patch", vec![vec![0.5]])
        .with_model("colmodernvbert")
        .with_store_as(BlockDType::I8);
    assert_eq!(
        serde_json::to_value(&rep).unwrap(),
        json!({"name": "patch", "kind": "patch_multivector", "model": "colmodernvbert",
               "vectors": [[0.5]], "store_as": "i8"})
    );
    let dense = RepresentationInput::dense("alt", vec![vec![0.5]]);
    let v = serde_json::to_value(&dense).unwrap();
    assert_eq!(v["kind"], "dense");
    assert_eq!(v["store_as"], "f32");
    assert!(v.get("model").is_none());
}

#[tokio::test]
async fn get_record_manifest_and_vectors() {
    let mut server = mockito::Server::new_async().await;
    let manifest = json_mock(
        &mut server,
        "GET",
        "/v1/namespaces/docs/records/r1",
        200,
        r#"{"id":"r1","dimension":4,"representations":[
             {"name":"tokens","kind":"token_multivector","model":"colbert-small","dim":2,"count":2,"dtype":"f16","bytes":8},
             {"name":"future","kind":"holographic","dim":3,"count":1,"dtype":"e4m3","bytes":3}],
            "unsupported_representations":1,"metadata":{"source":"demo"},"ttl_seconds":60,"expires_at":1893456000}"#,
    )
    .match_query(Matcher::UrlEncoded("include_vectors".into(), "false".into()))
    .create_async()
    .await;
    let full = json_mock(
        &mut server,
        "GET",
        "/v1/namespaces/docs/records/r1",
        200,
        r#"{"id":"r1","values":[0.5,0.25,0.125,1.0],"dimension":4,"representations":[
             {"name":"tokens","kind":"token_multivector","dim":2,"count":2,"dtype":"f32","bytes":16,
              "vectors":[[0.5,0.25],[0.125,1.0]]}]}"#,
    )
    .match_query(Matcher::UrlEncoded("include_vectors".into(), "true".into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();

    let view = client.get_record("docs", "r1", false).await.unwrap();
    assert_eq!(view.dimension, 4);
    assert!(view.values.is_none());
    assert_eq!(view.unsupported_representations, 1);
    assert_eq!(view.ttl_seconds, Some(60));
    assert_eq!(view.representations.len(), 2);
    let tokens = &view.representations[0];
    assert_eq!(tokens.kind, RepresentationKind::TokenMultivector);
    assert_eq!(tokens.model, "colbert-small");
    assert_eq!((tokens.dim, tokens.count, tokens.bytes), (2, 2, 8));
    assert_eq!(tokens.dtype, BlockDType::F16);
    assert!(tokens.vectors.is_none());
    // Kinds / dtypes written by a newer server survive as Unknown.
    let future = &view.representations[1];
    assert!(!future.kind.is_known());
    assert_eq!(future.kind.as_str(), "holographic");
    assert_eq!(future.dtype.as_str(), "e4m3");

    let view = client.get_record("docs", "r1", true).await.unwrap();
    assert_eq!(view.values.as_deref(), Some(&[0.5, 0.25, 0.125, 1.0][..]));
    assert_eq!(
        view.representations[0].vectors.as_ref().unwrap(),
        &vec![vec![0.5, 0.25], vec![0.125, 1.0]]
    );
    manifest.assert_async().await;
    full.assert_async().await;
}

#[tokio::test]
async fn records_oversize_is_payload_too_large_and_missing_is_not_found() {
    let mut server = mockito::Server::new_async().await;
    let big = json_mock(
        &mut server,
        "POST",
        "/v1/namespaces/docs/records",
        413,
        r#"{"error":"records[0] (id 'r1'): representation 'tokens' holds 5000 vectors, over the limit of 4096","code":"PAYLOAD_TOO_LARGE"}"#,
    )
    .create_async()
    .await;
    let missing = json_mock(
        &mut server,
        "GET",
        "/v1/namespaces/docs/records/nope",
        404,
        r#"{"error":"Vector not found: nope","code":"VECTOR_NOT_FOUND","resource":"vector"}"#,
    )
    .match_query(Matcher::Any)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let e = client
        .upsert_records("docs", vec![RecordInput::new("r1", vec![0.5])])
        .await
        .unwrap_err();
    assert!(matches!(e, ClientError::PayloadTooLarge { .. }));
    let e = client.get_record("docs", "nope", false).await.unwrap_err();
    assert!(e.is_not_found());
    big.assert_async().await;
    missing.assert_async().await;
}

// ============================================================================
// Per-request `lang` and `attachment_ref`
// ============================================================================

#[test]
fn lang_and_attachment_ref_are_absent_unless_set() {
    let store = serde_json::to_value(StoreMemoryRequest::new("a", "c")).unwrap();
    assert!(store.get("lang").is_none() && store.get("attachment_ref").is_none());
    let recall = serde_json::to_value(RecallRequest::new("a", "q")).unwrap();
    assert!(recall.get("lang").is_none());
    let batch = serde_json::to_value(BatchStoreMemoryRequest::new(
        "a",
        vec![BatchStoreMemoryItem::new("x")],
    ))
    .unwrap();
    assert!(batch.get("lang").is_none());
    assert!(batch["memories"][0].get("attachment_ref").is_none());
    let update = serde_json::to_value(UpdateMemoryRequest::default()).unwrap();
    assert_eq!(update, json!({}));

    let store = serde_json::to_value(
        StoreMemoryRequest::new("a", "c")
            .with_lang("de")
            .with_attachment_ref(REF),
    )
    .unwrap();
    assert_eq!(store["lang"], "de");
    assert_eq!(store["attachment_ref"], REF);
}

#[tokio::test]
async fn store_memory_sends_lang_and_attachment_ref() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/memory/store",
        200,
        r#"{"memory":{"id":"m1","agent_id":"a","attachment_ref":"sha256:x"},"embedding_time_ms":3}"#,
    )
    .match_body(Matcher::PartialJson(json!({
        "agent_id": "a",
        "content": "Treffen morgen um drei",
        "lang": "de",
        "attachment_ref": REF
    })))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .store_memory(
            StoreMemoryRequest::new("a", "Treffen morgen um drei")
                .with_lang("de")
                .with_attachment_ref(REF),
        )
        .await
        .unwrap();
    assert_eq!(r.memory_id, "m1");
    m.assert_async().await;
}

#[tokio::test]
async fn recall_and_search_send_lang_and_parse_attachment_ref() {
    let mut server = mockito::Server::new_async().await;
    let body = format!(
        r#"{{"memories":[{{"memory":{{"id":"m1","content":"voice note","memory_type":"episodic","importance":0.5,"created_at":1,"last_accessed_at":1,"access_count":0,"attachment_ref":"{REF}"}},"score":0.9}}],"total_found":1}}"#
    );
    let recall = json_mock(&mut server, "POST", "/v1/memory/recall", 200, &body)
        .match_body(Matcher::PartialJson(
            json!({"agent_id": "a", "query": "wann ist das Treffen", "lang": "de"}),
        ))
        .create_async()
        .await;
    let search = json_mock(&mut server, "POST", "/v1/memory/search", 200, &body)
        .match_body(Matcher::PartialJson(json!({"lang": "fr"})))
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .recall(RecallRequest::new("a", "wann ist das Treffen").with_lang("de"))
        .await
        .unwrap();
    assert_eq!(r.memories[0].attachment_ref.as_deref(), Some(REF));
    client
        .search_memories(RecallRequest::new("a", "q").with_lang("fr"))
        .await
        .unwrap();
    recall.assert_async().await;
    search.assert_async().await;
}

#[tokio::test]
async fn batch_store_sends_request_level_lang_and_item_attachment_ref() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/memories/store/batch",
        200,
        r#"{"stored":[{"id":"m1","content":"x","agent_id":"a","created_at":1}],"stored_count":1,"total_embedding_time_ms":2}"#,
    )
    .match_body(Matcher::PartialJson(json!({
        "agent_id": "a",
        "lang": "es",
        "memories": [{"content": "x", "attachment_ref": REF}]
    })))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let resp = client
        .store_memories_batch(
            BatchStoreMemoryRequest::new(
                "a",
                vec![BatchStoreMemoryItem::new("x").with_attachment_ref(REF)],
            )
            .with_lang("es"),
        )
        .await
        .unwrap();
    assert_eq!(resp.stored_count, 1);
    m.assert_async().await;
}

#[tokio::test]
async fn update_memory_sends_lang() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "PUT",
        "/v1/memory/update/m1",
        200,
        r#"{"id":"m1","agent_id":"a"}"#,
    )
    .match_query(Matcher::UrlEncoded("agent_id".into(), "a".into()))
    .match_body(Matcher::Json(json!({"lang": "it"})))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client
        .update_memory(
            "a",
            "m1",
            UpdateMemoryRequest {
                lang: Some("it".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    m.assert_async().await;
}

#[tokio::test]
async fn extract_entities_without_types_omits_entity_types() {
    // The v0.12.0 server answers `"entity_types": null` with a 422; no types
    // must leave the field out so the server applies its defaults.
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/memories/extract",
        200,
        r#"{"entities":[]}"#,
    )
    .match_body(Matcher::Json(json!({"content": "Anna met Bob in Paris"})))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .extract_entities("Anna met Bob in Paris", None)
        .await
        .unwrap();
    assert!(r.entities.is_empty());
    m.assert_async().await;
}

#[tokio::test]
async fn extract_endpoints_send_lang() {
    let mut server = mockito::Server::new_async().await;
    let ner = json_mock(
        &mut server,
        "POST",
        "/v1/memories/extract",
        200,
        r#"{"entities":[{"entity_type":"person","value":"Anna","score":0.9}]}"#,
    )
    .match_body(Matcher::Json(json!({
        "content": "Anna kommt morgen",
        "entity_types": ["person"],
        "lang": "de"
    })))
    .create_async()
    .await;
    let unified = json_mock(
        &mut server,
        "POST",
        "/v1/extract",
        200,
        r#"{"entities":[],"provider":"openai","duration_ms":1.5}"#,
    )
    .match_body(Matcher::Json(json!({
        "text": "Anna kommt morgen",
        "lang": "de",
        "extractor_override": {"provider": "openai", "model": "gpt-4o-mini"}
    })))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .extract_entities_with_lang("Anna kommt morgen", Some(vec!["person".into()]), Some("de"))
        .await
        .unwrap();
    assert_eq!(r.entities[0].value, "Anna");
    client
        .extract_text_with_lang(
            "Anna kommt morgen",
            None,
            Some("openai"),
            Some("gpt-4o-mini"),
            Some("de"),
        )
        .await
        .unwrap();
    ner.assert_async().await;
    unified.assert_async().await;
}

// ============================================================================
// Namespace config: PUT replaces, PATCH merges (TRACKER K34)
// ============================================================================

#[tokio::test]
async fn put_namespace_entity_config_clears_entity_types() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "PUT",
        "/v1/namespaces/ns/config",
        200,
        r#"{"namespace":"ns","extract_entities":true,"entity_types":[]}"#,
    )
    .match_body(Matcher::Json(
        json!({"extract_entities": true, "entity_types": []}),
    ))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let cfg = client
        .put_namespace_entity_config(
            "ns",
            NamespaceNerConfig {
                extract_entities: true,
                entity_types: None,
            },
        )
        .await
        .unwrap();
    assert!(cfg.extract_entities);
    assert!(cfg.entity_types.is_empty());
    m.assert_async().await;
}

#[tokio::test]
async fn patch_with_an_empty_list_clears_and_none_keeps() {
    let mut server = mockito::Server::new_async().await;
    let clear = json_mock(
        &mut server,
        "PATCH",
        "/v1/namespaces/ns/config",
        200,
        r#"{"namespace":"ns","extract_entities":true,"entity_types":[]}"#,
    )
    .match_body(Matcher::Json(
        json!({"extract_entities": true, "entity_types": []}),
    ))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client
        .configure_namespace_ner(
            "ns",
            NamespaceNerConfig {
                extract_entities: true,
                entity_types: Some(vec![]),
            },
        )
        .await
        .unwrap();
    clear.assert_async().await;

    // `None` must not put entity_types on the wire (PATCH merges: unchanged).
    let body = serde_json::to_value(NamespaceNerConfig {
        extract_entities: true,
        entity_types: None,
    })
    .unwrap();
    assert_eq!(body, json!({"extract_entities": true}));
}

// ============================================================================
// Capabilities (v0.12 document, with the sections added since the first cut)
// ============================================================================

const CAPS: &str = r#"{
  "capabilities_version": 1, "server_version": "0.12.0", "api_versions": ["v1"],
  "default_model": "colbert-small",
  "models": [
    {"name": "bge-large", "aliases": [], "dimension": 1024, "max_seq_length": 512,
     "effective_max_seq_length": 512, "active": false, "modality": "text"},
    {"name": "colbert-small", "aliases": ["answerai-colbert-small"], "dimension": 96,
     "max_seq_length": 512, "effective_max_seq_length": 512, "active": true, "modality": "text"}
  ],
  "index_kinds": ["hnsw", "ivfpq"], "vector_index_kinds": ["hnsw", "ivfpq"],
  "live_vector_index_kinds": ["hnsw"], "distance_metrics": ["cosine", "euclidean", "dot_product"],
  "search_mode": "rabitq", "search_modes_accepted": "hybrid, binary, float, scalar (alias sq), rabitq",
  "fulltext_language": "en", "on_disk_format_version": 1,
  "records": {"enabled": true, "representation_kinds": ["dense", "token_multivector", "patch_multivector"],
              "dtypes": ["f32", "f16", "i8"], "max_representations": 8, "max_vectors": 4096, "max_bytes": 8388608},
  "scoring": {"strategy": "late-interaction", "strategies_accepted": "single-vector, late-interaction",
    "late_interaction": {"enabled": true, "model_supported": true, "lane": "text",
      "token_slot": "colbert", "fde_slot": "colbert.fde", "fde_k_sim": 4, "fde_d_proj": 16,
      "fde_reps": 10, "fde_dim": 10240, "candidates": 200}},
  "attachments": {"enabled": true, "max_bytes": 26214400,
    "transcription": {"model": "whisper-tiny.en", "models": ["whisper-tiny.en"],
      "media_types": ["audio/wav", "audio/x-wav"], "languages": ["en"], "sample_rate_hz": 16000}},
  "vision": {"enabled": false, "model": "colmodernvbert", "models": ["colmodernvbert"],
    "media_types": ["image/png"], "dimension": 128, "patch_slot": "patch",
    "patch_fde_slot": "patch.fde", "tile_size": 512, "longest_edge": 2048,
    "image_seq_len": 64, "max_tiles": 17},
  "query_languages": ["en", "de", "fr", "es", "it", "pt", "nl"],
  "reembed_pending": false, "unreadable_records": 2,
  "late_interaction_stats": {"searches": 10, "reranked": 9, "future_counter": 1.5}
}"#;

#[test]
fn capabilities_v012_sections_are_typed() {
    let caps: ServerCapabilities = serde_json::from_str(CAPS).unwrap();
    assert_eq!(caps.default_model, EmbeddingModel::ColbertSmall);
    assert!(caps.default_model.is_known());
    assert!(caps.active_model().is_some());
    assert_eq!(caps.scoring.strategy, "late-interaction");
    assert!(caps.scoring.late_interaction.enabled);
    assert!(caps.scoring.late_interaction.model_supported);
    assert_eq!(caps.scoring.late_interaction.lane, "text");
    assert_eq!(caps.scoring.late_interaction.fde_slot, "colbert.fde");
    assert_eq!(caps.scoring.late_interaction.candidates, 200);
    assert!(caps.scoring.late_interaction.extra.contains_key("fde_dim"));
    assert!(caps.attachments.enabled);
    assert_eq!(caps.attachments.max_bytes, 26_214_400);
    assert_eq!(caps.attachments.transcription.model, "whisper-tiny.en");
    assert_eq!(caps.attachments.transcription.languages, vec!["en"]);
    assert_eq!(caps.attachments.transcription.sample_rate_hz, 16_000);
    assert!(!caps.vision.enabled);
    assert_eq!(caps.vision.dimension, 128);
    assert_eq!(caps.vision.patch_slot, "patch");
    assert_eq!(caps.unreadable_records, 2);
    // A counter that is not an integer must not fail the whole document.
    assert_eq!(caps.late_interaction_stats["reranked"], 9);
    assert_eq!(caps.late_interaction_stats["future_counter"], 1.5);
}

#[test]
fn a_v0110_capabilities_document_without_the_new_sections_still_parses() {
    let caps: ServerCapabilities =
        serde_json::from_str(r#"{"capabilities_version":1,"server_version":"0.12.0"}"#).unwrap();
    assert!(!caps.attachments.enabled);
    assert!(!caps.vision.enabled);
    assert_eq!(caps.scoring.strategy, "");
    assert_eq!(caps.unreadable_records, 0);
}

#[tokio::test]
async fn capabilities_endpoint_returns_the_typed_document() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(&mut server, "GET", "/v1/capabilities", 200, CAPS)
        .expect(1)
        .create_async()
        .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let caps = client.capabilities().await.unwrap();
    assert!(caps.attachments.enabled);
    // Cached: no second request.
    let again = client.capabilities().await.unwrap();
    assert!(std::sync::Arc::ptr_eq(&caps, &again));
    m.assert_async().await;
}

#[test]
fn colbert_small_is_a_known_model() {
    assert_eq!(
        EmbeddingModel::from("colbert-small"),
        EmbeddingModel::ColbertSmall
    );
    assert_eq!(
        serde_json::to_string(&EmbeddingModel::ColbertSmall).unwrap(),
        r#""colbert-small""#
    );
    assert!(EmbeddingModel::known().contains(&EmbeddingModel::ColbertSmall));
}

// ============================================================================
// Knowledge graph contract (shapes the v0.12.0 / v0.11.108 server sends)
// ============================================================================

#[tokio::test]
async fn memory_link_sends_agent_id_and_parses_the_flat_answer() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/memories/mem-a/links",
        200,
        r#"{"from_id":"mem-a","to_id":"mem-b","edge_type":"linked_by"}"#,
    )
    .match_body(Matcher::Json(json!({
        "target_id": "mem-b",
        "agent_id": "agent-1",
        "label": "follow-up"
    })))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .memory_link("agent-1", "mem-a", "mem-b", Some("follow-up"))
        .await
        .unwrap();
    assert_eq!(r.from_id, "mem-a");
    assert_eq!(r.to_id, "mem-b");
    assert_eq!(r.edge_type, EdgeType::LinkedBy);
    assert_eq!(r.edge.source_id, "mem-a");
    assert_eq!(r.edge.target_id, "mem-b");
    assert_eq!(r.edge.weight, 1.0);
    m.assert_async().await;
}

#[tokio::test]
async fn memory_link_without_label_omits_it() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/memories/mem-a/links",
        200,
        r#"{"from_id":"mem-a","to_id":"mem-b","edge_type":"linked_by"}"#,
    )
    .match_body(Matcher::Json(
        json!({"target_id": "mem-b", "agent_id": "agent-1"}),
    ))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    client
        .memory_link("agent-1", "mem-a", "mem-b", None)
        .await
        .unwrap();
    m.assert_async().await;
}

#[test]
fn graph_link_response_accepts_the_nested_edge_shape() {
    let r: dakera_client::GraphLinkResponse = serde_json::from_value(json!({
        "edge": {"id": "e1", "source_id": "a", "target_id": "b", "edge_type": "linked_by",
                 "weight": 0.5, "created_at": 7}
    }))
    .unwrap();
    assert_eq!(r.edge.id, "e1");
    assert_eq!(r.from_id, "a");
    assert_eq!(r.to_id, "b");
    assert_eq!(r.edge.weight, 0.5);
}

#[tokio::test]
async fn memory_graph_parses_nodes_with_their_edges() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/memories/mem-a/graph",
        200,
        r#"{"root_id":"mem-a","depth":2,"node_count":2,"nodes":[
            {"memory_id":"mem-a","depth":0,"edges":[]},
            {"memory_id":"mem-b","depth":1,"edges":[
                {"from_id":"mem-a","to_id":"mem-b","edge_type":"supersedes","weight":0.97,"created_at":1790874016}
            ]}
        ]}"#,
    )
    .match_query(Matcher::UrlEncoded("depth".into(), "2".into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let g = client
        .memory_graph("mem-a", dakera_client::GraphOptions::new().depth(2))
        .await
        .unwrap();
    assert_eq!(g.node_count, 2);
    assert_eq!(g.nodes[1].memory_id, "mem-b");
    assert_eq!(g.nodes[1].edges.len(), 1);
    assert_eq!(g.edges.len(), 1, "node edges are collected on the graph");
    assert_eq!(g.edges[0].source_id, "mem-a");
    assert_eq!(g.edges[0].target_id, "mem-b");
    assert_eq!(g.edges[0].edge_type, EdgeType::Supersedes);
    assert_eq!(g.edges[0].created_at, 1_790_874_016);
    assert!(g.edges[0].id.is_empty());
    m.assert_async().await;
}

#[test]
fn unknown_edge_types_do_not_break_parsing() {
    let e: dakera_client::GraphEdge = serde_json::from_value(json!({
        "from_id": "a", "to_id": "b", "edge_type": "some_future_type", "weight": 1.0, "created_at": 1
    }))
    .unwrap();
    assert_eq!(e.edge_type, EdgeType::Unknown);
}

#[tokio::test]
async fn memory_path_sends_to_and_parses_hop_count() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/memories/mem-a/path",
        200,
        r#"{"from_id":"mem-a","to_id":"mem-c","path":["mem-a","mem-b","mem-c"],"hop_count":2}"#,
    )
    .match_query(Matcher::UrlEncoded("to".into(), "mem-c".into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let p = client.memory_path("mem-a", "mem-c").await.unwrap();
    assert_eq!(p.source_id, "mem-a");
    assert_eq!(p.target_id, "mem-c");
    assert_eq!(p.hops, 2);
    assert_eq!(p.path.len(), 3);
    assert!(p.edges.is_empty());
    m.assert_async().await;
}

#[tokio::test]
async fn agent_graph_export_parses_the_json_answer() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/agents/agent-1/graph/export",
        200,
        r#"{"agent_id":"agent-1","namespace":"_dakera_agent_agent-1","node_count":2,"edge_count":1,
            "edges":[{"from_id":"mem-a","to_id":"mem-b","edge_type":"linked_by","weight":1.0,"created_at":5}]}"#,
    )
    .match_query(Matcher::Any)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let x = client.agent_graph_export("agent-1", "json").await.unwrap();
    assert_eq!(x.namespace, "_dakera_agent_agent-1");
    assert_eq!(x.format, "json");
    assert_eq!(x.node_count, 2);
    assert_eq!(x.edge_count, 1);
    assert_eq!(x.edges[0].source_id, "mem-a");
    assert_eq!(x.edges[0].edge_type, EdgeType::LinkedBy);
    m.assert_async().await;
}

#[tokio::test]
async fn knowledge_query_parses_server_edges() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/knowledge/query",
        200,
        r#"{"agent_id":"agent-1","node_count":2,"edge_count":1,
            "edges":[{"from_id":"mem-a","to_id":"mem-b","edge_type":"related_to","weight":0.91,"created_at":5}]}"#,
    )
    .match_query(Matcher::UrlEncoded("agent_id".into(), "agent-1".into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let q = client
        .knowledge_query("agent-1", None, None, None, None, None)
        .await
        .unwrap();
    assert_eq!(q.edge_count, 1);
    assert_eq!(q.edges[0].source_id, "mem-a");
    assert_eq!(q.edges[0].target_id, "mem-b");
    assert_eq!(q.edges[0].edge_type, EdgeType::RelatedTo);
    m.assert_async().await;
}

#[tokio::test]
async fn memory_entities_fills_the_memory_id() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/memory/entities/mem-a",
        200,
        r#"{"entities":[{"entity_type":"person","value":"Anna","score":0.9}],"count":1}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.memory_entities("mem-a").await.unwrap();
    assert_eq!(r.memory_id, "mem-a");
    assert_eq!(r.count, 1);
    assert_eq!(r.entities[0].value, "Anna");
    m.assert_async().await;
}

#[tokio::test]
async fn search_memories_reads_total_count() {
    // POST /v1/memory/search answers {memories, total_count, rerank_report, effective_top_k}.
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/memory/search",
        200,
        r#"{"memories":[],"total_count":17,"rerank_report":{"requested":false,"applied":false},"effective_top_k":5}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .search_memories(RecallRequest::new("agent-1", "q"))
        .await
        .unwrap();
    assert_eq!(r.total_found, 17);
    m.assert_async().await;
}

#[tokio::test]
async fn memory_graph_filters_types_client_side_and_does_not_send_them() {
    let mut server = mockito::Server::new_async().await;
    // The query must be exactly `depth=2`: no `types` parameter.
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/memories/mem-a/graph",
        200,
        r#"{"root_id":"mem-a","depth":2,"node_count":3,"nodes":[
            {"memory_id":"mem-a","depth":0,"edges":[]},
            {"memory_id":"mem-b","depth":1,"edges":[
                {"from_id":"mem-a","to_id":"mem-b","edge_type":"linked_by","weight":1.0,"created_at":1}
            ]},
            {"memory_id":"mem-c","depth":1,"edges":[
                {"from_id":"mem-a","to_id":"mem-c","edge_type":"related_to","weight":0.9,"created_at":2}
            ]}
        ]}"#,
    )
    .match_query(Matcher::Exact("depth=2".into()))
    .expect(2)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();

    let g = client
        .memory_graph(
            "mem-a",
            dakera_client::GraphOptions::new()
                .depth(2)
                .types(vec![EdgeType::RelatedTo]),
        )
        .await
        .unwrap();
    assert_eq!(g.nodes.len(), 3, "nodes are kept");
    assert_eq!(g.edges.len(), 1);
    assert_eq!(g.edges[0].target_id, "mem-c");
    assert!(g.nodes[1].edges.is_empty(), "linked_by edge filtered out");
    assert_eq!(g.nodes[2].edges.len(), 1);

    // An empty filter keeps everything.
    let all = client
        .memory_graph(
            "mem-a",
            dakera_client::GraphOptions::new().depth(2).types(vec![]),
        )
        .await
        .unwrap();
    assert_eq!(all.edges.len(), 2);
    m.assert_async().await;
}

// ============================================================================
// Ops / analytics answers as the v0.12.0 server sends them (captured live)
// ============================================================================

#[tokio::test]
async fn fulltext_stats_reads_unique_terms() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/namespaces/ns/fulltext/stats",
        200,
        r#"{"document_count":1,"unique_terms":3,"avg_doc_length":3.0}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let s = client.fulltext_stats("ns").await.unwrap();
    assert_eq!(s.document_count, 1);
    assert_eq!(s.term_count, 3);
    assert_eq!(s.avg_doc_length, 3.0);
    m.assert_async().await;
}

#[tokio::test]
async fn get_kpis_reads_the_nested_answer() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/kpis",
        200,
        r#"{"timestamp":1790876073,"kpis":{"recall_latency_p50_ms":0.0,"recall_latency_p99_ms":0.0,
            "store_latency_p50_ms":92.677484,"api_error_rate_5xx_pct":25.0,"active_agents_count":1,
            "session_count_weekly":4,"cross_agent_network_node_count":0,"memory_retention_7d_pct":100.0}}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let k = client.get_kpis().await.unwrap();
    assert_eq!(k.timestamp, 1_790_876_073);
    assert_eq!(k.session_count_week, 4);
    assert_eq!(k.active_agents_count, 1);
    assert!((k.store_latency_p50_ms - 92.677484).abs() < 1e-9);
    m.assert_async().await;
}

#[tokio::test]
async fn cluster_replication_reads_the_server_answer() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/admin/cluster/replication",
        200,
        r#"{"enabled":true,"replication_factor":3,"healthy_replicas":2,"degraded_replicas":1,
            "unhealthy_replicas":0,"replication_lag":[{"node_id":"n2","lag_ms":40,"pending_ops":7,
            "last_sync":1790876000}],"health":"degraded"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.admin_cluster_replication().await.unwrap();
    assert!(r.enabled);
    assert_eq!(r.total_nodes, 3);
    assert_eq!(r.health, "degraded");
    assert_eq!(r.replication_lag[0].pending_ops, 7);
    assert_eq!(r.replication_lag[0].last_sync, 1_790_876_000);
    m.assert_async().await;
}

#[tokio::test]
async fn admin_list_slow_queries_returns_the_entries() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/admin/slow-queries",
        200,
        r#"{"queries":[{"namespace":"ns","query_type":"vector","duration_ms":250.0}],
            "threshold_ms":100.0,"total":1}"#,
    )
    .match_query(Matcher::Any)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let q = client
        .admin_list_slow_queries(None, None, Some(5))
        .await
        .unwrap();
    assert_eq!(q.len(), 1);
    assert_eq!(q[0]["namespace"], "ns");
    m.assert_async().await;
}

#[tokio::test]
async fn analytics_latency_reads_the_histogram() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/analytics/latency",
        200,
        r#"{"buckets":[{"lower_ms":0.0,"upper_ms":1.0,"count":61,"percentage":76.25},
            {"lower_ms":1000.0,"upper_ms":null,"count":0,"percentage":0.0}],
            "avg_ms":8.2,"p50_ms":0.4,"p95_ms":91.4,"p99_ms":104.4,"period":"24h"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let l = client.analytics_latency(None, None).await.unwrap();
    assert_eq!(l.period, "24h");
    assert_eq!(l.buckets.len(), 2);
    assert_eq!(l.buckets[0].count, 61);
    assert_eq!(l.buckets[1].upper_ms, None);
    assert!((l.p99_ms - 104.4).abs() < 1e-9);
    m.assert_async().await;
}

#[tokio::test]
async fn analytics_throughput_reads_the_rates() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/analytics/throughput",
        200,
        r#"{"queries_per_second":1.5,"inserts_per_second":0.5,"deletes_per_second":0.25,
            "data_points":[{"timestamp":1790876054,"queries_per_second":1.0,"inserts_per_second":0.0,
            "deletes_per_second":0.0}],"period":"24h"}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let t = client.analytics_throughput(None, None).await.unwrap();
    assert_eq!(t.queries_per_second, 1.5);
    assert_eq!(t.operations_per_second, 2.25);
    assert_eq!(t.data_points.len(), 1);
    m.assert_async().await;
}

#[tokio::test]
async fn analytics_storage_reads_the_breakdown() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/analytics/storage",
        200,
        r#"{"total_bytes":10548,"index_bytes":1640,"vector_bytes":8204,"metadata_bytes":320,
            "fulltext_bytes":384,"namespace_breakdown":[
            {"namespace":"_dakera_agent_a","total_bytes":8320,"vector_count":2,"dimension":1024},
            {"namespace":"pfx","total_bytes":0,"vector_count":0}]}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let s = client.analytics_storage(None).await.unwrap();
    assert_eq!(s.data_bytes, 8204 + 320 + 384);
    assert_eq!(s.namespace_breakdown.len(), 2);
    assert_eq!(s.by_namespace["_dakera_agent_a"].bytes, 8320);
    assert_eq!(s.by_namespace["pfx"].vector_count, 0);
    assert_eq!(s.namespace_breakdown[1].dimension, None);
    m.assert_async().await;
}

// ============================================================================
// Agents / feedback answers as the v0.12.0 server sends them (captured live)
// ============================================================================

#[tokio::test]
async fn agent_stats_reads_numeric_timestamps() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/agents/a1/stats",
        200,
        r#"{"agent_id":"a1","total_memories":2,"memories_by_type":{"episodic":2},"total_sessions":0,
            "active_sessions":0,"avg_importance":0.65,"oldest_memory_at":1790876257,
            "newest_memory_at":1790876258}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let s = client.agent_stats("a1").await.unwrap();
    assert_eq!(s.total_memories, 2);
    assert_eq!(s.oldest_memory_at.as_deref(), Some("1790876257"));
    assert_eq!(s.newest_memory_at.as_deref(), Some("1790876258"));
    m.assert_async().await;
}

#[tokio::test]
async fn wake_up_reads_numeric_memory_timestamps() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/agents/a1/wake-up",
        200,
        r#"{"agent_id":"a1","memories":[{"id":"m1","memory_type":"episodic","content":"Anna lives in Berlin",
            "agent_id":"a1","importance":0.8,"tags":[],"created_at":1790876257,
            "last_accessed_at":1790876257,"access_count":0}],"total_available":2}"#,
    )
    .match_query(Matcher::Any)
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let w = client.wake_up("a1", Some(3), None).await.unwrap();
    assert_eq!(w.total_available, 2);
    assert_eq!(w.memories[0].created_at.as_deref(), Some("1790876257"));
    m.assert_async().await;
}

#[tokio::test]
async fn consolidate_agent_reads_the_skipped_answer() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "POST",
        "/v1/agents/a1/consolidate",
        200,
        r#"{"agent_id":"a1","reason":"consolidation disabled for this agent","skipped":true}"#,
    )
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client.consolidate_agent("a1").await.unwrap();
    assert_eq!(r.skipped, Some(true));
    assert_eq!(r.memories_scanned, 0);
    assert!(r.reason.unwrap().contains("disabled"));
    m.assert_async().await;
}

#[tokio::test]
async fn patch_memory_importance_reads_the_updated_memory() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "PATCH",
        "/v1/memories/m1/importance",
        200,
        r#"{"id":"m1","memory_type":"episodic","content":"Anna lives in Berlin","agent_id":"a1",
            "importance":0.7,"tags":[],"created_at":1790876257,"last_accessed_at":1790876257,"access_count":0}"#,
    )
    .match_body(Matcher::PartialJson(json!({"agent_id": "a1"})))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let r = client
        .patch_memory_importance("m1", "a1", 0.7)
        .await
        .unwrap();
    assert_eq!(r.memory_id, "m1");
    assert!((r.new_importance - 0.7).abs() < 1e-6);
    assert_eq!(r.agent_id, "a1");
    m.assert_async().await;
}

#[tokio::test]
async fn feedback_history_sends_agent_id() {
    let mut server = mockito::Server::new_async().await;
    let m = json_mock(
        &mut server,
        "GET",
        "/v1/memories/m1/feedback",
        200,
        r#"{"memory_id":"m1","entries":[{"signal":"upvote","timestamp":1790876274,
            "old_importance":0.5,"new_importance":0.575}]}"#,
    )
    .match_query(Matcher::UrlEncoded("agent_id".into(), "a1".into()))
    .create_async()
    .await;
    let client = DakeraClient::new(server.url()).unwrap();
    let h = client
        .get_memory_feedback_history("m1", "a1")
        .await
        .unwrap();
    assert_eq!(h.entries.len(), 1);
    m.assert_async().await;
}
