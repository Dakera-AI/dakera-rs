# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.7] - 2026-03-31

### Added
- **KG-2: Graph Query & Export bindings:**
  - `DakeraClient::knowledge_query(agent_id, root_id?, edge_type?, min_weight?, max_depth?, limit?)`
    — filter-based DSL query over the memory knowledge graph
    (`GET /v1/knowledge/query`). Returns `Result<KgQueryResponse>`.
  - `DakeraClient::knowledge_path(agent_id, from_id, to_id)` — BFS shortest path
    between two memory IDs (`GET /v1/knowledge/path`). Returns `Result<KgPathResponse>`.
  - `DakeraClient::knowledge_export(agent_id, format?)` — export the full graph
    as JSON or GraphML (`GET /v1/knowledge/export`). Returns `Result<KgExportResponse>`
    for `format=Some("json")` or default JSON.
  - New types: `KgQueryResponse`, `KgPathResponse`, `KgExportResponse`.

## [0.9.6] - 2026-03-30

### Added
- **GLiNER Entity Extraction via ODE sidecar (ODE-2):**
  - `DakeraClient::ode_extract_entities(req)` — extract named entities from text
    using the dakera-ode GLiNER sidecar (`POST /ode/extract`). Returns
    `Result<ExtractEntitiesResponse>` with per-entity character offsets,
    confidence scores, model variant, and processing time in ms.
  - `DakeraClientBuilder::ode_url(url)` — configure the ODE sidecar URL.
  - New types: `OdeEntity`, `ExtractEntitiesRequest`, `ExtractEntitiesResponse`.

## [0.9.5] - 2026-03-30

### Added
- **AES-256-GCM Encryption Key Rotation (SEC-3):**
  - `DakeraClient::rotate_encryption_key(new_key, namespace?)` — re-encrypt all
    memory content blobs with a new AES-256-GCM key
    (`POST /v1/admin/encryption/rotate-key`). Pass `namespace=None` to rotate
    all namespaces. Returns `Result<RotateEncryptionKeyResponse>`. Requires
    Admin scope.
  - New types: `RotateEncryptionKeyRequest`, `RotateEncryptionKeyResponse`
    (fields: `rotated`, `skipped`, `namespaces`).

## [0.9.4] - 2026-03-30

### Added
- **Memory Import/Export (DX-1):**
  - `DakeraClient::import_memories(data, format, agent_id?, namespace?)` — import
    memories from Mem0, Zep, JSONL, or CSV (`POST /v1/import`). Returns
    `MemoryImportResponse`.
  - `DakeraClient::export_memories(format, agent_id?, namespace?, limit?)` — export
    memories in a portable format (`GET /v1/export`). Returns `MemoryExportResponse`.
  - New types: `MemoryImportResponse`, `MemoryExportResponse`.
- **Business-Event Audit Log (OBS-1):**
  - `DakeraClient::list_audit_events(query)` — paginated audit log query
    (`GET /v1/audit`). Returns `AuditListResponse`.
  - `DakeraClient::stream_audit_events(agent_id?, event_type?)` — live SSE stream
    of audit events (`GET /v1/audit/stream`). Returns
    `Receiver<Result<DakeraEvent>>`.
  - `DakeraClient::export_audit(format, agent_id?, event_type?, from?, to?)` —
    bulk export audit entries (`POST /v1/audit/export`). Returns
    `AuditExportResponse`.
  - New types: `AuditEvent`, `AuditListResponse`, `AuditExportResponse`, `AuditQuery`.
- **DBSCAN Adaptive Consolidation (CE-6):** `ConsolidateRequest` now has an
  optional `config: Option<ConsolidationConfig>` field for algorithm selection
  (`"dbscan"` or `"greedy"`) and DBSCAN parameter tuning. `ConsolidateResponse`
  includes an optional `log: Vec<ConsolidationLogEntry>`.
  New types: `ConsolidationConfig`, `ConsolidationLogEntry`.
