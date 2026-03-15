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
