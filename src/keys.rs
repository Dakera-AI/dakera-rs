//! API Key management for the Dakera client.
//!
//! Provides methods for creating, listing, rotating, and managing API keys.

use serde::{Deserialize, Serialize};

use crate::error::{ClientError, Result};
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

/// API key info (without sensitive data).
///
/// Returned by `GET /admin/keys`, `GET /admin/keys/{id}`, both `PATCH` key
/// routes and the namespace-key listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInfo {
    /// Key id (`dk_key_...`).
    pub key_id: String,
    /// Human-readable name.
    pub name: String,
    /// Scope (`read`, `write`, `admin`, `super_admin`).
    pub scope: String,
    /// Namespace grants: `None` = every namespace. Since server v0.12.2 an
    /// entry may be a prefix pattern `p*` (one trailing star).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespaces: Option<Vec<String>>,
    /// Creation time (Unix seconds).
    pub created_at: u64,
    /// Expiry (Unix seconds), if set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// Whether the key is active.
    pub active: bool,
    /// Grant syntax the server reads `namespaces` with (server v0.12.2+):
    /// `1` = exact names and `p*` patterns; `0` = a key created before
    /// v0.12.2, whose `foo*` entries are literal names that grant nothing
    /// until its namespaces are saved again. `None` from older servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grants_version: Option<u32>,
    /// Entries of `namespaces` that grant nothing (server v0.12.2+): a legacy
    /// `foo*`, or a server-internal name such as `_dakera_sessions`. Empty
    /// when there are none (the server omits the field then).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inert_namespaces: Vec<String>,
}

/// List keys response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListKeysResponse {
    /// The keys.
    pub keys: Vec<KeyInfo>,
    /// Number of keys.
    pub total: usize,
}

/// Generic success response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeySuccessResponse {
    /// Whether the operation succeeded.
    pub success: bool,
    /// Server message.
    pub message: String,
}

/// Rotate key response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotateKeyResponse {
    /// The new API key (shown only once!)
    pub new_key: String,
    /// The NEW key's id. Rotation mints a key with its own id; the rotated
    /// key keeps [`old_key_id`](Self::old_key_id).
    pub key_id: String,
    /// The id of the key that was rotated (server v0.12.2+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_key_id: Option<String>,
    /// With a grace period: when the old key stops working (Unix seconds).
    /// `None` when the old key was deactivated at once, or from servers
    /// before v0.12.2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_key_expires_at: Option<u64>,
    /// Warning message
    pub warning: String,
}

/// Body of `POST /admin/keys/{key_id}/rotate` (server v0.12.2+).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RotateKeyRequest {
    /// Seconds the old key keeps working after the rotation (0..=604800,
    /// i.e. up to 7 days). `None` or `0`: the old key is deactivated at once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grace_secs: Option<u64>,
}

/// Longest rotation grace period the server accepts (7 days), in seconds.
pub const MAX_ROTATION_GRACE_SECS: u64 = 604_800;

/// Body of `PATCH /admin/keys/{key_id}` and
/// `PATCH /v1/namespaces/{namespace}/keys/{key_id}` (server v0.12.2+).
///
/// At least one field must be set. A key's scope, secret, expiry and state
/// cannot be changed this way.
///
/// `namespaces` has three meanings on the wire, all expressible here:
///
/// | value                  | JSON                      | effect                    |
/// |------------------------|---------------------------|---------------------------|
/// | `None`                 | field absent              | namespaces unchanged      |
/// | `Some(None)`           | `"namespaces": null`      | every namespace           |
/// | `Some(Some(vec![]))`   | `"namespaces": []`        | no namespace              |
/// | `Some(Some(list))`     | `"namespaces": [...]`     | replaces the list         |
///
/// Saving the namespaces moves a pre-v0.12.2 key to the current grant syntax
/// (its `p*` patterns become active).
///
/// ```
/// use dakera_client::UpdateKeyRequest;
///
/// let rename = UpdateKeyRequest::new().with_name("ci");
/// assert_eq!(serde_json::to_string(&rename).unwrap(), r#"{"name":"ci"}"#);
///
/// let all = UpdateKeyRequest::new().with_all_namespaces();
/// assert_eq!(serde_json::to_string(&all).unwrap(), r#"{"namespaces":null}"#);
///
/// let some = UpdateKeyRequest::new().with_namespaces(vec!["team-*".to_string()]);
/// assert_eq!(serde_json::to_string(&some).unwrap(), r#"{"namespaces":["team-*"]}"#);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateKeyRequest {
    /// New name; `None` leaves it unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// New namespace grants; see the type documentation for the three cases.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "double_option"
    )]
    pub namespaces: Option<Option<Vec<String>>>,
}

