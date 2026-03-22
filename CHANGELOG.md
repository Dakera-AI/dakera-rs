# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.1] - 2026-03-22

### Added
- `BatchMemoryFilter` / `BatchRecallRequest` / `BatchRecallResponse` / `BatchForgetRequest` /
  `BatchForgetResponse` — typed models for batch memory operations
- `DakeraClient::batch_recall()` — `POST /v1/memories/recall/batch` — recall memories for
  multiple agents in a single request
- `DakeraClient::batch_forget()` — `DELETE /v1/memories/forget/batch` — forget memories for
  multiple agents in a single request
- `RateLimitHeaders` struct with `from_response()` constructor + `last_rate_limit_headers()`
  accessor — exposes `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`

## [0.7.0] - 2026-03-22

### Added
- `RetryConfig` struct with `max_retries`, `base_delay`, `max_delay`, and `jitter` fields
- `ClientOptions.retry_backoff` (`Option<RetryConfig>`) — overrides `max_retries` when set
- `ClientOptions.connect_timeout` — sets TCP dial timeout independently of overall request timeout
- HTTP 429 responses respect the `Retry-After` header; falls back to exponential backoff
- 5xx responses retried up to `max_retries` times; 4xx (except 429) never retried

## [0.6.2] - 2026-03-21

### Added
- `CrossAgentNetworkResponse.node_count` field (`usize`, `#[serde(default)]`) — reflects the
  `node_count` field added in dakera server v0.6.2 (PR #26). Defaults to `0` when absent so
  responses from older server versions remain valid.
- SSE endpoints now support `?api_key=<key>` query-parameter authentication in addition to
  the `Authorization: Bearer` header. Useful when constructing streaming URLs for clients that
  cannot send custom headers (e.g. browser-native `EventSource`).

## [0.3.0] - 2026-03-19

### Added
- `fetch()` / `fetch_by_ids()` — retrieve vectors by ID (`POST /v1/namespaces/{ns}/fetch`)
- `upsert_text()` — upsert text documents with automatic server-side embedding
- `query_text()` / `query_text_simple()` — natural language queries with auto-embedding
- `batch_query_text()` — batch text queries in a single request
- New types: `FetchRequest`, `FetchResponse`, `UpsertTextRequest`, `TextUpsertResponse`,
  `QueryTextRequest`, `TextQueryResponse`, `BatchQueryTextRequest`, `BatchQueryTextResponse`,
  `TextDocument`, `EmbeddingModel`

### Changed
- Full API parity with Python, JS, and Go SDKs

## [0.2.0] - 2025-06-15

### Added

- HTTP client with async/await support via reqwest
- gRPC client with connection pooling and HTTP/2 multiplexing
- Vector operations: upsert, query, delete, batch query
- Column-format upsert for efficient bulk operations
- Full-text search with BM25 ranking
- Hybrid search combining vector similarity and text search
- Multi-vector search with positive/negative vectors and MMR
- Unified query API with flexible ranking options
- Query explain for execution plan analysis
- Aggregation queries with grouping support
- Export with cursor-based pagination
- Cache warming for hot data
- Memory management: store, recall, forget, search, consolidate
- Session management: start, end, list, get memories
- Knowledge graph operations: build, summarize, deduplicate
- Agent management: list, stats, memories, sessions
- API key management: create, list, rotate, delete, usage stats
- Admin operations: cluster status, namespace management, index rebuild
- Cache management: stats, clear
- Runtime configuration: get, update
- Quota management: get, set, delete
- Slow query monitoring
- Backup and restore
- TTL management and cleanup
- Analytics: overview, latency, throughput, storage
- System diagnostics and job management
- Feature flags: `http-client` (default), `grpc`, `full`
- Builder pattern for client configuration
- Comprehensive error types with retryable classification
