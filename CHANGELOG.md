# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.8.1] - 2026-03-23

### Changed
- Bumped to match core v0.8.1 release. No code changes — `HybridSearchRequest.vector` was already
  `Option<Vec<f32>>` with correct endpoint `/v1/namespaces/{ns}/hybrid`. Version sync only.

## [0.8.0] - 2026-03-23

### Changed
- `HybridSearchRequest.vector` is now `Option<Vec<f32>>` (was `Vec<f32>`). The field is omitted
  from the JSON payload when `None`, causing the server to fall back to BM25-only full-text search.
  Existing callers using `HybridSearchRequest::new(vector, ...)` continue to work unchanged.

### Added
- `HybridSearchRequest::text_only(text, top_k)` — convenience constructor for BM25-only search
  without a query vector. (core v0.8.0 / dakera-mcp PR#20)

## [0.7.3] - 2026-03-23

### Added
- `StoreMemoryRequest`: new `ttl_seconds: Option<u64>` and `expires_at: Option<u64>` fields
  with corresponding builder methods `with_ttl()` and `with_expires_at()` (DECAY-3).
  `expires_at` takes precedence over `ttl_seconds`; memory is hard-deleted on expiry.
- `DecayConfigResponse`, `DecayConfigUpdateRequest`, `DecayConfigUpdateResponse` types (DECAY-1)
- `LastDecayCycleStats`, `DecayStatsResponse` types (DECAY-2)
- `DakeraClient::decay_config()` — `GET /admin/decay/config` — current strategy, half-life,
  and min-importance threshold (DECAY-1). Requires Admin scope.
- `DakeraClient::decay_update_config()` — `PUT /admin/decay/config` — live config update with
  no restart required (DECAY-1). All fields optional.
- `DakeraClient::decay_stats()` — `GET /admin/decay/stats` — cumulative counters and
  last-cycle snapshot (DECAY-2). Requires Admin scope.

## [0.7.2] - 2026-03-23

### Added
- `AutoPilotConfig`, `AutoPilotStatusResponse`, `DedupResultSnapshot`, `ConsolidationResultSnapshot` types
- `AutoPilotConfigRequest`, `AutoPilotConfigResponse` types for runtime configuration updates
- `AutoPilotTriggerAction` enum (`dedup`, `consolidate`, `all`), `AutoPilotTriggerRequest`,
  `AutoPilotTriggerResponse`, `AutoPilotDedupResult`, `AutoPilotConsolidationResult` types
- `DakeraClient::autopilot_status()` — `GET /admin/autopilot/status` — current config + last-run stats (PILOT-1)
- `DakeraClient::autopilot_update_config()` — `PUT /admin/autopilot/config` — live config update (PILOT-2)
- `DakeraClient::autopilot_trigger()` — `POST /admin/autopilot/trigger` — manual dedup/consolidation cycle (PILOT-3)
- `RuntimeConfig` extended with `autopilot_enabled`, `autopilot_dedup_threshold`,
  `autopilot_dedup_interval_hours`, `autopilot_consolidation_interval_hours` fields

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
