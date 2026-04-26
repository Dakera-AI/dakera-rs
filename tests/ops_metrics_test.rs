//! Integration tests for the INFRA-3 Prometheus metrics endpoint.
//!
//! Tests `DakeraClient::ops_metrics()` — `GET /v1/ops/metrics` (Admin scope).

use dakera_client::DakeraClient;

const PROMETHEUS_TEXT: &str = "# HELP dakera_memory_store_total Total memory store operations\n\
# TYPE dakera_memory_store_total counter\n\
dakera_memory_store_total 42\n\
# HELP dakera_memory_count Current stored memory count\n\
# TYPE dakera_memory_count gauge\n\
dakera_memory_count 1024\n";

#[tokio::test]
async fn test_ops_metrics_returns_prometheus_text() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/ops/metrics")
        .with_status(200)
        .with_header("content-type", "text/plain; version=0.0.4; charset=utf-8")
        .with_body(PROMETHEUS_TEXT)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let result = client.ops_metrics().await.unwrap();

    assert!(result.contains("dakera_memory_store_total"));
    assert!(result.contains("dakera_memory_count 1024"));
    mock.assert_async().await;
}

#[tokio::test]
async fn test_ops_metrics_uses_get_method() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/ops/metrics")
        .with_status(200)
        .with_header("content-type", "text/plain")
        .with_body("# empty\n")
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    client.ops_metrics().await.unwrap();

    mock.assert_async().await;
}

#[tokio::test]
async fn test_ops_metrics_authorization_error_on_403() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/ops/metrics")
        .with_status(403)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":"Admin scope required","code":"AUTHORIZATION_ERROR"}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(server.url()).unwrap();
    let err = client.ops_metrics().await.unwrap_err();

    assert!(
        matches!(err, dakera_client::ClientError::Authorization { .. }),
        "expected Authorization error, got: {err:?}"
    );
    mock.assert_async().await;
}
