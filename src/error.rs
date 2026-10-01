//! Error types for the Dakera client SDK

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::capabilities::CapabilityKind;

/// Result type alias for Dakera client operations
pub type Result<T> = std::result::Result<T, ClientError>;

/// Typed error codes from the Dakera server API
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServerErrorCode {
    NamespaceNotFound,
    VectorNotFound,
    DimensionMismatch,
    EmptyVector,
    InvalidRequest,
    StorageError,
    InternalError,
    QuotaExceeded,
    ServiceUnavailable,
    AuthenticationRequired,
    InvalidApiKey,
    ApiKeyExpired,
    InsufficientScope,
    NamespaceAccessDenied,
    /// 413: the request body is over a configured limit (v0.12).
    PayloadTooLarge,
    /// 501: the route exists but its feature flag is off (v0.12).
    FeatureDisabled,
    /// 501: the configured backend cannot perform the operation.
    NotImplemented,
    /// 409: the request conflicts with the server's current state.
    Conflict,
    /// 403: cross-origin state-changing request refused (auth disabled).
    CrossOriginRequestRefused,
    /// 429.
    RateLimitExceeded,
    /// 504: a query ran past `query_timeout_ms`.
    QueryTimeout,
    /// 404: no route matches the path (v0.12 router rejection, JSON body).
    RouteNotFound,
    /// 405.
    MethodNotAllowed,
    /// 415.
    UnsupportedMediaType,
    /// 408: the request did not complete within `DAKERA_REQUEST_TIMEOUT`.
    RequestTimeout,
    /// 404: no API key with that id.
    ApiKeyNotFound,
    /// 404: no operations / transcription / image-index job with that id
    /// (after a server restart too: jobs live in memory).
    JobNotFound,
    #[serde(other)]
    Unknown,
}

/// Errors that can occur when using the Dakera client
#[derive(Error, Debug)]
pub enum ClientError {
    /// HTTP request failed
    #[cfg(feature = "http-client")]
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// gRPC request failed
    #[cfg(feature = "grpc")]
    #[error("gRPC request failed: {0}")]
    Grpc(String),

    /// JSON serialization/deserialization failed
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Server returned an error response
    #[error("Server error ({status}): {message}")]
    Server {
        /// HTTP status code
        status: u16,
        /// Error message from server
        message: String,
        #[doc = "Typed error code from the server"]
        code: Option<ServerErrorCode>,
    },

    /// 403 Forbidden — insufficient scope or namespace access denied
    #[error("Authorization failed ({status}): {message}")]
    Authorization {
        status: u16,
        message: String,
        code: Option<ServerErrorCode>,
    },

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    Config(String),

    /// Namespace not found
    #[error("Namespace not found: {0}")]
    NamespaceNotFound(String),

    /// Vector not found
    #[error("Vector not found: {0}")]
    VectorNotFound(String),

    /// Invalid URL
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// Connection failed
    #[error("Connection failed: {0}")]
    Connection(String),

    /// Timeout
    #[error("Request timeout")]
    Timeout,

    /// HTTP 413 with code `QUOTA_EXCEEDED`: a `hard` namespace quota refused
    /// the write (enforced from server v0.12).  Not retryable: free space or
    /// raise the quota.
    #[error("Namespace quota exceeded: {message}")]
    QuotaExceeded {
        /// Server message.
        message: String,
        /// Server details (`namespace: ..., reason: ...`).
        details: Option<String>,
    },

    /// HTTP 413 for any other reason: the request itself is over a limit
    /// (attachment over `DAKERA_ATTACHMENT_MAX_BYTES`, a record over
    /// `DAKERA_RECORD_MAX_BYTES` / `_VECTORS`, a request body over the server's
    /// limit).  Not retryable: send less.
    #[error("Payload too large: {message}")]
    PayloadTooLarge {
        /// Server message (or the raw body when it is not JSON).
        message: String,
        /// Server details.
        details: Option<String>,
    },

    /// HTTP 501 `FEATURE_DISABLED`: the route exists but this server did not
    /// switch the feature on (attachments, records, vision).  `details` names
    /// the variable the operator sets.  Not retryable; see
    /// `DakeraClient::capabilities()` to check before calling.
    #[error("Feature not enabled on this server: {message}")]
    FeatureDisabled {
        /// Server message ("The records API is not enabled on this server").
        message: String,
        /// Server details ("set DAKERA_RECORDS to enable it").
        details: Option<String>,
    },

    /// HTTP 501 for any other reason: the configured backend cannot perform
    /// the operation.  Not retryable until the server is reconfigured.
    #[error("Not supported by this server's configuration: {message}")]
    NotImplemented {
        /// Server message.
        message: String,
        /// Server details (what configuration would support it).
        details: Option<String>,
    },

