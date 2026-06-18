//! HTTP server layer for etlp.
//!
//! This crate wires up the axum router that the `etlp` binary serves:
//! - **[`AppState`]** holds all cross-request shared state behind the correct
//!   synchronisation primitives.
//! - **[`build_router`]** assembles the full route table.
//! - **[`platform`]** contains cross-platform helpers for process management,
//!   folder opening, and path conversion.

pub mod error;
pub mod platform;
pub mod router;
pub mod routes;
pub mod state;

pub use error::{Result, ServerError};
pub use router::build_router;
pub use state::{AppState, SharedState};
