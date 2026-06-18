//! DandanPlay HTTP control API.
//!
//! DandanPlay exposes a REST API at `http://127.0.0.1:{port}/api/v1/`.
//! Optionally authenticated via a Bearer token (`api_key`).
//!
//! [`DanDanHandle`] wraps the spawned player process and the HTTP client.
//! [`DanDanHandle::stop_sec`] polls current video status until the player
//! exits, then looks up file size from the library for accurate tracking.

use std::collections::HashMap;
use std::io;
use std::process::{Child, Command, ExitStatus};
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;
use tracing::{info, warn};

use crate::mpv::LaunchArgs;

// ── Config ────────────────────────────────────────────────────────────────────

/// DandanPlay HTTP API connection settings.
#[derive(Debug, Clone)]
pub struct DanDanConfig {
    /// Local HTTP API port (from `[dandan] port` in config).
    pub port: u16,
    /// Optional API key for Bearer authentication.
    pub api_key: Option<String>,
}

// ── Error ─────────────────────────────────────────────────────────────────────

/// Errors from DandanPlay HTTP communication or process control.
#[derive(Debug, Error)]
pub enum DanDanError {
    /// HTTP or network error from reqwest.
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    /// OS-level IO error (process spawn, etc.).
    #[error("IO: {0}")]
    Io(#[from] io::Error),
    /// Could not connect to DandanPlay API after all retry attempts.
    #[error("failed to connect to DandanPlay API after retries")]
    ConnectTimeout,
}

// ── DTOs ─────────────────────────────────────────────────────────────────────

/// Current video status from `/api/v1/current/video`.
#[derive(Debug, Clone, Deserialize)]
pub struct DanDanVideoStatus {
    /// Fractional playback position, 0.0–1.0.
    #[serde(rename = "Position")]
    pub position: f64,
    /// Total duration in milliseconds.
    #[serde(rename = "Duration")]
    pub duration: f64,
    /// Internal episode ID (used to look up file size from library).
    #[serde(rename = "EpisodeId")]
    pub episode_id: i64,
    /// False when the position is being set (seek in progress).
    #[serde(rename = "Seekable")]
    pub seekable: bool,
}

impl DanDanVideoStatus {
    /// Current playback position in whole seconds.
    pub fn position_sec(&self) -> i64 {
        (self.duration * self.position / 1000.0) as i64
    }
}

/// A single library entry from `/api/v1/library`.
#[derive(Debug, Deserialize)]
pub struct DanDanLibraryItem {
    #[serde(rename = "EpisodeId")]
    pub episode_id: i64,
    #[serde(rename = "Size")]
    pub size: i64,
}

// ── DanDanClient ─────────────────────────────────────────────────────────────

/// HTTP client for DandanPlay's REST API.
#[derive(Clone)]
pub struct DanDanClient {
    client: reqwest::Client,
    /// `http://127.0.0.1:{port}/api/v1`
    base_url: String,
    /// `Bearer <api_key>` or empty.
    auth_header: Option<String>,
}

impl DanDanClient {
    /// Build a client for the given config.
    pub fn new(cfg: &DanDanConfig) -> Result<Self, DanDanError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(200))
            .build()?;
        let auth_header = cfg
            .api_key
            .as_deref()
            .filter(|k| !k.is_empty())
            .map(|k| format!("Bearer {k}"));
        Ok(Self {
            client,
            base_url: format!("http://127.0.0.1:{}/api/v1", cfg.port),
            auth_header,
        })
    }

    /// Used by tests to redirect to a local mock server.
    #[cfg(test)]
    pub(crate) fn with_base_url(
        base_url: &str,
        api_key: Option<&str>,
    ) -> Result<Self, DanDanError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(500))
            .build()?;
        let auth_header = api_key
            .filter(|k| !k.is_empty())
            .map(|k| format!("Bearer {k}"));
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_owned(),
            auth_header,
        })
    }

    fn get_builder(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}/{}", self.base_url, path);
        let req = self.client.get(url);
        if let Some(auth) = &self.auth_header {
            req.header("Authorization", auth)
        } else {
            req
        }
    }

    /// Fetch current video status.
    pub async fn current_video(
        &self,
    ) -> Result<DanDanVideoStatus, DanDanError> {
        Ok(self
            .get_builder("current/video")
            .send()
            .await?
            .json::<DanDanVideoStatus>()
            .await?)
    }

    /// Seek to `ms` milliseconds from the beginning.
    pub async fn seek(&self, ms: u64) -> Result<(), DanDanError> {
        let url = format!("{}/control/seek/{ms}", self.base_url);
        self.client.get(url).send().await?;
        Ok(())
    }

    /// Fetch the local library: episode_id → file size mapping.
    pub async fn library(&self) -> Result<HashMap<i64, i64>, DanDanError> {
        let items = self
            .get_builder("library")
            .send()
            .await?
            .json::<Vec<DanDanLibraryItem>>()
            .await?;
        Ok(items.into_iter().map(|i| (i.episode_id, i.size)).collect())
    }
}

