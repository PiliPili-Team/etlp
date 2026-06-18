//! Assemble the axum `Router` with all registered routes.

use axum::Router;
use axum::http::StatusCode;
use axum::routing::{get, post};
use tower_http::normalize_path::NormalizePathLayer;

use crate::routes::download::{action_route, dl_route, gui_route, pl_route};
use crate::routes::media::send_media_file;
use crate::routes::play::{emby_to_local_player, plex_to_local_player};
use crate::routes::util::{
    miss_runtime_start_sec, open_folder_route, play_media_file, trakt_auth,
};
use crate::state::SharedState;

/// Build the complete axum `Router`, wiring up all routes and injecting
/// `state` as axum [`State`] into every handler.
pub fn build_router(state: SharedState) -> Router {
    Router::new()
        // Health / favicon
        .route("/", get(health))
        .route("/favicon.ico", get(health))
        // Primary ToLocalPlayer endpoints
        .route("/embyToLocalPlayer", post(emby_to_local_player))
        .route("/plexToLocalPlayer", post(plex_to_local_player))
        // Download manager endpoints
        .route("/gui", post(gui_route))
        .route("/dl", post(dl_route))
        .route("/pl", post(pl_route))
        .route("/action", post(action_route))
        // Utility endpoints
        .route("/send_media_file", get(send_media_file))
        .route("/miss_runtime_start_sec", get(miss_runtime_start_sec))
        .route("/trakt_auth", get(trakt_auth))
        .route("/openFolder", post(open_folder_route))
        .route("/playMediaFile", post(play_media_file))
        .with_state(state)
        // Userscript sends trailing slashes (e.g. /embyToLocalPlayer/);
        // strip them before routing so exact-match routes are found.
        .layer(NormalizePathLayer::trim_trailing_slash())
}

/// `GET /` – simple liveness probe.
async fn health() -> StatusCode {
    StatusCode::OK
}
