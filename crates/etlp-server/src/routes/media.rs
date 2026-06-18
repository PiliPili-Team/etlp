//! HTTP Range streaming for local media files.
//!
//! `GET /send_media_file`: validates a token, checks the file extension, and
//! serves the file with proper `Content-Range` / 206 Partial Content support.

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use serde::Deserialize;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;
use tracing::{info, warn};

use crate::state::SharedState;

/// Query parameters for `GET /send_media_file`.
#[derive(Debug, Deserialize)]
pub struct SendMediaQuery {
    /// Security token matching `dev.http_server_token` in the config.
    #[serde(default)]
    pub token: String,
    /// URL-encoded absolute path to the local file.
    pub file_path: String,
}

/// `GET /send_media_file` – serve a local media or subtitle file with HTTP
/// Range support.
///
/// Returns:
/// - 403 if the token does not match the config.
/// - 400 if the file extension is not a recognised media/subtitle type.
/// - 404 if the file does not exist.
/// - 206 Partial Content for ranged requests.
/// - 200 for full-file requests.
pub async fn send_media_file(
    State(state): State<SharedState>,
    Query(params): Query<SendMediaQuery>,
    headers: HeaderMap,
) -> Response {
    let server_token = state
        .config
        .read()
        .ok()
        .and_then(|cfg| cfg.get("dev", "http_server_token").map(str::to_owned))
        .unwrap_or_default();

    if params.token != server_token {
        warn!("send_media_file: token mismatch (got {:?})", params.token);
        return StatusCode::FORBIDDEN.into_response();
    }

    let path = &params.file_path;

    if !has_valid_extension(path) {
        warn!("send_media_file: unsupported extension for {path:?}");
        return StatusCode::BAD_REQUEST.into_response();
    }

    let meta = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(_) => {
            return (StatusCode::NOT_FOUND, "File not found").into_response();
        }
    };
    let file_size = meta.len();

    let range_header = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);

    match range_header {
        Some(range) => serve_range(path, file_size, &range).await,
        None => serve_full(path, file_size).await,
    }
}

/// Serve the full file with a 200 response.
async fn serve_full(path: &str, file_size: u64) -> Response {
    info!("send_media_file: full {path:?} size={file_size}");
    let file = match File::open(path).await {
        Ok(f) => f,
        Err(e) => {
            warn!("send_media_file: open failed: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let stream = ReaderStream::new(file);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, file_size)
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Parse a `Range: bytes=START-END` header and serve the slice.
async fn serve_range(path: &str, file_size: u64, range: &str) -> Response {
    let (start, end) = parse_range(range, file_size);

    if start >= file_size || end >= file_size || start > end {
        let range_str = format!("bytes */{file_size}");
        return Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&range_str)
                    .unwrap_or_else(|_| HeaderValue::from_static("bytes */*")),
            )
            .body(Body::empty())
            .unwrap_or_else(|_| {
                StatusCode::RANGE_NOT_SATISFIABLE.into_response()
            });
    }

    let byte_count = end - start + 1;
    info!("send_media_file: range {start}-{end}/{file_size} path={path:?}");

    let mut file = match File::open(path).await {
        Ok(f) => f,
        Err(e) => {
            warn!("send_media_file: open failed: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    if let Err(e) = file.seek(std::io::SeekFrom::Start(start)).await {
        warn!("send_media_file: seek failed: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let limited = file.take(byte_count);
    let stream = ReaderStream::new(limited);
    let range_str = format!("bytes {start}-{end}/{file_size}");
    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&range_str)
                .unwrap_or_else(|_| HeaderValue::from_static("bytes 0-0/0")),
        )
        .header(header::CONTENT_LENGTH, byte_count)
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Parse a `bytes=START-END` range header, filling in defaults from `file_size`.
///
/// Returns `(start, end)` where both are inclusive byte offsets.
fn parse_range(header: &str, file_size: u64) -> (u64, u64) {
    let stripped = match header.strip_prefix("bytes=") {
        Some(s) => s,
        None => return (0, file_size.saturating_sub(1)),
    };
    let (start_str, end_str) = match stripped.split_once('-') {
        Some(pair) => pair,
        None => return (0, file_size.saturating_sub(1)),
    };
    let start: u64 = start_str.trim().parse().unwrap_or(0);
    let end: u64 = if end_str.trim().is_empty() {
        file_size.saturating_sub(1)
    } else {
        end_str
            .trim()
            .parse()
            .unwrap_or(file_size.saturating_sub(1))
    };
    (start, end)
}

const VIDEO_EXT: &[&str] = &[
    "webm", "mkv", "flv", "vob", "ogv", "ogg", "gifv", "mng", "mov", "avi",
    "qt", "wmv", "yuv", "rm", "asf", "amv", "mp4", "m4p", "m4v", "mpg", "mp2",
    "mpeg", "mpe", "mpv", "svi", "3gp", "3g2", "mxf", "roq", "nsv", "f4v",
    "f4p", "f4a", "f4b", "mod", "rrc",
];

const SUB_EXT: &[&str] = &[
    "srt", "sub", "ass", "ssa", "vtt", "sbv", "smi", "sami", "mpl", "txt",
    "dks", "pjs", "stl", "usf", "cdg", "idx", "ttml",
];

fn has_valid_extension(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    VIDEO_EXT.contains(&ext) || SUB_EXT.contains(&ext)
}

trait IntoResponseExt {
    fn into_response(self) -> Response;
}

impl IntoResponseExt for StatusCode {
    fn into_response(self) -> Response {
        axum::response::IntoResponse::into_response(self)
    }
}

impl IntoResponseExt for (StatusCode, &'static str) {
    fn into_response(self) -> Response {
        axum::response::IntoResponse::into_response(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_range_full_range() {
        assert_eq!(parse_range("bytes=0-99", 200), (0, 99));
    }

    #[test]
    fn parse_range_open_end() {
        assert_eq!(parse_range("bytes=100-", 200), (100, 199));
    }

    #[test]
    fn parse_range_open_start() {
        // "bytes=-" is technically invalid; we fall back gracefully.
        assert_eq!(parse_range("bytes=-", 200), (0, 199));
    }

    #[test]
    fn parse_range_no_prefix_falls_back() {
        assert_eq!(parse_range("invalid", 200), (0, 199));
    }

    #[test]
    fn has_valid_extension_video() {
        assert!(has_valid_extension("/media/film.mkv"));
        assert!(has_valid_extension("/media/clip.mp4"));
        assert!(!has_valid_extension("/media/doc.pdf"));
    }

    #[test]
    fn has_valid_extension_subtitle() {
        assert!(has_valid_extension("/subs/track.srt"));
        assert!(has_valid_extension("/subs/track.ass"));
    }

    #[test]
    fn has_valid_extension_case_insensitive() {
        assert!(has_valid_extension("/media/FILM.MKV"));
    }

    #[tokio::test]
    async fn send_media_file_wrong_token_returns_403() {
        use axum::http::{Method, Request};
        use tower::ServiceExt as _;

        use crate::router::build_router;
        use crate::state::test_helpers::test_state;

        let (state, _dir) = test_state();
        let app = build_router(state);
        let req = Request::builder()
            .method(Method::GET)
            .uri("/send_media_file?token=wrong&file_path=/tmp/x.mkv")
            .body(axum::body::Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }
}
