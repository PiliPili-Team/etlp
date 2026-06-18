//! Download-related route handlers: task manager, playlist status, and actions.
//!
//! Handles `/gui`, `/dl`, `/pl`, and `/action` endpoints.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{info, warn};

use crate::state::SharedState;

/// JSON body shared by the `/gui`, `/dl`, and `/pl` endpoints.
#[derive(Debug, Deserialize)]
pub struct GuiBody {
    /// Which download action to perform.
    pub gui_cmd: String,
    /// Stream URL for download commands.
    #[serde(default)]
    pub stream_url: String,
    /// Item / media-source id for keying tasks.
    #[serde(default)]
    pub media_source_id: String,
    /// Fallback to item_id when media_source_id is absent.
    #[serde(default)]
    pub item_id: String,
}

impl GuiBody {
    /// The effective task id: `media_source_id` if non-empty, else `item_id`.
    fn task_id(&self) -> &str {
        if !self.media_source_id.is_empty() {
            &self.media_source_id
        } else {
            &self.item_id
        }
    }
}

/// JSON body for the `/action` endpoint.
#[derive(Debug, Deserialize)]
pub struct ActionBody {
    /// The action to perform (e.g. `"resume_or_pause"`, `"delete"`).
    #[serde(default)]
    pub action: String,
    /// Stream URL, used for download actions.
    #[serde(default)]
    pub stream_url: String,
    /// Task id.
    #[serde(default)]
    pub media_source_id: String,
    #[serde(default)]
    pub item_id: String,
    /// Sparse-file name (for the `sparse_file` sub-action).
    #[serde(default)]
    pub name: String,
    /// Target file size in bytes (for `sparse_file`).
    #[serde(default)]
    pub size: i64,
}

impl ActionBody {
    fn task_id(&self) -> &str {
        if !self.media_source_id.is_empty() {
            &self.media_source_id
        } else {
            &self.item_id
        }
    }
}

/// `POST /gui` – dispatch a download-manager command from the task manager.
pub async fn gui_route(
    State(state): State<SharedState>,
    Json(body): Json<GuiBody>,
) -> (StatusCode, Json<Value>) {
    dispatch_gui_cmd(state, body).await
}

/// `POST /dl` – alias for `/gui` (Python treats them identically).
pub async fn dl_route(
    State(state): State<SharedState>,
    Json(body): Json<GuiBody>,
) -> (StatusCode, Json<Value>) {
    dispatch_gui_cmd(state, body).await
}

/// `POST /pl` – alias for `/gui`.
pub async fn pl_route(
    State(state): State<SharedState>,
    Json(body): Json<GuiBody>,
) -> (StatusCode, Json<Value>) {
    dispatch_gui_cmd(state, body).await
}

async fn dispatch_gui_cmd(
    state: SharedState,
    body: GuiBody,
) -> (StatusCode, Json<Value>) {
    info!("gui_cmd={:?} task_id={:?}", body.gui_cmd, body.task_id());
    let id = body.task_id().to_owned();
    let url = body.stream_url.clone();
    let cmd = body.gui_cmd.as_str();

    let state2 = state.clone();
    match cmd {
        "download_play" | "download_not_play" => {
            tokio::spawn(async move {
                let dm = state2.dl_manager.lock().await;
                if let Err(e) = dm.download_play(url, id).await {
                    warn!("download_play: {e}");
                }
            });
        }
        "download_only" => {
            tokio::spawn(async move {
                let dm = state2.dl_manager.lock().await;
                if let Err(e) = dm.download_only(url, id).await {
                    warn!("download_only: {e}");
                }
            });
        }
        "delete" | "delete_by_id" => {
            tokio::spawn(async move {
                let dm = state2.dl_manager.lock().await;
                dm.delete(&id).await;
            });
        }
        "resume_or_pause" => {
            tokio::spawn(async move {
                let dm = state2.dl_manager.lock().await;
                dm.resume_or_pause(&id).await;
            });
        }
        other => {
            warn!("unknown gui_cmd: {other:?}");
        }
    }

    (StatusCode::OK, Json(json!({"msg": "ok"})))
}

/// `POST /action` – low-level download task actions.
pub async fn action_route(
    State(state): State<SharedState>,
    Json(body): Json<ActionBody>,
) -> (StatusCode, Json<Value>) {
    info!("action={:?} task_id={:?}", body.action, body.task_id());
    let id = body.task_id().to_owned();
    let url = body.stream_url.clone();

    let state2 = state.clone();
    match body.action.as_str() {
        "resume_or_pause" => {
            tokio::spawn(async move {
                let dm = state2.dl_manager.lock().await;
                dm.resume_or_pause(&id).await;
            });
        }
        "delete" => {
            tokio::spawn(async move {
                let dm = state2.dl_manager.lock().await;
                dm.delete(&id).await;
            });
        }
        "download" => {
            tokio::spawn(async move {
                let dm = state2.dl_manager.lock().await;
                if let Err(e) = dm.download_only(url, id).await {
                    warn!("action download_only: {e}");
                }
            });
        }
        other => {
            warn!("unknown action: {other:?}");
        }
    }

    (StatusCode::OK, Json(json!({"msg": "ok"})))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tower::ServiceExt as _;

    use crate::router::build_router;
    use crate::state::test_helpers::test_state;

    #[tokio::test]
    async fn gui_route_returns_200() {
        let (state, _dir) = test_state();
        let app = build_router(state);
        let body = serde_json::json!({
            "gui_cmd": "resume_or_pause",
            "stream_url": "http://example.com/video.mkv",
            "item_id": "42"
        });
        let req = Request::builder()
            .method(Method::POST)
            .uri("/gui")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn action_route_returns_200() {
        let (state, _dir) = test_state();
        let app = build_router(state);
        let body = serde_json::json!({
            "action": "delete",
            "item_id": "99"
        });
        let req = Request::builder()
            .method(Method::POST)
            .uri("/action")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn unknown_gui_cmd_still_returns_200() {
        let (state, _dir) = test_state();
        let app = build_router(state);
        let body = serde_json::json!({
            "gui_cmd": "does_not_exist",
            "item_id": "1"
        });
        let req = Request::builder()
            .method(Method::POST)
            .uri("/gui")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
