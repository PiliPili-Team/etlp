//! Utility route handlers: miss-runtime start-sec, Trakt OAuth callback,
//! folder-open, and local media-file playback.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::info;

use crate::platform::{open_folder, open_media_file, warn_if_not_exists};
use crate::state::SharedState;

// ── /miss_runtime_start_sec ────────────────────────────────────────────────────

/// Query parameters for `GET /miss_runtime_start_sec`.
#[derive(Debug, Deserialize)]
pub struct MissRuntimeQuery {
    /// Media server hostname (part of the cache key).
    #[serde(default)]
    pub netloc: String,
    /// Item identifier.
    #[serde(default)]
    pub item_id: String,
    /// If present, **store** this value; if absent, **retrieve** it.
    #[serde(default)]
    pub stop_sec: Option<String>,
}

/// Response for a start-sec retrieval.
#[derive(Debug, Serialize)]
pub struct StartSecResponse {
    pub start_sec: i64,
}

/// `GET /miss_runtime_start_sec`
///
/// - With `?stop_sec=N`: stores `N` as the resume position for
///   `"{netloc}-{item_id}"`. Returns `{"msg": "ok"}`.
/// - Without `stop_sec`: returns `{"start_sec": N}` (0 when not set).
pub async fn miss_runtime_start_sec(
    State(state): State<SharedState>,
    Query(q): Query<MissRuntimeQuery>,
) -> (StatusCode, Json<Value>) {
    let key = format!("{}-{}", q.netloc, q.item_id);

    if let Some(sec_str) = q.stop_sec {
        let parsed: i64 =
            sec_str.trim().parse::<f64>().map(|f| f as i64).unwrap_or(0);
        match state.miss_runtime.write() {
            Ok(mut map) => {
                map.insert(key.clone(), parsed);
                info!("miss_runtime: stored {key}={parsed}");
            }
            Err(e) => {
                tracing::warn!("miss_runtime write lock poisoned: {e}");
            }
        }
        (StatusCode::OK, Json(json!({"msg": "ok"})))
    } else {
        let start_sec = state
            .miss_runtime
            .read()
            .ok()
            .and_then(|m| m.get(&key).copied())
            .unwrap_or(0);
        info!("miss_runtime: get {key}={start_sec}");
        (StatusCode::OK, Json(json!({"start_sec": start_sec})))
    }
}

// ── /trakt_auth ────────────────────────────────────────────────────────────────

/// Query parameters for `GET /trakt_auth`.
#[derive(Debug, Deserialize)]
pub struct TraktAuthQuery {
    /// OAuth authorisation code returned by Trakt after user approval.
    #[serde(default)]
    pub code: Option<String>,
}

/// `GET /trakt_auth` – OAuth redirect callback for Trakt.
///
/// Exchanges the `?code=…` parameter for a bearer token via the Trakt
/// Authorization Code Flow, then persists the token. Always returns 200 so
/// the browser page loads successfully regardless of outcome.
pub async fn trakt_auth(
    State(state): State<SharedState>,
    Query(q): Query<TraktAuthQuery>,
) -> (StatusCode, String) {
    let Some(code) = q.code else {
        return (StatusCode::OK, "etlp: trakt auth – no code".to_owned());
    };
    info!("trakt_auth: exchanging code");

    let (client_id, client_secret, redirect_uri, token_path) = {
        let cfg = match state.config.read() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("trakt_auth: config lock poisoned: {e}");
                return (
                    StatusCode::OK,
                    "etlp: trakt auth – config error".to_owned(),
                );
            }
        };
        let id = cfg.get_or("trakt", "client_id", "").to_owned();
        let secret = cfg.get_or("trakt", "client_secret", "").to_owned();
        let uri = cfg
            .get_or(
                "trakt",
                "redirect_uri",
                "http://localhost:58000/trakt_auth",
            )
            .to_owned();
        let path = state.working_dir.join("trakt_token.json");
        (id, secret, uri, path)
    };

    if client_id.is_empty() || client_secret.is_empty() {
        tracing::warn!(
            "trakt_auth: [trakt] client_id or client_secret missing"
        );
        return (
            StatusCode::OK,
            "etlp: trakt auth – credentials not configured".to_owned(),
        );
    }

    match etlp_sync::TraktApi::new(
        &client_id,
        &client_secret,
        "",
        &token_path,
        "https://api.trakt.tv",
    ) {
        Ok(mut api) => match api.exchange_code(&code, &redirect_uri).await {
            Ok(_) => {
                info!("trakt_auth: token saved");
                (StatusCode::OK, "etlp: trakt auth success".to_owned())
            }
            Err(e) => {
                tracing::warn!("trakt_auth: exchange_code failed: {e}");
                (
                    StatusCode::OK,
                    "etlp: trakt auth – exchange failed".to_owned(),
                )
            }
        },
        Err(e) => {
            tracing::warn!("trakt_auth: TraktApi::new failed: {e}");
            (StatusCode::OK, "etlp: trakt auth – init failed".to_owned())
        }
    }
}

