//! API Key management for the Dakera client.
//!
//! Provides methods for creating, listing, rotating, and managing API keys.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::DakeraClient;

// ============================================================================
// Key Types
// ============================================================================

/// Request to create a new API key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKeyRequest {
    /// Human-readable name for this key
    pub name: String,
    /// Scope/permission level (read, write, admin, super_admin)
    pub scope: String,
    /// Optional: restrict to specific namespaces
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespaces: Option<Vec<String>>,
    /// Optional: key expires in N days
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in_days: Option<u64>,
}

/// Response after creating an API key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKeyResponse {
    /// The API key ID (for management)
    pub key_id: String,
    /// The full API key (shown only once!)
    pub key: String,
    /// Key name
    pub name: String,
    /// Key scope
    pub scope: String,
    /// Namespaces this key can access
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespaces: Option<Vec<String>>,
    /// When the key was created (Unix timestamp)
    pub created_at: u64,
    /// When the key expires (Unix timestamp), if set
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// Warning message to save the key
    pub warning: String,
}

/// API key info (without sensitive data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInfo {
    pub key_id: String,
    pub name: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespaces: Option<Vec<String>>,
    pub created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    pub active: bool,
}

/// List keys response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListKeysResponse {
    pub keys: Vec<KeyInfo>,
    pub total: usize,
}

/// Generic success response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeySuccessResponse {
    pub success: bool,
    pub message: String,
}

/// Rotate key response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotateKeyResponse {
    /// The new API key (shown only once!)
    pub new_key: String,
    /// The key ID (unchanged)
    pub key_id: String,
    /// Warning message
    pub warning: String,
}

/// API key usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyUsageResponse {
    pub key_id: String,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub rate_limited_requests: u64,
    pub bytes_transferred: u64,
    pub avg_latency_ms: f64,
    #[serde(default)]
    pub by_endpoint: Vec<EndpointUsageInfo>,
    #[serde(default)]
    pub by_namespace: Vec<NamespaceUsageInfo>,
}

/// Usage statistics per endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointUsageInfo {
    pub endpoint: String,
    pub requests: u64,
    pub avg_latency_ms: f64,
}

/// Usage statistics per namespace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceUsageInfo {
    pub namespace: String,
    pub requests: u64,
    pub vectors_accessed: u64,
}

// ============================================================================
// Key Client Methods
// ============================================================================

