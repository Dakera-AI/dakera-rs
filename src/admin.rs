//! Admin operations for the Dakera client.
//!
//! Provides methods for cluster management, cache, configuration, quotas,
//! slow queries, backups, and TTL management.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::DakeraClient;

// ============================================================================
// Cluster Types
// ============================================================================

/// Ops stats response — Read-scoped; works with read-only API keys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsStats {
    pub version: String,
    pub total_vectors: u64,
    pub namespace_count: u64,
    pub uptime_seconds: u64,
    pub timestamp: u64,
    pub state: String,
}

/// Cluster status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatus {
    pub cluster_id: String,
    pub state: String,
    pub node_count: u32,
    pub total_vectors: u64,
    pub namespace_count: u64,
    pub version: String,
    pub timestamp: u64,
    /// Redis connectivity status (OPS-3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redis_healthy: Option<bool>,
}

/// Node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: String,
    pub address: String,
    pub role: String,
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub vector_count: u64,
    pub memory_bytes: u64,
    #[serde(default)]
    pub cpu_percent: f32,
    #[serde(default)]
    pub memory_percent: f32,
    pub last_heartbeat: u64,
}

/// Node list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeListResponse {
    pub nodes: Vec<NodeInfo>,
    pub total: u32,
}

// ============================================================================
// Namespace Admin Types
// ============================================================================

/// Index statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub index_type: String,
    pub is_built: bool,
    pub size_bytes: u64,
    pub indexed_vectors: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_rebuild: Option<u64>,
}

/// Detailed namespace statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceAdminInfo {
    pub name: String,
    pub vector_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension: Option<usize>,
    pub index_type: String,
    pub storage_bytes: u64,
    pub document_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<u64>,
    pub index_stats: IndexStats,
}

/// Namespace list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceListResponse {
    pub namespaces: Vec<NamespaceAdminInfo>,
    pub total: u64,
    pub total_vectors: u64,
}

/// Optimize namespace request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeRequest {
    #[serde(default)]
    pub force: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_index_type: Option<String>,
}

/// Optimize namespace response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    pub message: String,
}

// ============================================================================
// Index Admin Types
// ============================================================================

/// Index statistics for all namespaces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatsResponse {
    pub namespaces: HashMap<String, IndexStats>,
    pub total_indexed_vectors: u64,
    pub total_size_bytes: u64,
}

/// Rebuild index request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebuildIndexRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_type: Option<String>,
    #[serde(default)]
    pub force: bool,
}

/// Rebuild index response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebuildIndexResponse {
    pub success: bool,
    pub job_id: String,
    pub message: String,
}

// ============================================================================
// Cache Admin Types
// ============================================================================

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub enabled: bool,
    pub cache_type: String,
    pub entries: u64,
    pub size_bytes: u64,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub evictions: u64,
}

/// Clear cache request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearCacheRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

/// Clear cache response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearCacheResponse {
    pub success: bool,
    pub entries_cleared: u64,
    pub message: String,
}

// ============================================================================
// Configuration Types
// ============================================================================

/// Runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_vectors_per_namespace: Option<u64>,
    pub default_index_type: String,
    pub cache_enabled: bool,
    pub cache_max_size_bytes: u64,
    pub rate_limit_enabled: bool,
    pub rate_limit_rps: u32,
    pub query_timeout_ms: u64,
    /// Whether AutoPilot background tasks (dedup + consolidation) are enabled
    #[serde(default = "default_true")]
    pub autopilot_enabled: bool,
    /// Cosine-similarity threshold for AutoPilot deduplication (0.0–1.0)
    #[serde(default = "default_dedup_threshold")]
    pub autopilot_dedup_threshold: f32,
    /// How often AutoPilot deduplication runs (hours)
    #[serde(default = "default_dedup_interval")]
    pub autopilot_dedup_interval_hours: u64,
    /// How often AutoPilot consolidation runs (hours)
    #[serde(default = "default_consolidation_interval")]
    pub autopilot_consolidation_interval_hours: u64,
}

fn default_true() -> bool {
    true
}
fn default_dedup_threshold() -> f32 {
    0.93
}
fn default_dedup_interval() -> u64 {
    6
}
fn default_consolidation_interval() -> u64 {
    12
}

/// Update configuration response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfigResponse {
    pub success: bool,
    pub config: RuntimeConfig,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

// ============================================================================
// Quota Types
// ============================================================================

/// Quota configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_vectors: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_storage_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_queries_per_minute: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_writes_per_minute: Option<u64>,
}

/// Quota usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaUsage {
    #[serde(default)]
    pub current_vectors: u64,
    #[serde(default)]
    pub current_storage_bytes: u64,
    #[serde(default)]
    pub queries_this_minute: u64,
    #[serde(default)]
    pub writes_this_minute: u64,
}

