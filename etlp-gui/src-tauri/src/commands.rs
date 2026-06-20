//! Tauri command handlers exposed to the frontend.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};

use tauri::State;
use tower::Layer;
use tower_http::normalize_path::NormalizePathLayer;
use tracing::{debug, error, info, warn};

use etlp_config::Config;
use etlp_download::{
    DEFAULT_MAX_CONCURRENT, DEFAULT_MAX_PER_DOMAIN, DownloadManager,
};
use etlp_net::HttpClientBuilder;
use etlp_server::{AppState, SharedState, build_router, platform};

use crate::config_patch::patch_field;

// ── Managed state ─────────────────────────────────────────────────────────────

pub struct GuiState {
    pub running: AtomicBool,
    pub app_state: Mutex<Option<SharedState>>,
    pub shutdown_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    pub port: AtomicU16,
    pub log_file: Mutex<PathBuf>,
    pub log_read_pos: Mutex<u64>,
    pub log_handle: Mutex<Option<etlp_logging::LogHandle>>,
}

impl Default for GuiState {
    fn default() -> Self {
        let data = platform::data_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            running: AtomicBool::new(false),
            app_state: Mutex::new(None),
            shutdown_tx: Mutex::new(None),
            port: AtomicU16::new(58000),
            log_file: Mutex::new(data.join("etlp.log")),
            log_read_pos: Mutex::new(0),
            log_handle: Mutex::new(None),
        }
    }
}

// ── Config DTO ─────────────────────────────────────────────────────────────────

/// Flat, serialisable representation of all user-visible config fields.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfigDto {
    // [emby]
    pub player: String,
    pub fullscreen: bool,
    pub disable_audio: bool,
    // [dev] – player
    pub player_path: String,
    // [dev] – version/subtitle
    pub version_prefer: Vec<String>,
    pub subtitle_priority: Vec<String>,
    pub sub_extract_priority: Vec<String>,
    // [dev] – behaviour
    pub pretty_title: bool,
    pub kill_process_at_start: bool,
    pub last_ep_disable_playlist: bool,
    pub version_prefer_for_playlist: bool,
    // [dev] – network
    pub http_proxy: String,
    pub redirect_check_host: Vec<String>,
    pub skip_certificate_verify: bool,
    // [dev] – misc
    pub log_level: String,
    pub user_agent: String,
    pub mix_log: bool,
    pub disable_progress_report: bool,
    // [playlist]
    pub item_limit: u32,
    pub version_filter: String,
    // [gui]
    pub speed_limit_mb: u64,
    // [trakt]
    pub trakt_client_id: String,
    pub trakt_client_secret: String,
    pub trakt_user_name: String,
    pub trakt_enable_host: String,
    // [bangumi]
    pub bangumi_access_token: String,
    pub bangumi_enable_host: String,
    pub bangumi_username: String,
    pub bangumi_private: bool,
    pub bangumi_genres: String,
    // runtime (not from config file)
    pub config_path: String,
}

impl From<&Config> for ConfigDto {
    fn from(c: &Config) -> Self {
        Self {
            player: c.emby.player.clone(),
            fullscreen: c.emby.fullscreen,
            disable_audio: c.emby.disable_audio,
            player_path: c.dev.player_path.clone().unwrap_or_default(),
            version_prefer: c.dev.version_prefer.clone(),
            subtitle_priority: c.dev.subtitle_priority.clone(),
            sub_extract_priority: c.dev.sub_extract_priority.clone(),
            pretty_title: c.dev.pretty_title,
            kill_process_at_start: c.dev.kill_process_at_start,
            last_ep_disable_playlist: c.dev.last_ep_disable_playlist,
            version_prefer_for_playlist: c.dev.version_prefer_for_playlist,
            http_proxy: c.dev.http_proxy.clone().unwrap_or_default(),
            redirect_check_host: c.dev.redirect_check_host.clone(),
            skip_certificate_verify: c.dev.skip_certificate_verify,
            log_level: c.dev.log_level.clone(),
            user_agent: c.dev.user_agent.clone().unwrap_or_default(),
            mix_log: c.dev.mix_log,
            disable_progress_report: c.dev.disable_progress_report,
            item_limit: c.playlist.item_limit,
            version_filter: c.playlist.version_filter.clone(),
            speed_limit_mb: c.gui.speed_limit_mb,
            trakt_client_id: c.trakt.client_id.clone(),
            trakt_client_secret: c.trakt.client_secret.clone(),
            trakt_user_name: c.trakt.user_name.clone(),
            trakt_enable_host: c.trakt.enable_host.clone(),
            bangumi_access_token: c.bangumi.access_token.clone(),
            bangumi_enable_host: c.bangumi.enable_host.clone(),
            bangumi_username: c.bangumi.username.clone(),
            bangumi_private: c.bangumi.private,
            bangumi_genres: c.bangumi.genres.clone(),
            config_path: c.path().to_string_lossy().into_owned(),
        }
    }
}