- **External Extraction Providers (EXT-1):**
  - `DakeraClient::extract_text(text, namespace?, provider?, model?)` — extract
    entities from text (`POST /v1/extract`). Providers: `gliner` (bundled),
    `openai`, `anthropic`, `openrouter`, `ollama`. Returns `ExtractionResult`.
  - `DakeraClient::list_extract_providers()` — list available providers
    (`GET /v1/extract/providers`). Returns `Vec<ExtractionProviderInfo>`.
  - `DakeraClient::configure_namespace_extractor(namespace, provider, model?)` —
    set namespace default extractor (`PATCH /v1/namespaces/{ns}/extractor`).
  - New types: `ExtractionResult`, `ExtractionProviderInfo`.
- **Redis Health (OPS-3):** `ClusterStatus` gains `redis_healthy: Option<bool>`.
- **Cluster Env Aliases (DIST-1):** Documented `DAKERA_CLUSTER_NODE_ID`,
  `SEED_NODES`, `BIND_ADDR` server environment variables.
- **Memory Encryption (SEC-3):** Server supports AES-256-GCM at-rest encryption
  via `DAKERA_ENCRYPTION_KEY` — transparent to SDK clients.

## [0.9.3] - 2026-03-29

### Added
- **Prometheus Metrics (INFRA-3):** `DakeraClient::ops_metrics()` — returns the
  raw Prometheus text exposition format string from `GET /v1/ops/metrics` (Admin
  scope). Uses new `handle_text_response` for non-JSON bodies.

## [0.9.2] - 2026-03-27

### Added
- **Namespace-scoped API Keys (SEC-1):**
  - `DakeraClient::create_namespace_key(namespace, name, expires_in_days)` —
    create a scoped API key (`POST /v1/namespaces/{ns}/keys`). Returns
    `CreateNamespaceKeyResponse`. The raw key is shown **only once**.
  - `DakeraClient::list_namespace_keys(namespace)` — list all API keys for a
    namespace (`GET /v1/namespaces/{ns}/keys`). Returns `ListNamespaceKeysResponse`.
  - `DakeraClient::delete_namespace_key(namespace, key_id)` — revoke a namespace
    API key (`DELETE /v1/namespaces/{ns}/keys/{key_id}`). Returns
    `KeySuccessResponse`.
  - `DakeraClient::get_namespace_key_usage(namespace, key_id)` — usage stats for
    a key (`GET /v1/namespaces/{ns}/keys/{key_id}/usage`). Returns
    `NamespaceKeyUsageResponse`.
  - New types: `CreateNamespaceKeyRequest`, `CreateNamespaceKeyResponse`,
    `NamespaceKeyInfo`, `ListNamespaceKeysResponse`, `NamespaceKeyUsageResponse`,
    `KeySuccessResponse` — all re-exported from the crate root.

## [0.9.1] - 2026-03-26

### Added
- **Memory Feedback Loop (INT-1):**
  - `DakeraClient::feedback_memory(memory_id, agent_id, signal, note)` — submit feedback
    (upvote/downvote/flag) for a memory (`POST /v1/memories/{id}/feedback`). Returns
    `FeedbackResponse`.
  - `DakeraClient::patch_memory_importance(memory_id, agent_id, importance)` — directly set a
    memory's importance score (`PATCH /v1/memories/{id}/importance`). Returns `FeedbackResponse`.
  - `DakeraClient::get_memory_feedback_history(memory_id)` — retrieve all feedback events for a
    memory (`GET /v1/memories/{id}/feedback/history`). Returns `FeedbackHistoryResponse`.
  - `DakeraClient::get_agent_feedback_summary(agent_id)` — aggregate feedback counts and health
    score for an agent (`GET /v1/agents/{id}/feedback/summary`). Returns `AgentFeedbackSummary`.
  - `DakeraClient::get_feedback_health(agent_id)` — health score (mean importance of non-expired
    memories) for an agent (`GET /v1/feedback/health`). Returns `FeedbackHealthResponse`.
  - New types: `FeedbackSignal` (enum: `Upvote` / `Downvote` / `Flag`), `FeedbackResponse`,
    `FeedbackHistoryEntry`, `FeedbackHistoryResponse`, `MemoryFeedbackBody`,
    `MemoryImportancePatch`, `AgentFeedbackSummary`, `FeedbackHealthResponse` — all re-exported
    from the crate root.
  - Note: `LegacyFeedbackResponse` replaces the old `FeedbackResponse` from CE-4 entity
    extraction to avoid the name collision.

