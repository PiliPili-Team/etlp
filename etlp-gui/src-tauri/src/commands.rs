//! Tauri command handlers exposed to the frontend.
//!
//! Every command mirrors a user action in the UI:
//!   - Server lifecycle: `start_server`, `stop_server`, `get_server_status`
//!   - Config helpers: `reload_config`, `open_config_folder`, `edit_config`
//!   - System: `set_autostart`

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

// ── Managed state ─────────────────────────────────────────────────────────────

/// Cross-command shared state managed by Tauri.
pub struct GuiState {
    /// Whether the axum server is currently running.
    pub running: AtomicBool,
    /// Reference to the axum `AppState` while the server is active.
    pub app_state: Mutex<Option<SharedState>>,
    /// Shutdown signal sender; consumed once on `stop_server`.
    pub shutdown_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    /// The port the server is (or was last) listening on.
    pub port: AtomicU16,
}

impl Default for GuiState {
    fn default() -> Self {
        Self {
            running: AtomicBool::new(false),
            app_state: Mutex::new(None),
            shutdown_tx: Mutex::new(None),
            port: AtomicU16::new(58000),
        }
    }
}

// ── Server lifecycle ───────────────────────────────────────────────────────────

/// Start the etlp axum server.
///
/// Returns the port it bound to. Calling this when the server is already
/// running is a no-op and returns the current port.
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

    let config = Config::load_from_dir(&cfg_dir)
        .map_err(|e| format!("load config: {e}"))?;

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

/// Stop the etlp axum server.
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

/// Query current server state.
///
/// Returns `{ running: bool, port: u16 }`.
#[tauri::command]
pub fn get_server_status(state: State<'_, GuiState>) -> serde_json::Value {
    serde_json::json!({
        "running": state.running.load(Ordering::Acquire),
        "port":    state.port.load(Ordering::Acquire),
    })
}

// ── Config helpers ─────────────────────────────────────────────────────────────

/// Reload the TOML config from disk and apply it to the running server.
///
/// Silently succeeds when the server is not running — the new config will be
/// picked up automatically on the next `start_server` call.
#[tauri::command]
pub async fn reload_config(state: State<'_, GuiState>) -> Result<(), String> {
    let working_dir = platform::config_dir()
        .ok_or_else(|| "cannot determine config directory".to_owned())?;
    let new_config = Config::load_from_dir(&working_dir)
        .map_err(|e| format!("load config: {e}"))?;

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
    // ensure the config dir exists before opening an editor
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

// ── System ─────────────────────────────────────────────────────────────────────

/// Enable or disable launch-at-login via the OS autostart mechanism.
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

/// Query whether launch-at-login is currently enabled.
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

fn write_default_config(path: &PathBuf) -> Result<(), String> {
    let template = include_str!("../default_config.toml");
    std::fs::write(path, template)
        .map_err(|e| format!("write default config: {e}"))
}