// ── Server lifecycle ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn start_server(state: State<'_, GuiState>) -> Result<u16, String> {
    if state.running.load(Ordering::Acquire) {
        return Ok(state.port.load(Ordering::Acquire));
    }

    let cfg_dir = platform::config_dir()
        .ok_or_else(|| "cannot determine config directory".to_owned())?;
    std::fs::create_dir_all(&cfg_dir)
        .map_err(|e| format!("create config dir: {e}"))?;

    let data_dir = platform::data_dir()
        .ok_or_else(|| "cannot determine data directory".to_owned())?;
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("create data dir: {e}"))?;

    let config = load_or_default_config(&cfg_dir)?;

    // Apply the configured log level now that we have a running config.
    {
        let guard = state
            .log_handle
            .lock()
            .map_err(|e| format!("lock log_handle: {e}"))?;
        if let Some(handle) = guard.as_ref() {
            handle.set_level(&config.dev.log_level);
        }
    }

    let proxy = config.dev.proxy.clone();
    let cert_verify = !config.dev.skip_certificate_verify;
    let http_client = HttpClientBuilder::new()
        .proxy(proxy)
        .cert_verify(cert_verify)
        .user_agent(config.dev.user_agent.clone())
        .build()
        .map_err(|e| format!("build http client: {e}"))?;

    let speed_limit: u64 = config.gui.speed_limit_mb * 1024 * 1024;
    let cache_path = config
        .gui
        .server_cache_path
        .clone()
        .unwrap_or_else(|| data_dir.join("cache"));

    let dl_client = reqwest::Client::builder()
        .build()
        .map_err(|e| format!("build dl client: {e}"))?;
    let dl_manager = DownloadManager::new(
        cache_path,
        speed_limit,
        DEFAULT_MAX_CONCURRENT,
        DEFAULT_MAX_PER_DOMAIN,
        dl_client,
    );
    dl_manager.start_update_db_loop(30);

    let app_state =
        Arc::new(AppState::new(config, dl_manager, http_client, data_dir));

    let port = 58000u16;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("bind {addr}: {e}"))?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let router = build_router(Arc::clone(&app_state));

    {
        let mut st = state
            .app_state
            .lock()
            .map_err(|e| format!("lock app_state: {e}"))?;
        *st = Some(app_state);

        let mut tx = state
            .shutdown_tx
            .lock()
            .map_err(|e| format!("lock shutdown_tx: {e}"))?;
        *tx = Some(shutdown_tx);
    }

    state.port.store(port, Ordering::Release);
    state.running.store(true, Ordering::Release);

    // NormalizePathLayer strips trailing slashes before routing, so
    // /embyToLocalPlayer/ and /embyToLocalPlayer both resolve correctly.
    let app = NormalizePathLayer::trim_trailing_slash().layer(router);

    tauri::async_runtime::spawn(async move {
        let serve = axum::serve(listener, tower::make::Shared::new(app))
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
        if let Err(e) = serve.await {
            warn!("axum server exited: {e}");
        }
    });

    Ok(port)
}

#[tauri::command]
pub async fn stop_server(state: State<'_, GuiState>) -> Result<(), String> {
    if !state.running.load(Ordering::Acquire) {
        return Ok(());
    }

    let tx = {
        let mut guard = state
            .shutdown_tx
            .lock()
            .map_err(|e| format!("lock shutdown_tx: {e}"))?;
        guard.take()
    };
    if let Some(tx) = tx {
        let _ = tx.send(());
    }

    {
        let mut guard = state
            .app_state
            .lock()
            .map_err(|e| format!("lock app_state: {e}"))?;
        *guard = None;
    }

    state.running.store(false, Ordering::Release);
    Ok(())
}