/// Quota status for a namespace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaStatus {
    pub namespace: String,
    pub config: QuotaConfig,
    pub usage: QuotaUsage,
}

/// Quota list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaListResponse {
    pub quotas: Vec<QuotaStatus>,
    pub total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_config: Option<QuotaConfig>,
}

// ============================================================================
// Slow Query Types
// ============================================================================

/// Slow query entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowQueryEntry {
    pub id: String,
    pub timestamp: u64,
    pub namespace: String,
    pub query_type: String,
    pub duration_ms: f64,
    #[serde(default)]
    pub parameters: Option<serde_json::Value>,
    #[serde(default)]
    pub results_count: u64,
    #[serde(default)]
    pub vectors_scanned: u64,
}

/// Slow query list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowQueryListResponse {
    pub queries: Vec<SlowQueryEntry>,
    pub total: u64,
    pub threshold_ms: f64,
}

// ============================================================================
// Backup Types
// ============================================================================

/// Backup information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub backup_id: String,
    pub name: String,
    pub backup_type: String,
    pub status: String,
    pub namespaces: Vec<String>,
    pub vector_count: u64,
    pub size_bytes: u64,
    pub created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub encrypted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>,
}

/// List backups response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupListResponse {
    pub backups: Vec<BackupInfo>,
    pub total: u64,
}

/// Create backup request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBackupRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespaces: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypt: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>,
}

/// Create backup response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBackupResponse {
    pub backup: BackupInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_completion: Option<u64>,
}

/// Restore backup request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreBackupRequest {
    pub backup_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_namespaces: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overwrite: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point_in_time: Option<u64>,
}

/// Restore backup response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreBackupResponse {
    pub restore_id: String,
    pub status: String,
    pub backup_id: String,
    pub namespaces: Vec<String>,
    pub started_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_completion: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_percent: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vectors_restored: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ============================================================================
// AutoPilot Types (PILOT-1 / PILOT-2 / PILOT-3)
// ============================================================================

/// AutoPilot configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPilotConfig {
    pub enabled: bool,
    pub dedup_threshold: f32,
    pub dedup_interval_hours: u64,
    pub consolidation_interval_hours: u64,
}

/// Result snapshot from a deduplication cycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupResultSnapshot {
    pub namespaces_processed: usize,
    pub memories_scanned: usize,
    pub duplicates_removed: usize,
}

/// Result snapshot from a consolidation cycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationResultSnapshot {
    pub namespaces_processed: usize,
    pub memories_scanned: usize,
    pub clusters_merged: usize,
    pub memories_consolidated: usize,
}

/// PILOT-1: AutoPilot status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPilotStatusResponse {
    pub config: AutoPilotConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_dedup_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_consolidation_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_dedup: Option<DedupResultSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_consolidation: Option<ConsolidationResultSnapshot>,
    pub total_dedup_removed: u64,
    pub total_consolidated: u64,
}

/// PILOT-2: AutoPilot configuration update request (all fields optional)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutoPilotConfigRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dedup_threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dedup_interval_hours: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consolidation_interval_hours: Option<u64>,
}

/// PILOT-2: AutoPilot configuration update response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPilotConfigResponse {
    pub success: bool,
    pub config: AutoPilotConfig,
    pub message: String,
}

/// PILOT-3: Trigger action
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoPilotTriggerAction {
    Dedup,
    Consolidate,
    All,
}

/// PILOT-3: Trigger request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPilotTriggerRequest {
    pub action: AutoPilotTriggerAction,
}

/// Dedup result returned by a manual trigger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPilotDedupResult {
    pub namespaces_processed: usize,
    pub memories_scanned: usize,
    pub duplicates_removed: usize,
}

/// Consolidation result returned by a manual trigger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPilotConsolidationResult {
    pub namespaces_processed: usize,
    pub memories_scanned: usize,
    pub clusters_merged: usize,
    pub memories_consolidated: usize,
}

/// PILOT-3: Trigger response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPilotTriggerResponse {
    pub success: bool,
    pub action: AutoPilotTriggerAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dedup: Option<AutoPilotDedupResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consolidation: Option<AutoPilotConsolidationResult>,
    pub message: String,
}

// ============================================================================
// Decay Engine Types (DECAY-1 / DECAY-2)
// ============================================================================

/// DECAY-1: Current decay configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayConfigResponse {
    /// Decay strategy: "exponential", "linear", or "step"
    pub strategy: String,
    /// Half-life in hours
    pub half_life_hours: f64,
    /// Minimum importance threshold; memories below are hard-deleted on next cycle
    pub min_importance: f32,
}

