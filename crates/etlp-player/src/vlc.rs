//! VLC HTTP control API.
//!
//! VLC exposes a JSON HTTP API at `http://127.0.0.1:{port}/requests/`.
//! Authentication is HTTP Basic with an empty username and the password
//! `etlp`. All control commands are GET requests to `/requests/status.json`
//! with a `command=` query parameter.
//!
//! [`VlcHandle`] wraps the spawned VLC process and its [`VlcClient`] HTTP
//! connection. [`connect_vlc`] retries with a `[300, 500, 1500, 2000×15]`
//! delay sequence.

use std::io;
use std::path::Path;
use std::process::{Child, Command, ExitStatus};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;
use tracing::{info, warn};

use crate::mpv::LaunchArgs;

// ── Port allocation ───────────────────────────────────────────────────────────

static NEXT_PORT_SLOT: AtomicUsize = AtomicUsize::new(0);
const PORT_SLOT_COUNT: usize = 25;

/// Base HTTP port for VLC's built-in control interface.
///
/// Actual port = `PORT_BASE + slot`, where slot cycles 0..25.
pub const PORT_BASE: u16 = 58_423;

const VLC_PASSWD: &str = "embyToLocalPlayer";

// ── VlcPort ───────────────────────────────────────────────────────────────────

/// An allocated VLC HTTP interface port, rotating through 25 slots.
pub struct VlcPort {
    /// Assigned port number.
    pub port: u16,
}

impl VlcPort {
    /// Allocate the next available slot (wraps after 25 concurrent instances).
    pub fn generate() -> Self {
        let slot =
            NEXT_PORT_SLOT.fetch_add(1, Ordering::Relaxed) % PORT_SLOT_COUNT;
        Self {
            port: PORT_BASE + slot as u16,
        }
    }

    /// Wrap a known port value (e.g. read back from config or a test fixture).
    pub fn from_port(port: u16) -> Self {
        Self { port }
    }
}

// ── Error ─────────────────────────────────────────────────────────────────────

/// Errors that can arise when controlling VLC over its HTTP interface.
#[derive(Debug, Error)]
pub enum VlcError {
    /// HTTP or network error from reqwest.
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    /// OS-level IO error (process spawn, etc.).
    #[error("IO: {0}")]
    Io(#[from] io::Error),
    /// Could not connect to VLC HTTP API after all retry attempts.
    #[error("failed to connect to VLC HTTP API after retries")]
    ConnectTimeout,
}

// ── VlcStatus ─────────────────────────────────────────────────────────────────

/// Parsed fields from VLC's `/requests/status.json` response.
#[derive(Debug, Clone, Deserialize)]
pub struct VlcStatus {
    /// Current playback position in whole seconds.
    pub time: i64,
    /// Duration of the current media in whole seconds.
    pub length: i64,
    /// Raw `information` block (nested `category.meta.filename`).
    #[serde(default)]
    pub information: Option<Value>,
}

impl VlcStatus {
    /// Extract `information.category.meta.filename` from the nested JSON.
    ///
    /// Returns `None` when VLC is idle, the field is absent, or any
    /// intermediate level of the hierarchy is missing.
    pub fn filename(&self) -> Option<&str> {
        self.information
            .as_ref()
            .and_then(|v| v.get("category"))
            .and_then(|v| v.get("meta"))
            .and_then(|v| v.get("filename"))
            .and_then(Value::as_str)
    }
}

// ── VlcClient ─────────────────────────────────────────────────────────────────

/// HTTP client for VLC's JSON control API.
///
/// Cheap to clone — the underlying `reqwest::Client` connection pool is shared.
#[derive(Clone)]
pub struct VlcClient {
    client: reqwest::Client,
    /// `http://127.0.0.1:{port}/requests`
    base_url: String,
    passwd: String,
}

impl VlcClient {
    /// Build a client for the given port/password pair.
    ///
    /// Individual requests time out after 500 ms, matching the Python
    /// `requests_urllib(timeout=0.5)` calls.
    pub fn new(port: u16, passwd: &str) -> Result<Self, VlcError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(500))
            .build()?;
        Ok(Self {
            client,
            base_url: format!("http://127.0.0.1:{port}/requests"),
            passwd: passwd.to_owned(),
        })
    }

    /// Point the client at an arbitrary base URL (used in unit tests to
    /// redirect to a local mock server).
    #[cfg(test)]
    pub(crate) fn with_base_url(
        base_url: &str,
        passwd: &str,
    ) -> Result<Self, VlcError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(500))
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_owned(),
            passwd: passwd.to_owned(),
        })
    }

    /// Fetch current playback status from VLC.
    pub async fn status(&self) -> Result<VlcStatus, VlcError> {
        let url = format!("{}/status.json", self.base_url);
        Ok(self
            .client
            .get(&url)
            .basic_auth("", Some(&self.passwd))
            .send()
            .await?
            .json::<VlcStatus>()
            .await?)
    }

    /// Enqueue `path` (a local file path or percent-encoded URL) into VLC's
    /// playlist.
    pub async fn playlist_add(&self, path: &str) -> Result<(), VlcError> {
        self.command("in_enqueue", &[("input", path)]).await?;
        Ok(())
    }

    /// Send a control command to `/requests/status.json?command=cmd&...`.
    async fn command(
        &self,
        cmd: &str,
        extra: &[(&str, &str)],
    ) -> Result<Value, VlcError> {
        let url = format!("{}/status.json", self.base_url);
        let mut params: Vec<(&str, &str)> = vec![("command", cmd)];
        params.extend_from_slice(extra);
        Ok(self
            .client
            .get(&url)
            .basic_auth("", Some(&self.passwd))
            .query(&params)
            .send()
            .await?
            .json::<Value>()
            .await?)
    }
}