#[tauri::command]
pub fn get_server_status(state: State<'_, GuiState>) -> serde_json::Value {
    serde_json::json!({
        "running": state.running.load(Ordering::Acquire),
        "port":    state.port.load(Ordering::Acquire),
    })
}

// ── Config ─────────────────────────────────────────────────────────────────────

/// Return all user-visible config fields, loading from disk.
///
/// If no config file exists, writes a default one first so the UI always has
/// something to show and subsequent `update_config_field` calls have a file to
/// patch into.
#[tauri::command]
pub async fn get_config() -> Result<ConfigDto, String> {
    let cfg_dir = platform::config_dir()
        .ok_or_else(|| "cannot determine config directory".to_owned())?;
    std::fs::create_dir_all(&cfg_dir)
        .map_err(|e| format!("create config dir: {e}"))?;

    let config = load_or_default_config(&cfg_dir)?;
    Ok(ConfigDto::from(&config))
}

/// Patch exactly one field in the config file without rewriting the rest.
///
/// `section` is the TOML table name (e.g. `"emby"`, `"dev"`, `"playlist"`).
/// `key` is the key within that table.
/// `value` is a JSON value that is converted to the appropriate TOML type.
#[tauri::command]
pub async fn update_config_field(
    section: String,
    key: String,
    value: serde_json::Value,
) -> Result<(), String> {
    let cfg_dir = platform::config_dir()
        .ok_or_else(|| "cannot determine config directory".to_owned())?;
    // The app reads and writes the same single `config.toml`, so the path we
    // patch always matches the one that was loaded.
    let path = etlp_config::existing_config_path(&cfg_dir)
        .unwrap_or_else(|| cfg_dir.join("config.toml"));

    // If the file does not exist yet, write the default template so toml_edit
    // has a valid document to patch into.
    if !path.exists() {
        write_default_config(&path)?;
    }

    match patch_field(&path, &section, &key, &value) {
        Ok(()) => {
            info!(
                path = %path.display(),
                section = %section,
                key = %key,
                "config field saved"
            );
            Ok(())
        }
        Err(e) => {
            error!(
                path = %path.display(),
                section = %section,
                key = %key,
                "config field save failed: {e}"
            );
            Err(e)
        }
    }
}

/// Reload the in-memory config from disk and push to a running server.
///
/// Also applies any log-level change immediately so the new level takes
/// effect without requiring a full server restart.
#[tauri::command]
pub async fn reload_config(state: State<'_, GuiState>) -> Result<(), String> {
    let working_dir = platform::config_dir()
        .ok_or_else(|| "cannot determine config directory".to_owned())?;
    let new_config = load_or_default_config(&working_dir)?;

    // Apply log level before writing the new config so the level change is
    // visible in the logs that follow.
    {
        let guard = state
            .log_handle
            .lock()
            .map_err(|e| format!("lock log_handle: {e}"))?;
        if let Some(handle) = guard.as_ref() {
            handle.set_level(&new_config.dev.log_level);
        }
    }

    let guard = state
        .app_state
        .lock()
        .map_err(|e| format!("lock app_state: {e}"))?;

    if let Some(app_state) = guard.as_ref() {
        let mut cfg = app_state
            .config
            .write()
            .map_err(|e| format!("lock config: {e}"))?;
        *cfg = new_config;
    }

    Ok(())
}

/// Stop, wait briefly, then restart the server to pick up config changes.
#[tauri::command]
pub async fn restart_server(state: State<'_, GuiState>) -> Result<u16, String> {
    if state.running.load(Ordering::Acquire) {
        stop_server(state.clone()).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    }
    start_server(state).await
}

/// Open the configuration directory in the system file manager.
#[tauri::command]
pub async fn open_config_folder(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let dir = platform::config_dir()
        .ok_or_else(|| "cannot determine config directory".to_owned())?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("create config dir: {e}"))?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| format!("open folder: {e}"))
}

