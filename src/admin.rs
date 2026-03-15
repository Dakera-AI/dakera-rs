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

    /// Get cluster status overview
    pub async fn cluster_status(&self) -> Result<ClusterStatus> {
        let url = format!("{}/admin/cluster/status", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// List cluster nodes
    pub async fn cluster_nodes(&self) -> Result<NodeListResponse> {
        let url = format!("{}/admin/cluster/nodes", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // Namespace Administration
    // ====================================================================

    /// List all namespaces with detailed admin statistics
    pub async fn list_namespaces_admin(&self) -> Result<NamespaceListResponse> {
        let url = format!("{}/admin/namespaces", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Delete an entire namespace and all its data
    pub async fn delete_namespace_admin(&self, namespace: &str) -> Result<serde_json::Value> {
        let url = format!("{}/admin/namespaces/{}", self.base_url, namespace);
        let response = self.client.delete(&url).send().await?;
        self.handle_response(response).await
    }

    /// Optimize a namespace
    pub async fn optimize_namespace(
        &self,
        namespace: &str,
        request: OptimizeRequest,
    ) -> Result<OptimizeResponse> {
        let url = format!("{}/admin/namespaces/{}/optimize", self.base_url, namespace);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // Index Management
    // ====================================================================

    /// Get index statistics for all namespaces
    pub async fn index_stats(&self) -> Result<IndexStatsResponse> {
        let url = format!("{}/admin/indexes/stats", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Rebuild indexes
    pub async fn rebuild_indexes(
        &self,
        request: RebuildIndexRequest,
    ) -> Result<RebuildIndexResponse> {
        let url = format!("{}/admin/indexes/rebuild", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // Cache Management
    // ====================================================================

    /// Get cache statistics
    pub async fn cache_stats(&self) -> Result<CacheStats> {
        let url = format!("{}/admin/cache/stats", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Clear cache, optionally for a specific namespace
    pub async fn cache_clear(&self, namespace: Option<&str>) -> Result<ClearCacheResponse> {
        let url = format!("{}/admin/cache/clear", self.base_url);
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
        let url = format!("{}/admin/config", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Update runtime configuration
    pub async fn update_config(
        &self,
        updates: HashMap<String, serde_json::Value>,
    ) -> Result<UpdateConfigResponse> {
        let url = format!("{}/admin/config", self.base_url);
        let response = self.client.put(&url).json(&updates).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // Quotas
    // ====================================================================

    /// List all namespace quotas
    pub async fn get_quotas(&self) -> Result<QuotaListResponse> {
        let url = format!("{}/admin/quotas", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get quota for a specific namespace
    pub async fn get_quota(&self, namespace: &str) -> Result<QuotaStatus> {
        let url = format!("{}/admin/quotas/{}", self.base_url, namespace);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Set quota for a specific namespace
    pub async fn set_quota(
        &self,
        namespace: &str,
        config: QuotaConfig,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/admin/quotas/{}", self.base_url, namespace);
        let request = serde_json::json!({ "config": config });
        let response = self.client.put(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Delete quota for a specific namespace
    pub async fn delete_quota(&self, namespace: &str) -> Result<serde_json::Value> {
        let url = format!("{}/admin/quotas/{}", self.base_url, namespace);
        let response = self.client.delete(&url).send().await?;
        self.handle_response(response).await
    }

    /// Update quotas (alias for set_quota on default)
    pub async fn update_quotas(&self, config: Option<QuotaConfig>) -> Result<serde_json::Value> {
        let url = format!("{}/admin/quotas/default", self.base_url);
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
        let mut url = format!("{}/admin/slow-queries", self.base_url);
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
        let url = format!("{}/admin/slow-queries/summary", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Clear slow query log
    pub async fn clear_slow_queries(&self) -> Result<serde_json::Value> {
        let url = format!("{}/admin/slow-queries", self.base_url);
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
        let url = format!("{}/admin/backups", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// List all backups
    pub async fn list_backups(&self) -> Result<BackupListResponse> {
        let url = format!("{}/admin/backups", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get backup details by ID
    pub async fn get_backup(&self, backup_id: &str) -> Result<BackupInfo> {
        let url = format!("{}/admin/backups/{}", self.base_url, backup_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Restore from a backup
    pub async fn restore_backup(
        &self,
        request: RestoreBackupRequest,
    ) -> Result<RestoreBackupResponse> {
        let url = format!("{}/admin/backups/restore", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Delete a backup
    pub async fn delete_backup(&self, backup_id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/admin/backups/{}", self.base_url, backup_id);
        let response = self.client.delete(&url).send().await?;
        self.handle_response(response).await
    }

    // ====================================================================
    // TTL Management
    // ====================================================================

    /// Run TTL cleanup on expired vectors
    pub async fn ttl_cleanup(&self, namespace: Option<&str>) -> Result<TtlCleanupResponse> {
        let url = format!("{}/admin/ttl/cleanup", self.base_url);
        let request = TtlCleanupRequest {
            namespace: namespace.map(|s| s.to_string()),
        };
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Get TTL statistics
    pub async fn ttl_stats(&self) -> Result<TtlStatsResponse> {
        let url = format!("{}/admin/ttl/stats", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }
}