// ── Connect with retry ────────────────────────────────────────────────────────

/// Retry delays — first 5 s is an unconditional startup wait (matching
/// Python's `time.sleep(5)` before polling), then poll every 300 ms.
const STARTUP_WAIT_MS: u64 = 5_000;
const POLL_INTERVAL_MS: u64 = 300;
const MAX_RETRIES: u32 = 60; // ~18 s of polling after the startup wait

/// Wait for DandanPlay's HTTP API to become available.
pub async fn connect_dandan(
    cfg: &DanDanConfig,
) -> Result<DanDanClient, DanDanError> {
    let client = DanDanClient::new(cfg)?;
    tokio::time::sleep(Duration::from_millis(STARTUP_WAIT_MS)).await;
    for _ in 0..MAX_RETRIES {
        if client.current_video().await.is_ok() {
            info!("DandanPlay API connected on port {}", cfg.port);
            return Ok(client);
        }
        warn!("DandanPlay API not yet ready, retrying…");
        tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
    }
    Err(DanDanError::ConnectTimeout)
}

// ── Command-line builder ──────────────────────────────────────────────────────

/// Whether the media is an HTTP URL or a local file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DanDanLaunchMode {
    /// Local file: appended `|filePath=<name>` to tell DandanPlay the
    /// display name (last part of `media_title`).
    Local,
    /// HTTP URL: opened via the `ddplay:` URI scheme with Windows `start`.
    Http,
}

/// Build the DandanPlay argument list (without the executable path itself).
///
/// Returns `(args, mode)` where `mode` indicates whether this is an HTTP or
/// local launch.
///
/// Pure function — easy to unit-test without spawning a process.
pub fn build_dandan_args(args: &LaunchArgs) -> (Vec<String>, DanDanLaunchMode) {
    let is_http = args.media_path.starts_with("http");
    if is_http {
        let encoded = urlencoding_encode(&args.media_path);
        (vec![format!("ddplay:{encoded}")], DanDanLaunchMode::Http)
    } else {
        let file_name = args
            .media_title
            .rsplit("  |  ")
            .next()
            .unwrap_or(args.media_title.as_str());
        let path_with_name = if args.mount_disk_mode {
            args.media_path.clone()
        } else {
            format!("{}|filePath={file_name}", args.media_path)
        };
        (vec![path_with_name], DanDanLaunchMode::Local)
    }
}

/// Percent-encode a URL string (RFC 3986 unreserved + sub-delimiters).
fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b':'
            | b'/'
            | b'?'
            | b'#'
            | b'['
            | b']'
            | b'@'
            | b'!'
            | b'$'
            | b'&'
            | b'\''
            | b'('
            | b')'
            | b'*'
            | b'+'
            | b','
            | b';'
            | b'=' => out.push(b as char),
            _ => {
                out.push('%');
                let hi = b >> 4;
                let lo = b & 0xF;
                out.push(char::from(if hi < 10 {
                    b'0' + hi
                } else {
                    b'A' + hi - 10
                }));
                out.push(char::from(if lo < 10 {
                    b'0' + lo
                } else {
                    b'A' + lo - 10
                }));
            }
        }
    }
    out
}

