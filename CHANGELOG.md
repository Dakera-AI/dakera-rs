# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.11.3] - 2026-04-17

### Security
- Updated `rustls-webpki` from `0.103.10` to `0.103.12` (via Cargo.lock) to address
  GHSA-xgp8-3hg3-c2mh and GHSA-965h-392x-2mh5 (LOW, CVSS 2.2). These CVEs affect
  TLS certificate parsing in edge cases. Callers receive the fix automatically on upgrade;
  no API changes required.

## [0.11.2] - 2026-04-16

### Changed
- **v0.11.2:** Server default fusion strategy changed from `Rrf` to `MinMax`
  (CEO architecture decision, DAK-1948). MinMax +6.3pp overall Recall@10, +13.5pp temporal.
  Callers that pass `fusion: None` (the recommended pattern) will now get `MinMax` from the
  server. Pass `Some(FusionStrategy::Rrf)` explicitly to keep RRF behaviour. Updated doc
  comments to reflect the new server default. The Rust enum's `#[default]` attribute remains
  on `Rrf` for backwards-compatible `Default::default()` usage, but `RecallRequest` sends
  `None` by default so the server default applies.

## [0.11.1] - 2026-04-16

### Fixed
- `FusionStrategy::MinMax` now correctly serializes as `"minmax"` (was `"min_max"` due to the
  `#[serde(rename_all = "snake_case")]` default on the enum). Any caller using
  `FusionStrategy::MinMax` / `.with_fusion(FusionStrategy::MinMax)` prior to this release
  would have received a `422 Unprocessable Entity` from the server. Affects Rust only — Python,
  TypeScript, and Go serialized `"minmax"` correctly in v0.11.0.

## [0.11.0] - 2026-04-15

### Added
- **CE-14:** `FusionStrategy` enum (`FusionStrategy::Rrf` / `FusionStrategy::MinMax`) — controls hybrid score fusion.
- **CE-14:** `fusion: Option<FusionStrategy>` field on `RecallRequest` with `.with_fusion()` builder method. `None` uses server default (`Rrf`).
- **v0.11.0:** `neighborhood: Option<bool>` field on `RecallRequest` with `.with_neighborhood()` builder. Session-adjacent memory enrichment (±5 min). `None` uses server default (`true`). Use `.with_neighborhood(false)` to disable.


## [0.10.3] - 2026-04-15

### Fixed
- `ClientError::Http(_)` variants now correctly return `true` from `is_retryable()`. Previously, network-layer errors from reqwest (timeout, connect failure, and hyper-level errors like `IncompleteMessage`) were not classified as retryable, causing retry logic to skip them even when the server was transiently unavailable.

## [0.10.2] - 2026-04-13

### Added
- **CE-13:** `rerank: Option<bool>` field on `RecallRequest` (used by both `recall()` and `search_memories()`). `None` uses server default (`true` for recall, `false` for search). Use `.with_rerank(false)` to disable on latency-sensitive paths.
- **CE-13:** `EmbeddingModel::BgeLarge` variant (`"bge-large"`, 1024 dimensions). Now `#[default]` — matches new server default embedding model.

## [0.10.1] - 2026-04-12

### Added
- **Auth:** `DakeraClientBuilder::api_key(key)` — set the Bearer token for all requests. Falls back to the `DAKERA_API_KEY` environment variable automatically. Previously the SDK sent no `Authorization` header and could not authenticate against servers with `DAKERA_AUTH_ENABLED=true`.