/// DECAY-1: Runtime configuration update request (all fields optional)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DecayConfigUpdateRequest {
    /// Decay strategy: "exponential", "linear", or "step"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    /// Half-life in hours (must be > 0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub half_life_hours: Option<f64>,
    /// Minimum importance threshold 0.0–1.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_importance: Option<f32>,
}

/// DECAY-1: Runtime configuration update response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayConfigUpdateResponse {
    pub success: bool,
    pub config: DecayConfigResponse,
    pub message: String,
}

/// DECAY-2: Stats from a single decay cycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastDecayCycleStats {
    pub namespaces_processed: usize,
    pub memories_processed: usize,
    pub memories_decayed: usize,
    pub memories_deleted: usize,
}

/// DECAY-2: Decay activity counters and last-cycle snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayStatsResponse {
    /// Total memories whose importance was lowered by decay (all-time)
    pub total_decayed: u64,
    /// Total memories hard-deleted by decay or TTL expiry (all-time)
    pub total_deleted: u64,
    /// Unix timestamp of the last decay cycle (None if never run)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<u64>,
    /// Number of decay cycles completed since startup
    pub cycles_run: u64,
    /// Stats from the most recent decay cycle (None if never run)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_cycle: Option<LastDecayCycleStats>,
}

// ============================================================================
// TTL Types
// ============================================================================

/// TTL cleanup request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtlCleanupRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

/// TTL cleanup response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtlCleanupResponse {
    pub success: bool,
    pub vectors_removed: u64,
    pub namespaces_cleaned: Vec<String>,
    pub message: String,
}

/// TTL statistics for a namespace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtlStats {
    pub namespace: String,
    pub vectors_with_ttl: u64,
    pub expiring_within_hour: u64,
    pub expiring_within_day: u64,
    pub expired_pending_cleanup: u64,
}

/// TTL statistics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtlStatsResponse {
    pub namespaces: Vec<TtlStats>,
    pub total_with_ttl: u64,
    pub total_expired: u64,
}

// ============================================================================
// Admin Client Methods
// ============================================================================

impl DakeraClient {
    // ====================================================================
    // Cluster Management
    // ====================================================================