// ── /openFolder ───────────────────────────────────────────────────────────────

/// JSON body for `POST /openFolder`.
#[derive(Debug, Deserialize)]
pub struct OpenFolderBody {
    /// Absolute path to the directory to open.
    #[serde(default)]
    pub file_path: String,
}

/// `POST /openFolder` – open a directory in the platform file manager.
pub async fn open_folder_route(
    State(_state): State<SharedState>,
    Json(body): Json<OpenFolderBody>,
) -> StatusCode {
    warn_if_not_exists(&body.file_path);
    if let Err(e) = open_folder(&body.file_path) {
        tracing::warn!("open_folder: {e}");
    }
    StatusCode::OK
}

// ── /playMediaFile ─────────────────────────────────────────────────────────────

/// JSON body for `POST /playMediaFile`.
#[derive(Debug, Deserialize)]
pub struct PlayMediaFileBody {
    /// Absolute path to the media file.
    #[serde(default)]
    pub file_path: String,
    /// Path to the player binary (empty → shell-open).
    #[serde(default)]
    pub player_path: String,
}

/// `POST /playMediaFile` – open a local file with the configured player.
pub async fn play_media_file(
    State(_state): State<SharedState>,
    Json(body): Json<PlayMediaFileBody>,
) -> StatusCode {
    warn_if_not_exists(&body.file_path);
    if let Err(e) = open_media_file(&body.file_path, &body.player_path) {
        tracing::warn!("play_media_file: {e}");
    }
    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tower::ServiceExt as _;

    use crate::router::build_router;
    use crate::state::test_helpers::test_state;

    #[tokio::test]
    async fn miss_runtime_store_and_retrieve() {
        let (state, _dir) = test_state();
        let app = build_router(state.clone());

        // Store
        let req = Request::builder()
            .method(Method::GET)
            .uri(
                "/miss_runtime_start_sec\
                 ?netloc=emby%3A8096&item_id=42&stop_sec=300",
            )
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Retrieve – share the same Arc<AppState> so the store is visible.
        let app2 = build_router(state);
        let req2 = Request::builder()
            .method(Method::GET)
            .uri("/miss_runtime_start_sec?netloc=emby%3A8096&item_id=42")
            .body(Body::empty())
            .unwrap();
        let res2 = app2.oneshot(req2).await.unwrap();
        assert_eq!(res2.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res2.into_body(), usize::MAX)
            .await
            .unwrap();
        let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val.get("start_sec"), Some(&serde_json::json!(300)));
    }

    #[tokio::test]
    async fn miss_runtime_returns_zero_for_unknown_key() {
        let (state, _dir) = test_state();
        let app = build_router(state);
        let req = Request::builder()
            .method(Method::GET)
            .uri("/miss_runtime_start_sec?netloc=x&item_id=999")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(val.get("start_sec"), Some(&serde_json::json!(0)));
    }

    #[tokio::test]
    async fn trakt_auth_no_code_returns_200() {
        let (state, _dir) = test_state();
        let app = build_router(state);
        let req = Request::builder()
            .method(Method::GET)
            .uri("/trakt_auth")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_check_returns_200() {
        let (state, _dir) = test_state();
        let app = build_router(state);
        let req = Request::builder()
            .method(Method::GET)
            .uri("/")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn open_folder_returns_200() {
        let (state, _dir) = test_state();
        let app = build_router(state);
        let body = serde_json::json!({"file_path": "/tmp"});
        let req = Request::builder()
            .method(Method::POST)
            .uri("/openFolder")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn play_media_file_returns_200() {
        let (state, _dir) = test_state();
        let app = build_router(state);
        let body = serde_json::json!({"file_path": "/tmp/test.mkv", "player_path": ""});
        let req = Request::builder()
            .method(Method::POST)
            .uri("/playMediaFile")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
