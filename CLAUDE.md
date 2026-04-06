# dakera-rs

Rust client SDK for the Dakera AI agent memory platform (crate: `dakera-client`).

## Key Commands
```bash
cargo build          # Build
cargo test           # Run tests (set DAKERA_API_URL + DAKERA_API_KEY for integration tests)
cargo clippy         # Lint with -D warnings
cargo fmt            # Format
cargo publish        # Publish to crates.io
```

## Architecture
- `src/client.rs` — DakeraClient struct; async reqwest HTTP client with auth
- `src/memory.rs` — Memory CRUD: store, recall, search, forget
- `src/agents.rs` — Agent management operations
- `src/knowledge.rs` — Knowledge graph query and export
- `src/analytics.rs`, `src/admin.rs` — Admin and analytics endpoints
- `src/keys.rs` — API key management
- `src/types.rs` — All request/response types (serde)
- `src/error.rs` — DakeraError enum

## Conventions
- async-first using tokio; sync wrapper behind optional feature flag
- Version in Cargo.toml matches server version (e.g., 0.9.13)
- SDK batch: all 4 SDKs (py, js, rs, go) sync together after a server API change
