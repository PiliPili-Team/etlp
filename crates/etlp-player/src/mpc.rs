//! MPC-HC (Media Player Classic - Home Cinema) HTTP control interface.
//!
//! MPC-HC exposes a web interface at `http://localhost:{port}/variables.html`
//! that returns an HTML page with playback state encoded as `<p id="key">value</p>`
//! elements. This module implements the HTML parser, HTTP client, arg builder,
//! and process handle.

use std::collections::HashMap;
use std::io;
use std::process::{Child, Command, ExitStatus};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use thiserror::Error;
use tracing::{info, warn};

use crate::mpv::LaunchArgs;

// ── Port allocation ───────────────────────────────────────────────────────────

static NEXT_PORT_SLOT: AtomicUsize = AtomicUsize::new(0);
const PORT_SLOT_COUNT: usize = 25;

/// Base HTTP port for MPC-HC's web interface.
pub const PORT_BASE: u16 = 58_423;

/// Allocated MPC-HC web-interface port, rotating through 25 slots.
pub struct MpcPort {
    pub port: u16,
}

impl MpcPort {
    pub fn generate() -> Self {
        let slot =
            NEXT_PORT_SLOT.fetch_add(1, Ordering::Relaxed) % PORT_SLOT_COUNT;
        Self {
            port: PORT_BASE + slot as u16,
        }
    }

    pub fn from_port(port: u16) -> Self {
        Self { port }
    }
}

// ── Error ─────────────────────────────────────────────────────────────────────

/// Errors from MPC-HC HTTP communication or process control.
#[derive(Debug, Error)]
pub enum MpcError {
    /// HTTP or network error from reqwest.
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    /// OS-level IO error (process spawn, etc.).
    #[error("IO: {0}")]
    Io(#[from] io::Error),
    /// The `variables.html` page could not be parsed to extract required fields.
    #[error("could not parse MPC-HC status from HTML")]
    ParseError,
    /// Could not connect to MPC-HC HTTP interface after all retry attempts.
    #[error("failed to connect to MPC-HC HTTP interface after retries")]
    ConnectTimeout,
}

// ── HTML parser ───────────────────────────────────────────────────────────────

/// Parse MPC-HC's `/variables.html` into an `id → value` map.
///
/// The page structure is:
/// ```html
/// <p id="position">12345</p>
/// <p id="state">2</p>
/// ```
/// Only `split`/`split_once` are used — no indexing, no regex, no panics.
pub fn parse_mpc_html(html: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for segment in html.split(r#"id=""#) {
        let Some((key, after_key)) = segment.split_once('"') else {
            continue;
        };
        let Some((_, after_gt)) = after_key.split_once('>') else {
            continue;
        };
        let Some((val, _)) = after_gt.split_once('<') else {
            continue;
        };
        map.insert(key.to_owned(), val.trim().to_owned());
    }
    map
}

// ── MpcStatus ─────────────────────────────────────────────────────────────────

/// Playback state extracted from MPC-HC's `variables.html`.
#[derive(Debug, Clone)]
pub struct MpcStatus {
    /// Current playback position in milliseconds.
    pub position_ms: i64,
    /// Total duration in milliseconds.
    pub duration_ms: i64,
    /// Playback state: -1 = stopped / no media, 1 = paused, 2 = playing.
    pub state: i32,
    /// Path of the currently loaded file.
    pub filepath: String,
}

impl MpcStatus {
    /// Build from the id→value map produced by [`parse_mpc_html`].
    ///
    /// Returns `None` if the `position` field is missing or unparseable.
    pub fn from_map(map: &HashMap<String, String>) -> Option<Self> {
        Some(Self {
            position_ms: map.get("position").and_then(|s| s.parse().ok())?,
            duration_ms: map
                .get("duration")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            state: map.get("state").and_then(|s| s.parse().ok()).unwrap_or(-1),
            filepath: map.get("filepath").cloned().unwrap_or_default(),
        })
    }

    /// Current position in whole seconds.
    pub fn position_sec(&self) -> i64 {
        self.position_ms / 1000
    }

    /// Total duration in whole seconds.
    pub fn duration_sec(&self) -> i64 {
        self.duration_ms / 1000
    }
}

// ── MpcClient ─────────────────────────────────────────────────────────────────

/// HTTP client for MPC-HC's web interface.
#[derive(Clone)]
pub struct MpcClient {
    client: reqwest::Client,
    /// `http://localhost:{port}/variables.html`
    url: String,
}

impl MpcClient {
    /// Build a client for the given port.
    ///
    /// Individual requests time out after 1 s — matching the Python
    /// `requests_urllib(timeout=1)` polling call.
    pub fn new(port: u16) -> Result<Self, MpcError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(1))
            .build()?;
        Ok(Self {
            client,
            url: format!("http://localhost:{port}/variables.html"),
        })
    }

    /// Used by tests to redirect to a local mock server.
    #[cfg(test)]
    pub(crate) fn with_url(url: &str) -> Result<Self, MpcError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(1))
            .build()?;
        Ok(Self {
            client,
            url: url.to_owned(),
        })
    }

    /// Fetch and parse the current playback status.
    pub async fn status(&self) -> Result<MpcStatus, MpcError> {
        let html = self.client.get(&self.url).send().await?.text().await?;
        let map = parse_mpc_html(&html);
        MpcStatus::from_map(&map).ok_or(MpcError::ParseError)
    }
}