/// Launch the editor the user has associated with `.toml` files on Windows.
///
/// Uses the shell "open" verb, exactly as double-clicking the file in Explorer
/// would, so a user-configured association is honoured. Returns `false` when no
/// application is associated with the extension (`SE_ERR_NOASSOC`), letting the
/// caller fall back to a built-in editor.
#[cfg(target_os = "windows")]
fn open_with_association(path: &std::path::Path) -> bool {
    use std::os::windows::ffi::OsStrExt as _;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{PCWSTR, w};

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    // SAFETY: `wide` is a valid null-terminated UTF-16 buffer that lives for
    // the duration of the call; the verb literal and null params are constant.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW returns a value greater than 32 on success; values <= 32
    // are error codes such as SE_ERR_NOASSOC (no associated application).
    result.0.addr() > 32
}

/// Open the config file in the user's editor of choice.
///
/// On Windows we first honour the file association via the shell "open" verb
/// and only fall back to `notepad.exe` when `.toml` has no associated app, so a
/// user who set their own editor gets it instead of always landing in Notepad.
#[tauri::command]
pub async fn edit_config(app: tauri::AppHandle) -> Result<(), String> {
    let path = config_file_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create config dir: {e}"))?;
    }
    if !path.exists() {
        write_default_config(&path)?;
    }
    #[cfg(target_os = "windows")]
    {
        // `app` drives the opener on other platforms; the shell verb and the
        // Notepad fallback need no handle.
        let _ = &app;
        if open_with_association(&path) {
            return Ok(());
        }
        std::process::Command::new("notepad.exe")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("open notepad: {e}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        use tauri_plugin_opener::OpenerExt as _;
        app.opener()
            .open_path(path.to_string_lossy(), None::<&str>)
            .map_err(|e| format!("open config file: {e}"))
    }
}

// ── Third-party authorization ───────────────────────────────────────────────────

/// Open the Trakt OAuth authorization page in the default browser.
///
/// Reads `client_id` and `redirect_uri` from the config and builds the URL with
/// [`etlp_sync::trakt_authorize_url`]. The local `/trakt_auth` callback (served
/// by the running etlp server) then exchanges the returned code for a token, so
/// the user should start the service before authorizing.
#[tauri::command]
pub async fn authorize_trakt(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;

    let cfg_dir = platform::config_dir()
        .ok_or_else(|| "cannot determine config directory".to_owned())?;
    let config = load_or_default_config(&cfg_dir)?;

    if config.trakt.client_id.is_empty() {
        warn!("authorize_trakt: client_id is empty");
        return Err("Trakt client_id is not configured".to_owned());
    }

    let url = etlp_sync::trakt_authorize_url(
        &config.trakt.client_id,
        &config.trakt.redirect_uri,
    );
    info!(redirect_uri = %config.trakt.redirect_uri, "authorize_trakt: opening authorization page");
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("open trakt authorize page: {e}"))
}

/// Open the bgm.tv access-token generation page in the default browser.
///
/// bgm.tv uses long-lived personal tokens rather than an OAuth redirect flow,
/// so "authorizing" simply means minting a new token and pasting it into the
/// config.
#[tauri::command]
pub async fn authorize_bangumi(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;

    info!("authorize_bangumi: opening token page");
    app.opener()
        .open_url(etlp_sync::BangumiApi::TOKEN_PAGE_URL, None::<&str>)
        .map_err(|e| format!("open bangumi token page: {e}"))
}

// ── Logs ───────────────────────────────────────────────────────────────────────

/// Return new log lines since the last call (incremental tail).
///
/// `since_bytes` is the byte offset to read from; pass `0` for the beginning.
/// `path`, when provided, reads that file instead of the default app log —
/// used to view the mpv log (default or a user-picked file).
/// Returns `{ lines: [...], next_bytes: u64 }`.
#[tauri::command]
pub async fn get_log_lines(
    state: State<'_, GuiState>,
    since_bytes: u64,
    path: Option<String>,
) -> Result<serde_json::Value, String> {
    let log_path = match path {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => state
            .log_file
            .lock()
            .map_err(|e| format!("lock log_file: {e}"))?
            .clone(),
    };

    if !log_path.exists() {
        return Ok(serde_json::json!({ "lines": [], "next_bytes": 0u64 }));
    }

    let content =
        std::fs::read(&log_path).map_err(|e| format!("read log file: {e}"))?;

    let start = since_bytes as usize;
    let slice = if start < content.len() {
        &content[start..]
    } else {
        &[]
    };

    let text = String::from_utf8_lossy(slice);
    let lines: Vec<&str> = text.lines().collect();
    let next_bytes = content.len() as u64;

    Ok(serde_json::json!({
        "lines": lines,
        "next_bytes": next_bytes,
    }))
}

