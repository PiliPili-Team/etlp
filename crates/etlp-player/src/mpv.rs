//! High-level mpv control.
//!
//! [`MpvHandle`] wraps the spawned mpv process and its [`MpvClient`] IPC
//! connection. Typed command methods (`time_pos`, `set_chapter_list`, …)
//! build on the raw JSON IPC implemented in [`crate::transport`].

use std::sync::atomic::{AtomicUsize, Ordering};

use etlp_core::{IntroMarkers, Subtitle};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::transport::{ClientError, EventHandler, MpvClient};

// ── Error ─────────────────────────────────────────────────────────────────────

/// Errors from mpv process launch or IPC communication.
#[derive(Debug, Error)]
pub enum PlayerError {
    /// IPC-level error (connect, read, write, mpv error string).
    #[error("IPC: {0}")]
    Ipc(#[from] ClientError),
    /// OS-level IO error (spawn, socket).
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    /// JSON (de)serialization error.
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// Could not connect to mpv IPC after all retry attempts.
    #[error("failed to connect to mpv IPC after retries")]
    ConnectTimeout,
}

// ── IPC socket path ───────────────────────────────────────────────────────────

/// Cycling slot counter (0..25, wraps).
static NEXT_SLOT: AtomicUsize = AtomicUsize::new(0);
const SLOT_COUNT: usize = 25;

/// Platform-specific mpv IPC socket / named-pipe path.
///
/// Unix:    `/tmp/pipe_name<X>.pipe`
/// Windows: `\\.\pipe\pipe_name<X>`
///
/// where `<X>` is a letter A–Y rotated across up to 25 concurrent instances.
#[derive(Debug, Clone)]
pub struct IpcPath {
    /// Full path passed to `--input-ipc-server` and to `MpvClient::connect`.
    pub path: std::path::PathBuf,
}

impl IpcPath {
    /// Generate a new, unused socket path.
    pub fn generate() -> Self {
        let slot = NEXT_SLOT.fetch_add(1, Ordering::Relaxed) % SLOT_COUNT;
        let suffix = char::from(b'A' + slot as u8);

        #[cfg(unix)]
        {
            Self {
                path: std::path::PathBuf::from(format!(
                    "/tmp/pipe_name{suffix}.pipe"
                )),
            }
        }
        #[cfg(windows)]
        {
            Self {
                path: std::path::PathBuf::from(format!(
                    r"\\.\pipe\pipe_name{suffix}"
                )),
            }
        }
    }

    /// Override with a user-configured static path (dev config `mpv_input_ipc_server`).
    pub fn from_static(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

// ── Chapter ───────────────────────────────────────────────────────────────────

/// A single entry in mpv's internal playlist.
///
/// Returned by `get_property playlist` — only the fields relevant for
/// redirect and progress tracking are deserialized.
#[derive(Debug, Clone, Deserialize)]
pub struct MpvPlaylistEntry {
    /// The URL or local path currently held in this slot.
    pub filename: String,
    /// `true` when this is the entry currently being played.
    #[serde(default)]
    pub playing: Option<bool>,
    /// `true` when this is the "current" entry (selected but may be paused).
    #[serde(default)]
    pub current: Option<bool>,
}

impl MpvPlaylistEntry {
    /// Returns `true` when mpv marks this entry as currently playing.
    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing.unwrap_or(false) || self.current.unwrap_or(false)
    }
}

/// A chapter entry as mpv reports / accepts it.
///
/// Maps to the JSON object `{"title": "...", "time": ...}` used by mpv's
/// `get_property chapter-list` / `set_property chapter-list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    pub title: String,
    /// Chapter start time in seconds.
    pub time: f64,
}

impl Chapter {
    /// Build an `[Opening, Main]` chapter list from intro markers.
    ///
    /// Returns `None` when the markers are incomplete.
    pub fn from_intro(intro: &IntroMarkers) -> Option<Vec<Self>> {
        let (start, end) = (intro.start?, intro.end?);
        Some(vec![
            Chapter {
                title: "Opening".into(),
                time: start as f64,
            },
            Chapter {
                title: "Main".into(),
                time: end as f64,
            },
        ])
    }
}

// ── LoadMode ──────────────────────────────────────────────────────────────────

/// How `loadfile` adds the new file to mpv's playlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadMode {
    /// Replace the current playlist and start playing.
    Replace,
    /// Append to the end; start if the playlist is empty.
    AppendPlay,
    /// Append to the end without starting (used when `--pause` is active).
    Append,
    /// Insert at a specific index (requires mpv ≥ 0.38 / new `loadfile` cmd).
    InsertAt(i64),
}