// ── Connect with retry ────────────────────────────────────────────────────────

/// Retry delays in milliseconds for MPC-HC connect attempts.
const RETRY_DELAYS_MS: &[u64] = &[
    300, 500, 1500, 2000, 2000, 2000, 2000, 2000, 2000, 2000, 2000, 2000, 2000,
    2000, 2000, 2000, 2000, 2000,
];

/// Try to connect to MPC-HC's web interface with exponential back-off.
pub async fn connect_mpc(port: u16) -> Result<MpcClient, MpcError> {
    let client = MpcClient::new(port)?;
    for &delay_ms in RETRY_DELAYS_MS {
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        if client.status().await.is_ok() {
            info!("MPC-HC web interface connected on port {port}");
            return Ok(client);
        }
        warn!("MPC-HC not yet ready on port {port}, retrying…");
    }
    Err(MpcError::ConnectTimeout)
}

// ── Command-line builder ──────────────────────────────────────────────────────

/// Build the MPC-HC argument list (without the executable path itself).
///
/// Pure function — easy to unit-test without spawning a process.
pub fn build_mpc_args(args: &LaunchArgs, port: &MpcPort) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    // Primary media file.
    out.push(args.media_path.clone());

    // External subtitle.
    if let Some(sub) = args.sub.external.as_deref()
        && !sub.is_empty()
    {
        out.push("/sub".to_owned());
        out.push(sub.to_owned());
    }

    // Start position in milliseconds.
    if let Some(sec) = args.start_sec {
        out.push("/start".to_owned());
        out.push(format!("{}", (sec * 1000.0) as i64));
    }

    if args.fullscreen {
        out.push("/fullscreen".to_owned());
    }

    // Auto-play and close on finish.
    out.push("/play".to_owned());
    out.push("/close".to_owned());

    // Web interface port for remote control.
    out.push("/webport".to_owned());
    out.push(port.port.to_string());

    out
}

// ── MpcHandle ─────────────────────────────────────────────────────────────────

/// A running MPC-HC instance with an attached HTTP control client.
pub struct MpcHandle {
    /// Connected web interface client.
    pub client: MpcClient,
    /// Path to the MPC-HC executable (needed for `/add` playlist operations).
    pub exe: String,
    child: Option<Child>,
}

impl MpcHandle {
    /// Spawn MPC-HC and wait for its web interface to respond.
    pub async fn spawn(args: &LaunchArgs) -> Result<Self, MpcError> {
        let port = MpcPort::generate();
        let mpc_args = build_mpc_args(args, &port);
        let child = Command::new(&args.exe).args(&mpc_args).spawn()?;
        let client = connect_mpc(port.port).await?;
        Ok(Self {
            client,
            exe: args.exe.clone(),
            child: Some(child),
        })
    }

    /// Non-blocking check whether the MPC-HC process has exited.
    pub fn try_wait(&mut self) -> Option<io::Result<Option<ExitStatus>>> {
        self.child.as_mut().map(|c| c.try_wait())
    }

    /// OS process id of the spawned MPC-HC process, if available.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    /// Add a file to the MPC-HC playlist by spawning an extra process call.
    ///
    /// MPC-HC accepts `/add <path>` as a command-line argument to a second
    /// instance, which is then forwarded to the running instance.
    pub fn playlist_add(
        &self,
        path: &str,
        sub_file: Option<&str>,
    ) -> Result<(), MpcError> {
        let mut cmd = Command::new(&self.exe);
        cmd.arg("/add").arg(path);
        if let Some(sub) = sub_file {
            cmd.arg("/sub").arg(sub);
        }
        cmd.spawn()?.wait()?;
        Ok(())
    }

