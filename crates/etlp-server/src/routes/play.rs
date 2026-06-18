//! Main ToLocalPlayer route handlers.
//!
//! `POST /embyToLocalPlayer` and `POST /plexToLocalPlayer` are the primary
//! entry points called by the Tampermonkey userscript. Both respond with
//! HTTP 200 immediately and then drive the full play chain in a spawned task.

use std::sync::atomic::Ordering;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use serde_json::{Value, json};
use tracing::{info, warn};

use etlp_media_server::parse::EmbyParseConfig;
use etlp_media_server::plex::{PlexParseConfig, PlexReceivedData};
use etlp_media_server::received::ReceivedData;
use etlp_media_server::{parse_received_data_emby, parse_received_data_plex};

use crate::state::SharedState;

/// `POST /embyToLocalPlayer` – Emby and Jellyfin userscript entry point.
///
/// Responds 200 immediately; parsing and player launch run in a background
/// task so the browser is not left waiting.
pub async fn emby_to_local_player(
    State(state): State<SharedState>,
    Json(received): Json<ReceivedData>,
) -> (StatusCode, Json<Value>) {
    if received.show_task_manager {
        info!("show_task_manager requested (GUI not available)");
        return (
            StatusCode::OK,
            Json(json!({"msg": "task manager not available"})),
        );
    }

    reload_config(&state);

    let state2 = state.clone();
    tokio::spawn(async move {
        start_emby_play(state2, received).await;
    });

    (StatusCode::OK, Json(json!({"msg": "ok"})))
}

/// `POST /plexToLocalPlayer` – Plex userscript entry point.
pub async fn plex_to_local_player(
    State(state): State<SharedState>,
    Json(received): Json<PlexReceivedData>,
) -> (StatusCode, Json<Value>) {
    reload_config(&state);

    let state2 = state.clone();
    tokio::spawn(async move {
        start_plex_play(state2, received).await;
    });

    (StatusCode::OK, Json(json!({"msg": "ok"})))
}

/// Attempt a config reload; logs a warning on failure but never fails the
/// request.
fn reload_config(state: &SharedState) {
    match state.config.write() {
        Ok(mut cfg) => {
            if let Err(e) = cfg.reload() {
                warn!("config reload failed: {e}");
            }
        }
        Err(e) => warn!("config write lock poisoned: {e}"),
    }
}

/// Background task: parse Emby/Jellyfin payload and launch the player.
async fn start_emby_play(state: SharedState, received: ReceivedData) {
    let (parse_cfg, redirect_cache) = {
        let cfg = match state.config.read() {
            Ok(c) => c,
            Err(e) => {
                warn!("config read lock poisoned: {e}");
                return;
            }
        };
        let pc = EmbyParseConfig::from_config(&cfg);
        let rc = state.redirect_cache.clone();
        (pc, rc)
    };

    let mut data = match parse_received_data_emby(
        &received,
        &parse_cfg,
        &state.http_client,
        &redirect_cache,
    )
    .await
    {
        Ok(d) => d,
        Err(e) => {
            warn!("parse_received_data_emby: {e}");
            return;
        }
    };
    // Fall back to the persistent device ID when the request omits one.
    if data.device_id.is_empty() {
        data.device_id = state.device_id.clone();
    }

    // Enforce one-instance-mode: reject if another player is already running.
    if state
        .player_running
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        warn!(
            "player already running (one_instance_mode); \
             skipping item_id={}",
            data.item_id
        );
        return;
    }

    info!(
        "starting play: server={} item_id={} file={:?}",
        data.server.as_str(),
        data.item_id,
        data.file_path,
    );

    // TODO(6.4): build player command, construct PlayerHandle, start
    //  PlayerManager, register playlist, collect stop times, write progress.

    state.player_running.store(false, Ordering::Release);
}

/// Background task: parse Plex payload and launch the player.
async fn start_plex_play(state: SharedState, received: PlexReceivedData) {
    let plex_cfg = {
        let cfg = match state.config.read() {
            Ok(c) => c,
            Err(e) => {
                warn!("config read lock poisoned: {e}");
                return;
            }
        };
        PlexParseConfig {
            force_disk_prefixes: cfg.split_list(
                "dev",
                "force_disk_mode_path",
                ',',
            ),
            subtitle_priority: cfg.split_list("dev", "subtitle_priority", ','),
            path_pairs: cfg.path_translation_pairs(),
        }
    };

    let items = match parse_received_data_plex(&received, &plex_cfg) {
        Ok(v) => v,
        Err(e) => {
            warn!("parse_received_data_plex: {e}");
            return;
        }
    };

    let Some(data) = items.into_iter().next() else {
        warn!("plex payload contained no items");
        return;
    };

    if state
        .player_running
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        warn!(
            "player already running; skipping plex item_id={}",
            data.item_id
        );
        return;
    }

    info!(
        "starting plex play: item_id={} file={:?}",
        data.item_id, data.file_path,
    );

    // TODO(6.4): build player command and start PlayerManager.

    state.player_running.store(false, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tower::ServiceExt as _;

    use crate::router::build_router;
    use crate::state::test_helpers::test_state;

    #[tokio::test]
    async fn emby_route_returns_200() {
        let (state, _dir) = test_state();
        let app = build_router(state);
        let body = serde_json::json!({
            "playbackUrl": "http://emby:8096/emby/Items/1/PlaybackInfo?X-Emby-Token=tok",
            "ApiClient": {"_serverAddress": "http://emby:8096", "_serverVersion": "4.9"},
            "request": {"headers": {}},
            "playbackData": {"PlaySessionId": "s1", "MediaSources": []},
            "extraData": {"mainEpInfo": {"Id": "1"}, "episodesInfo": []},
            "mountDiskEnable": "false"
        });
        let req = Request::builder()
            .method(Method::POST)
            .uri("/embyToLocalPlayer")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn plex_route_returns_200() {
        let (state, _dir) = test_state();
        let app = build_router(state);
        let body = serde_json::json!({
            "playbackUrl": "http://plex:32400/library/metadata/42?X-Plex-Token=t",
            "mountDiskEnable": "false",
            "playbackData": {"MediaContainer": {"Metadata": []}}
        });
        let req = Request::builder()
            .method(Method::POST)
            .uri("/plexToLocalPlayer")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn show_task_manager_returns_200() {
        let (state, _dir) = test_state();
        let app = build_router(state);
        let body = serde_json::json!({
            "showTaskManager": true,
            "playbackUrl": "",
            "playbackData": {"PlaySessionId": "", "MediaSources": []},
            "extraData": {},
            "mountDiskEnable": "false"
        });
        let req = Request::builder()
            .method(Method::POST)
            .uri("/embyToLocalPlayer")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