impl LoadMode {
    fn as_str(self) -> &'static str {
        match self {
            LoadMode::Replace => "replace",
            LoadMode::AppendPlay => "append-play",
            LoadMode::Append => "append",
            LoadMode::InsertAt(_) => "insert-at",
        }
    }

    fn index(self) -> i64 {
        match self {
            LoadMode::InsertAt(i) => i,
            _ => -1,
        }
    }
}

// ── LoadOptions ───────────────────────────────────────────────────────────────

/// Per-file options appended to the `loadfile` command.
///
/// Corresponds to the `options` string in Python's
/// `mpv.command('loadfile', path, mode, index, options)`.
#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
    /// Title shown in mpv's OSD and in the playlist.
    pub media_title: Option<String>,
    /// Start position in seconds (None = from beginning).
    pub start_sec: Option<f64>,
    /// External subtitle URL or path.
    pub sub_file: Option<String>,
    /// Previously active main subtitle (for `sub-files-remove/append`).
    pub sub_file_prev: Option<String>,
    /// Embedded subtitle track index.
    pub sub_inner_index: Option<i64>,
    /// Path to a ffmetadata chapters file.
    pub chapters_file: Option<String>,
}

impl LoadOptions {
    /// Render as the comma-separated options string expected by `loadfile`.
    pub fn build(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        if let Some(t) = &self.media_title {
            parts.push(format!("force-media-title=\"{t}\""));
            parts.push(format!("osd-playing-msg=\"{t}\""));
        }
        if let Some(s) = self.start_sec {
            parts.push(format!("start={s}"));
        }
        if let Some(f) = &self.sub_file {
            if let Some(prev) = &self.sub_file_prev {
                parts.push(format!("sub-files-remove={prev}"));
                parts.push(format!("sub-files-append={prev}"));
                parts.push(format!("sub-files-append={f}"));
            } else {
                parts.push(format!("sub-file={f}"));
            }
        }
        if let Some(i) = self.sub_inner_index {
            parts.push(format!("sid={i}"));
        }
        if let Some(c) = &self.chapters_file {
            parts.push(format!("chapters-file=\"{c}\""));
        }
        parts.join(",")
    }
}

// ── LaunchArgs ────────────────────────────────────────────────────────────────

/// Everything needed to start mpv and connect to it.
///
/// Field values are resolved from config before constructing this struct —
/// `etlp-player` intentionally does not depend on `etlp-config`.
#[derive(Clone)]
pub struct LaunchArgs {
    /// Path to the mpv / iina-cli / mpvnet executable.
    pub exe: String,
    /// The first file/URL to play.
    pub media_path: String,
    /// Human-readable title for OSD.
    pub media_title: String,
    /// Start position (None = from beginning).
    pub start_sec: Option<f64>,
    /// Subtitle for the first episode.
    pub sub: Subtitle,
    /// Pause at start and build a playlist.
    pub is_multiple_episodes: bool,
    /// Disk / Blu-ray mode (changes subtitle + start_sec handling).
    pub mount_disk_mode: bool,
    /// Intro markers for the first episode (chapter injection).
    pub intro: IntroMarkers,
    /// Start fullscreen.
    pub fullscreen: bool,
    /// Disable audio track.
    pub disable_audio: bool,
    /// HTTP proxy `host:port` forwarded to mpv.
    pub http_proxy: Option<String>,
    /// Override IPC socket path (dev config `mpv_input_ipc_server`).
    pub static_ipc: Option<String>,
    /// Event handler forwarded to [`MpvClient`].
    pub event_handler: Option<EventHandler>,
}

// ── Command-line builder ──────────────────────────────────────────────────────