    /// Poll MPC-HC status until it exits; return the last observed position (s).
    ///
    /// Maintains a two-slot debounce buffer and returns the penultimate value
    /// to avoid the brief reset-to-zero that some players perform just before
    /// closing.
    pub async fn stop_sec(&self) -> Option<i64> {
        let mut prev: Option<i64> = None;
        let mut curr: Option<i64> = None;
        loop {
            match self.client.status().await {
                Ok(s) if s.state == -1 => {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                }
                Ok(s) => {
                    prev = curr;
                    curr = Some(s.position_sec());
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Err(_) => {
                    let result = prev.or(curr);
                    info!("MPC-HC stopped, last position: {result:?}s");
                    return result;
                }
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use etlp_core::{IntroMarkers, Subtitle};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn base_args() -> LaunchArgs {
        LaunchArgs {
            exe: "mpc-hc64.exe".to_owned(),
            media_path: "C:\\Videos\\movie.mkv".to_owned(),
            media_title: "Test Movie".to_owned(),
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

    const SAMPLE_HTML: &str = r#"<html><body>
<p id="file"></p>
<p id="filepath">C:\Videos\movie.mkv</p>
<p id="state">2</p>
<p id="position">12345</p>
<p id="duration">3600000</p>
<p id="version">1.9.25</p>
</body></html>"#;

    // ── parse_mpc_html ────────────────────────────────────────────────────────

    #[test]
    fn parse_extracts_all_fields() {
        let map = parse_mpc_html(SAMPLE_HTML);
        assert_eq!(
            map.get("filepath").map(String::as_str),
            Some("C:\\Videos\\movie.mkv")
        );
        assert_eq!(map.get("state").map(String::as_str), Some("2"));
        assert_eq!(map.get("position").map(String::as_str), Some("12345"));
        assert_eq!(map.get("duration").map(String::as_str), Some("3600000"));
        assert_eq!(map.get("version").map(String::as_str), Some("1.9.25"));
    }

    #[test]
    fn parse_empty_html_returns_empty_map() {
        let map = parse_mpc_html("<html></html>");
        assert!(map.is_empty());
    }

    // ── MpcStatus ─────────────────────────────────────────────────────────────

    #[test]
    fn status_from_map_full() {
        let map = parse_mpc_html(SAMPLE_HTML);
        let s = MpcStatus::from_map(&map).expect("parse ok");
        assert_eq!(s.position_ms, 12345);
        assert_eq!(s.duration_ms, 3600000);
        assert_eq!(s.state, 2);
        assert_eq!(s.filepath, "C:\\Videos\\movie.mkv");
        assert_eq!(s.position_sec(), 12);
        assert_eq!(s.duration_sec(), 3600);
    }

    #[test]
    fn status_from_empty_map_returns_none() {
        let map = HashMap::new();
        assert!(MpcStatus::from_map(&map).is_none());
    }

    // ── build_mpc_args ────────────────────────────────────────────────────────

    #[test]
    fn args_basic_contains_media_and_controls() {
        let port = MpcPort::from_port(58_423);
        let out = build_mpc_args(&base_args(), &port);
        assert!(out.contains(&"C:\\Videos\\movie.mkv".to_owned()));
        assert!(out.contains(&"/play".to_owned()));
        assert!(out.contains(&"/close".to_owned()));
        assert!(out.contains(&"/webport".to_owned()));
        assert!(out.contains(&"58423".to_owned()));
    }

    #[test]
    fn args_no_fullscreen_by_default() {
        let port = MpcPort::from_port(58_423);
        let out = build_mpc_args(&base_args(), &port);
        assert!(!out.contains(&"/fullscreen".to_owned()));
    }

    #[test]
    fn args_fullscreen() {
        let mut a = base_args();
        a.fullscreen = true;
        let port = MpcPort::from_port(58_423);
        let out = build_mpc_args(&a, &port);
        assert!(out.contains(&"/fullscreen".to_owned()));
    }

    #[test]
    fn args_with_sub() {
        let mut a = base_args();
        a.sub.external = Some("C:\\subs\\movie.srt".to_owned());
        let port = MpcPort::from_port(58_423);
        let out = build_mpc_args(&a, &port);
        assert!(out.contains(&"/sub".to_owned()));
        assert!(out.contains(&"C:\\subs\\movie.srt".to_owned()));
    }

    #[test]
    fn args_with_start_sec() {
        let mut a = base_args();
        a.start_sec = Some(90.0);
        let port = MpcPort::from_port(58_423);
        let out = build_mpc_args(&a, &port);
        assert!(out.contains(&"/start".to_owned()));
        assert!(out.contains(&"90000".to_owned())); // 90s * 1000ms
    }

    // ── MpcClient (via wiremock) ──────────────────────────────────────────────

    #[tokio::test]
    async fn client_status_parses_html() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/variables.html"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(SAMPLE_HTML),
            )
            .mount(&server)
            .await;

        let url = format!("{}/variables.html", server.uri());
        let client = MpcClient::with_url(&url).expect("build client");
        let s = client.status().await.expect("status ok");
        assert_eq!(s.position_ms, 12345);
        assert_eq!(s.state, 2);
    }

    #[tokio::test]
    async fn client_status_parse_error_on_bad_html() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/variables.html"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("<html><body>no ids here</body></html>"),
            )
            .mount(&server)
            .await;

        let url = format!("{}/variables.html", server.uri());
        let client = MpcClient::with_url(&url).expect("build client");
        let result = client.status().await;
        assert!(matches!(result, Err(MpcError::ParseError)));
    }
}
