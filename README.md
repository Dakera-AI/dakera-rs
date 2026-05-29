<p align="center">
  <img src="https://github.com/dakera-ai.png" alt="Dakera AI" width="80" />
</p>

<h1 align="center">dakera-rs</h1>

<p align="center">
  Rust client for <a href="https://dakera.ai">Dakera AI</a> — the memory engine for AI agents
</p>

<p align="center">
  <a href="https://github.com/Dakera-AI/dakera-rs/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/Dakera-AI/dakera-rs/actions/workflows/ci.yml/badge.svg" /></a>
  <a href="https://crates.io/crates/dakera-client"><img alt="Crate" src="https://img.shields.io/crates/v/dakera-client?logo=rust" /></a>
  <a href="https://crates.io/crates/dakera-client"><img alt="Downloads" src="https://img.shields.io/crates/d/dakera-client" /></a>
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/github/license/Dakera-AI/dakera-rs" /></a>
  <a href="https://docs.rs/dakera-client"><img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-dakera--client-blue?style=flat-square" /></a>
  <a href="https://dakera.ai/benchmark"><img alt="LoCoMo 88.2%" src="https://img.shields.io/badge/LoCoMo-88.2%25-22c55e?style=flat-square" /></a>
</p>

---

## Why Dakera?

| | Dakera | Others |
|---|---|---|
| **LoCoMo accuracy** | **88.2%** (1,540 Q standard eval) | 60–92% |
| **Deployment** | Single binary, Docker one-liner | External vector DB + embedding service required |
| **Embeddings** | Built-in — no OpenAI key needed | Requires external embedding API |
| **Search modes** | Vector · BM25 · Hybrid · Knowledge Graph | Usually one or two |
| **Transport** | HTTP (reqwest) + gRPC (tonic), zero-copy | HTTP only |

