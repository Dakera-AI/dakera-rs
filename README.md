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
  <a href="https://dakera.ai/playground"><img alt="Playground" src="https://img.shields.io/badge/playground-try_it-ff6b35?style=flat-square" /></a>
</p>

---

## Why Dakera?

| | Dakera | Others |
|---|---|---|
| **LoCoMo Recall@20** | **88.2%** (1,536 Q · LLM-judged retrieval recall) | not directly comparable |
| **Deployment** | Single binary, Docker one-liner | External vector DB + embedding service required |
| **Embeddings** | Built-in — no OpenAI key needed | Requires external embedding API |
| **Search modes** | Vector · BM25 · Hybrid · Knowledge Graph | Usually one or two |
| **Transport** | HTTP (reqwest) + gRPC (tonic), zero-copy | HTTP only |

→ [Try the playground](https://dakera.ai/playground) · [Full benchmark results](https://dakera.ai/benchmark) · [dakera.ai](https://dakera.ai)

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
dakera-client = "0.12"
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
dakera-client = { version = "0.12", features = ["grpc"] }
```

---

## Quick Start

```rust
// Cargo.toml: dakera-client = "0.12"
let client = DakeraClient::builder("http://localhost:3000").api_key("dk-mykey").build()?;
client.store_memory(StoreMemoryRequest::new("my-agent", "User prefers brevity")).await?;
```

Full example — store, recall, upsert, and hybrid search:

```rust
use dakera_client::{DakeraClient, StoreMemoryRequest, RecallRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DakeraClient::builder("http://localhost:3000")
        .api_key("dk-mykey")
        .build()?;

    // Store an agent memory
    let mem = client
        .store_memory(
            StoreMemoryRequest::new("my-agent", "User prefers concise responses with code examples")
                .with_importance(0.9),
        )
        .await?;
    println!("Stored: {}", mem.memory_id);

    // Recall memories (semantic search)
    let response = client
        .recall(RecallRequest::new("my-agent", "what does the user prefer?").with_top_k(5))
        .await?;
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

## What's new in v0.12.0

`dakera-client` 0.12.0 supports the Dakera server **v0.12.0** and keeps working against
**v0.11.108** servers. Every v0.12 feature is additive; the routes that need a v0.12 server
answer `404` on v0.11.108 and the v0.12-only opt-in features answer `501 FEATURE_DISABLED` on a
v0.12 server that did not switch them on. The server-side upgrade guide is
[docs/v0.12/UPGRADE.md](https://github.com/Dakera-AI/dakera/blob/main/docs/v0.12/UPGRADE.md) (release
notes: [RELEASE_NOTES.md](https://github.com/Dakera-AI/dakera/blob/main/docs/v0.12/RELEASE_NOTES.md)).

- **Health that tells the truth** — `health()` is healthy only for a `2xx` answer with status
  `healthy` (v0.11 of this crate reported a starting server's `503` as healthy). `ready()` parses the
  `503` body of a starting server instead of failing, `live()` is the process probe, and
  `wait_until_ready(timeout)` waits on `GET /health/ready` honouring `Retry-After`. A v0.12 server
  binds its port while models load, so point readiness at `/health/ready`.
- **`Retry-After`** — every v0.12 `503` carries it. `ClientError::ServiceUnavailable { retry_after, .. }`
  and `RateLimitExceeded` expose it (`ClientError::retry_after()`), and `execute_with_retry` waits the
  advertised time (capped at `RetryConfig::max_delay`) instead of its own backoff.
- **Clean error mapping** — every v0.12 error body is JSON. `413` is `QuotaExceeded` (a hard namespace
  quota, now enforced) or `PayloadTooLarge` (an over-size attachment, record or body); `501` is
  `FeatureDisabled` (the message names the variable, for example `DAKERA_ATTACHMENTS`) or
  `NotImplemented`; the new codes are in `ServerErrorCode` (now exported).
- **`capabilities()`** — `GET /v1/capabilities`, now with the `scoring`, `attachments`, `vision` and
  `unreadable_records` sections next to models, index kinds (`ivfpq`), search modes (`rabitq`,
  `search_modes_accepted` prose parsed), record kinds and dtypes and `query_languages`.
- **Attachments** — `upload_attachment`, `list_attachments`, `download_attachment`,
  `delete_attachment`, speech-to-text jobs (`transcribe_attachment`, `transcription_status`), image
  indexing jobs (`index_image_attachment`, `index_image_status`) and `wait_for_attachment_job`.
  `StoreMemoryRequest::with_attachment_ref` points a memory at an upload.
- **Records with named representations** — `upsert_records` and `get_record` (`RecordInput`,
  `RepresentationInput` with kinds `dense` / `token_multivector` / `patch_multivector` and dtypes
  `f32` / `f16` / `i8`).
- **Per-request `lang`** — `with_lang` on `StoreMemoryRequest`, `BatchStoreMemoryRequest`,
  `RecallRequest` (also used for search), `UpdateMemoryRequest::lang`, `extract_entities_with_lang`,
  `extract_text_with_lang`. Supported languages: `capabilities().query_languages`.
- **Models** — `EmbeddingModel::BgeM3` and `ColbertSmall`; unknown strings from newer servers are kept
  as `Unknown(String)` instead of failing.
- **Namespace config** — `put_namespace_entity_config` (`PUT`, replaces: an omitted `entity_types`
  clears the list). `PATCH` (`configure_namespace_ner`) merges and refuses unknown fields on v0.12.
- **gRPC** — v0.12 requires an API key on every call but `Health`. `GrpcClientConfig::with_api_key`
  (or `DAKERA_API_KEY`) sends it as `x-api-key`; gRPC status errors are mapped
  (`UNAUTHENTICATED` is an auth error). The RPC paths are now `/dakera.v1.VectorService/...`, the
  server's actual proto package.

### Compatibility

| Server | Works | Notes |
|---|---|---|
| v0.11.108 | yes | v0.12-only calls return `404` (capabilities, records, attachments); everything else is unchanged |
| v0.12.0 | yes | opt-in features answer `501 FEATURE_DISABLED` until the operator enables them |

Merge and publish this release only after the v0.12.0 server release.

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
- **T-I-F Reliability** — `TifScore` struct and `evaluate_tif()` for Truth-Indeterminacy-Falsity scoring of memory reliability
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