/// Clear the log position counter so the next `get_log_lines(0)` re-reads all.
#[tauri::command]
pub async fn clear_log_position(
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let mut pos = state
        .log_read_pos
        .lock()
        .map_err(|e| format!("lock log_read_pos: {e}"))?;
    *pos = 0;
    Ok(())
}

/// Open the directory containing the application log file.
#[tauri::command]
pub async fn open_log_folder(
    app: tauri::AppHandle,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let log_path = state
        .log_file
        .lock()
        .map_err(|e| format!("lock log_file: {e}"))?
        .clone();
    let dir = log_path
        .parent()
        .ok_or_else(|| "log file has no parent directory".to_owned())?;
    std::fs::create_dir_all(dir).map_err(|e| format!("create log dir: {e}"))?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| format!("open folder: {e}"))
}

// ── File picker ────────────────────────────────────────────────────────────────

/// Open a native file-picker and return the selected path as a string.
///
/// Returns `None` when the user cancels.
#[tauri::command]
pub async fn pick_player_path(
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt as _;

    let path = app
        .dialog()
        .file()
        .set_title("选择播放器可执行文件")
        .blocking_pick_file();

    Ok(path.map(|p| p.to_string()))
}

/// Return the log file paths the GUI knows about.
///
/// Returns `{ app_log: String|null, mpv_log: String|null }`.
#[tauri::command]
pub async fn get_log_paths(
    state: State<'_, GuiState>,
) -> Result<serde_json::Value, String> {
    let app_log = state
        .log_file
        .lock()
        .map_err(|e| format!("lock log_file: {e}"))?
        .to_string_lossy()
        .into_owned();

    // mpv log path comes from the config dev.mpv_log_file if set.
    let cfg_dir = platform::config_dir();
    let mpv_log = cfg_dir
        .as_ref()
        .and_then(|d| Config::load_from_dir(d).ok())
        .and_then(|c| {
            c.dev
                .mpv_input_ipc_server
                .as_ref()
                .and_then(|_| platform::data_dir().map(|d| d.join("mpv.log")))
        });

    Ok(serde_json::json!({
        "app_log": app_log,
        "mpv_log": mpv_log.map(|p| p.to_string_lossy().into_owned()),
    }))
}

/// Check whether a file path exists on disk.
#[tauri::command]
pub fn path_exists(path: String) -> bool {
    std::path::Path::new(&path).exists()
}

// ── App info ───────────────────────────────────────────────────────────────────

/// Return the application version string from the Cargo manifest.
#[tauri::command]
pub fn get_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// List the available font families on the current system.
///
/// Returns a deduplicated, sorted list of font names derived from the system
/// font directories.  Common cross-platform fonts are prepended so they appear
/// at the top of any picker.
#[tauri::command]
pub fn list_system_fonts() -> Vec<String> {
    let mut fonts: Vec<String> = Vec::new();

    // System-specific font directories
    #[cfg(target_os = "macos")]
    let dirs: &[&str] = &[
        "/System/Library/Fonts",
        "/System/Library/Fonts/Supplemental",
        "/Library/Fonts",
    ];
    #[cfg(target_os = "windows")]
    let dirs: &[&str] = &[r"C:\Windows\Fonts"];
    #[cfg(target_os = "linux")]
    let dirs: &[&str] = &["/usr/share/fonts", "/usr/local/share/fonts"];
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "linux"
    )))]
    let dirs: &[&str] = &[];

    for dir in dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let s = name.to_string_lossy();
                for ext in &[".ttf", ".otf", ".ttc", ".dfont"] {
                    if s.to_lowercase().ends_with(ext) {
                        let stem = s[..s.len() - ext.len()].to_string();
                        fonts.push(stem);
                        break;
                    }
                }
            }
        }
    }

    // Always include safe cross-platform presets at the top
    let presets = [
        "system-ui",
        "-apple-system",
        "SF Pro Text",
        "Helvetica Neue",
        "Arial",
        "Segoe UI",
        "Roboto",
        "Noto Sans CJK SC",
        "PingFang SC",
        "Microsoft YaHei",
        "Source Han Sans SC",
    ];
    for p in presets.iter().rev() {
        let s = p.to_string();
        if !fonts.contains(&s) {
            fonts.insert(0, s);
        }
    }

    fonts.sort();
    fonts.dedup();
    fonts
}