    /// Get server stats (version, total_vectors, namespace_count, uptime_seconds, timestamp).
    ///
    /// Requires Read scope — works with read-only API keys, unlike `cluster_status`.
    pub async fn ops_stats(&self) -> Result<OpsStats> {
        let url = format!("{}/v1/ops/stats", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get Prometheus metrics in text exposition format (INFRA-3).
    ///
    /// Requires Admin scope. Returns the raw Prometheus text exposition
    /// format string suitable for scraping by a Prometheus server.
    pub async fn ops_metrics(&self) -> Result<String> {
        let url = format!("{}/v1/ops/metrics", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_text_response(response).await
    }

    /// Get cluster status overview
    pub async fn cluster_status(&self) -> Result<ClusterStatus> {
        let url = format!("{}/v1/admin/cluster/status", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// List cluster nodes
    pub async fn cluster_nodes(&self) -> Result<NodeListResponse> {
        let url = format!("{}/v1/admin/cluster/nodes", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // Namespace Administration
    // ====================================================================

    /// List all namespaces with detailed admin statistics
    pub async fn list_namespaces_admin(&self) -> Result<NamespaceListResponse> {
        let url = format!("{}/v1/admin/namespaces", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Delete an entire namespace and all its data
    pub async fn delete_namespace_admin(&self, namespace: &str) -> Result<serde_json::Value> {
        let url = format!("{}/v1/admin/namespaces/{}", self.base_url, namespace);
        let response = self.client.delete(&url).send().await?;
        self.handle_response(response).await
    }

    /// Optimize a namespace
    pub async fn optimize_namespace(
        &self,
        namespace: &str,
        request: OptimizeRequest,
    ) -> Result<OptimizeResponse> {
        let url = format!(
            "{}/v1/admin/namespaces/{}/optimize",
            self.base_url, namespace
        );
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // Index Management
    // ====================================================================

    /// Get index statistics for all namespaces
    pub async fn index_stats(&self) -> Result<IndexStatsResponse> {
        let url = format!("{}/v1/admin/indexes/stats", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Rebuild indexes
    pub async fn rebuild_indexes(
        &self,
        request: RebuildIndexRequest,
    ) -> Result<RebuildIndexResponse> {
        let url = format!("{}/v1/admin/indexes/rebuild", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // Cache Management
    // ====================================================================

    /// Get cache statistics
    pub async fn cache_stats(&self) -> Result<CacheStats> {
        let url = format!("{}/v1/admin/cache/stats", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Clear cache, optionally for a specific namespace
    pub async fn cache_clear(&self, namespace: Option<&str>) -> Result<ClearCacheResponse> {
        let url = format!("{}/v1/admin/cache/clear", self.base_url);
        let request = ClearCacheRequest {
            namespace: namespace.map(|s| s.to_string()),
        };
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // Configuration
    // ====================================================================

    /// Get runtime configuration
    pub async fn get_config(&self) -> Result<RuntimeConfig> {
        let url = format!("{}/v1/admin/config", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Update runtime configuration
    pub async fn update_config(
        &self,
        updates: HashMap<String, serde_json::Value>,
    ) -> Result<UpdateConfigResponse> {
        let url = format!("{}/v1/admin/config", self.base_url);
        let response = self.client.put(&url).json(&updates).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // Quotas
    // ====================================================================

    /// List all namespace quotas
    pub async fn get_quotas(&self) -> Result<QuotaListResponse> {
        let url = format!("{}/v1/admin/quotas", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get quota for a specific namespace
    pub async fn get_quota(&self, namespace: &str) -> Result<QuotaStatus> {
        let url = format!("{}/v1/admin/quotas/{}", self.base_url, namespace);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Set quota for a specific namespace
    pub async fn set_quota(
        &self,
        namespace: &str,
        config: QuotaConfig,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/v1/admin/quotas/{}", self.base_url, namespace);
        let request = serde_json::json!({ "config": config });
        let response = self.client.put(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Delete quota for a specific namespace
    pub async fn delete_quota(&self, namespace: &str) -> Result<serde_json::Value> {
        let url = format!("{}/v1/admin/quotas/{}", self.base_url, namespace);
        let response = self.client.delete(&url).send().await?;
        self.handle_response(response).await
    }

    /// Update quotas (alias for set_quota on default)
    pub async fn update_quotas(&self, config: Option<QuotaConfig>) -> Result<serde_json::Value> {
        let url = format!("{}/v1/admin/quotas/default", self.base_url);
        let request = serde_json::json!({ "config": config });
        let response = self.client.put(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // Slow Queries
    // ====================================================================

    /// List recent slow queries
    pub async fn slow_queries(
        &self,
        limit: Option<usize>,
        namespace: Option<&str>,
        query_type: Option<&str>,
    ) -> Result<SlowQueryListResponse> {
        let mut url = format!("{}/v1/admin/slow-queries", self.base_url);
        let mut params = Vec::new();
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if let Some(ns) = namespace {
            params.push(format!("namespace={}", ns));
        }
        if let Some(qt) = query_type {
            params.push(format!("query_type={}", qt));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get slow query summary and patterns
    pub async fn slow_query_summary(&self) -> Result<serde_json::Value> {
        let url = format!("{}/v1/admin/slow-queries/summary", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Clear slow query log
    pub async fn clear_slow_queries(&self) -> Result<serde_json::Value> {
        let url = format!("{}/v1/admin/slow-queries", self.base_url);
        let response = self.client.delete(&url).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // Backups
    // ====================================================================

    /// Create a new backup
    pub async fn create_backup(
        &self,
        request: CreateBackupRequest,
    ) -> Result<CreateBackupResponse> {
        let url = format!("{}/v1/admin/backups", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// List all backups
    pub async fn list_backups(&self) -> Result<BackupListResponse> {
        let url = format!("{}/v1/admin/backups", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get backup details by ID
    pub async fn get_backup(&self, backup_id: &str) -> Result<BackupInfo> {
        let url = format!("{}/v1/admin/backups/{}", self.base_url, backup_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Restore from a backup
    pub async fn restore_backup(
        &self,
        request: RestoreBackupRequest,
    ) -> Result<RestoreBackupResponse> {
        let url = format!("{}/v1/admin/backups/restore", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Delete a backup
    pub async fn delete_backup(&self, backup_id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/v1/admin/backups/{}", self.base_url, backup_id);
        let response = self.client.delete(&url).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // TTL Management
    // ====================================================================

    /// Configure TTL for a namespace.
    pub async fn configure_ttl(
        &self,
        namespace: &str,
        ttl_seconds: u64,
        strategy: Option<&str>,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/v1/admin/namespaces/{}/ttl", self.base_url, namespace);
        let mut body = serde_json::json!({ "ttl_seconds": ttl_seconds });
        if let Some(s) = strategy {
            body["strategy"] = serde_json::Value::String(s.to_string());
        }
        let response = self.client.post(&url).json(&body).send().await?;
        self.handle_response(response).await
    }

    /// Run TTL cleanup on expired vectors
    pub async fn ttl_cleanup(&self, namespace: Option<&str>) -> Result<TtlCleanupResponse> {
        let url = format!("{}/v1/admin/ttl/cleanup", self.base_url);
        let request = TtlCleanupRequest {
            namespace: namespace.map(|s| s.to_string()),
        };
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Get TTL statistics
    pub async fn ttl_stats(&self) -> Result<TtlStatsResponse> {
        let url = format!("{}/v1/admin/ttl/stats", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // AutoPilot Management (PILOT-1 / PILOT-2 / PILOT-3)
    // ====================================================================

    /// Get AutoPilot status: current config and last-run statistics (PILOT-1)
    pub async fn autopilot_status(&self) -> Result<AutoPilotStatusResponse> {
        let url = format!("{}/v1/admin/autopilot/status", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Update AutoPilot configuration at runtime (PILOT-2)
    ///
    /// All fields are optional — omit any field to keep its current value.
    pub async fn autopilot_update_config(
        &self,
        request: AutoPilotConfigRequest,
    ) -> Result<AutoPilotConfigResponse> {
        let url = format!("{}/v1/admin/autopilot/config", self.base_url);
        let response = self.client.put(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Manually trigger an AutoPilot dedup or consolidation cycle (PILOT-3)
    ///
    /// Use `AutoPilotTriggerAction::Dedup`, `::Consolidate`, or `::All`.
    /// The cycle runs synchronously and returns inline results.
    pub async fn autopilot_trigger(
        &self,
        action: AutoPilotTriggerAction,
    ) -> Result<AutoPilotTriggerResponse> {
        let url = format!("{}/v1/admin/autopilot/trigger", self.base_url);
        let request = AutoPilotTriggerRequest { action };
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // Decay Engine Management (DECAY-1 / DECAY-2)
    // ====================================================================

    /// Get current decay engine configuration (DECAY-1).
    ///
    /// Returns the active strategy, half-life, and minimum importance threshold.
    /// Requires Admin scope.
    pub async fn decay_config(&self) -> Result<DecayConfigResponse> {
        let url = format!("{}/v1/admin/decay/config", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Update decay engine configuration at runtime (DECAY-1).
    ///
    /// Changes take effect on the next decay cycle — no restart required.
    /// All fields are optional; omit any to keep its current value.
    /// Requires Admin scope.
    pub async fn decay_update_config(
        &self,
        request: DecayConfigUpdateRequest,
    ) -> Result<DecayConfigUpdateResponse> {
        let url = format!("{}/v1/admin/decay/config", self.base_url);
        let response = self.client.put(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Get decay activity counters and last-cycle snapshot (DECAY-2).
    ///
    /// Returns cumulative totals (memories decayed/deleted, cycles run) and
    /// per-cycle statistics from the most recent run. Requires Admin scope.
    pub async fn decay_stats(&self) -> Result<DecayStatsResponse> {
        let url = format!("{}/v1/admin/decay/stats", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // Product KPI Snapshot (OBS-2)
    // ====================================================================

    /// Return a point-in-time product KPI snapshot (OBS-2).
    ///
    /// Calls `GET /v1/kpis`. Returns 8 operational metrics covering latency,
    /// error rate, and retention. Sub-millisecond — served from in-memory
    /// counters. Requires Admin scope.
    pub async fn get_kpis(&self) -> Result<KpiSnapshot> {
        let url = format!("{}/kpis", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // CE-54: Fulltext Reindex
    // ========================================================================

    /// Backfill the BM25 fulltext index for memories stored before CE-12 auto-indexing (CE-54).
    ///
    /// Calls `POST /v1/admin/fulltext/reindex`. Requires Admin scope.
    ///
    /// Scans all memories in `namespace` (or every agent namespace when `None`) and adds
    /// any missing from the BM25 index. Safe to call multiple times — already-indexed
    /// memories are counted in `total_skipped` and not re-processed.
    pub async fn admin_fulltext_reindex(
        &self,
        namespace: Option<&str>,
    ) -> Result<FulltextReindexResponse> {
        let url = format!("{}/v1/admin/fulltext/reindex", self.base_url);
        let body = serde_json::json!({ "namespace": namespace });
        let response = self.client.post(&url).json(&body).send().await?;
        self.handle_response(response).await
    }

    // =========================================================================
    // Cluster & Maintenance
    // =========================================================================

    /// GET /v1/admin/cluster/replication — cluster replication status.
    pub async fn admin_cluster_replication(&self) -> Result<crate::types::ReplicationStatus> {
        let url = format!("{}/v1/admin/cluster/replication", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// GET /v1/admin/cluster/shards — list shards.
    pub async fn admin_list_shards(&self) -> Result<crate::types::ShardListResponse> {
        let url = format!("{}/v1/admin/cluster/shards", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// POST /v1/admin/cluster/shards/rebalance — rebalance shards.
    pub async fn admin_rebalance_shards(
        &self,
        request: crate::types::ShardRebalanceRequest,
    ) -> Result<crate::types::ShardRebalanceResponse> {
        let url = format!("{}/v1/admin/cluster/shards/rebalance", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// GET /v1/admin/cluster/maintenance — maintenance mode status.
    pub async fn admin_maintenance_status(&self) -> Result<crate::types::MaintenanceStatus> {
        let url = format!("{}/v1/admin/cluster/maintenance", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// POST /v1/admin/cluster/maintenance/enable — enable maintenance mode.
    pub async fn admin_enable_maintenance(
        &self,
        request: crate::types::EnableMaintenanceRequest,
    ) -> Result<crate::types::MaintenanceStatus> {
        let url = format!("{}/v1/admin/cluster/maintenance/enable", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// POST /v1/admin/cluster/maintenance/disable — disable maintenance mode.
    pub async fn admin_disable_maintenance(
        &self,
        request: crate::types::DisableMaintenanceRequest,
    ) -> Result<crate::types::MaintenanceStatus> {
        let url = format!("{}/v1/admin/cluster/maintenance/disable", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // =========================================================================
    // Quotas
    // =========================================================================

    /// GET /v1/admin/quotas — list all namespace quotas.
    pub async fn admin_list_quotas(&self) -> Result<crate::types::QuotaListResponse> {
        let url = format!("{}/v1/admin/quotas", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// GET /v1/admin/quotas/default — get default quota configuration.
    pub async fn admin_get_default_quota(&self) -> Result<crate::types::DefaultQuotaResponse> {
        let url = format!("{}/v1/admin/quotas/default", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// PUT /v1/admin/quotas/default — set default quota configuration.
    pub async fn admin_set_default_quota(
        &self,
        request: crate::types::SetDefaultQuotaRequest,
    ) -> Result<crate::types::SetQuotaResponse> {
        let url = format!("{}/v1/admin/quotas/default", self.base_url);
        let response = self.client.put(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// GET /v1/admin/quotas/{namespace} — get namespace quota.
    pub async fn admin_get_quota(&self, namespace: &str) -> Result<crate::types::QuotaStatus> {
        let url = format!("{}/v1/admin/quotas/{}", self.base_url, namespace);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// PUT /v1/admin/quotas/{namespace} — set namespace quota.
    pub async fn admin_set_quota(
        &self,
        namespace: &str,
        request: crate::types::SetQuotaRequest,
    ) -> Result<crate::types::SetQuotaResponse> {
        let url = format!("{}/v1/admin/quotas/{}", self.base_url, namespace);
        let response = self.client.put(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// DELETE /v1/admin/quotas/{namespace} — remove namespace quota.
    pub async fn admin_delete_quota(&self, namespace: &str) -> Result<serde_json::Value> {
        let url = format!("{}/v1/admin/quotas/{}", self.base_url, namespace);
        let response = self.client.delete(&url).send().await?;
        self.handle_response(response).await
    }

    /// POST /v1/admin/quotas/{namespace}/check — check if operation would exceed quota.
    pub async fn admin_check_quota(
        &self,
        namespace: &str,
        request: crate::types::QuotaCheckRequest,
    ) -> Result<crate::types::QuotaCheckResult> {
        let url = format!("{}/v1/admin/quotas/{}/check", self.base_url, namespace);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // =========================================================================
    // Slow Queries
    // =========================================================================

    /// GET /v1/admin/slow-queries — list recent slow queries.
    pub async fn admin_list_slow_queries(
        &self,
        namespace: Option<&str>,
        query_type: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<serde_json::Value>> {
        let mut url = format!("{}/v1/admin/slow-queries", self.base_url);
        let mut params = Vec::new();
        if let Some(ns) = namespace {
            params.push(format!("namespace={}", ns));
        }
        if let Some(qt) = query_type {
            params.push(format!("query_type={}", qt));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// GET /v1/admin/slow-queries/summary — slow query summary.
    pub async fn admin_slow_query_summary(&self) -> Result<serde_json::Value> {
        let url = format!("{}/v1/admin/slow-queries/summary", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// DELETE /v1/admin/slow-queries — clear slow query log.
    pub async fn admin_clear_slow_queries(
        &self,
        namespace: Option<&str>,
    ) -> Result<serde_json::Value> {
        let mut url = format!("{}/v1/admin/slow-queries", self.base_url);
        if let Some(ns) = namespace {
            url.push_str(&format!("?namespace={}", ns));
        }
        let response = self.client.delete(&url).send().await?;
        self.handle_response(response).await
    }

    /// PATCH /v1/admin/slow-queries/config — update slow query configuration.
    pub async fn admin_update_slow_query_config(
        &self,
        config: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/v1/admin/slow-queries/config", self.base_url);
        let response = self.client.patch(&url).json(&config).send().await?;
        self.handle_response(response).await
    }

    // =========================================================================
    // Backups
    // =========================================================================

    /// GET /v1/admin/backups — list all backups.
    pub async fn admin_list_backups(&self) -> Result<crate::types::BackupListResponse> {
        let url = format!("{}/v1/admin/backups", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// POST /v1/admin/backups — create a new backup.
    pub async fn admin_create_backup(
        &self,
        request: crate::types::CreateBackupRequest,
    ) -> Result<crate::types::CreateBackupResponse> {
        let url = format!("{}/v1/admin/backups", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// GET /v1/admin/backups/{id} — get backup details.
    pub async fn admin_get_backup(&self, backup_id: &str) -> Result<crate::types::AdminBackupInfo> {
        let url = format!("{}/v1/admin/backups/{}", self.base_url, backup_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// DELETE /v1/admin/backups/{id} — delete a backup.
    pub async fn admin_delete_backup(&self, backup_id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/v1/admin/backups/{}", self.base_url, backup_id);
        let response = self.client.delete(&url).send().await?;
        self.handle_response(response).await
    }

    /// GET /v1/admin/backups/schedule — get backup schedule.
    pub async fn admin_get_backup_schedule(&self) -> Result<crate::types::BackupSchedule> {
        let url = format!("{}/v1/admin/backups/schedule", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// POST /v1/admin/backups/schedule — update backup schedule.
    pub async fn admin_update_backup_schedule(
        &self,
        request: crate::types::UpdateBackupScheduleRequest,
    ) -> Result<crate::types::BackupSchedule> {
        let url = format!("{}/v1/admin/backups/schedule", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// POST /v1/admin/backups/restore — restore from backup.
    pub async fn admin_restore_backup(
        &self,
        request: crate::types::RestoreBackupRequest,
    ) -> Result<crate::types::RestoreBackupResponse> {
        let url = format!("{}/v1/admin/backups/restore", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// GET /v1/admin/backups/restore/{id} — restore operation status.
    pub async fn admin_get_restore_status(
        &self,
        restore_id: &str,
    ) -> Result<crate::types::RestoreBackupResponse> {
        let url = format!("{}/v1/admin/backups/restore/{}", self.base_url, restore_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // =========================================================================
    // Ops — Diagnostics & Jobs
    // =========================================================================

    /// GET /ops/diagnostics — system diagnostics.
    pub async fn ops_diagnostics(&self) -> Result<serde_json::Value> {
        let url = format!("{}/ops/diagnostics", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// GET /ops/jobs — list background jobs.
    pub async fn ops_list_jobs(&self) -> Result<Vec<crate::types::JobInfo>> {
        let url = format!("{}/ops/jobs", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// GET /ops/jobs/{id} — get job status.
    pub async fn ops_get_job(&self, job_id: &str) -> Result<crate::types::JobInfo> {
        let url = format!("{}/ops/jobs/{}", self.base_url, job_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// POST /ops/compact — trigger compaction.
    pub async fn ops_compact(
        &self,
        request: crate::types::CompactionRequest,
    ) -> Result<crate::types::CompactionResponse> {
        let url = format!("{}/ops/compact", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// POST /ops/shutdown — request graceful shutdown.
    pub async fn ops_shutdown(&self) -> Result<serde_json::Value> {
        let url = format!("{}/ops/shutdown", self.base_url);
        let response = self.client.post(&url).send().await?;
        self.handle_response(response).await
    }

    // =========================================================================
    // Backup Download / Upload
    // =========================================================================

    /// Download a backup as gzipped bytes via `GET /v1/admin/backups/{id}/download`.
    pub async fn download_backup(&self, backup_id: &str) -> Result<Vec<u8>> {
        let url = format!("{}/v1/admin/backups/{}/download", self.base_url, backup_id);
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::ClientError::Server {
                status: status.as_u16(),
                message: body,
                code: None,
            });
        }
        Ok(response.bytes().await?.to_vec())
    }

    /// Upload a backup from gzipped bytes via `POST /v1/admin/backups/upload`.
    pub async fn upload_backup(&self, data: Vec<u8>) -> Result<crate::types::CreateBackupResponse> {
        let url = format!("{}/v1/admin/backups/upload", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/gzip")
            .body(data)
            .send()
            .await?;
        self.handle_response(response).await
    }

    // =========================================================================
    // Storage Tier Overview
    // =========================================================================

    /// Get storage tier overview via `GET /v1/admin/storage/tiers`.
    pub async fn storage_tier_overview(&self) -> Result<crate::types::StorageTierOverview> {
        let url = format!("{}/v1/admin/storage/tiers", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // =========================================================================
    // Background Activity
    // =========================================================================

    /// Get background activity metrics via `GET /v1/admin/background-activity`.
    pub async fn background_activity(&self) -> Result<serde_json::Value> {
        let url = format!("{}/v1/admin/background-activity", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // =========================================================================
    // Memory Type Stats
    // =========================================================================

    /// Get per-type memory statistics via `GET /v1/admin/memory-type-stats`.
    pub async fn memory_type_stats(&self) -> Result<crate::types::MemoryTypeStatsResponse> {
        let url = format!("{}/v1/admin/memory-type-stats", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // =========================================================================
    // Migrate Namespace Dimensions
    // =========================================================================

    /// Migrate namespace embedding dimensions via `POST /v1/admin/namespaces/migrate-dimensions`.
    pub async fn migrate_namespace_dimensions(
        &self,
        request: crate::types::MigrateNamespaceDimensionsRequest,
    ) -> Result<crate::types::MigrateDimensionsResponse> {
        let url = format!("{}/v1/admin/namespaces/migrate-dimensions", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // =========================================================================
    // ReembedJob Force-Drain (v0.11.82+)
    // =========================================================================

    /// Synchronously drain all static vectors to full ONNX quality via
    /// `POST /v1/admin/reembed/drain` (v0.11.82+).
    ///
    /// Runs the re-embedding upgrade loop until zero `_embedding_kind=static`
    /// candidates remain across all namespaces, or `request.timeout_secs` elapses.
    /// Requires Admin scope. Useful as a pre-benchmark steady-state gate when
    /// `DAKERA_TIERED=1`.
    ///
    /// A [`DrainReembedResponse::remaining`] of `0` guarantees all vectors are at
    /// full ONNX quality.
    pub async fn drain_reembed(
        &self,
        request: crate::types::DrainReembedRequest,
    ) -> Result<crate::types::DrainReembedResponse> {
        let url = format!("{}/v1/admin/reembed/drain", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Return the count of static vectors pending re-embedding via
    /// `GET /v1/admin/reembed/static-count` (v0.11.91+).
    ///
    /// Operators can poll this alongside [`drain_reembed`][Self::drain_reembed]
    /// to monitor drain progress. A [`StaticCountResponse::static_count`] of `0`
    /// means steady state — all vectors are at full ONNX quality.
    ///
    /// Requires Admin scope.
    pub async fn admin_reembed_static_count(
        &self,
    ) -> Result<crate::types::StaticCountResponse> {
        let url = format!("{}/v1/admin/reembed/static-count", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }
}

// ============================================================================
// Product KPI Snapshot (OBS-2)
// ============================================================================

/// Point-in-time product KPI snapshot returned by `GET /v1/kpis` (OBS-2).
///
/// All latency values are in milliseconds. Rate/percentage values are in the
/// range `0.0`–`100.0`. Integer counts are unsigned.
///
/// Requires Admin scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KpiSnapshot {
    /// Median recall latency across all namespaces over the last minute (ms).
    pub recall_latency_p50_ms: f64,
    /// 99th-percentile recall latency across all namespaces over the last minute (ms).
    pub recall_latency_p99_ms: f64,
    /// Median store latency across all namespaces over the last minute (ms).
    pub store_latency_p50_ms: f64,
    /// 5xx error rate as a percentage of total API requests over the last minute.
    pub api_error_rate_5xx_pct: f64,
    /// Distinct agent identifiers that stored or recalled a memory in the last 24 hours.
    pub active_agents_count: u64,
    /// Total sessions created in the rolling 7-day window.
    pub session_count_week: u64,
    /// Current number of nodes in the cross-agent knowledge graph.
    pub cross_agent_network_node_count: u64,
    /// Percentage of memories created 7 days ago that are still active.
    pub memory_retention_7d_pct: f64,
}

// ============================================================================
// CE-54: Fulltext Reindex (Admin)
// ============================================================================

/// Per-namespace result from `POST /v1/admin/fulltext/reindex` (CE-54).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulltextReindexNamespaceResult {
    /// Namespace that was scanned.
    pub namespace: String,
    /// Total vectors examined.
    pub vectors_scanned: usize,
    /// Memories newly added to the BM25 index.
    pub newly_indexed: usize,
    /// Memories already in the BM25 index (skipped).
    pub already_indexed: usize,
    /// Memories that could not be parsed.
    pub parse_failures: usize,
}

/// Response from `POST /v1/admin/fulltext/reindex` (CE-54).
///
/// Returned by [`DakeraClient::admin_fulltext_reindex`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulltextReindexResponse {
    /// Number of namespaces scanned.
    pub namespaces_processed: usize,
    /// Total memories newly added to BM25 across all namespaces.
    pub total_indexed: usize,
    /// Total memories already in the BM25 index (skipped).
    pub total_skipped: usize,
    /// Per-namespace breakdown.
    pub details: Vec<FulltextReindexNamespaceResult>,
}
