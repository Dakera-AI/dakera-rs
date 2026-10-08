//! Analytics operations for the Dakera client.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::types::UnavailableNamespace;
use crate::DakeraClient;

// ============================================================================
// Analytics Types
// ============================================================================

/// Analytics overview response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsOverview {
    pub total_queries: u64,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub queries_per_second: f64,
    pub error_rate: f64,
    pub cache_hit_rate: f64,
    pub storage_used_bytes: u64,
    pub total_vectors: u64,
    pub total_namespaces: u64,
    pub uptime_seconds: u64,
    /// Namespaces left out of this answer (an error, or no answer within the
    /// server's per-namespace deadline; server v0.12.2+). Empty when every
    /// namespace answered.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unavailable: Vec<UnavailableNamespace>,
}

/// Latency analytics response
///
/// The server answers `{buckets, avg_ms, p50_ms, p95_ms, p99_ms, period}`;
/// `max_ms` and `by_operation` are not part of it (0 / empty).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyAnalytics {
    pub period: String,
    pub avg_ms: f64,
    #[serde(default)]
    pub p50_ms: f64,
    #[serde(default)]
    pub p95_ms: f64,
    #[serde(default)]
    pub p99_ms: f64,
    #[serde(default)]
    pub max_ms: f64,
    #[serde(default)]
    pub by_operation: std::collections::HashMap<String, OperationLatency>,
    /// Latency histogram.
    #[serde(default)]
    pub buckets: Vec<LatencyBucket>,
}

/// One bucket of the latency histogram.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyBucket {
    pub lower_ms: f64,
    /// Upper bound; `None` for the open-ended last bucket.
    #[serde(default)]
    pub upper_ms: Option<f64>,
    pub count: u64,
    #[serde(default)]
    pub percentage: f64,
}

/// Per-operation latency stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationLatency {
    pub avg_ms: f64,
    pub p95_ms: f64,
    pub count: u64,
}

/// Throughput analytics response
///
/// The server answers `{queries_per_second, inserts_per_second,
/// deletes_per_second, data_points, period}`. `operations_per_second` is
/// their sum; `total_operations` and `by_operation` are not reported (0 /
/// empty).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputAnalytics {
    pub period: String,
    #[serde(default)]
    pub total_operations: u64,
    #[serde(default)]
    pub operations_per_second: f64,
    #[serde(default)]
    pub by_operation: std::collections::HashMap<String, u64>,
    #[serde(default)]
    pub queries_per_second: f64,
    #[serde(default)]
    pub inserts_per_second: f64,
    #[serde(default)]
    pub deletes_per_second: f64,
    /// Time series of the rates.
    #[serde(default)]
    pub data_points: Vec<ThroughputDataPoint>,
}

/// One sample of the throughput time series.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputDataPoint {
    pub timestamp: u64,
    pub queries_per_second: f64,
    pub inserts_per_second: f64,
    pub deletes_per_second: f64,
}

/// Storage analytics response
///
/// The server answers `{total_bytes, index_bytes, vector_bytes,
/// metadata_bytes, fulltext_bytes, namespace_breakdown}`. `data_bytes` is
/// vector + metadata + full-text bytes and `by_namespace` is built from
/// `namespace_breakdown`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageAnalytics {
    pub total_bytes: u64,
    pub index_bytes: u64,
    #[serde(default)]
    pub data_bytes: u64,
    #[serde(default)]
    pub by_namespace: std::collections::HashMap<String, NamespaceStorage>,
    #[serde(default)]
    pub vector_bytes: u64,
    #[serde(default)]
    pub metadata_bytes: u64,
    #[serde(default)]
    pub fulltext_bytes: u64,
    #[serde(default)]
    pub namespace_breakdown: Vec<NamespaceStorageInfo>,
    /// Namespaces left out of this answer (an error, or no answer within the
    /// server's per-namespace deadline; server v0.12.2+). Empty when every
    /// namespace answered.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unavailable: Vec<UnavailableNamespace>,
}