// ── System ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn set_autostart(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    if enabled {
        app.autolaunch()
            .enable()
            .map_err(|e| format!("enable autostart: {e}"))
    } else {
        app.autolaunch()
            .disable()
            .map_err(|e| format!("disable autostart: {e}"))
    }
}

#[tauri::command]
pub async fn get_autostart(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch()
        .is_enabled()
        .map_err(|e| format!("query autostart: {e}"))
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn config_file_path() -> Result<PathBuf, String> {
    let dir = platform::config_dir()
        .ok_or_else(|| "cannot determine config directory".to_owned())?;
    // Resolve to the config the app loaded so "edit config" opens the file the
    // user is actually using, not an empty shadow `config.toml`.
    Ok(etlp_config::existing_config_path(&dir)
        .unwrap_or_else(|| dir.join("config.toml")))
}

/// Load config, writing the default template only when no file exists at all.
///
/// IO errors and parse errors preserve the user's file on disk — we return
/// an in-memory default instead of overwriting potentially recoverable data.
pub(crate) fn load_or_default_config(
    cfg_dir: &std::path::Path,
) -> Result<Config, String> {
    use etlp_config::ConfigError;
    let result = match Config::load_from_dir(cfg_dir) {
        Ok(c) => {
            info!(
                path = %c.path().display(),
                "config loaded"
            );
            Ok(c)
        }
        Err(ConfigError::NotFound(_)) => {
            let path = cfg_dir.join("config.toml");
            write_default_config(&path)?;
            match Config::load_file(&path) {
                Ok(c) => {
                    info!(
                        path = %c.path().display(),
                        "default config written and loaded"
                    );
                    Ok(c)
                }
                Err(e) => {
                    error!("load default config {}: {e}", path.display());
                    Err(format!("load default config: {e}"))
                }
            }
        }
        Err(ConfigError::Io { path, source }) => {
            error!(
                path = %path.display(),
                "config IO error: {source} — running with defaults"
            );
            Ok(Config::with_defaults(path))
        }
        Err(ConfigError::Parse { path, source }) => {
            error!(
                path = %path.display(),
                "config parse error: {source} — running with defaults"
            );
            Ok(Config::with_defaults(path))
        }
    }?;

    // Emit current config as JSON at debug level so problems can be diagnosed
    // without grepping TOML files.
    let dto = ConfigDto::from(&result);
    match serde_json::to_string(&dto) {
        Ok(json) => debug!(config_json = %json, "current config"),
        Err(e) => warn!("config JSON serialise failed: {e}"),
    }

    Ok(result)
}

fn write_default_config(path: &std::path::Path) -> Result<(), String> {
    let template = default_config_template();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create config dir: {e}"))?;
    }
    match std::fs::write(path, &template) {
        Ok(()) => {
            info!(path = %path.display(), "default config written");
            Ok(())
        }
        Err(e) => {
            error!(path = %path.display(), "write default config failed: {e}");
            Err(format!("write default config: {e}"))
        }
    }
}

/// Resolve the template used to seed a brand-new config.
///
/// Prefers a user-provided `~/Downloads/config.toml` so users can drop in a
/// pre-filled file and have it adopted verbatim on first run; falls back to
/// the template embedded at build time when none is present (or unreadable).
fn default_config_template() -> String {
    if let Some(home) = dirs::home_dir() {
        let user_template = home.join("Downloads").join("config.toml");
        if user_template.is_file() {
            match std::fs::read_to_string(&user_template) {
                Ok(contents) => {
                    info!(
                        path = %user_template.display(),
                        "seeding config from user template in Downloads"
                    );
                    return contents;
                }
                Err(e) => warn!(
                    path = %user_template.display(),
                    "read user template failed: {e} — using embedded default"
                ),
            }
        }
    }
    include_str!("../default_config.toml").to_owned()
}