// ── Connect with retry ────────────────────────────────────────────────────────

/// Retry delays in milliseconds.
///
/// Pattern: `[300, 500, 1500] + [2000] × 15` = 18 total attempts.
const RETRY_DELAYS_MS: &[u64] = &[
    300, 500, 1500, 2000, 2000, 2000, 2000, 2000, 2000, 2000, 2000, 2000, 2000,
    2000, 2000, 2000, 2000, 2000,
];

/// Try to connect to VLC's HTTP API, sleeping between each attempt.
pub async fn connect_vlc(
    port: u16,
    passwd: &str,
) -> Result<VlcClient, VlcError> {
    let client = VlcClient::new(port, passwd)?;
    for &delay_ms in RETRY_DELAYS_MS {
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        if client.status().await.is_ok() {
            info!("VLC HTTP API connected on port {port}");
            return Ok(client);
        }
        warn!("VLC not yet ready on port {port}, retrying…");
    }
    Err(VlcError::ConnectTimeout)
}

// ── Command-line builder ──────────────────────────────────────────────────────

/// Build the VLC argument list (without the executable path itself).
///
/// Pure function — easy to unit-test without spawning a process.
///
/// The returned `Vec<String>` is passed directly to `Command::new(exe).args(…)`.
pub fn build_vlc_args(args: &LaunchArgs, port: &VlcPort) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    // On Windows, select the Qt GUI interface explicitly.
    #[cfg(windows)]
    out.extend(["-I".to_owned(), "qt".to_owned()]);

    // HTTP control interface — must come before the media path.
    out.extend([
        "--extraintf".to_owned(),
        "http".to_owned(),
        "--http-host".to_owned(),
        "127.0.0.1".to_owned(),
        "--http-port".to_owned(),
        port.port.to_string(),
        "--http-password".to_owned(),
        VLC_PASSWD.to_owned(),
    ]);

    // Windows-only: single-instance + playlist-enqueue mode.
    #[cfg(windows)]
    {
        out.push("--one-instance".to_owned());
        out.push("--playlist-enqueue".to_owned());
    }

    // Media path — wrap in a URI for disk / Blu-ray mode.
    let media_path = if args.mount_disk_mode {
        let has_ext = Path::new(&args.media_path)
            .extension()
            .is_some_and(|e| !e.is_empty());
        if has_ext {
            format!("file://{}", args.media_path)
        } else {
            format!("bluray://{}", args.media_path)
        }
    } else {
        args.media_path.clone()
    };
    out.push(media_path);

    // Subtitle — VLC doesn't support HTTP subtitle URLs; those must be
    // downloaded to a local temp file by the server layer first.
    if let Some(sub) = args.sub.external.as_deref()
        && !sub.is_empty()
        && !sub.starts_with("http")
    {
        out.push(format!(":sub-file={sub}"));
    }

    // Item-level start time.
    if let Some(sec) = args.start_sec {
        out.push(format!(":start-time={sec}"));
    }

    // Exit VLC when the last playlist item finishes.
    out.push("--play-and-exit".to_owned());

    if args.fullscreen {
        out.push("--fullscreen".to_owned());
    }

    out
}

