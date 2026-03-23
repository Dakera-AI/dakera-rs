# dakera-rs

[![CI](https://github.com/dakera-ai/dakera-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/dakera-ai/dakera-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/dakera-client.svg)](https://crates.io/crates/dakera-client)
[![docs.rs](https://docs.rs/dakera-client/badge.svg)](https://docs.rs/dakera-client)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust: 1.70+](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)

Rust client SDK for [Dakera](https://dakera.ai) — an AI agent memory platform.

## Features

- **HTTP and gRPC transports** — choose the protocol that fits your workload
- **Async/await** — built on Tokio for non-blocking I/O
- **Type-safe API** — fully typed request/response models with serde
- **Memory management** — store, recall, and forget agent memories
- **Knowledge graphs** — build and query knowledge graphs from memories
- **Agent management** — list agents, sessions, and statistics
- **Vector operations** — upsert, query, delete, batch query, hybrid search
- **Full-text search** — BM25-based search with hybrid vector+text support
- **Admin & analytics** — cluster management, cache control, backups, quotas
- **gRPC connection pooling** — HTTP/2 multiplexing with round-robin load balancing

## Installation

```sh
cargo add dakera-client
```

Or add to your `Cargo.toml`:

```toml
[dependencies]
dakera-client = "0.3"
```

## Quick Start

```rust
use dakera_client::{DakeraClient, UpsertRequest, QueryRequest, Vector};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DakeraClient::new("http://localhost:3000")?;

    // Check health
    let health = client.health().await?;
    println!("Server healthy: {}", health.healthy);

    // Upsert vectors
    let request = UpsertRequest {
        vectors: vec![
            Vector {
                id: "vec1".to_string(),
                values: vec![0.1, 0.2, 0.3, 0.4],
                metadata: None,
            },
        ],
    };
    client.upsert("my-namespace", request).await?;

    // Query for similar vectors
    let query = QueryRequest {
        vector: vec![0.1, 0.2, 0.3, 0.4],
        top_k: 10,
        filter: None,
        include_metadata: true,
    };
    let results = client.query("my-namespace", query).await?;

    for match_ in results.matches {
        println!("ID: {}, Score: {}", match_.id, match_.score);
    }

    Ok(())
}
```

## Agent Memory

```rust
use dakera_client::{DakeraClient, memory::{StoreMemoryRequest, RecallRequest}};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DakeraClient::new("http://localhost:3000")?;

    // Store a memory
    let request = StoreMemoryRequest::new("agent-1", "The user prefers dark mode")
        .with_importance(0.8)
        .with_tags(vec!["preferences".to_string()]);
    let stored = client.store_memory(request).await?;
    println!("Stored: {}", stored.memory_id);

    // Recall memories
    let request = RecallRequest::new("agent-1", "user preferences")
        .with_top_k(5);
    let recalled = client.recall(request).await?;
    for memory in recalled.memories {
        println!("{}: {} (score: {})", memory.id, memory.content, memory.score);
    }

    Ok(())
}
```

## gRPC Client

Enable the `grpc` feature for high-performance gRPC communication with connection pooling:

```toml
[dependencies]
dakera-client = { version = "0.3", features = ["grpc"] }
```

```rust
use dakera_client::grpc::{GrpcClient, GrpcClientConfig, GrpcConnectionPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Single client with HTTP/2 multiplexing
    let config = GrpcClientConfig::default()
        .with_endpoint("http://localhost:50051")
        .with_concurrency_limit(100);
    let client = GrpcClient::connect(config).await?;

    // Or use a connection pool for higher throughput
    let pool = GrpcConnectionPool::new(GrpcClientConfig::default(), 4).await?;
    let client = pool.get();

    let health = client.health().await?;
    println!("Healthy: {}", health.healthy);

    Ok(())
}
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `http-client` | Yes | HTTP client via reqwest with rustls |
| `grpc` | No | gRPC client with connection pooling via tonic |
| `full` | No | Enables both `http-client` and `grpc` |

## Related Repositories

| Repository | Description |
|------------|-------------|
| [dakera](https://github.com/dakera-ai/dakera) | Core AI agent memory engine |
| [dakera-py](https://github.com/dakera-ai/dakera-py) | Python SDK |
| [dakera-js](https://github.com/dakera-ai/dakera-js) | JavaScript/TypeScript SDK |
| [dakera-go](https://github.com/dakera-ai/dakera-go) | Go SDK |
| [dakera-mcp](https://github.com/dakera-ai/dakera-mcp) | MCP server for AI agents |
| [dakera-cli](https://github.com/dakera-ai/dakera-cli) | Command-line interface |
| [dakera-dashboard](https://github.com/dakera-ai/dakera-dashboard) | Admin web UI |
| [dakera-deploy](https://github.com/dakera-ai/dakera-deploy) | Deployment configurations |
| [dakera-docs](https://github.com/dakera-ai/dakera-docs) | Documentation and API reference |

## License

MIT License - see [LICENSE](LICENSE) for details.