    /// HTTP 503.  Every v0.12 `503` carries `Retry-After` (whole seconds).
    /// Retryable; `execute_with_retry` waits the advertised time.
    #[error("Service unavailable: {message}")]
    ServiceUnavailable {
        /// Server message.
        message: String,
        /// Why (server details), e.g. the server is starting or a memory budget is full.
        details: Option<String>,
        /// Value of the `Retry-After` response header in seconds, if present.
        retry_after: Option<u64>,
    },

    /// Rate limit exceeded (HTTP 429)
    #[error("Rate limit exceeded — retry after {retry_after:?}")]
    RateLimitExceeded {
        /// Value of the `Retry-After` response header in seconds, if present.
        retry_after: Option<u64>,
    },

    /// R9 / DAK-10004: refused *before* sending because the server's
    /// advertised capabilities (`GET /v1/capabilities`) do not include what
    /// was asked for.  The message names what the server does accept.
    #[error("{kind} '{requested}' is not supported by Dakera server v{server_version}; supported {kind} values: {}", .supported.join(", "))]
    UnsupportedCapability {
        /// Which registry was checked (`model`, `index_kind`, ...).
        kind: CapabilityKind,
        /// The wire string that was rejected.
        requested: String,
        /// The wire strings the server advertises for `kind`.
        supported: Vec<String>,
        /// The server's version, as it reported it.
        server_version: String,
    },
}

impl ClientError {
    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            #[cfg(feature = "http-client")]
            // All reqwest Http errors are network-layer (connection closed, timeout, connect
            // failure, hyper IncompleteMessage, etc.). API-level errors (4xx/5xx) are parsed
            // by the client and surfaced as ClientError::Server, never as ClientError::Http.
            ClientError::Http(_) => true,
            #[cfg(feature = "grpc")]
            ClientError::Grpc(_) => true, // gRPC errors are generally retryable
            ClientError::Server { status, .. } => *status >= 500 || *status == 408,
            ClientError::ServiceUnavailable { .. } => true,
            ClientError::Connection(_) => true,
            ClientError::Timeout => true,
            ClientError::RateLimitExceeded { .. } => true,
            _ => false,
        }
    }

    /// How long the server asked the client to wait before retrying
    /// (`Retry-After` on a 429 or 503), if it said.
    pub fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            ClientError::RateLimitExceeded { retry_after }
            | ClientError::ServiceUnavailable { retry_after, .. } => {
                retry_after.map(std::time::Duration::from_secs)
            }
            _ => None,
        }
    }

    /// `true` for HTTP 413 (quota or over-size request).
    pub fn is_payload_too_large(&self) -> bool {
        matches!(
            self,
            ClientError::QuotaExceeded { .. } | ClientError::PayloadTooLarge { .. }
        ) || matches!(self, ClientError::Server { status: 413, .. })
    }

    /// `true` when the server has the feature switched off (501 `FEATURE_DISABLED`).
    pub fn is_feature_disabled(&self) -> bool {
        matches!(self, ClientError::FeatureDisabled { .. })
    }

    /// Check if the error is a not found error
    pub fn is_not_found(&self) -> bool {
        match self {
            ClientError::Server { status, code, .. } => {
                *status == 404
                    || matches!(
                        code,
                        Some(ServerErrorCode::NamespaceNotFound)
                            | Some(ServerErrorCode::VectorNotFound)
                    )
            }
            ClientError::NamespaceNotFound(_) => true,
            ClientError::VectorNotFound(_) => true,
            _ => false,
        }
    }

    /// Check if the error is an authorization/authentication error
    pub fn is_auth_error(&self) -> bool {
        matches!(self, ClientError::Authorization { .. })
            || matches!(self, ClientError::Server { status: 401, .. })
    }
}

/// The JSON error body every v0.12 error carries.
#[derive(Deserialize)]
struct ErrorBody {
    error: Option<String>,
    code: Option<ServerErrorCode>,
    details: Option<serde_json::Value>,
}