// ── DanDanHandle ──────────────────────────────────────────────────────────────

/// A running DandanPlay instance with an attached HTTP control client.
pub struct DanDanHandle {
    /// Connected HTTP API client.
    pub client: DanDanClient,
    /// Whether this is an HTTP stream launch (affects stop_sec logic).
    pub is_http: bool,
    /// Start position for seek-on-connect.
    pub start_sec: Option<f64>,
    child: Option<Child>,
}

impl DanDanHandle {
    /// Spawn DandanPlay and wait for its HTTP API to respond.
    ///
    /// For HTTP URLs, the player is launched via `start ddplay:url` on Windows;
    /// for local files it is launched directly.
    pub async fn spawn(
        args: &LaunchArgs,
        cfg: &DanDanConfig,
    ) -> Result<Self, DanDanError> {
        let (dandan_args, mode) = build_dandan_args(args);
        let child = match mode {
            DanDanLaunchMode::Local => {
                Some(Command::new(&args.exe).args(&dandan_args).spawn()?)
            }
            DanDanLaunchMode::Http => {
                // On Windows: `start "" ddplay:url` via cmd.exe
                #[cfg(windows)]
                {
                    Command::new("cmd")
                        .args(["/C", "start", ""])
                        .args(&dandan_args)
                        .spawn()?;
                    None
                }
                #[cfg(not(windows))]
                {
                    Some(Command::new(&args.exe).args(&dandan_args).spawn()?)
                }
            }
        };

        let client = connect_dandan(cfg).await?;

        // Seek to start_sec after API is ready (HTTP stream mode).
        if let Some(sec) = args.start_sec
            && mode == DanDanLaunchMode::Http
        {
            let _ = client.seek((sec * 1000.0) as u64).await;
        }

        Ok(Self {
            client,
            is_http: mode == DanDanLaunchMode::Http,
            start_sec: args.start_sec,
            child,
        })
    }

    /// Non-blocking check whether the DandanPlay process has exited.
    pub fn try_wait(&mut self) -> Option<io::Result<Option<ExitStatus>>> {
        self.child.as_mut().map(|c| c.try_wait())
    }