/// Build the mpv argument list (without the executable path itself).
///
/// Pure function — easy to unit-test without spawning a process.
pub fn build_args(args: &LaunchArgs, ipc: &IpcPath) -> Vec<String> {
    let exe_lower = args.exe.to_lowercase();
    let is_iina = exe_lower.contains("iina");
    let is_mpvnet = exe_lower.contains("mpvnet");
    let is_darwin = cfg!(target_os = "macos");

    let mut cmd: Vec<String> = vec![args.media_path.clone()];

    if let Some(idx) = args.sub.inner_index {
        cmd.push(format!("--sid={idx}"));
    }
    if let Some(sub) = &args.sub.external
        && !is_iina
        && !is_mpvnet
    {
        cmd.push(format!("--sub-files-toggle={sub}"));
        // iina + !mount_disk needs save_sub_file; deferred to server layer.
    }

    if !args.mount_disk_mode || !is_iina {
        cmd.push(format!("--force-media-title={}", args.media_title));
        cmd.push(format!("--osd-playing-msg={}", args.media_title));
    }

    if !args.mount_disk_mode {
        cmd.push("--force-window=immediate".into());
        if let Some(proxy) = &args.http_proxy {
            cmd.push(format!("--http-proxy=http://{proxy}"));
            if args.is_multiple_episodes {
                cmd.push("--cache=no".into());
            }
        }
    }

    if let Some(sec) = args.start_sec {
        // iina in disk mode: start_sec would affect next episode — skip.
        if !(is_iina && args.mount_disk_mode) {
            cmd.push(format!("--start={sec}"));
        }
    }

    if is_darwin {
        cmd.push("--focus-on=open".into());
    }

    let ipc_str = ipc.path.to_string_lossy();
    cmd.push(format!("--input-ipc-server={ipc_str}"));
    cmd.push("--script-opts-append=autoload-disabled=yes".into());

    if args.fullscreen {
        cmd.push("--fullscreen=yes".into());
    }
    if args.disable_audio {
        cmd.push("--no-audio".into());
    }
    if args.is_multiple_episodes {
        cmd.push("--pause".into());
    }

    // iina passes mpv args with a `--mpv-` prefix instead of `--`.
    if is_darwin && is_iina {
        cmd = cmd
            .into_iter()
            .map(|s| {
                if s.starts_with("--") {
                    format!("--mpv-{}", s.trim_start_matches("--"))
                } else {
                    s
                }
            })
            .collect();
    }

    cmd
}

// ── Retry-connect ─────────────────────────────────────────────────────────────

/// Connect to the mpv IPC socket with exponential retry.
///
/// Delay sequence: `[0.3, 0.5, 1.5]` then `2.0` × 15 attempts.
pub async fn connect_with_retry(
    path: &std::path::Path,
    event_handler: Option<EventHandler>,
) -> Result<MpvClient, PlayerError> {
    const INITIAL_DELAYS_MS: &[u64] = &[300, 500, 1500];
    const LONG_DELAY_MS: u64 = 2000;
    const LONG_RETRIES: usize = 15;

    let total_attempts = INITIAL_DELAYS_MS.len() + LONG_RETRIES;
    for attempt in 0..total_attempts {
        let delay_ms = INITIAL_DELAYS_MS
            .get(attempt)
            .copied()
            .unwrap_or(LONG_DELAY_MS);
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        if let Ok(client) =
            MpvClient::connect(path, event_handler.clone()).await
        {
            return Ok(client);
        }
    }
    Err(PlayerError::ConnectTimeout)
}

// ── MpvHandle ─────────────────────────────────────────────────────────────────

/// A running mpv instance: spawned OS process + live IPC connection.
pub struct MpvHandle {
    /// The IPC client — use directly for raw commands not covered by the typed API.
    pub client: MpvClient,
    /// The child process handle.  `None` in unit tests / when constructed without spawn.
    child: Option<std::process::Child>,
    /// True when the executable is iina-cli (macOS mpv front-end).
    pub is_iina: bool,
    /// True when the executable is mpvnet (Windows mpv front-end).
    pub is_mpvnet: bool,
}

impl MpvHandle {
    /// Start mpv and return a connected handle.
    ///
    /// Spawns the process, waits for the IPC socket to appear (with retry),
    /// injects intro chapters, and sends the `etlp-cmd-pipe` script message
    /// that some companion Lua scripts use.
    pub async fn spawn(args: LaunchArgs) -> Result<Self, PlayerError> {
        let ipc = match &args.static_ipc {
            Some(p) => IpcPath::from_static(p),
            None => IpcPath::generate(),
        };
        let cmd_args = build_args(&args, &ipc);
        let child = std::process::Command::new(&args.exe)
            .args(&cmd_args)
            .spawn()?;

        let is_iina = args.exe.to_lowercase().contains("iina");
        let is_mpvnet = args.exe.to_lowercase().contains("mpvnet");

        let client =
            connect_with_retry(&ipc.path, args.event_handler.clone()).await?;

        let handle = Self {
            client,
            child: Some(child),
            is_iina,
            is_mpvnet,
        };

        // Inject intro chapters into the first episode if available.
        if let Some(chapters) = Chapter::from_intro(&args.intro) {
            handle.set_chapter_list(&chapters).await?;
        }

        // Notify companion Lua scripts of the IPC pipe path.
        let pipe_str = ipc.path.to_string_lossy();
        handle
            .client
            .command(
                "script-message",
                &[json!("etlp-cmd-pipe"), json!(pipe_str.as_ref())],
            )
            .await
            .ok(); // non-fatal; no Lua script may be loaded

        Ok(handle)
    }

