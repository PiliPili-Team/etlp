//! Unified error type for the HTTP server layer.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

/// Errors produced by route handlers and server startup.
#[derive(Debug, Error)]
pub enum ServerError {
    /// Request body could not be parsed as JSON.
    #[error("failed to parse request body: {0}")]
    Json(#[from] serde_json::Error),

    /// Configuration file could not be read or reloaded.
    #[error("config error: {0}")]
    Config(String),

    /// Media-server payload parsing failed.
    #[error("parse error: {0}")]
    Parse(String),

    /// Filesystem or I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid or missing query parameter.
    #[error("invalid query: {0}")]
    BadQuery(String),

    /// Request was rejected (e.g. invalid token).
    #[error("forbidden: {0}")]
    Forbidden(String),
}

/// Convenience alias for route handler results.
pub type Result<T> = std::result::Result<T, ServerError>;

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = match &self {
            ServerError::Json(_) | ServerError::BadQuery(_) => {
                StatusCode::BAD_REQUEST
            }
            ServerError::Forbidden(_) => StatusCode::FORBIDDEN,
            ServerError::Config(_)
            | ServerError::Parse(_)
            | ServerError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string()).into_response()
    }
}