/// One namespace of the storage breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceStorageInfo {
    pub namespace: String,
    pub total_bytes: u64,
    pub vector_count: u64,
    #[serde(default)]
    pub dimension: Option<usize>,
}

/// Per-namespace storage stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceStorage {
    pub bytes: u64,
    pub vector_count: u64,
}

// ============================================================================
// Analytics Client Methods
// ============================================================================

impl DakeraClient {
    /// Get analytics overview
    pub async fn analytics_overview(
        &self,
        period: Option<&str>,
        namespace: Option<&str>,
    ) -> Result<AnalyticsOverview> {
        let mut url = format!("{}/v1/analytics/overview", self.base_url);
        let mut params = Vec::new();
        if let Some(p) = period {
            params.push(format!("period={}", p));
        }
        if let Some(ns) = namespace {
            params.push(format!("namespace={}", ns));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get latency analytics
    pub async fn analytics_latency(
        &self,
        period: Option<&str>,
        namespace: Option<&str>,
    ) -> Result<LatencyAnalytics> {
        let mut url = format!("{}/v1/analytics/latency", self.base_url);
        let mut params = Vec::new();
        if let Some(p) = period {
            params.push(format!("period={}", p));
        }
        if let Some(ns) = namespace {
            params.push(format!("namespace={}", ns));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get throughput analytics
    pub async fn analytics_throughput(
        &self,
        period: Option<&str>,
        namespace: Option<&str>,
    ) -> Result<ThroughputAnalytics> {
        let mut url = format!("{}/v1/analytics/throughput", self.base_url);
        let mut params = Vec::new();
        if let Some(p) = period {
            params.push(format!("period={}", p));
        }
        if let Some(ns) = namespace {
            params.push(format!("namespace={}", ns));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        let response = self.client.get(&url).send().await?;
        let mut t: ThroughputAnalytics = self.handle_response(response).await?;
        if t.operations_per_second == 0.0 {
            t.operations_per_second =
                t.queries_per_second + t.inserts_per_second + t.deletes_per_second;
        }
        Ok(t)
    }

    /// Get storage analytics
    pub async fn analytics_storage(&self, namespace: Option<&str>) -> Result<StorageAnalytics> {
        let mut url = format!("{}/v1/analytics/storage", self.base_url);
        if let Some(ns) = namespace {
            url.push_str(&format!("?namespace={}", ns));
        }
        let response = self.client.get(&url).send().await?;
        let mut st: StorageAnalytics = self.handle_response(response).await?;
        if st.data_bytes == 0 {
            st.data_bytes = st.vector_bytes + st.metadata_bytes + st.fulltext_bytes;
        }
        if st.by_namespace.is_empty() {
            st.by_namespace = st
                .namespace_breakdown
                .iter()
                .map(|n| {
                    (
                        n.namespace.clone(),
                        NamespaceStorage {
                            bytes: n.total_bytes,
                            vector_count: n.vector_count,
                        },
                    )
                })
                .collect();
        }
        Ok(st)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // AnalyticsOverview
    // -------------------------------------------------------------------------

    #[test]
    fn test_analytics_overview_deserializes_all_numeric_fields() {
        let json = r#"{
            "total_queries": 100,
            "avg_latency_ms": 12.5,
            "p95_latency_ms": 30.0,
            "p99_latency_ms": 60.0,
            "queries_per_second": 5.0,
            "error_rate": 0.01,
            "cache_hit_rate": 0.87,
            "storage_used_bytes": 1048576,
            "total_vectors": 95,
            "total_namespaces": 3,
            "uptime_seconds": 86400
        }"#;
        let overview: AnalyticsOverview = serde_json::from_str(json).unwrap();
        assert_eq!(overview.total_queries, 100);
        assert!((overview.avg_latency_ms - 12.5).abs() < 1e-6);
        assert!((overview.cache_hit_rate - 0.87).abs() < 1e-6);
        assert_eq!(overview.uptime_seconds, 86400);
    }

    // -------------------------------------------------------------------------
    // LatencyAnalytics
    // -------------------------------------------------------------------------

    #[test]
    fn test_latency_analytics_by_operation_defaults_empty() {
        let json = r#"{
            "period": "1h",
            "avg_ms": 10.0,
            "p50_ms": 8.0,
            "p95_ms": 25.0,
            "p99_ms": 50.0,
            "max_ms": 120.0
        }"#;
        let la: LatencyAnalytics = serde_json::from_str(json).unwrap();
        assert_eq!(la.period, "1h");
        assert!(la.by_operation.is_empty());
    }

    #[test]
    fn test_latency_analytics_with_by_operation() {
        let json = r#"{
            "period": "24h",
            "avg_ms": 15.0,
            "p50_ms": 12.0,
            "p95_ms": 40.0,
            "p99_ms": 80.0,
            "max_ms": 200.0,
            "by_operation": {
                "recall": {"avg_ms": 18.0, "p95_ms": 45.0, "count": 3000},
                "store": {"avg_ms": 10.0, "p95_ms": 30.0, "count": 2000}
            }
        }"#;
        let la: LatencyAnalytics = serde_json::from_str(json).unwrap();
        assert_eq!(la.by_operation.len(), 2);
        assert!((la.by_operation["recall"].avg_ms - 18.0).abs() < 1e-6);
        assert_eq!(la.by_operation["store"].count, 2000);
    }

    // -------------------------------------------------------------------------
    // OperationLatency
    // -------------------------------------------------------------------------

    #[test]
    fn test_operation_latency_deserializes() {
        let json = r#"{"avg_ms": 9.5, "p95_ms": 22.0, "count": 100}"#;
        let op: OperationLatency = serde_json::from_str(json).unwrap();
        assert!((op.avg_ms - 9.5).abs() < 1e-6);
        assert_eq!(op.count, 100);
    }

    // -------------------------------------------------------------------------
    // ThroughputAnalytics
    // -------------------------------------------------------------------------

    #[test]
    fn test_throughput_analytics_by_operation_defaults_empty() {
        let json = r#"{
            "period": "1h",
            "operations_per_second": 42.5,
            "total_operations": 153000
        }"#;
        let ta: ThroughputAnalytics = serde_json::from_str(json).unwrap();
        assert!((ta.operations_per_second - 42.5).abs() < 1e-6);
        assert!(ta.by_operation.is_empty());
    }

    // -------------------------------------------------------------------------
    // StorageAnalytics
    // -------------------------------------------------------------------------

    #[test]
    fn test_storage_analytics_by_namespace_defaults_empty() {
        let json = r#"{
            "total_bytes": 2097152,
            "index_bytes": 512000,
            "data_bytes": 1585152
        }"#;
        let sa: StorageAnalytics = serde_json::from_str(json).unwrap();
        assert_eq!(sa.total_bytes, 2097152);
        assert!(sa.by_namespace.is_empty());
    }

    #[test]
    fn test_storage_analytics_with_namespaces() {
        let json = r#"{
            "total_bytes": 4194304,
            "index_bytes": 1048576,
            "data_bytes": 3145728,
            "by_namespace": {
                "default": {"bytes": 2097152, "vector_count": 500},
                "archive": {"bytes": 2097152, "vector_count": 500}
            }
        }"#;
        let sa: StorageAnalytics = serde_json::from_str(json).unwrap();
        assert_eq!(sa.by_namespace.len(), 2);
        assert_eq!(sa.by_namespace["default"].vector_count, 500);
    }

    // -------------------------------------------------------------------------
    // NamespaceStorage
    // -------------------------------------------------------------------------

    #[test]
    fn test_namespace_storage_deserializes() {
        let json = r#"{"bytes": 1048576, "vector_count": 256}"#;
        let ns: NamespaceStorage = serde_json::from_str(json).unwrap();
        assert_eq!(ns.bytes, 1048576);
        assert_eq!(ns.vector_count, 256);
    }
}
