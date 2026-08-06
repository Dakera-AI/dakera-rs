//! Analytics operations for the Dakera client.

use serde::{Deserialize, Serialize};

use crate::error::Result;
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
}

/// Latency analytics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyAnalytics {
    pub period: String,
    pub avg_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    #[serde(default)]
    pub by_operation: std::collections::HashMap<String, OperationLatency>,
}

/// Per-operation latency stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationLatency {
    pub avg_ms: f64,
    pub p95_ms: f64,
    pub count: u64,
}

/// Throughput analytics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputAnalytics {
    pub period: String,
    pub total_operations: u64,
    pub operations_per_second: f64,
    #[serde(default)]
    pub by_operation: std::collections::HashMap<String, u64>,
}

/// Storage analytics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageAnalytics {
    pub total_bytes: u64,
    pub index_bytes: u64,
    pub data_bytes: u64,
    #[serde(default)]
    pub by_namespace: std::collections::HashMap<String, NamespaceStorage>,
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
        self.handle_response(response).await
    }

    /// Get storage analytics
    pub async fn analytics_storage(&self, namespace: Option<&str>) -> Result<StorageAnalytics> {
        let mut url = format!("{}/v1/analytics/storage", self.base_url);
        if let Some(ns) = namespace {
            url.push_str(&format!("?namespace={}", ns));
        }
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
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
