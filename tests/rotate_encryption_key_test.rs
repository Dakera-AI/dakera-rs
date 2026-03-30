//! Integration tests for SEC-3 AES-256-GCM encryption key rotation endpoint.
//!
//! Tests `DakeraClient::rotate_encryption_key()` — `POST /v1/admin/encryption/rotate-key`.

use dakera_client::DakeraClient;

#[tokio::test]
async fn test_rotate_encryption_key_returns_response() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/admin/encryption/rotate-key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"rotated":42,"skipped":3,"namespaces":["ns-a","ns-b"]}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(&server.url()).unwrap();
    let result = client
        .rotate_encryption_key("new-secret-passphrase", None)
        .await
        .unwrap();

    assert_eq!(result.rotated, 42);
    assert_eq!(result.skipped, 3);
    assert_eq!(result.namespaces, vec!["ns-a", "ns-b"]);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_rotate_encryption_key_with_namespace() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/admin/encryption/rotate-key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"rotated":5,"skipped":0,"namespaces":["my-ns"]}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(&server.url()).unwrap();
    let result = client
        .rotate_encryption_key("new-key", Some("my-ns"))
        .await
        .unwrap();

    assert_eq!(result.rotated, 5);
    assert_eq!(result.namespaces, vec!["my-ns"]);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_rotate_encryption_key_empty_response_defaults() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/v1/admin/encryption/rotate-key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"rotated":0,"skipped":0,"namespaces":[]}"#)
        .create_async()
        .await;

    let client = DakeraClient::new(&server.url()).unwrap();
    let result = client.rotate_encryption_key("any-key", None).await.unwrap();

    assert_eq!(result.rotated, 0);
    assert_eq!(result.skipped, 0);
    assert!(result.namespaces.is_empty());
}
