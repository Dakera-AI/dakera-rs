# ⚡ dakera-rs

Rust client for Dakera AI — store, recall, and search agent memories against a Dakera instance.

Part of [Dakera AI](https://dakera.ai) — the memory engine for AI agents.

---

## Install

```toml
# Cargo.toml
[dependencies]
dakera-client = "0.9"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

```rust
use dakera_client::{DakeraClient, Config, UpsertRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = DakeraClient::new(Config {
        base_url: "http://localhost:3300".to_string(),
        api_key: "your-key".to_string(),
        ..Default::default()
    });

    // Store a vector
    client.vectors().upsert(UpsertRequest {
        id: "vec-001".to_string(),
        values: vec![0.1, 0.2, 0.3],
        metadata: None,
    }).await?;

    // Search
    let results = client.fulltext().search("completed task", 5).await?;
    for r in results.results {
        println!("{} {:.3}", r.id, r.score);
    }

    Ok(())
}
```

## Connect to Dakera

```rust
// Self-hosted
let client = DakeraClient::new(Config {
    base_url: "http://your-server:3300".to_string(),
    api_key: "your-key".to_string(),
    ..Default::default()
});

// Cloud (early access)
let client = DakeraClient::new(Config {
    base_url: "https://api.dakera.ai".to_string(),
    api_key: "your-key".to_string(),
    ..Default::default()
});
```

## Documentation

→ [Full docs](https://dakera.ai/docs)  
→ [API reference](https://dakera.ai/docs/api)  
→ [Rust SDK reference](https://dakera.ai/docs/sdk/rust)

## Related

| Repo | What it is |
|---|---|
| [dakera-py](https://github.com/dakera-ai/dakera-py) | Python SDK |
| [dakera-js](https://github.com/dakera-ai/dakera-js) | TypeScript SDK |
| [dakera-go](https://github.com/dakera-ai/dakera-go) | Go SDK |
| [dakera-cli](https://github.com/dakera-ai/dakera-cli) | CLI |
| [dakera-mcp](https://github.com/dakera-ai/dakera-mcp) | MCP server · 83 tools |
| [dakera-deploy](https://github.com/dakera-ai/dakera-deploy) | Self-host Dakera |

---

*Part of the Dakera AI open core. The engine is proprietary. The tools are yours.*