impl ClientError {
    /// Map an HTTP error answer to the matching error.
    ///
    /// `retry_after` is the `Retry-After` header in whole seconds.  `text` is
    /// the raw body: every v0.12 error is JSON (`{"error", "code", "details"?,
    /// "resource"?}`), but a v0.11 server or a proxy may send anything, which
    /// is kept verbatim as the message.
    pub(crate) fn from_http_response(status: u16, retry_after: Option<u64>, text: String) -> Self {
        if status == 429 {
            return ClientError::RateLimitExceeded { retry_after };
        }

        let (message, code, details) = match serde_json::from_str::<ErrorBody>(&text) {
            Ok(body) => (
                body.error.unwrap_or_else(|| text.clone()),
                body.code,
                body.details.map(|d| match d {
                    serde_json::Value::String(s) => s,
                    other => other.to_string(),
                }),
            ),
            Err(_) => (text, None, None),
        };

        match status {
            401 => ClientError::Server {
                status,
                message,
                code,
            },
            403 => ClientError::Authorization {
                status,
                message,
                code,
            },
            404 => match &code {
                Some(ServerErrorCode::NamespaceNotFound) => ClientError::NamespaceNotFound(message),
                Some(ServerErrorCode::VectorNotFound) => ClientError::VectorNotFound(message),
                _ => ClientError::Server {
                    status,
                    message,
                    code,
                },
            },
            413 => {
                if code == Some(ServerErrorCode::QuotaExceeded) {
                    ClientError::QuotaExceeded { message, details }
                } else {
                    ClientError::PayloadTooLarge { message, details }
                }
            }
            501 => {
                // Name the switch to turn on in the message itself.
                let message = match &details {
                    Some(d) => format!("{message} ({d})"),
                    None => message,
                };
                if code == Some(ServerErrorCode::FeatureDisabled) {
                    ClientError::FeatureDisabled { message, details }
                } else {
                    ClientError::NotImplemented { message, details }
                }
            }
            503 => {
                // The server's message is the generic "Service unavailable";
                // the reason (starting, memory budget, ...) is in `details`.
                let message = match &details {
                    Some(d) if message == "Service unavailable" => d.clone(),
                    _ => message,
                };
                ClientError::ServiceUnavailable {
                    message,
                    details,
                    retry_after,
                }
            }
            _ => ClientError::Server {
                status,
                message,
                code,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(status: u16, retry_after: Option<u64>, body: &str) -> ClientError {
        ClientError::from_http_response(status, retry_after, body.to_string())
    }

    #[test]
    fn non_json_bodies_are_kept_as_the_message() {
        match map(503, Some(9), "upstream connect error") {
            ClientError::ServiceUnavailable {
                message,
                details,
                retry_after,
            } => {
                assert_eq!(message, "upstream connect error");
                assert!(details.is_none());
                assert_eq!(retry_after, Some(9));
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(
            map(413, None, "too big"),
            ClientError::PayloadTooLarge { .. }
        ));
    }

    #[test]
    fn a_503_without_retry_after_is_still_retryable() {
        let e = map(
            503,
            None,
            r#"{"error":"Service unavailable","code":"SERVICE_UNAVAILABLE"}"#,
        );
        assert!(e.is_retryable());
        assert_eq!(e.retry_after(), None);
    }

    #[test]
    fn a_501_without_the_feature_code_is_not_implemented() {
        assert!(matches!(
            map(501, None, r#"{"error":"nope","code":"NOT_IMPLEMENTED"}"#),
            ClientError::NotImplemented { .. }
        ));
        assert!(matches!(
            map(501, None, "Not Implemented"),
            ClientError::NotImplemented { .. }
        ));
        assert!(matches!(
            map(
                501,
                None,
                r#"{"error":"off","code":"FEATURE_DISABLED","details":"set DAKERA_VISION to enable it"}"#
            ),
            ClientError::FeatureDisabled { .. }
        ));
    }

    #[test]
    fn auth_and_other_statuses_are_unchanged() {
        assert!(matches!(
            map(401, None, r#"{"error":"API key required","code":"AUTHENTICATION_REQUIRED"}"#),
            ClientError::Server { status: 401, .. }
        ));
        assert!(matches!(
            map(403, None, r#"{"error":"nope","code":"INSUFFICIENT_SCOPE"}"#),
            ClientError::Authorization { status: 403, .. }
        ));
        let e = map(504, None, r#"{"error":"slow","code":"QUERY_TIMEOUT"}"#);
        assert!(matches!(
            e,
            ClientError::Server {
                status: 504,
                code: Some(ServerErrorCode::QueryTimeout),
                ..
            }
        ));
        assert!(e.is_retryable());
        assert!(matches!(
            map(404, None, r#"{"error":"x","code":"NAMESPACE_NOT_FOUND"}"#),
            ClientError::NamespaceNotFound(_)
        ));
    }

    #[test]
    fn structured_details_are_stringified() {
        match map(
            413,
            None,
            r#"{"error":"over","code":"QUOTA_EXCEEDED","details":{"limit":5}}"#,
        ) {
            ClientError::QuotaExceeded { details, .. } => {
                assert_eq!(details.as_deref(), Some(r#"{"limit":5}"#))
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
