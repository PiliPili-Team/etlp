//! Unified error and result types for etlp-sync.

use thiserror::Error;

/// All errors that can arise from Trakt or Bangumi sync operations.
#[derive(Debug, Error)]
pub enum SyncError {
    /// HTTP transport failure.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON (de)serialization failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Filesystem I/O failure.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// No persisted token file found.
    #[error("no saved token at {path}")]
    TokenNotFound { path: String },

    /// Device-flow authorization timed out before the user approved.
    #[error("OAuth device-flow timed out")]
    DeviceFlowTimeout,

    /// The remote API returned an error status.
    #[error("API error HTTP {status}: {body}")]
    Api { status: u16, body: String },

    /// Refresh token is absent or rejected; re-auth is required.
    #[error("invalid or missing refresh token")]
    InvalidRefreshToken,

    /// The response body was missing an expected field.
    #[error("missing field in response: {field}")]
    MissingField { field: &'static str },
}

/// Convenience alias used throughout this crate.
pub type Result<T> = std::result::Result<T, SyncError>;