impl UpdateKeyRequest {
    /// An empty update (set at least one field before sending it).
    pub fn new() -> Self {
        Self::default()
    }

    /// Rename the key.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Replace the key's namespace list (`[]` = no namespace).
    pub fn with_namespaces(mut self, namespaces: Vec<String>) -> Self {
        self.namespaces = Some(Some(namespaces));
        self
    }

    /// Grant every namespace (`"namespaces": null`; unrestricted callers only).
    pub fn with_all_namespaces(mut self) -> Self {
        self.namespaces = Some(None);
        self
    }

    /// Whether the update changes nothing (the server answers 400 to it).
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.namespaces.is_none()
    }
}

/// `Some(None)` for an explicit `null`, `Some(Some(v))` for a value; a
/// missing field stays `None` through `#[serde(default)]`.
fn double_option<'de, D, T>(deserializer: D) -> std::result::Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// Response of `GET /v1/auth/whoami` (server v0.12.2+): the key the request
/// authenticates with, as the server reads it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoamiResponse {
    /// Key id (`auth-disabled` when the server runs without authentication).
    pub key_id: String,
    /// Key name.
    pub name: String,
    /// Scope (`read`, `write`, `admin`, `super_admin`).
    pub scope: String,
    /// Namespace grants; `None` = every namespace.
    #[serde(default)]
    pub namespaces: Option<Vec<String>>,
    /// Whether the key reaches every namespace (`null` or `["*"]`).
    #[serde(default)]
    pub unrestricted: bool,
    /// Expiry (Unix seconds), if set.
    #[serde(default)]
    pub expires_at: Option<u64>,
    /// Grant syntax version (see [`KeyInfo::grants_version`]).
    #[serde(default)]
    pub grants_version: u32,
    /// Grants that grant nothing (see [`KeyInfo::inert_namespaces`]).
    #[serde(default)]
    pub inert_namespaces: Vec<String>,
    /// `false` when the server runs with authentication off.
    #[serde(default = "default_true")]
    pub auth_enabled: bool,
}

