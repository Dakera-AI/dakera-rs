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
}