// ── VlcHandle ─────────────────────────────────────────────────────────────────

/// A running VLC instance with an attached HTTP control client.
pub struct VlcHandle {
    /// Connected HTTP API client.
    pub client: VlcClient,
    child: Option<Child>,
}

impl VlcHandle {
    /// Spawn VLC with HTTP interface enabled and wait for the API to respond.
    pub async fn spawn(args: &LaunchArgs) -> Result<Self, VlcError> {
        let port = VlcPort::generate();
        let vlc_args = build_vlc_args(args, &port);
        let child = Command::new(&args.exe).args(&vlc_args).spawn()?;
        let client = connect_vlc(port.port, VLC_PASSWD).await?;
        Ok(Self {
            client,
            child: Some(child),
        })
    }

    /// Non-blocking check whether the VLC process has exited.
    pub fn try_wait(&mut self) -> Option<io::Result<Option<ExitStatus>>> {
        self.child.as_mut().map(|c| c.try_wait())
    }

    /// Poll VLC status until the process exits; return the last observed
    /// playback position in whole seconds. Polls every 500 ms.
    pub async fn stop_sec(&self) -> Option<i64> {
        let mut last: Option<i64> = None;
        loop {
            match self.client.status().await {
                Ok(s) => {
                    if s.time > 0 {
                        last = Some(s.time);
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Err(_) => {
                    info!("VLC stopped, last position: {last:?}s");
                    return last;
                }
            }
        }
    }

    /// Add a file/URL to the VLC playlist.
    pub async fn playlist_add(&self, path: &str) -> Result<(), VlcError> {
        self.client.playlist_add(path).await
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use etlp_core::{IntroMarkers, Subtitle};
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // ── helpers ───────────────────────────────────────────────────────────────

    fn base_args() -> LaunchArgs {
        LaunchArgs {
            exe: "vlc".to_owned(),
            media_path: "/data/video.mkv".to_owned(),
            media_title: "Test Video".to_owned(),
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
        }
    }

    fn status_json(time: i64, length: i64) -> serde_json::Value {
        json!({
            "time": time,
            "length": length,
            "information": {
                "category": {
                    "meta": {
                        "filename": "video.mkv"
                    }
                }
            }
        })
    }

    // ── VlcPort ───────────────────────────────────────────────────────────────

    #[test]
    fn port_generate_in_range() {
        let p = VlcPort::generate();
        assert!(p.port >= PORT_BASE && p.port < PORT_BASE + 25);
    }

    #[test]
    fn port_generate_unique_consecutive() {
        let a = VlcPort::generate();
        let b = VlcPort::generate();
        assert_ne!(a.port, b.port);
    }

    #[test]
    fn port_from_port_roundtrip() {
        let p = VlcPort::from_port(58_430);
        assert_eq!(p.port, 58_430);
    }

    // ── VlcStatus ─────────────────────────────────────────────────────────────

    #[test]
    fn status_filename_present() {
        let raw = json!({
            "time": 120,
            "length": 3600,
            "information": {
                "category": {
                    "meta": {
                        "filename": "episode01.mkv"
                    }
                }
            }
        });
        let s: VlcStatus = serde_json::from_value(raw).expect("parse");
        assert_eq!(s.filename(), Some("episode01.mkv"));
        assert_eq!(s.time, 120);
        assert_eq!(s.length, 3600);
    }

    #[test]
    fn status_filename_absent_when_idle() {
        let raw = json!({ "time": 0, "length": 0 });
        let s: VlcStatus = serde_json::from_value(raw).expect("parse");
        assert_eq!(s.filename(), None);
    }

    #[test]
    fn status_filename_absent_when_meta_missing() {
        let raw = json!({
            "time": 10,
            "length": 100,
            "information": { "category": {} }
        });
        let s: VlcStatus = serde_json::from_value(raw).expect("parse");
        assert_eq!(s.filename(), None);
    }

    // ── build_vlc_args ────────────────────────────────────────────────────────

    #[test]
    fn args_basic_contains_http_interface() {
        let port = VlcPort::from_port(58_423);
        let out = build_vlc_args(&base_args(), &port);
        assert!(out.contains(&"--extraintf".to_owned()));
        assert!(out.contains(&"http".to_owned()));
        assert!(out.contains(&"--http-port".to_owned()));
        assert!(out.contains(&"58423".to_owned()));
        assert!(out.contains(&"--http-password".to_owned()));
        assert!(out.contains(&"embyToLocalPlayer".to_owned()));
    }

    #[test]
    fn args_basic_contains_media_path() {
        let port = VlcPort::from_port(58_423);
        let out = build_vlc_args(&base_args(), &port);
        assert!(out.contains(&"/data/video.mkv".to_owned()));
    }

    #[test]
    fn args_basic_ends_with_play_and_exit() {
        let port = VlcPort::from_port(58_423);
        let out = build_vlc_args(&base_args(), &port);
        assert_eq!(out.last(), Some(&"--play-and-exit".to_owned()));
    }

    #[test]
    fn args_with_local_sub() {
        let mut a = base_args();
        a.sub.external = Some("/tmp/sub.srt".to_owned());
        let port = VlcPort::from_port(58_423);
        let out = build_vlc_args(&a, &port);
        assert!(out.iter().any(|s| s == ":sub-file=/tmp/sub.srt"));
    }

    #[test]
    fn args_http_sub_is_skipped() {
        let mut a = base_args();
        a.sub.external = Some("http://example.com/sub.srt".to_owned());
        let port = VlcPort::from_port(58_423);
        let out = build_vlc_args(&a, &port);
        assert!(!out.iter().any(|s| s.starts_with(":sub-file=")));
    }

    #[test]
    fn args_with_start_sec() {
        let mut a = base_args();
        a.start_sec = Some(123.5);
        let port = VlcPort::from_port(58_423);
        let out = build_vlc_args(&a, &port);
        assert!(out.iter().any(|s| s == ":start-time=123.5"));
    }

    #[test]
    fn args_mount_disk_with_ext_uses_file_uri() {
        let mut a = base_args();
        a.mount_disk_mode = true;
        a.media_path = "/mnt/disc/video.iso".to_owned();
        let port = VlcPort::from_port(58_423);
        let out = build_vlc_args(&a, &port);
        assert!(out.iter().any(|s| s == "file:///mnt/disc/video.iso"));
    }

    #[test]
    fn args_mount_disk_without_ext_uses_bluray_uri() {
        let mut a = base_args();
        a.mount_disk_mode = true;
        a.media_path = "/mnt/disc/BDMV".to_owned();
        let port = VlcPort::from_port(58_423);
        let out = build_vlc_args(&a, &port);
        assert!(out.iter().any(|s| s == "bluray:///mnt/disc/BDMV"));
    }

    #[test]
    fn args_fullscreen_flag() {
        let mut a = base_args();
        a.fullscreen = true;
        let port = VlcPort::from_port(58_423);
        let out = build_vlc_args(&a, &port);
        assert!(out.contains(&"--fullscreen".to_owned()));
    }

    // ── VlcClient (via wiremock) ───────────────────────────────────────────────

    #[tokio::test]
    async fn client_status_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/requests/status.json"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(status_json(42, 3600)),
            )
            .mount(&server)
            .await;

        let base = format!("{}/requests", server.uri());
        let client =
            VlcClient::with_base_url(&base, VLC_PASSWD).expect("build client");
        let s = client.status().await.expect("status ok");
        assert_eq!(s.time, 42);
        assert_eq!(s.length, 3600);
        assert_eq!(s.filename(), Some("video.mkv"));
    }

    #[tokio::test]
    async fn client_playlist_add_sends_in_enqueue() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/requests/status.json"))
            .and(query_param("command", "in_enqueue"))
            .and(query_param("input", "/data/ep2.mkv"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"time":0,"length":0})),
            )
            .mount(&server)
            .await;

        let base = format!("{}/requests", server.uri());
        let client =
            VlcClient::with_base_url(&base, VLC_PASSWD).expect("build client");
        client
            .playlist_add("/data/ep2.mkv")
            .await
            .expect("playlist_add ok");
    }

    #[tokio::test]
    async fn client_status_http_error_returns_vlc_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/requests/status.json"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let base = format!("{}/requests", server.uri());
        let client = VlcClient::with_base_url(&base, "wrongpasswd")
            .expect("build client");
        // 401 with no JSON body → JSON decode error, still a VlcError::Http
        let result = client.status().await;
        assert!(result.is_err());
    }
}