impl DakeraClient {
    /// Create a new API key
    pub async fn create_key(&self, request: CreateKeyRequest) -> Result<CreateKeyResponse> {
        let url = format!("{}/admin/keys", self.base_url);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// List all API keys
    pub async fn list_keys(&self) -> Result<ListKeysResponse> {
        let url = format!("{}/admin/keys", self.base_url);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get a specific API key by ID
    pub async fn get_key(&self, key_id: &str) -> Result<KeyInfo> {
        let url = format!("{}/admin/keys/{}", self.base_url, key_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Delete (revoke) an API key
    pub async fn delete_key(&self, key_id: &str) -> Result<KeySuccessResponse> {
        let url = format!("{}/admin/keys/{}", self.base_url, key_id);
        let response = self.client.delete(&url).send().await?;
        self.handle_response(response).await
    }

    /// Deactivate an API key (soft delete)
    pub async fn deactivate_key(&self, key_id: &str) -> Result<KeySuccessResponse> {
        let url = format!("{}/admin/keys/{}/deactivate", self.base_url, key_id);
        let response = self.client.post(&url).send().await?;
        self.handle_response(response).await
    }

    /// Rotate an API key (creates new key, deactivates old)
    pub async fn rotate_key(&self, key_id: &str) -> Result<RotateKeyResponse> {
        let url = format!("{}/admin/keys/{}/rotate", self.base_url, key_id);
        let response = self.client.post(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get API key usage statistics
    pub async fn key_usage(&self, key_id: &str) -> Result<ApiKeyUsageResponse> {
        let url = format!("{}/admin/keys/{}/usage", self.base_url, key_id);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // ========================================================================
    // Namespace-Scoped API Keys — SEC-1
    // ========================================================================

    /// Create a namespace-scoped API key (SEC-1).
    ///
    /// The `key` field in the response is shown **only once** — store it securely.
    pub async fn create_namespace_key(
        &self,
        namespace: &str,
        request: CreateNamespaceKeyRequest,
    ) -> Result<CreateNamespaceKeyResponse> {
        let url = format!("{}/v1/namespaces/{}/keys", self.base_url, namespace);
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// List all API keys scoped to a namespace (SEC-1).
    pub async fn list_namespace_keys(&self, namespace: &str) -> Result<ListNamespaceKeysResponse> {
        let url = format!("{}/v1/namespaces/{}/keys", self.base_url, namespace);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Revoke a namespace-scoped API key (SEC-1).
    pub async fn delete_namespace_key(
        &self,
        namespace: &str,
        key_id: &str,
    ) -> Result<KeySuccessResponse> {
        let url = format!(
            "{}/v1/namespaces/{}/keys/{}",
            self.base_url, namespace, key_id
        );
        let response = self.client.delete(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get usage statistics for a namespace-scoped API key (SEC-1).
    pub async fn namespace_key_usage(
        &self,
        namespace: &str,
        key_id: &str,
    ) -> Result<NamespaceKeyUsageResponse> {
        let url = format!(
            "{}/v1/namespaces/{}/keys/{}/usage",
            self.base_url, namespace, key_id
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Alias for [`namespace_key_usage`](Self::namespace_key_usage) matching Python/JS naming.
    pub async fn get_namespace_key_usage(
        &self,
        namespace: &str,
        key_id: &str,
    ) -> Result<NamespaceKeyUsageResponse> {
        self.namespace_key_usage(namespace, key_id).await
    }
}

// ============================================================================
// Namespace Key Types (SEC-1)
// ============================================================================

/// Request body for `POST /v1/namespaces/:namespace/keys` (SEC-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNamespaceKeyRequest {
    /// Human-readable label for this key.
    pub name: String,
    /// Optional: key expires in N days from now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in_days: Option<u64>,
}

/// Response from `POST /v1/namespaces/:namespace/keys` (SEC-1).
///
/// The `key` field contains the raw API key and is **shown only once**.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNamespaceKeyResponse {
    pub key_id: String,
    /// The raw API key — store it securely, cannot be retrieved again.
    pub key: String,
    pub name: String,
    pub namespace: String,
    pub created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    pub warning: String,
}

/// Namespace-scoped API key metadata — no secret included (SEC-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceKeyInfo {
    pub key_id: String,
    pub name: String,
    pub namespace: String,
    pub created_at: u64,
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
}

/// Response from `GET /v1/namespaces/:namespace/keys` (SEC-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListNamespaceKeysResponse {
    pub namespace: String,
    pub keys: Vec<NamespaceKeyInfo>,
    pub total: usize,
}

/// Response from `GET /v1/namespaces/:namespace/keys/:key_id/usage` (SEC-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceKeyUsageResponse {
    pub key_id: String,
    pub namespace: String,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub bytes_transferred: u64,
    pub avg_latency_ms: f64,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_namespace_key_request_serializes_without_expiry() {
        let req = CreateNamespaceKeyRequest {
            name: "ci-runner".to_string(),
            expires_in_days: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"name\":\"ci-runner\""));
        assert!(!json.contains("expires_in_days"));
    }

    #[test]
    fn test_create_namespace_key_request_serializes_with_expiry() {
        let req = CreateNamespaceKeyRequest {
            name: "ci-runner".to_string(),
            expires_in_days: Some(30),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"expires_in_days\":30"));
    }

    #[test]
    fn test_namespace_key_info_deserializes() {
        let json = r#"{
            "key_id": "key-abc",
            "name": "ci-runner",
            "namespace": "prod-ns",
            "created_at": 1774000000,
            "active": true
        }"#;
        let info: NamespaceKeyInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.key_id, "key-abc");
        assert_eq!(info.namespace, "prod-ns");
        assert!(info.active);
        assert!(info.expires_at.is_none());
    }

    #[test]
    fn test_namespace_key_usage_response_deserializes() {
        let json = r#"{
            "key_id": "key-abc",
            "namespace": "prod-ns",
            "total_requests": 1000,
            "successful_requests": 980,
            "failed_requests": 20,
            "bytes_transferred": 512000,
            "avg_latency_ms": 12.4
        }"#;
        let usage: NamespaceKeyUsageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(usage.total_requests, 1000);
        assert!((usage.avg_latency_ms - 12.4).abs() < 0.001);
    }

    #[test]
    fn test_list_namespace_keys_response_deserializes() {
        let json = r#"{
            "namespace": "prod-ns",
            "keys": [],
            "total": 0
        }"#;
        let resp: ListNamespaceKeysResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.namespace, "prod-ns");
        assert_eq!(resp.total, 0);
        assert!(resp.keys.is_empty());
    }
}
