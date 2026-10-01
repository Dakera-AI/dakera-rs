//! The gRPC client must send the API key (`x-api-key`) on every RPC.
//!
//! A real tonic server (h2c on a local port) stands in for the Dakera server:
//! it records the path and the call metadata of every request and answers
//! `UNAUTHENTICATED` as a trailers-only response, which also exercises the
//! client's `grpc-status` mapping.
#![cfg(feature = "grpc")]

use std::convert::Infallible;
use std::future::{ready, Ready};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use dakera_client::grpc::{GrpcClient, GrpcClientConfig};
use dakera_client::{ClientError, Vector};

type Seen = Arc<Mutex<Vec<(String, Option<String>)>>>;

#[derive(Clone, Default)]
struct Probe {
    seen: Seen,
}

impl tonic::server::NamedService for Probe {
    const NAME: &'static str = "dakera.v1.VectorService";
}

impl tower::Service<http::Request<tonic::body::Body>> for Probe {
    type Response = http::Response<tonic::body::Body>;
    type Error = Infallible;
    type Future = Ready<Result<Self::Response, Infallible>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<tonic::body::Body>) -> Self::Future {
        let key = req
            .headers()
            .get("x-api-key")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        self.seen
            .lock()
            .unwrap()
            .push((req.uri().path().to_string(), key));
        let response = http::Response::builder()
            .status(200)
            .header("content-type", "application/grpc")
            .header("grpc-status", "16")
            .header("grpc-message", "API key required")
            .body(tonic::body::Body::empty())
            .unwrap();
        ready(Ok(response))
    }
}

async fn start() -> (String, Seen) {
    let probe = Probe::default();
    let seen = probe.seen.clone();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    tokio::spawn(async move {
        let _ = tonic::transport::Server::builder()
            .add_service(probe)
            .serve(addr)
            .await;
    });
    (format!("http://127.0.0.1:{port}"), seen)
}

async fn connect(config: GrpcClientConfig) -> GrpcClient {
    for _ in 0..100 {
        if let Ok(client) = GrpcClient::connect(config.clone()).await {
            return client;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the test gRPC server did not come up");
}

#[tokio::test]
async fn every_rpc_sends_the_api_key_and_uses_the_server_service_path() {
    let (endpoint, seen) = start().await;
    let client = connect(GrpcClientConfig::new(endpoint).with_api_key("dk-test")).await;

    let results: Vec<Result<(), ClientError>> = vec![
        client.health().await.map(|_| ()),
        client.get_namespace("ns").await.map(|_| ()),
        client.delete_namespace("ns").await.map(|_| ()),
        client
            .upsert("ns", vec![Vector::new("v1", vec![0.5, 0.5])])
            .await
            .map(|_| ()),
        client
            .query("ns", vec![0.5, 0.5], 3, "cosine", false, false)
            .await
            .map(|_| ()),
        client
            .delete_vectors("ns", vec!["v1".to_string()])
            .await
            .map(|_| ()),
        client
            .warm_cache("ns", vec!["v1".to_string()])
            .await
            .map(|_| ()),
    ];

    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 7, "one request per RPC: {seen:?}");
    let mut paths: Vec<&str> = seen.iter().map(|(p, _)| p.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        [
            "/dakera.v1.VectorService/DeleteNamespace",
            "/dakera.v1.VectorService/DeleteVectors",
            "/dakera.v1.VectorService/GetNamespace",
            "/dakera.v1.VectorService/Health",
            "/dakera.v1.VectorService/Query",
            "/dakera.v1.VectorService/Upsert",
            "/dakera.v1.VectorService/WarmCache",
        ]
    );
    for (path, key) in &seen {
        assert_eq!(key.as_deref(), Some("dk-test"), "{path} sent no API key");
    }

    // The trailers-only UNAUTHENTICATED answer is an auth error, not "Response too short".
    for result in results {
        let err = result.unwrap_err();
        assert!(err.is_auth_error(), "{err:?}");
        assert!(err.to_string().contains("UNAUTHENTICATED"), "{err}");
    }
}