### Fixed
- `StoreMemoryResponse` now correctly deserializes the server's nested `{memory:{id:...}}` format. Previously the struct expected flat `{memory_id, agent_id, namespace}` causing a deserialization failure on every `store_memory` call.
- `RecallResponse.total_found` is now `#[serde(default)]` — the server does not return this field and deserialization was failing.
- `ConsolidateResponse` now deserializes from the server's actual format (`memories_removed`, `source_memory_ids`) instead of the fictional flat format (`consolidated_count`, `new_memories`).
- `ConsolidateResponse` `consolidate()` now calls `POST /v1/memory/consolidate` (correct path) instead of `POST /v1/agents/{id}/memories/consolidate` (which returned 404).
- `CompressResponse` now matches the server's DBSCAN compress response format (`originals_deprecated`, `memories_scanned`, `clusters_found`, `summaries_created`) replacing the fictional `memories_before/after/removed_count` fields.
- `MemoryType` enum now serializes as lowercase (`"episodic"`, `"semantic"`, `"procedural"`, `"working"`). Previously serialized as PascalCase (`"Episodic"`, ...) causing HTTP 422 on every `store_memory` call.
- `DakeraClient::health()` now correctly parses the server health response. The server returns `{"status":"healthy"}` (a string field) but the SDK was attempting to deserialize it into a `HealthResponse` with `healthy: bool`, causing a deserialization error on every health check. Fixed by parsing the JSON body flexibly and mapping `status == "healthy"` to `healthy = true`, with fallback to the legacy `healthy: bool` field for forward-compat.

## [0.10.0] - 2026-04-12

### Added
- **CE-10:** `RoutingMode` enum (`Auto | Vector | Bm25 | Hybrid`, serializes as `"auto" | "vector" | "bm25" | "hybrid"`) — controls which retrieval index to use for recall and search.
- **CE-10:** `routing: Option<RoutingMode>` field on `RecallRequest` + `with_routing()` builder method.
- **CE-12:** `DakeraClient::compress(agent_id)` method — calls `POST /v1/agents/{id}/compress` and returns `CompressResponse`.
- **CE-12:** `CompressResponse` struct with `agent_id`, `memories_before`, `memories_after`, `removed_count`, `duration_ms?`.
- **CE-10:** `MemoryPolicy::dedup_on_store: Option<bool>` — enable similarity deduplication at store time.
- **CE-10:** `MemoryPolicy::dedup_threshold: Option<f32>` — cosine-similarity threshold for store-time deduplication.

## [0.9.15] - 2026-04-08

### Notes
- Version bump to match server v0.9.15. No SDK API changes.
- Server changes (transparent to SDK callers):
  - **DAK-1691:** Session-end auto-consolidation — `end_session` now triggers server-side DBSCAN clustering of near-duplicate session memories, soft-expiring them with a 30-day TTL. High-importance memories (>0.8) are protected. No request/response signature change.
  - **DAK-1689:** HNSW post-filter ANN fix — filtered vector queries are now O(N·ANN) instead of O(N·linear). No SDK change.

## [0.9.14] - 2026-04-07

### Added
- **DAK-1690: Agent wake-up context endpoint:**
  - `DakeraClient::wake_up(agent_id, top_n, min_importance)` — `GET /v1/agents/{agent_id}/wake-up` — returns a `WakeUpResponse` with top-N memories ranked by importance × recency decay. Sub-millisecond; no embedding inference. Requires Read scope.
  - `WakeUpResponse` struct (`agent_id`, `memories: Vec<Memory>`, `total_available: u32`) and `Memory` struct exported from crate root.

## [0.9.13] - 2026-04-07

### Fixed
- **Session type fix (DAK-1548):** `Session.id` is now correctly mapped (was `session_id`). `start_session()` and `end_session()` now correctly deserialize wrapped server responses. Added `SessionStartResponse` and `SessionEndResponse` types — `end_session()` now returns `SessionEndResponse` exposing `memory_count: usize`.

## [0.9.12] - 2026-04-06

### Added
- **OBS-2: Product KPI Snapshot endpoint:**
  - `DakeraClient::get_kpis()` — `GET /v1/kpis` — returns a `KpiSnapshot` with 8 real-time
    operational metrics. Sub-millisecond; served from in-memory counters. Requires Admin scope.
  - `KpiSnapshot` struct exported from the crate root:
    - `recall_latency_p50_ms` / `recall_latency_p99_ms` (`f64`) — median/p99 recall latency (ms)
    - `store_latency_p50_ms` (`f64`) — median store latency (ms)
    - `api_error_rate_5xx_pct` (`f64`) — 5xx error rate as a percentage of total requests
    - `active_agents_count` (`u64`) — distinct agents active in the last 24 hours
    - `session_count_week` (`u64`) — sessions created in the rolling 7-day window
    - `cross_agent_network_node_count` (`u64`) — nodes in the cross-agent knowledge graph
    - `memory_retention_7d_pct` (`f64`) — percentage of memories from 7 days ago still active