    /// Check whether the child process has already exited.
    ///
    /// Returns `None` when there is no child (test / externally-managed handle).
    pub fn try_wait(
        &mut self,
    ) -> Option<std::io::Result<Option<std::process::ExitStatus>>> {
        self.child.as_mut().map(std::process::Child::try_wait)
    }
}

// ── High-level command helpers ────────────────────────────────────────────────

impl MpvHandle {
    // ── Properties ────────────────────────────────────────────────────────────

    /// Current playback position in seconds, or `None` when mpv has not yet
    /// started playing (property unavailable).
    pub async fn time_pos(&self) -> Result<Option<f64>, PlayerError> {
        let v = self
            .client
            .command("get_property", &[json!("time-pos")])
            .await?;
        Ok(v.and_then(|x| x.as_f64()))
    }

    /// Media title as mpv sees it (may differ from the forced title).
    pub async fn media_title(&self) -> Result<Option<String>, PlayerError> {
        let v = self
            .client
            .command("get_property", &[json!("media-title")])
            .await?;
        Ok(v.and_then(|x| x.as_str().map(str::to_owned)))
    }

    /// Playback speed multiplier.
    pub async fn speed(&self) -> Result<Option<f64>, PlayerError> {
        let v = self
            .client
            .command("get_property", &[json!("speed")])
            .await?;
        Ok(v.and_then(|x| x.as_f64()))
    }

    /// Total media duration in seconds.
    pub async fn duration(&self) -> Result<Option<f64>, PlayerError> {
        let v = self
            .client
            .command("get_property", &[json!("duration")])
            .await?;
        Ok(v.and_then(|x| x.as_f64()))
    }

    /// The current chapter index (0-based), or `None` when unavailable.
    pub async fn chapter_index(&self) -> Result<Option<i64>, PlayerError> {
        let v = self
            .client
            .command("get_property", &[json!("chapter")])
            .await?;
        Ok(v.and_then(|x| x.as_i64()))
    }

    /// The current chapter list.
    pub async fn chapter_list(&self) -> Result<Vec<Chapter>, PlayerError> {
        let v = self
            .client
            .command("get_property", &[json!("chapter-list")])
            .await?;
        match v {
            None => Ok(Vec::new()),
            Some(val) => Ok(serde_json::from_value(val)?),
        }
    }

    // ── Mutating commands ─────────────────────────────────────────────────────

    /// Pause or resume playback.
    pub async fn set_pause(&self, paused: bool) -> Result<(), PlayerError> {
        let val = if paused { "yes" } else { "no" };
        self.client
            .command("set", &[json!("pause"), json!(val)])
            .await?;
        Ok(())
    }

    /// Inject a chapter list into the currently playing file.
    pub async fn set_chapter_list(
        &self,
        chapters: &[Chapter],
    ) -> Result<(), PlayerError> {
        let v = serde_json::to_value(chapters)?;
        self.client
            .command("set_property", &[json!("chapter-list"), v])
            .await?;
        Ok(())
    }

    /// Skip to the next chapter (add 1 to the chapter index).
    ///
    /// Used by the intro-skip logic to jump past an `Opening` chapter.
    pub async fn advance_chapter(&self) -> Result<(), PlayerError> {
        self.client
            .command("add", &[json!("chapter"), json!(1i64)])
            .await?;
        Ok(())
    }

    /// Set an arbitrary mpv property by name.
    pub async fn set_property_value(
        &self,
        name: &str,
        value: Value,
    ) -> Result<(), PlayerError> {
        self.client
            .command("set_property", &[json!(name), value])
            .await?;
        Ok(())
    }

    // ── Playlist ──────────────────────────────────────────────────────────────

