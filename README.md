# dakera-rs

Rust client for Dakera AI — store, recall, and search agent memories against a Dakera instance.

Part of [Dakera AI](https://dakera.ai) — the memory engine for AI agents.

> The Dakera memory engine scores **87.8% on LoCoMo** (1,540 questions, standard eval) — [benchmark details](https://dakera.ai/benchmark)

---

## Run Dakera

You need a running Dakera server before using this SDK. The fastest way:

```bash
docker run -d \
  --name dakera \
  -p 3300:3300 \
  -e DAKERA_ROOT_API_KEY=dk-mykey \
  ghcr.io/dakera-ai/dakera:latest
```

For persistent storage (recommended for anything beyond a quick test):

```bash
curl -sSfL https://raw.githubusercontent.com/Dakera-AI/dakera-deploy/main/docker-compose.yml \
  -o docker-compose.yml
DAKERA_API_KEY=dk-mykey docker compose up -d

curl http://localhost:3300/health  # -> {"status":"ok"}
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

- `http-client` (default) — async HTTP via reqwest
- `grpc` — gRPC transport with connection pooling via tonic
- `full` — both HTTP and gRPC

## Quick Start

```rust
use dakera_client::{DakeraClient, StoreMemoryRequest, RecallRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DakeraClient::builder("http://localhost:3300")
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

    // Full-text search
    let results = client.search_text("my-namespace", "completed task", 5).await?;
    for r in &results.results {
        println!("{}: {:.3}", r.id, r.score);
    }

    Ok(())
}
```

## Features

- **Agent Memory** — store, recall, search, and forget memories with importance scoring
- **Sessions** — group memories by conversation with auto-consolidation on session end
- **Knowledge Graph** — traverse memory relationships, find paths, export graphs
- **Vector Search** — ANN queries with metadata filters and batch operations
- **Full-Text Search** — BM25 ranking with stemming and stop-word filtering
- **Hybrid Search** — combine vector similarity with keyword matching
- **Text Auto-Embedding** — server-side embedding generation (no local model needed)
- **Feedback Loop** — upvote/downvote/flag memories to improve recall quality
- **Entity Extraction** — GLiNER NER for automatic entity detection
- **Streaming** — SSE event subscriptions with auto-reconnect
- **Dual Transport** — HTTP (default) and gRPC with connection pooling
- **Typed Filters** — `filter::eq()`, `filter::gt()`, `filter::contains()` DSL
- **Retry & Rate Limiting** — built-in exponential backoff and rate-limit header tracking
- **Builder Pattern** — fluent `DakeraClientBuilder` for configuration

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

## Connect to Dakera

```rust
use dakera_client::DakeraClient;

// Self-hosted
let client = DakeraClient::builder("http://your-server:3300")
    .api_key("your-key")
    .build()?;

// Cloud (early access)
let client = DakeraClient::builder("https://api.dakera.ai")
    .api_key("your-key")
    .build()?;

// With custom timeouts
let client = DakeraClient::builder("http://localhost:3300")
    .api_key("your-key")
    .timeout_secs(60)
    .max_retries(5)
    .build()?;
```

## Documentation

-> [Full docs](https://dakera.ai/docs)  
-> [API reference](https://dakera.ai/docs/api)  
-> [Rust SDK reference](https://docs.rs/dakera-client)

## Related

| Repo | What it is |
|---|---|
| [dakera-py](https://github.com/dakera-ai/dakera-py) | Python SDK |
| [dakera-js](https://github.com/dakera-ai/dakera-js) | TypeScript SDK |
| [dakera-go](https://github.com/dakera-ai/dakera-go) | Go SDK |
| [dakera-cli](https://github.com/dakera-ai/dakera-cli) | CLI |
| [dakera-mcp](https://github.com/dakera-ai/dakera-mcp) | MCP server |
| [dakera-deploy](https://github.com/dakera-ai/dakera-deploy) | Self-host Dakera |

---

*Part of the Dakera AI open core. The engine is proprietary. The tools are yours.*
