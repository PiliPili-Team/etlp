//! Shared error vocabulary for the core crate.

use thiserror::Error;

/// Errors raised by core-level parsing and conversion helpers.
#[derive(Debug, Error)]
pub enum CoreError {
    /// A value that the domain requires was missing or empty.
    #[error("missing required field: {0}")]
    MissingField(&'static str),

    /// A string could not be mapped onto a known enum variant.
    #[error("unknown {kind}: {value}")]
    UnknownVariant {
        /// The category being parsed (e.g. "player").
        kind: &'static str,
        /// The offending input value.
        value: String,
    },
}

/// Convenience alias for results produced inside the core crate.
pub type Result<T> = std::result::Result<T, CoreError>;