    /// Detect whether mpv supports the new `loadfile` command format (≥ 0.38).
    ///
    /// The new format includes an `index` argument between the mode and the
    /// options string.  Cache the result and pass it to [`Self::loadfile`].
    pub async fn detect_new_loadfile_format(&self) -> bool {
        let result = self
            .client
            .command("get_property", &[json!("command-list")])
            .await;
        match result {
            Ok(Some(val)) => {
                let cmds = match val.as_array() {
                    Some(a) => a,
                    None => return false,
                };
                cmds.iter().any(|c| {
                    c.get("name").and_then(Value::as_str) == Some("loadfile")
                        && c.get("args").and_then(Value::as_array).is_some_and(
                            |args| {
                                args.iter().any(|a| {
                                    a.get("name").and_then(Value::as_str)
                                        == Some("index")
                                })
                            },
                        )
                })
            }
            _ => false,
        }
    }

    /// Add or replace a file in mpv's playlist.
    ///
    /// `new_format`: pass the result of [`Self::detect_new_loadfile_format`],
    /// cached after the first call.  When `false`, the `index` argument is
    /// omitted (old mpv compat) and `LoadMode::InsertAt` is silently ignored.
    pub async fn loadfile(
        &self,
        path: &str,
        mode: LoadMode,
        opts: &LoadOptions,
        new_format: bool,
    ) -> Result<(), PlayerError> {
        let opts_str = opts.build();
        let mut args: Vec<Value> = vec![json!(path), json!(mode.as_str())];
        if new_format {
            args.push(json!(mode.index()));
        }
        if !opts_str.is_empty() {
            args.push(json!(opts_str));
        }
        self.client.command("loadfile", &args).await?;
        Ok(())
    }

    /// Jump to a playlist index.
    pub async fn playlist_play_index(
        &self,
        index: i64,
    ) -> Result<(), PlayerError> {
        self.client
            .command("playlist-play-index", &[json!(index)])
            .await?;
        Ok(())
    }

    /// Step to the previous playlist entry.
    pub async fn playlist_prev(&self) -> Result<(), PlayerError> {
        self.client.command("playlist-prev", &[]).await?;
        Ok(())
    }

    /// Step to the next playlist entry.
    pub async fn playlist_next(&self) -> Result<(), PlayerError> {
        self.client.command("playlist-next", &[]).await?;
        Ok(())
    }

    /// Retrieve mpv's current playlist.
    pub async fn get_playlist(
        &self,
    ) -> Result<Vec<MpvPlaylistEntry>, PlayerError> {
        let val = self
            .client
            .command("get_property", &[json!("playlist")])
            .await?;
        match val {
            None => Ok(Vec::new()),
            Some(v) => {
                let entries: Vec<MpvPlaylistEntry> = serde_json::from_value(v)?;
                Ok(entries)
            }
        }
    }

    /// Move the playlist entry at `from` to position `to`.
    pub async fn playlist_move(
        &self,
        from: i64,
        to: i64,
    ) -> Result<(), PlayerError> {
        self.client
            .command("playlist-move", &[json!(from), json!(to)])
            .await?;
        Ok(())
    }

    /// Remove the playlist entry at `index`.
    pub async fn playlist_remove(&self, index: i64) -> Result<(), PlayerError> {
        self.client
            .command("playlist-remove", &[json!(index)])
            .await?;
        Ok(())
    }

    // ── Misc ─────────────────────────────────────────────────────────────────

    /// Expand mpv property references in `expr` and return the result.
    pub async fn expand_text(
        &self,
        expr: &str,
    ) -> Result<Option<String>, PlayerError> {
        let v = self.client.command("expand-text", &[json!(expr)]).await?;
        Ok(v.and_then(|x| x.as_str().map(str::to_owned)))
    }

    /// Send a `script-message` command to all loaded Lua/JS scripts.
    pub async fn script_message(
        &self,
        args: &[&str],
    ) -> Result<(), PlayerError> {
        let json_args: Vec<Value> = args.iter().map(|s| json!(*s)).collect();
        self.client.command("script-message", &json_args).await?;
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt as _, BufReader};

    // ── test helpers ──────────────────────────────────────────────────────────

    /// Create a `MpvHandle` backed by a `tokio::io::duplex()` pair.
    fn make_handle() -> (MpvHandle, tokio::io::DuplexStream) {
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (r, w) = tokio::io::split(client_io);
        let client = MpvClient::from_halves(Box::new(r), Box::new(w), None);
        let handle = MpvHandle {
            client,
            child: None,
            is_iina: false,
            is_mpvnet: false,
        };
        (handle, server_io)
    }

