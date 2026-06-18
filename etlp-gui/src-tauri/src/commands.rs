//! Tauri command handlers exposed to the frontend.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};

use tauri::State;
use tracing::warn;

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
    // [playlist]
    pub item_limit: u32,
    pub version_filter: String,
    // [gui]
    pub speed_limit_mb: u64,
    // [trakt]
    pub trakt_client_id: String,
    pub trakt_client_secret: String,
    pub trakt_enable_host: String,
    // [bangumi]
    pub bangumi_access_token: String,
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
            item_limit: c.playlist.item_limit,
            version_filter: c.playlist.version_filter.clone(),
            speed_limit_mb: c.gui.speed_limit_mb,
            trakt_client_id: c.trakt.client_id.clone(),
            trakt_client_secret: c.trakt.client_secret.clone(),
            trakt_enable_host: c.trakt.enable_host.clone(),
            bangumi_access_token: c
                .bangumi
                .access_token
                .clone()
                .unwrap_or_default(),
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

    tauri::async_runtime::spawn(async move {
        let serve =
            axum::serve(listener, router).with_graceful_shutdown(async move {
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
    let path = cfg_dir.join("config.toml");

    // If the file does not exist yet, write the default template so toml_edit
    // has a valid document to patch into.
    if !path.exists() {
        write_default_config(&path)?;
    }

    patch_field(&path, &section, &key, &value)
}

/// Reload the in-memory config from disk and push to a running server.
#[tauri::command]
pub async fn reload_config(state: State<'_, GuiState>) -> Result<(), String> {
    let working_dir = platform::config_dir()
        .ok_or_else(|| "cannot determine config directory".to_owned())?;
    let new_config = load_or_default_config(&working_dir)?;

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

/// Open the config file in the default system text editor.
#[tauri::command]
pub async fn edit_config(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let path = config_file_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create config dir: {e}"))?;
    }
    if !path.exists() {
        write_default_config(&path)?;
    }
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| format!("open config file: {e}"))
}

// ── Logs ───────────────────────────────────────────────────────────────────────

/// Return new log lines since the last call (incremental tail).
///
/// `since_bytes` is the byte offset to read from; pass `0` for the beginning.
/// Returns `{ lines: [...], next_bytes: u64 }`.
#[tauri::command]
pub async fn get_log_lines(
    state: State<'_, GuiState>,
    since_bytes: u64,
) -> Result<serde_json::Value, String> {
    let log_path = state
        .log_file
        .lock()
        .map_err(|e| format!("lock log_file: {e}"))?
        .clone();

    if !log_path.exists() {
        return Ok(serde_json::json!({ "lines": [], "next_bytes": 0u64 }));
    }

    let content = std::fs::read(&log_path)
        .map_err(|e| format!("read log file: {e}"))?;

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
pub async fn clear_log_position(state: State<'_, GuiState>) -> Result<(), String> {
    let mut pos = state
        .log_read_pos
        .lock()
        .map_err(|e| format!("lock log_read_pos: {e}"))?;
    *pos = 0;
    Ok(())
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
    Ok(dir.join("config.toml"))
}

/// Load config, or write a default and load that.
pub(crate) fn load_or_default_config(cfg_dir: &std::path::Path) -> Result<Config, String> {
    match Config::load_from_dir(cfg_dir) {
        Ok(c) => Ok(c),
        Err(_) => {
            let path = cfg_dir.join("config.toml");
            write_default_config(&path)?;
            Config::load_file(&path).map_err(|e| format!("load default config: {e}"))
        }
    }
}

fn write_default_config(path: &std::path::Path) -> Result<(), String> {
    let template = include_str!("../default_config.toml");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create config dir: {e}"))?;
    }
    std::fs::write(path, template)
        .map_err(|e| format!("write default config: {e}"))
}
