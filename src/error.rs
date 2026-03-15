//! Error types for the Dakera client SDK

use thiserror::Error;

/// Result type alias for Dakera client operations
pub type Result<T> = std::result::Result<T, ClientError>;

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
}

impl ClientError {
    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            #[cfg(feature = "http-client")]
            ClientError::Http(e) => e.is_timeout() || e.is_connect(),
            #[cfg(feature = "grpc")]
            ClientError::Grpc(_) => true, // gRPC errors are generally retryable
            ClientError::Server { status, .. } => *status >= 500,
            ClientError::Connection(_) => true,
            ClientError::Timeout => true,
            _ => false,
        }
    }

    /// Check if the error is a not found error
    pub fn is_not_found(&self) -> bool {
        match self {
            ClientError::Server { status, .. } => *status == 404,
            ClientError::NamespaceNotFound(_) => true,
            ClientError::VectorNotFound(_) => true,
            _ => false,
        }
    }
}
