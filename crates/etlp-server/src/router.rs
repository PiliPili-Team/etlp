//! Assemble the axum `Router` with all registered routes.

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::routing::{get, post};
use tower_http::trace::{
    DefaultMakeSpan, DefaultOnFailure, DefaultOnResponse, TraceLayer,
};
use tracing::Level;

use crate::routes::download::{action_route, dl_route, gui_route, pl_route};
use crate::routes::media::send_media_file;
use crate::routes::play::{emby_to_local_player, plex_to_local_player};
use crate::routes::util::{
    miss_runtime_start_sec, open_folder_route, play_media_file, trakt_auth,
};
use crate::state::SharedState;

// ── Route path constants ──────────────────────────────────────────────────────

pub const ROUTE_HEALTH: &str = "/";
pub const ROUTE_FAVICON: &str = "/favicon.ico";

/// Primary entry point (short alias, same logic as `ROUTE_EMBY`).
pub const ROUTE_ETLP: &str = "/etlp";
pub const ROUTE_EMBY: &str = "/embyToLocalPlayer";
pub const ROUTE_PLEX: &str = "/plexToLocalPlayer";

pub const ROUTE_GUI: &str = "/gui";
pub const ROUTE_DL: &str = "/dl";
pub const ROUTE_PL: &str = "/pl";
pub const ROUTE_ACTION: &str = "/action";

pub const ROUTE_SEND_MEDIA_FILE: &str = "/send_media_file";
pub const ROUTE_MISS_RUNTIME_START_SEC: &str = "/miss_runtime_start_sec";
pub const ROUTE_TRAKT_AUTH: &str = "/trakt_auth";
pub const ROUTE_OPEN_FOLDER: &str = "/openFolder";
pub const ROUTE_PLAY_MEDIA_FILE: &str = "/playMediaFile";

// ── Router ────────────────────────────────────────────────────────────────────

/// Build the complete axum `Router`, wiring up all routes and injecting
/// `state` as axum [`State`] into every handler.
pub fn build_router(state: SharedState) -> Router {
    Router::new()
        // Health / favicon
        .route(ROUTE_HEALTH, get(health))
        .route(ROUTE_FAVICON, get(health))
        // Primary ToLocalPlayer endpoints.
        // /etlp is a short alias for /embyToLocalPlayer — identical handler.
        .route(ROUTE_ETLP, post(emby_to_local_player))
        .route(ROUTE_EMBY, post(emby_to_local_player))
        .route(ROUTE_PLEX, post(plex_to_local_player))
        // Download manager endpoints
        .route(ROUTE_GUI, post(gui_route))
        .route(ROUTE_DL, post(dl_route))
        .route(ROUTE_PL, post(pl_route))
        .route(ROUTE_ACTION, post(action_route))
        // Utility endpoints
        .route(ROUTE_SEND_MEDIA_FILE, get(send_media_file))
        .route(ROUTE_MISS_RUNTIME_START_SEC, get(miss_runtime_start_sec))
        .route(ROUTE_TRAKT_AUTH, get(trakt_auth))
        .route(ROUTE_OPEN_FOLDER, post(open_folder_route))
        .route(ROUTE_PLAY_MEDIA_FILE, post(play_media_file))
        .with_state(state)
        // The play endpoints receive full season episode lists which can exceed
        // axum's 2 MB default on large series. 32 MB covers any realistic
        // payload while still bounding the server against runaway requests.
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
        // Log every request/response at INFO so 422 rejections and routing
        // mismatches are visible even when the handler itself is never called.
        // Note: NormalizePathLayer is applied in main.rs, wrapping this Router
        // as the outermost service so it runs before routing decisions are made.
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO))
                .on_failure(DefaultOnFailure::new().level(Level::ERROR)),
        )
}

/// `GET /` – simple liveness probe.
async fn health() -> StatusCode {
    StatusCode::OK
}