→ [Full benchmark results](https://dakera.ai/benchmark) · [dakera.ai](https://dakera.ai)

---

## Run Dakera

```bash
docker run -d \
  --name dakera \
  -p 3000:3000 \
  -e DAKERA_ROOT_API_KEY=dk-mykey \
  ghcr.io/dakera-ai/dakera:latest

curl http://localhost:3000/health  # → {"status":"ok"}
```

For persistent storage with Docker Compose:

```bash
curl -sSfL https://raw.githubusercontent.com/Dakera-AI/dakera-deploy/main/docker-compose.yml \
  -o docker-compose.yml
DAKERA_API_KEY=dk-mykey docker compose up -d
```

Full deployment guide (Docker Compose, Kubernetes, Helm): [dakera-deploy](https://github.com/Dakera-AI/dakera-deploy)

---

## Install

```toml
# Cargo.toml
[dependencies]
dakera-client = "0.11"
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

Feature flags:

| Feature | Default | Description |
|---|---|---|
| `http-client` | ✅ | Async HTTP via `reqwest` |
| `grpc` | — | gRPC transport with connection pooling via `tonic` |
| `full` | — | Both HTTP and gRPC |

For gRPC (lower latency in high-throughput workloads):

```toml
dakera-client = { version = "0.11", features = ["grpc"] }
```

---

## Quick Start

```rust
use dakera_client::{DakeraClient, StoreMemoryRequest, RecallRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DakeraClient::builder("http://localhost:3000")
        .api_key("dk-mykey")
        .build()?;

    // Store an agent memory
    let mem = client.store_memory(StoreMemoryRequest {
        agent_id: "my-agent".to_string(),
        content: "User prefers concise responses with code examples".to_string(),
        importance: Some(0.9),
        ..Default::default()
    }).await?;
    println!("Stored: {}", mem.memory_id);

    // Recall memories (semantic search)
    let response = client.recall(RecallRequest {
        agent_id: "my-agent".to_string(),
        query: "what does the user prefer?".to_string(),
        top_k: Some(5),
        ..Default::default()
    }).await?;
    for m in &response.memories {
        println!("[{:.2}] {}", m.importance, m.content);
    }

    // Upsert vectors
    client.upsert("my-namespace", dakera_client::UpsertRequest {
        vectors: vec![dakera_client::Vector {
            id: "vec1".to_string(),
            values: vec![0.1, 0.2, 0.3],
            metadata: None,
        }],
    }).await?;

    // Hybrid search (vector + BM25)
    let results = client.hybrid_search("my-namespace", "completed task", 5).await?;
    for r in &results.results {
        println!("{}: {:.3}", r.id, r.score);
    }

    Ok(())
}
```

---

## Features

- **Agent Memory** — store, recall, search, and forget memories with importance scoring
- **Sessions** — group memories by conversation with auto-consolidation on session end
- **Knowledge Graph** — traverse memory relationships, find paths, export graphs
- **Vector Search** — ANN queries with metadata filters and batch operations
- **Full-Text Search** — BM25 ranking with stemming and stop-word filtering
- **Hybrid Search** — combine vector similarity with keyword matching
- **Text Auto-Embedding** — server-side embedding generation (no local model needed)
- **Namespaces** — isolated vector stores per project, tenant, or use case
- **Feedback Loop** — upvote/downvote/flag memories to improve recall quality
- **Entity Extraction** — GLiNER NER for automatic entity detection
- **SSE Streaming** — Server-sent event subscriptions with auto-reconnect
- **Dual Transport** — HTTP (default) and gRPC with connection pooling
- **Typed Filters** — `filter::eq()`, `filter::gt()`, `filter::contains()` DSL
- **`From<T>` for FusionStrategy** — ergonomic enum conversions, idiomatic Rust API
- **Retry & Rate Limiting** — built-in exponential backoff and rate-limit header tracking
- **Builder Pattern** — fluent `DakeraClientBuilder` for configuration

---

## Connect to Dakera

```rust
use dakera_client::DakeraClient;

// Self-hosted
let client = DakeraClient::builder("http://your-server:3000")
    .api_key("your-key")
    .build()?;

// Cloud (early access)
let client = DakeraClient::builder("http://localhost:3000")
    .api_key("your-key")
    .build()?;

// With custom timeouts
let client = DakeraClient::builder("http://localhost:3000")
    .api_key("your-key")
    .timeout_secs(60)
    .max_retries(5)
    .build()?;
```

---

## Examples

See the [`examples/`](examples/) directory:

- [`basic.rs`](examples/basic.rs) — vectors, namespaces, queries, filters
- [`memory.rs`](examples/memory.rs) — store/recall memories, sessions, agent stats
- [`advanced.rs`](examples/advanced.rs) — text embedding, full-text, hybrid search, filter DSL

Run examples with:

```bash
cargo run --example basic
cargo run --example memory
cargo run --example advanced
```

---

## Resources

| | |
|---|---|
| [Documentation](https://dakera.ai/docs) | Full API reference and guides |
| [Rust SDK docs](https://docs.rs/dakera-client) | docs.rs API reference |
| [Benchmark](https://dakera.ai/benchmark) | LoCoMo evaluation results |
| [dakera.ai](https://dakera.ai) | Website and early access |
| [GitHub Org](https://github.com/dakera-ai) | All public repos |
| [dakera-deploy](https://github.com/Dakera-AI/dakera-deploy) | Self-hosting guide |

### Other SDKs

| SDK | Package |
|---|---|
| [dakera-py](https://github.com/dakera-ai/dakera-py) | `dakera` (PyPI) |
| [dakera-js](https://github.com/dakera-ai/dakera-js) | `@dakera-ai/dakera` (npm) |
| [dakera-go](https://github.com/dakera-ai/dakera-go) | `github.com/dakera-ai/dakera-go` |
| [dakera-cli](https://github.com/dakera-ai/dakera-cli) | CLI tool |
| [dakera-mcp](https://github.com/dakera-ai/dakera-mcp) | MCP server for Claude/Cursor |

---

<p align="center">
  <a href="https://dakera.ai">dakera.ai</a> ·
  <a href="https://dakera.ai/docs">Docs</a> ·
  <a href="https://dakera.ai/benchmark">Benchmark</a> ·
  <a href="https://dakera.ai#cta">Request Early Access</a>
</p>

<p align="center"><sub>Built with Rust. Single binary. Zero external dependencies.</sub></p>