    /// Poll DandanPlay status until the player exits; return the last
    /// observed position in whole seconds.
    pub async fn stop_sec(&self) -> Option<i64> {
        let mut last_sec: Option<i64> = None;
        let mut stop_flag = false;
        loop {
            match self.client.current_video().await {
                Ok(status) => {
                    let pos_sec = status.position_sec();
                    if pos_sec > 0 {
                        last_sec = Some(pos_sec);
                    }
                    stop_flag = !status.seekable && status.position > 0.0;
                    if stop_flag && self.is_http {
                        break;
                    }
                    if self.is_http && status.position > 0.98 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Err(_) => {
                    if stop_flag {
                        info!("DandanPlay stop_flag, exiting");
                        break;
                    }
                    info!("DandanPlay API unreachable, stopping");
                    break;
                }
            }
        }
        info!("DandanPlay stopped, last position: {last_sec:?}s");
        last_sec
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use etlp_core::{IntroMarkers, Subtitle};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn base_args() -> LaunchArgs {
        LaunchArgs {
            exe: "dandanplay.exe".to_owned(),
            media_path: "C:\\Videos\\movie.mkv".to_owned(),
            media_title: "Anime S01  |  movie.mkv".to_owned(),
            start_sec: None,
            sub: Subtitle::default(),
            is_multiple_episodes: false,
            mount_disk_mode: false,
            intro: IntroMarkers::default(),
            fullscreen: false,
            disable_audio: false,
            http_proxy: None,
            static_ipc: None,
            event_handler: None,
            playlist_start: None,
            mpv_log_file: None,
        }
    }

    fn video_status_json(
        position: f64,
        duration: f64,
        ep_id: i64,
    ) -> serde_json::Value {
        json!({
            "Position": position,
            "Duration": duration,
            "EpisodeId": ep_id,
            "Seekable": true
        })
    }

    // ── DanDanVideoStatus ─────────────────────────────────────────────────────

    #[test]
    fn video_status_position_sec() {
        let s = DanDanVideoStatus {
            position: 0.5,
            duration: 3_600_000.0,
            episode_id: 1,
            seekable: true,
        };
        assert_eq!(s.position_sec(), 1800);
    }

    #[test]
    fn video_status_zero_position() {
        let s = DanDanVideoStatus {
            position: 0.0,
            duration: 3_600_000.0,
            episode_id: 1,
            seekable: true,
        };
        assert_eq!(s.position_sec(), 0);
    }

    // ── build_dandan_args ─────────────────────────────────────────────────────

    #[test]
    fn args_local_file_appends_file_path() {
        let (out, mode) = build_dandan_args(&base_args());
        assert_eq!(mode, DanDanLaunchMode::Local);
        let first = out.first().expect("should have arg");
        assert!(first.contains("|filePath=movie.mkv"));
        assert!(first.starts_with("C:\\Videos\\movie.mkv"));
    }

    #[test]
    fn args_http_url_produces_ddplay_scheme() {
        let mut a = base_args();
        a.media_path = "http://192.168.1.1/stream/movie.mkv".to_owned();
        let (out, mode) = build_dandan_args(&a);
        assert_eq!(mode, DanDanLaunchMode::Http);
        let first = out.first().expect("should have arg");
        assert!(first.starts_with("ddplay:"));
    }

    #[test]
    fn args_mount_disk_mode_no_filepath_suffix() {
        let mut a = base_args();
        a.mount_disk_mode = true;
        let (out, mode) = build_dandan_args(&a);
        assert_eq!(mode, DanDanLaunchMode::Local);
        let first = out.first().expect("should have arg");
        assert!(!first.contains("|filePath="));
    }

    // ── urlencoding_encode ────────────────────────────────────────────────────

    #[test]
    fn encode_space_and_chinese() {
        let s = "http://x.com/path with space";
        let encoded = urlencoding_encode(s);
        assert!(encoded.contains("%20"));
    }

    #[test]
    fn encode_preserves_http_scheme() {
        let s = "http://192.168.1.1/movie.mkv";
        let encoded = urlencoding_encode(s);
        assert!(encoded.starts_with("http://"));
    }

    // ── DanDanClient (via wiremock) ───────────────────────────────────────────

    #[tokio::test]
    async fn client_current_video_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/current/video"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(video_status_json(
                    0.5,
                    3_600_000.0,
                    42,
                )),
            )
            .mount(&server)
            .await;

        let base = format!("{}/api/v1", server.uri());
        let client =
            DanDanClient::with_base_url(&base, None).expect("build client");
        let s = client.current_video().await.expect("ok");
        assert_eq!(s.episode_id, 42);
        assert!((s.position - 0.5).abs() < 1e-9);
        assert_eq!(s.position_sec(), 1800);
    }

    #[tokio::test]
    async fn client_library_builds_map() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/library"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {"EpisodeId": 1, "Size": 100_000},
                {"EpisodeId": 2, "Size": 200_000}
            ])))
            .mount(&server)
            .await;

        let base = format!("{}/api/v1", server.uri());
        let client =
            DanDanClient::with_base_url(&base, None).expect("build client");
        let lib = client.library().await.expect("ok");
        assert_eq!(lib.get(&1), Some(&100_000));
        assert_eq!(lib.get(&2), Some(&200_000));
    }

    #[tokio::test]
    async fn client_current_video_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/current/video"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let base = format!("{}/api/v1", server.uri());
        let client =
            DanDanClient::with_base_url(&base, None).expect("build client");
        let result = client.current_video().await;
        assert!(result.is_err());
    }
}