### Server-side only (no SDK changes required)
- **v0.9.12 performance fixes:** session-agent index lookup reduced to O(1); memory counters
  now updated via atomic increments; S3 flushes are async (non-blocking).

## [0.9.11] - 2026-04-01

### Added
- **KG-3: Deep Associative Recall bindings:**
  - `RecalledMemory` gains `depth: Option<u8>` — the KG hop at which an associated memory was found (skipped on serialise when `None`).
  - `RecallRequest` gains two new optional fields:
    - `associated_memories_depth: Option<u8>` — KG traversal depth 1–3 (default: `1`).
    - `associated_memories_min_weight: Option<f32>` — minimum KG edge weight (default: `0.0`).
  - Builder methods `with_associated_depth(depth: u8)` (implies `include_associated = true`) and `with_associated_min_weight(weight: f32)`.
  - Fully backward-compatible: omitting new fields retains depth-1 (COG-2) behaviour.
- **COG-3: Proactive Memory Consolidation bindings:**
  - `MemoryPolicy` struct gains four new optional fields:
    - `consolidation_enabled: Option<bool>` — opt-in background DBSCAN deduplication (default: `false`).
    - `consolidation_threshold: Option<f32>` — cosine-similarity epsilon (default: `0.92`).
    - `consolidation_interval_hours: Option<u32>` — background job interval in hours (default: `24`).
    - `consolidated_count: Option<u64>` — **read-only** lifetime merge count (server-managed; skipped on serialise).
  - `MemoryPolicy::default()` initialises all four COG-3 fields with server defaults.
- **SEC-5: Per-namespace rate limiting bindings:**
  - `MemoryPolicy` struct gains three new optional fields:
    - `rate_limit_enabled: Option<bool>` — opt-in per-namespace rate limiting (default: `false`).
    - `rate_limit_stores_per_minute: Option<u32>` — max store ops/min; `None` = unlimited (default).
    - `rate_limit_recalls_per_minute: Option<u32>` — max recall ops/min; `None` = unlimited (default).
  - `MemoryPolicy::default()` sets `rate_limit_enabled: Some(false)` and the two limit fields to `None`.
  - When a limit is exceeded the server returns HTTP 429; the existing `DakeraError::RateLimit` variant is returned.

## [0.9.9] - 2026-03-31

### Added
- **CE-7: Time-Window Recall bindings:**
  - `RecallRequest` gains two new optional fields: `since: Option<String>`
    and `until: Option<String>` (ISO-8601 timestamps).
  - New builder methods: `.with_since(ts)` and `.with_until(ts)`.
  - Filters are applied server-side before semantic ranking.
  - Invalid ISO-8601 values return a `400` error from the server.

## [0.9.8] - 2026-03-31

### Added
- **COG-2: Associative Recall bindings:**
  - `RecallRequest` gains two new fields: `include_associated: bool`
    (default false) and `associated_memories_cap: Option<u32>`.
  - New builder methods: `.with_associated()` and
    `.with_associated_cap(cap)`.
  - `RecallResponse` gains `associated_memories: Option<Vec<RecalledMemory>>`
    — present when `include_associated` was set.
- **COG-1: Cognitive Memory Lifecycle bindings:**
  - `get_memory_policy(namespace)` — retrieve the memory lifecycle policy
    (`GET /v1/namespaces/{namespace}/memory_policy`). Returns `MemoryPolicy`.
  - `set_memory_policy(namespace, policy)` — set the lifecycle policy
    (`PUT /v1/namespaces/{namespace}/memory_policy`).
  - New type: `MemoryPolicy` — `Option`-wrapped fields for type-specific TTLs,
    per-type decay curves (`working_decay`, `episodic_decay`, `semantic_decay`,
    `procedural_decay` — one of `"exponential"`, `"linear"`, `"step"`,
    `"power_law"`, `"logarithmic"`, `"flat"`), and spaced repetition
    (`spaced_repetition_factor`, `spaced_repetition_base_interval_seconds`).
    Implements `Default` with COG-1 server defaults.

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