    /// Drive the server side: read `count` requests, call `handler(req)` for
    /// each, and send back `{request_id, error: "success", data: <handler result>}`.
    async fn serve_n(
        server: tokio::io::DuplexStream,
        count: usize,
        handler: impl Fn(Value) -> Value + Send + 'static,
    ) {
        tokio::spawn(async move {
            let (r, mut w) = tokio::io::split(server);
            let mut reader = BufReader::new(r);
            for _ in 0..count {
                let mut line = String::new();
                if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                    break;
                }
                let req: Value =
                    serde_json::from_str(line.trim_end_matches('\n'))
                        .expect("parse");
                let id =
                    req.get("request_id").and_then(Value::as_u64).expect("id");
                let data = handler(req);
                let resp =
                    json!({"request_id": id, "error": "success", "data": data});
                let mut bytes = serde_json::to_vec(&resp).expect("encode");
                bytes.push(b'\n');
                w.write_all(&bytes).await.expect("write");
            }
        });
    }

    // ── IpcPath ───────────────────────────────────────────────────────────────

    #[test]
    fn ipc_path_generates_unique_paths() {
        let a = IpcPath::generate();
        let b = IpcPath::generate();
        assert_ne!(a.path, b.path);
    }

    #[cfg(unix)]
    #[test]
    fn ipc_path_unix_format() {
        let p = IpcPath::generate();
        let s = p.path.to_string_lossy();
        assert!(s.starts_with("/tmp/pipe_name"));
        assert!(s.ends_with(".pipe"));
    }

    // ── Chapter ───────────────────────────────────────────────────────────────

    #[test]
    fn chapter_from_intro_complete() {
        let markers = IntroMarkers {
            start: Some(0),
            end: Some(90),
        };
        let chapters = Chapter::from_intro(&markers).expect("should build");
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters.first().map(|c| c.title.as_str()), Some("Opening"));
        assert_eq!(chapters.get(1).map(|c| c.time), Some(90.0));
    }

    #[test]
    fn chapter_from_intro_incomplete() {
        let markers = IntroMarkers {
            start: Some(0),
            end: None,
        };
        assert!(Chapter::from_intro(&markers).is_none());
    }

    // ── LoadOptions ───────────────────────────────────────────────────────────

    #[test]
    fn load_options_title_only() {
        let opts = LoadOptions {
            media_title: Some("Ep 1".into()),
            ..LoadOptions::default()
        };
        let s = opts.build();
        assert!(s.contains("force-media-title=\"Ep 1\""));
        assert!(s.contains("osd-playing-msg=\"Ep 1\""));
    }

    #[test]
    fn load_options_full() {
        let opts = LoadOptions {
            media_title: Some("Ep 2".into()),
            start_sec: Some(0.0),
            sub_file: Some("http://x/s.srt".into()),
            sub_inner_index: Some(3),
            chapters_file: Some("/tmp/chap.txt".into()),
            sub_file_prev: None,
        };
        let s = opts.build();
        assert!(s.contains("start=0"));
        assert!(s.contains("sub-file=http://x/s.srt"));
        assert!(s.contains("sid=3"));
        assert!(s.contains("chapters-file=\"/tmp/chap.txt\""));
    }

    #[test]
    fn load_options_sub_file_rotation() {
        let opts = LoadOptions {
            sub_file: Some("new.srt".into()),
            sub_file_prev: Some("main.srt".into()),
            ..LoadOptions::default()
        };
        let s = opts.build();
        assert!(s.contains("sub-files-remove=main.srt"));
        assert!(s.contains("sub-files-append=main.srt"));
        assert!(s.contains("sub-files-append=new.srt"));
    }

    #[test]
    fn load_options_empty() {
        let opts = LoadOptions::default();
        assert!(opts.build().is_empty());
    }

    // ── build_args ────────────────────────────────────────────────────────────

    fn default_launch() -> LaunchArgs {
        LaunchArgs {
            exe: "/usr/local/bin/mpv".into(),
            media_path: "http://x/v.mkv".into(),
            media_title: "Test Ep".into(),
            start_sec: Some(30.0),
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

    #[test]
    fn build_args_contains_required_flags() {
        let args = default_launch();
        let ipc = IpcPath::generate();
        let cmd = build_args(&args, &ipc);
        let flat = cmd.join(" ");
        assert!(flat.contains("http://x/v.mkv"));
        assert!(flat.contains("--force-media-title=Test Ep"));
        assert!(flat.contains("--start=30"));
        assert!(flat.contains("--input-ipc-server="));
        assert!(flat.contains("autoload-disabled=yes"));
    }

    #[test]
    fn build_args_fullscreen_and_no_audio() {
        let mut args = default_launch();
        args.fullscreen = true;
        args.disable_audio = true;
        let ipc = IpcPath::generate();
        let cmd = build_args(&args, &ipc);
        let flat = cmd.join(" ");
        assert!(flat.contains("--fullscreen=yes"));
        assert!(flat.contains("--no-audio"));
    }

    #[test]
    fn build_args_multiple_episodes_adds_pause() {
        let mut args = default_launch();
        args.is_multiple_episodes = true;
        let ipc = IpcPath::generate();
        let cmd = build_args(&args, &ipc);
        assert!(cmd.iter().any(|s| s == "--pause"));
    }

    #[test]
    fn build_args_proxy_with_multiple_episodes_disables_cache() {
        let mut args = default_launch();
        args.is_multiple_episodes = true;
        args.http_proxy = Some("127.0.0.1:8080".into());
        let ipc = IpcPath::generate();
        let cmd = build_args(&args, &ipc);
        let flat = cmd.join(" ");
        assert!(flat.contains("--http-proxy=http://127.0.0.1:8080"));
        assert!(flat.contains("--cache=no"));
    }

    // ── MpvHandle commands ────────────────────────────────────────────────────

    #[tokio::test]
    async fn time_pos_returns_float() {
        let (handle, server) = make_handle();
        serve_n(server, 1, |_| json!(42.5)).await;
        let pos = handle.time_pos().await.expect("ok");
        assert_eq!(pos, Some(42.5));
    }

    #[tokio::test]
    async fn time_pos_returns_none_when_unavailable() {
        let (handle, server) = make_handle();
        // Property unavailable → ClientError mapped to Ok(None) by transport
        tokio::spawn(async move {
            let (r, mut w) = tokio::io::split(server);
            let mut reader = BufReader::new(r);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("read");
            let req: Value = serde_json::from_str(line.trim_end_matches('\n'))
                .expect("parse");
            let id = req.get("request_id").and_then(Value::as_u64).expect("id");
            let resp = json!({"request_id": id, "error": "property unavailable", "data": null});
            let mut bytes = serde_json::to_vec(&resp).expect("encode");
            bytes.push(b'\n');
            w.write_all(&bytes).await.expect("write");
        });
        let pos = handle.time_pos().await.expect("ok");
        assert_eq!(pos, None);
    }

    #[tokio::test]
    async fn set_chapter_list_sends_correct_command() {
        let (handle, server) = make_handle();
        let received: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let rx = Arc::clone(&received);
        serve_n(server, 1, move |req| {
            *rx.lock().unwrap() = Some(req);
            json!(null)
        })
        .await;

        let chapters = vec![
            Chapter {
                title: "Opening".into(),
                time: 0.0,
            },
            Chapter {
                title: "Main".into(),
                time: 90.0,
            },
        ];
        handle.set_chapter_list(&chapters).await.expect("ok");

        let req = received.lock().unwrap().clone().expect("received");
        let cmd = req.get("command").and_then(Value::as_array).expect("cmd");
        assert_eq!(cmd.first().and_then(Value::as_str), Some("set_property"));
        assert_eq!(cmd.get(1).and_then(Value::as_str), Some("chapter-list"));
        let sent_list = cmd.get(2).and_then(Value::as_array).expect("list");
        assert_eq!(sent_list.len(), 2);
        assert_eq!(
            sent_list
                .first()
                .and_then(|c| c.get("title"))
                .and_then(Value::as_str),
            Some("Opening")
        );
    }

    #[tokio::test]
    async fn advance_chapter_sends_add_chapter_1() {
        let (handle, server) = make_handle();
        let received: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let rx = Arc::clone(&received);
        serve_n(server, 1, move |req| {
            *rx.lock().unwrap() = Some(req);
            json!(null)
        })
        .await;

        handle.advance_chapter().await.expect("ok");

        let req = received.lock().unwrap().clone().expect("received");
        let cmd = req.get("command").and_then(Value::as_array).expect("cmd");
        assert_eq!(cmd.first().and_then(Value::as_str), Some("add"));
        assert_eq!(cmd.get(1).and_then(Value::as_str), Some("chapter"));
        assert_eq!(cmd.get(2).and_then(Value::as_i64), Some(1));
    }

    #[tokio::test]
    async fn set_pause_sends_correct_value() {
        let (handle, server) = make_handle();
        let received: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let rx = Arc::clone(&received);
        serve_n(server, 1, move |req| {
            *rx.lock().unwrap() = Some(req);
            json!(null)
        })
        .await;

        handle.set_pause(true).await.expect("ok");

        let req = received.lock().unwrap().clone().expect("received");
        let cmd = req.get("command").and_then(Value::as_array).expect("cmd");
        assert_eq!(cmd.first().and_then(Value::as_str), Some("set"));
        assert_eq!(cmd.get(2).and_then(Value::as_str), Some("yes"));
    }

    #[tokio::test]
    async fn loadfile_new_format_includes_index() {
        let (handle, server) = make_handle();
        let received: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let rx = Arc::clone(&received);
        serve_n(server, 1, move |req| {
            *rx.lock().unwrap() = Some(req);
            json!(null)
        })
        .await;

        let opts = LoadOptions {
            media_title: Some("Ep 2".into()),
            start_sec: Some(0.0),
            ..LoadOptions::default()
        };
        handle
            .loadfile("http://x/ep2.mkv", LoadMode::Append, &opts, true)
            .await
            .expect("ok");

        let req = received.lock().unwrap().clone().expect("received");
        let cmd = req.get("command").and_then(Value::as_array).expect("cmd");
        assert_eq!(cmd.first().and_then(Value::as_str), Some("loadfile"));
        assert_eq!(
            cmd.get(1).and_then(Value::as_str),
            Some("http://x/ep2.mkv")
        );
        assert_eq!(cmd.get(2).and_then(Value::as_str), Some("append"));
        // index = -1
        assert_eq!(cmd.get(3).and_then(Value::as_i64), Some(-1));
        // options string present as 5th element
        let opts_str = cmd.get(4).and_then(Value::as_str).expect("opts");
        assert!(opts_str.contains("force-media-title=\"Ep 2\""));
    }

    #[tokio::test]
    async fn loadfile_old_format_omits_index() {
        let (handle, server) = make_handle();
        let received: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let rx = Arc::clone(&received);
        serve_n(server, 1, move |req| {
            *rx.lock().unwrap() = Some(req);
            json!(null)
        })
        .await;

        let opts = LoadOptions {
            media_title: Some("Ep 3".into()),
            ..LoadOptions::default()
        };
        handle
            .loadfile("http://x/ep3.mkv", LoadMode::Append, &opts, false)
            .await
            .expect("ok");

        let req = received.lock().unwrap().clone().expect("received");
        let cmd = req.get("command").and_then(Value::as_array).expect("cmd");
        // Old format: [loadfile, url, mode, options_string]  — 4 elements
        assert_eq!(cmd.len(), 4);
        assert_eq!(
            cmd.get(3)
                .and_then(Value::as_str)
                .map(|s| s.contains("force-media-title")),
            Some(true)
        );
    }

    #[tokio::test]
    async fn script_message_sends_all_args() {
        let (handle, server) = make_handle();
        let received: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let rx = Arc::clone(&received);
        serve_n(server, 1, move |req| {
            *rx.lock().unwrap() = Some(req);
            json!(null)
        })
        .await;

        handle
            .script_message(&["etlp-playlist-done"])
            .await
            .expect("ok");

        let req = received.lock().unwrap().clone().expect("received");
        let cmd = req.get("command").and_then(Value::as_array).expect("cmd");
        assert_eq!(cmd.first().and_then(Value::as_str), Some("script-message"));
        assert_eq!(
            cmd.get(1).and_then(Value::as_str),
            Some("etlp-playlist-done")
        );
    }

    #[tokio::test]
    async fn detect_new_loadfile_format_parses_command_list() {
        let (handle, server) = make_handle();
        // Simulate mpv returning a command-list that includes `loadfile` with an
        // `index` arg.
        serve_n(server, 1, |_| {
            json!([
                {"name": "loadfile", "args": [
                    {"name": "url"},
                    {"name": "flags"},
                    {"name": "index"},
                    {"name": "options"}
                ]},
                {"name": "quit", "args": []}
            ])
        })
        .await;
        assert!(handle.detect_new_loadfile_format().await);
    }

    #[tokio::test]
    async fn detect_new_loadfile_format_old_mpv() {
        let (handle, server) = make_handle();
        // Old mpv: loadfile has no `index` arg.
        serve_n(server, 1, |_| {
            json!([
                {"name": "loadfile", "args": [
                    {"name": "url"},
                    {"name": "flags"},
                    {"name": "options"}
                ]}
            ])
        })
        .await;
        assert!(!handle.detect_new_loadfile_format().await);
    }
}