fn default_true() -> bool {
    true
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

    /// Rotate an API key and let the old key keep working for `grace_secs`
    /// seconds (server v0.12.2+; `POST /admin/keys/{id}/rotate` with
    /// `{"grace_secs": N}`).
    ///
    /// The old key works until `min(its expiry, now + grace_secs)`
    /// ([`RotateKeyResponse::old_key_expires_at`]). `grace_secs` is at most
    /// [`MAX_ROTATION_GRACE_SECS`]; a larger value is refused with
    /// [`ClientError::InvalidRequest`] before anything is sent. `0` behaves
    /// like [`rotate_key`](Self::rotate_key).
    ///
    /// A server before v0.12.2 ignores the body and deactivates the old key
    /// at once; its answer has no [`RotateKeyResponse::old_key_id`], which is
    /// how to tell.
    pub async fn rotate_key_with_grace(
        &self,
        key_id: &str,
        grace_secs: u64,
    ) -> Result<RotateKeyResponse> {
        if grace_secs > MAX_ROTATION_GRACE_SECS {
            return Err(ClientError::InvalidRequest(format!(
                "grace_secs must be at most {MAX_ROTATION_GRACE_SECS} (7 days), got {grace_secs}"
            )));
        }
        let url = format!("{}/admin/keys/{}/rotate", self.base_url, key_id);
        let body = RotateKeyRequest {
            grace_secs: Some(grace_secs),
        };
        let response = self.client.post(&url).json(&body).send().await?;
        self.handle_response(response).await
    }

    /// Rename a key or replace its namespaces (server v0.12.2+;
    /// `PATCH /admin/keys/{key_id}`, unrestricted `super_admin` only).
    ///
    /// Returns the stored key. An empty update is refused with
    /// [`ClientError::InvalidRequest`] before anything is sent. The server
    /// answers 400 for the root key or an invalid grant, 404 for an unknown
    /// key and 409 for an inactive one.
    pub async fn update_key(&self, key_id: &str, request: &UpdateKeyRequest) -> Result<KeyInfo> {
        if request.is_empty() {
            return Err(ClientError::InvalidRequest(
                "update_key needs a name or namespaces to change".to_string(),
            ));
        }
        let url = format!("{}/admin/keys/{}", self.base_url, key_id);
        let response = self.client.patch(&url).json(request).send().await?;
        self.handle_response(response).await
    }

    /// Return the key this client authenticates with, as the server reads it
    /// (server v0.12.2+; `GET /v1/auth/whoami`, any valid key).
    pub async fn whoami(&self) -> Result<WhoamiResponse> {
        let url = format!("{}/v1/auth/whoami", self.base_url);
        let response = self.client.get(&url).send().await?;
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

    /// Rename a key of `namespace` or replace its namespaces (server v0.12.2+;
    /// `PATCH /v1/namespaces/{namespace}/keys/{key_id}`, namespace admins).
    ///
    /// Every new grant must be contained in the caller's own grants (403
    /// otherwise), and `namespaces: null` needs an unrestricted caller. A key
    /// the caller cannot manage answers 404. An empty update is refused with
    /// [`ClientError::InvalidRequest`] before anything is sent.
    pub async fn update_namespace_key(
        &self,
        namespace: &str,
        key_id: &str,
        request: &UpdateKeyRequest,
    ) -> Result<KeyInfo> {
        if request.is_empty() {
            return Err(ClientError::InvalidRequest(
                "update_namespace_key needs a name or namespaces to change".to_string(),
            ));
        }
        let url = format!(
            "{}/v1/namespaces/{}/keys/{}",
            self.base_url, namespace, key_id
        );
        let response = self.client.patch(&url).json(request).send().await?;
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
    /// Scope of the new key (`read`, `write` or `admin`; at most the
    /// caller's). The server requires it.
    pub scope: String,
    /// Namespaces granted beyond the path namespace. Since server v0.12.2
    /// they may be `p*` patterns, are validated (400 for junk entries) and
    /// must be contained in the caller's grants.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_namespaces: Option<Vec<String>>,
    /// Optional: key expires in N days from now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in_days: Option<u64>,
}

impl CreateNamespaceKeyRequest {
    /// A request for a key named `name` with `scope`.
    pub fn new(name: impl Into<String>, scope: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            scope: scope.into(),
            extra_namespaces: None,
            expires_in_days: None,
        }
    }
}

/// Response from `POST /v1/namespaces/:namespace/keys` (SEC-1).
///
/// The `key` field contains the raw API key and is **shown only once**.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNamespaceKeyResponse {
    /// Key id.
    pub key_id: String,
    /// The raw API key — store it securely, cannot be retrieved again.
    pub key: String,
    /// Key name.
    pub name: String,
    /// The path namespace (empty when the server does not echo it; it sends
    /// `namespaces` instead).
    #[serde(default)]
    pub namespace: String,
    /// Key scope.
    #[serde(default)]
    pub scope: String,
    /// Every namespace the key is granted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespaces: Option<Vec<String>>,
    /// Creation time (Unix seconds).
    pub created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    pub warning: String,
}

/// Namespace-scoped API key metadata — no secret included (SEC-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceKeyInfo {
    /// Key id.
    pub key_id: String,
    /// Key name.
    pub name: String,
    /// The path namespace (empty when the server does not echo it; the
    /// server sends `namespaces`).
    #[serde(default)]
    pub namespace: String,
    /// Creation time (Unix seconds).
    pub created_at: u64,
    /// Whether the key is active.
    pub active: bool,
    /// Expiry (Unix seconds), if set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// Key scope.
    #[serde(default)]
    pub scope: String,
    /// Every namespace the key is granted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespaces: Option<Vec<String>>,
    /// Grant syntax version (server v0.12.2+, see [`KeyInfo::grants_version`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grants_version: Option<u32>,
    /// Grants that grant nothing (server v0.12.2+).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inert_namespaces: Vec<String>,
}

/// Response from `GET /v1/namespaces/:namespace/keys` (SEC-1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListNamespaceKeysResponse {
    /// The path namespace (empty when the server does not echo it).
    #[serde(default)]
    pub namespace: String,
    /// The keys.
    pub keys: Vec<NamespaceKeyInfo>,
    /// Number of keys.
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
        let req = CreateNamespaceKeyRequest::new("ci-runner", "write");
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"name\":\"ci-runner\""));
        assert!(json.contains("\"scope\":\"write\""));
        assert!(!json.contains("expires_in_days"));
        assert!(!json.contains("extra_namespaces"));
    }

    #[test]
    fn test_create_namespace_key_request_serializes_with_expiry() {
        let req = CreateNamespaceKeyRequest {
            expires_in_days: Some(30),
            ..CreateNamespaceKeyRequest::new("ci-runner", "read")
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