## [0.9.0] - 2026-03-26

### Added
- **Memory Knowledge Graph API (SDK-9 / CE-5 pre-impl):**
  - `DakeraClient::memory_graph(memory_id, depth, types)` — returns the graph of memories
    connected to `memory_id` (`GET /v1/memories/{id}/graph`). Depth and edge-type filters
    are optional.
  - `DakeraClient::memory_path(source_id, target_id)` — shortest path between two memory
    nodes (`GET /v1/memories/{id}/path`).
  - `DakeraClient::memory_link(source_id, target_id, edge_type)` — create a directed edge
    between two memories (`POST /v1/memories/{id}/links`).
  - `DakeraClient::agent_graph_export(agent_id, format)` — export the full memory graph for
    an agent as JSON or CSV (`GET /v1/agents/{id}/graph/export`).
  - New types: `EdgeType`, `GraphEdge`, `GraphNode`, `MemoryGraph`, `GraphPath`,
    `GraphLinkResponse`, `GraphExport` — all re-exported from the crate root.
  - **Note:** requires server CE-5 for end-to-end functionality; unit tests use mocked
    responses and pass fully against the current server (server CE-5 / DAK-1002).
- **Real-time memory event streaming (SDK-10):**
  - `DakeraClient::subscribe_agent_events(agent_id, tag_filter, reconnect)` — async stream
    yielding `MemoryEvent` from `GET /v1/events/stream`. Supports tag-based filtering and
    optional auto-reconnect. Skips the `connected` handshake event automatically.

## [0.8.6] - 2026-03-25

### Changed
- `OpsStats` struct — added `state: String` field (`"healthy"` or `"degraded"`) reflecting
  storage health. Syncs with core DAK-918 (`/v1/ops/stats` fix).

## [0.8.5] - 2026-03-25

### Added
- `DakeraClient::ops_stats()` — new Read-scoped endpoint `GET /v1/ops/stats` returns `OpsStats`
  (`version`, `total_vectors`, `namespace_count`, `uptime_seconds`, `timestamp`). Works with
  read-only API keys; use instead of `cluster_status()` when Admin scope is unavailable
  (core DAK-852).
- `OpsStats` struct re-exported from the crate root.

> **Note:** v0.8.4 was a Python-only security patch (urllib3 CVE) and was not released for
> this crate. This release jumps from v0.8.3 to v0.8.5 to realign all SDKs at the same version.

## [0.8.2] - 2026-03-23

### Added
- `DakeraEvent::Connected { timestamp }` — new variant for the SSE `connected` handshake event
  emitted on stream subscription by all SSE endpoints (core DAK-720).
- `MemoryEvent`: SSE `connected` handshake event now deserialises correctly. The `type` JSON key
  is accepted as an alias for `event_type`, and `agent_id` defaults to `""` when absent.
  Callers receive a `MemoryEvent { event_type: "connected", agent_id: "", timestamp }`.
- `StoreMemoryRequest.expires_at` — optional explicit expiry Unix timestamp (seconds). Takes
  precedence over `ttl_seconds` when both are set. Use `StoreMemoryRequest::with_expires_at(ts)`
  (builder method already in `memory.rs`) (core DECAY-3 / DAK-740).

### Changed
- `MemoryEvent.event_type` — now `#[serde(alias = "type", default)]` to handle the `connected`
  event JSON shape without breaking existing callers.
- `MemoryEvent.agent_id` — now `#[serde(default)]`; empty string for `connected` events.

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
