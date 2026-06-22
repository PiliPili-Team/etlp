//! Tauri command handlers exposed to the frontend.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};

use tauri::State;
use tower::Layer;
use tower_http::normalize_path::NormalizePathLayer;
use tracing::{error, info, warn};

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
    /// Monotonic instant the service last started, or `None` while stopped.
    /// Lives in the backend so uptime survives window reloads but resets on a
    /// process restart (the in-process server dies with the app), keeping the
    /// reported uptime tied to the service's real lifecycle rather than a
    /// client-persisted timestamp.
    pub started_at: Mutex<Option<std::time::Instant>>,
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
            started_at: Mutex::new(None),
            log_file: Mutex::new(platform::log_dir_in(&data).join("etlp.log")),
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
    pub silent_start: bool,
    pub check_update: bool,
    // [trakt]
    pub trakt_client_id: String,
    pub trakt_client_secret: String,
    pub trakt_user_name: String,
    pub trakt_enable_host: String,
    pub trakt_allow_duplicate: bool,
    // [bangumi]
    pub bangumi_access_token: String,
    pub bangumi_enable_host: String,
    pub bangumi_username: String,
    pub bangumi_private: bool,
    pub bangumi_genres: String,
    pub bangumi_subject_map: Vec<String>,
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
            silent_start: c.gui.silent_start,
            check_update: c.gui.check_update,
            trakt_client_id: c.trakt.client_id.clone(),
            trakt_client_secret: c.trakt.client_secret.clone(),
            trakt_user_name: c.trakt.user_name.clone(),
            trakt_enable_host: c.trakt.enable_host.clone(),
            trakt_allow_duplicate: c.trakt.allow_duplicate,
            bangumi_access_token: c.bangumi.access_token.clone(),
            bangumi_enable_host: c.bangumi.enable_host.clone(),
            bangumi_username: c.bangumi.username.clone(),
            bangumi_private: c.bangumi.private,
            bangumi_genres: c.bangumi.genres.clone(),
            bangumi_subject_map: c.bangumi.subject_map.clone(),
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
    // Relocate any legacy flat-layout files and ensure the log/ dir exists.
    platform::migrate_layout(&data_dir);
    std::fs::create_dir_all(platform::log_dir_in(&data_dir)).ok();

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
    if let Ok(mut started) = state.started_at.lock() {
        *started = Some(std::time::Instant::now());
    }
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
    if let Ok(mut started) = state.started_at.lock() {
        *started = None;
    }
    Ok(())
}

#[tauri::command]
pub fn get_server_status(state: State<'_, GuiState>) -> serde_json::Value {
    let running = state.running.load(Ordering::Acquire);
    // Authoritative uptime: elapsed since the service started, or null while
    // stopped. Computed from a monotonic clock so it is correct across window
    // reloads and immune to wall-clock adjustments.
    let uptime_secs = running
        .then(|| {
            state
                .started_at
                .lock()
                .ok()
                .and_then(|g| g.map(|t| t.elapsed().as_secs()))
        })
        .flatten();
    serde_json::json!({
        "running":     running,
        "port":        state.port.load(Ordering::Acquire),
        "uptime_secs": uptime_secs,
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

/// Validate one Bangumi subject-mapping line and return its canonical form.
///
/// On success returns the normalised single-line DSL (ready to append to the
/// list). On failure returns a stable i18n key (e.g. `map_err_provider`) the
/// frontend localises, including `map_err_duplicate` when the entry's key
/// collides with one already in `existing`.
#[tauri::command]
pub fn validate_bangumi_mapping(
    line: String,
    existing: Vec<String>,
) -> Result<String, String> {
    let parsed =
        etlp_sync::parse_mapping(&line).map_err(|e| e.code().to_owned())?;

    // Reject a duplicate key (same provider + id + kind + season).
    let collides = etlp_sync::parse_mappings(&existing).iter().any(|m| {
        m.provider == parsed.provider
            && m.provider_id == parsed.provider_id
            && m.is_movie == parsed.is_movie
            && m.season == parsed.season
    });
    if collides {
        return Err("map_err_duplicate".to_owned());
    }
    Ok(parsed.to_canonical())
}

/// Validate a `version_filter` regular expression against the same engine the
/// server uses (the Rust `regex` crate, which—unlike JS—has no lookaround or
/// backreferences). Newlines are stripped first to mirror `version_filter`'s
/// own preprocessing. Returns `Ok(())` when valid, or the engine's error detail.
#[tauri::command]
pub fn validate_regex(pattern: String) -> Result<(), String> {
    let single_line: String = pattern.split('\n').collect();
    regex::Regex::new(&single_line)
        .map(|_| ())
        .map_err(|e| e.to_string())
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

/// Sentinel returned when the provider has no credentials configured yet. The
/// frontend maps it to a localized "not configured" message.
pub const ERR_NOT_CONFIGURED: &str = "NOT_CONFIGURED";

/// Refresh-result sentinel: existing credentials are already valid.
pub const AUTH_VALID: &str = "AUTH_VALID";

/// Refresh-result sentinel: an authorization page was opened for the user to
/// complete the flow in a browser.
pub const AUTH_OPENED: &str = "AUTH_OPENED";

/// Build a [`TraktApi`] from the saved config, pointing at the data-dir token.
///
/// Returns `Ok(None)` when Trakt is not configured (empty `client_id`).
fn build_trakt_api() -> Result<Option<etlp_sync::TraktApi>, String> {
    let cfg_dir = platform::config_dir()
        .ok_or_else(|| "cannot determine config directory".to_owned())?;
    let config = load_or_default_config(&cfg_dir)?;
    if config.trakt.client_id.is_empty() {
        return Ok(None);
    }
    let token_dir = platform::data_dir()
        .ok_or_else(|| "cannot determine data directory".to_owned())?;
    let token_path = token_dir.join(etlp_sync::TraktApi::TOKEN_FILE_NAME);
    let api = etlp_sync::TraktApi::new(
        &config.trakt.client_id,
        &config.trakt.client_secret,
        &config.trakt.user_name,
        &token_path,
        etlp_sync::TraktApi::DEFAULT_BASE_URL,
    )
    .map_err(|e| format!("init trakt client: {e}"))?;
    Ok(Some(api))
}

/// Build a [`BangumiApi`] from the saved config.
///
/// Returns `Ok(None)` when Bangumi is not configured (empty `access_token`).
fn build_bangumi_api() -> Result<Option<etlp_sync::BangumiApi>, String> {
    let cfg_dir = platform::config_dir()
        .ok_or_else(|| "cannot determine config directory".to_owned())?;
    let config = load_or_default_config(&cfg_dir)?;
    if config.bangumi.access_token.is_empty() {
        return Ok(None);
    }
    let api = etlp_sync::BangumiApi::new(
        &config.bangumi.username,
        &config.bangumi.access_token,
        config.bangumi.private,
        etlp_sync::BangumiApi::DEFAULT_BASE_URL,
    )
    .map_err(|e| format!("init bangumi client: {e}"))?;
    Ok(Some(api))
}

/// Refresh the Trakt authorization.
///
/// Loads the saved token and tries to refresh it; if no valid token can be
/// obtained, opens the OAuth authorization page (the running `/trakt_auth`
/// callback then completes the exchange). Returns [`AUTH_VALID`] or
/// [`AUTH_OPENED`].
#[tauri::command]
pub async fn refresh_trakt_auth(
    app: tauri::AppHandle,
) -> Result<String, String> {
    use tauri_plugin_opener::OpenerExt as _;

    let Some(mut api) = build_trakt_api()? else {
        return Err(ERR_NOT_CONFIGURED.to_owned());
    };

    match api.ensure_auth().await {
        Ok(true) => {
            info!("refresh_trakt_auth: authorization valid");
            Ok(AUTH_VALID.to_owned())
        }
        Ok(false) => {
            let cfg_dir = platform::config_dir().ok_or_else(|| {
                "cannot determine config directory".to_owned()
            })?;
            let config = load_or_default_config(&cfg_dir)?;
            let url = etlp_sync::trakt_authorize_url(
                &config.trakt.client_id,
                &config.trakt.redirect_uri,
            );
            info!("refresh_trakt_auth: no valid token, opening authorize page");
            app.opener()
                .open_url(url, None::<&str>)
                .map_err(|e| format!("open trakt authorize page: {e}"))?;
            Ok(AUTH_OPENED.to_owned())
        }
        Err(e) => Err(format!("trakt auth failed: {e}")),
    }
}

/// Refresh the Bangumi authorization.
///
/// bgm.tv uses a long-lived personal token, so "refresh" means re-validating
/// it; when invalid, the token page is opened. Returns [`AUTH_VALID`] or
/// [`AUTH_OPENED`].
#[tauri::command]
pub async fn refresh_bangumi_auth(
    app: tauri::AppHandle,
) -> Result<String, String> {
    use tauri_plugin_opener::OpenerExt as _;

    let Some(api) = build_bangumi_api()? else {
        return Err(ERR_NOT_CONFIGURED.to_owned());
    };

    match api.verify_token().await {
        Ok(()) => {
            info!("refresh_bangumi_auth: authorization valid");
            Ok(AUTH_VALID.to_owned())
        }
        Err(etlp_sync::SyncError::Unauthorized) => {
            info!("refresh_bangumi_auth: token invalid, opening token page");
            app.opener()
                .open_url(etlp_sync::BangumiApi::TOKEN_PAGE_URL, None::<&str>)
                .map_err(|e| format!("open bangumi token page: {e}"))?;
            Ok(AUTH_OPENED.to_owned())
        }
        Err(e) => Err(format!("bangumi auth check failed: {e}")),
    }
}

/// Test whether the Trakt authorization currently works.
///
/// Returns `true` when a valid (or refreshable) token is present, `false` when
/// interactive re-authorization is required.
#[tauri::command]
pub async fn test_trakt_auth() -> Result<bool, String> {
    let Some(mut api) = build_trakt_api()? else {
        return Err(ERR_NOT_CONFIGURED.to_owned());
    };
    let ok = api.ensure_auth().await.map_err(|e| format!("{e}"))?;
    info!(ok, "test_trakt_auth");
    Ok(ok)
}

/// Test whether the Bangumi access token currently works.
#[tauri::command]
pub async fn test_bangumi_auth() -> Result<bool, String> {
    let Some(api) = build_bangumi_api()? else {
        return Err(ERR_NOT_CONFIGURED.to_owned());
    };
    match api.verify_token().await {
        Ok(()) => {
            info!("test_bangumi_auth: ok");
            Ok(true)
        }
        Err(etlp_sync::SyncError::Unauthorized) => {
            info!("test_bangumi_auth: unauthorized");
            Ok(false)
        }
        Err(e) => Err(format!("bangumi auth check failed: {e}")),
    }
}

// ── Logs ───────────────────────────────────────────────────────────────────────

/// Resolve the log path to read: an explicit `path`, else the default app log.
fn resolve_log_path(
    state: &GuiState,
    path: Option<String>,
) -> Result<PathBuf, String> {
    match path {
        Some(p) if !p.is_empty() => Ok(PathBuf::from(p)),
        _ => Ok(state
            .log_file
            .lock()
            .map_err(|e| format!("lock log_file: {e}"))?
            .clone()),
    }
}

/// Read up to `max_lines` whole lines ending at byte offset `end`, scanning the
/// file backwards in fixed chunks so a multi-hundred-MB log is never fully
/// loaded. Returns `(start_offset, lines)` where bytes `[start_offset, end)`
/// exactly cover the returned lines (each line excludes its trailing `\n`).
/// `start_offset == 0` means the file head was reached (no older lines remain).
fn read_lines_before(
    path: &std::path::Path,
    end: u64,
    max_lines: usize,
) -> std::io::Result<(u64, Vec<String>)> {
    use std::io::{Read as _, Seek as _, SeekFrom};

    const CHUNK: u64 = 64 * 1024;
    let mut file = std::fs::File::open(path)?;
    let mut start = end;
    let mut buf: Vec<u8> = Vec::new();

    // Grow `buf` backwards until it holds more than `max_lines` newlines (so the
    // last `max_lines` lines are fully contained) or the file head is reached.
    while start > 0 {
        let read = CHUNK.min(start);
        start -= read;
        file.seek(SeekFrom::Start(start))?;
        let mut chunk = vec![0u8; read as usize];
        file.read_exact(&mut chunk)?;
        chunk.extend_from_slice(&buf);
        buf = chunk;
        if buf.iter().filter(|&&b| b == b'\n').count() > max_lines {
            break;
        }
    }

    // Split `buf` (covering [start, end)) into lines, tracking absolute offsets.
    let mut lines: Vec<(u64, &[u8])> = Vec::new();
    let mut line_start = 0usize;
    for (i, &b) in buf.iter().enumerate() {
        if b == b'\n' {
            lines.push((start + line_start as u64, &buf[line_start..i]));
            line_start = i + 1;
        }
    }
    if line_start < buf.len() {
        lines.push((start + line_start as u64, &buf[line_start..]));
    }

    let keep_from = lines.len().saturating_sub(max_lines);
    let kept = lines.get(keep_from..).unwrap_or(&[]);
    let start_offset = kept.first().map(|&(o, _)| o).unwrap_or(end);
    let out: Vec<String> = kept
        .iter()
        .map(|&(_, s)| {
            String::from_utf8_lossy(s.strip_suffix(b"\r").unwrap_or(s))
                .into_owned()
        })
        .collect();
    Ok((start_offset, out))
}

/// Return the newest `max_lines` lines of the log (the initial page).
///
/// `path` selects the file (default app log when absent). Returns
/// `{ lines, start_bytes, next_bytes }`:
/// - `next_bytes`: current file length — pass to [`get_log_lines`] to live-tail.
/// - `start_bytes`: byte offset where the returned block begins — pass to
///   [`read_log_before`] to page in older lines (`0` ⇒ no older lines).
#[tauri::command]
pub async fn tail_log(
    state: State<'_, GuiState>,
    max_lines: usize,
    path: Option<String>,
) -> Result<serde_json::Value, String> {
    let log_path = resolve_log_path(&state, path)?;
    if !log_path.exists() {
        return Ok(serde_json::json!({
            "lines": [], "start_bytes": 0u64, "next_bytes": 0u64,
        }));
    }
    let len = std::fs::metadata(&log_path)
        .map_err(|e| format!("stat log file: {e}"))?
        .len();
    let (start_bytes, lines) = read_lines_before(&log_path, len, max_lines)
        .map_err(|e| format!("read log file: {e}"))?;
    Ok(serde_json::json!({
        "lines": lines,
        "start_bytes": start_bytes,
        "next_bytes": len,
    }))
}

/// Page in up to `max_lines` older lines ending just before `before_bytes`.
///
/// Returns `{ lines, start_bytes }` where `start_bytes` is the new oldest
/// offset (`0` ⇒ the file head was reached).
#[tauri::command]
pub async fn read_log_before(
    state: State<'_, GuiState>,
    before_bytes: u64,
    max_lines: usize,
    path: Option<String>,
) -> Result<serde_json::Value, String> {
    let log_path = resolve_log_path(&state, path)?;
    if !log_path.exists() || before_bytes == 0 {
        return Ok(serde_json::json!({ "lines": [], "start_bytes": 0u64 }));
    }
    let (start_bytes, lines) =
        read_lines_before(&log_path, before_bytes, max_lines)
            .map_err(|e| format!("read log file: {e}"))?;
    Ok(serde_json::json!({
        "lines": lines,
        "start_bytes": start_bytes,
    }))
}

/// Live-tail: return only the bytes appended since `since_bytes`.
///
/// Seeks straight to `since_bytes` so only the new tail is read, not the whole
/// file. Returns `{ lines: [...], next_bytes: u64 }`.
#[tauri::command]
pub async fn get_log_lines(
    state: State<'_, GuiState>,
    since_bytes: u64,
    path: Option<String>,
) -> Result<serde_json::Value, String> {
    use std::io::{Read as _, Seek as _, SeekFrom};

    let log_path = resolve_log_path(&state, path)?;
    if !log_path.exists() {
        return Ok(serde_json::json!({ "lines": [], "next_bytes": 0u64 }));
    }

    let mut file = std::fs::File::open(&log_path)
        .map_err(|e| format!("open log file: {e}"))?;
    let len = file
        .metadata()
        .map_err(|e| format!("stat log file: {e}"))?
        .len();

    // Truncation/rotation guard: if the file shrank below our cursor, restart.
    let from = if since_bytes > len { 0 } else { since_bytes };
    if from >= len {
        return Ok(serde_json::json!({ "lines": [], "next_bytes": len }));
    }

    file.seek(SeekFrom::Start(from))
        .map_err(|e| format!("seek log file: {e}"))?;
    let mut buf = Vec::with_capacity((len - from) as usize);
    file.read_to_end(&mut buf)
        .map_err(|e| format!("read log file: {e}"))?;

    let text = String::from_utf8_lossy(&buf);
    let lines: Vec<&str> = text.lines().collect();

    Ok(serde_json::json!({
        "lines": lines,
        "next_bytes": len,
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

/// Empty a log file in place (the "Clear" button in the Logs tab).
///
/// `path` selects the file, matching [`tail_log`]'s resolution: absent/empty
/// means the app log. The app log is held open by this process's logger, so it
/// is truncated through the shared [`LogHandle`] — that takes the writer's lock
/// and rewinds the cursor, avoiding a torn write or a sparse hole. Other files
/// (e.g. the mpv log, written by an external process) are truncated directly.
#[tauri::command]
pub async fn clear_log_file(
    state: State<'_, GuiState>,
    path: Option<String>,
) -> Result<(), String> {
    let target = resolve_log_path(&state, path)?;
    let app_log = state
        .log_file
        .lock()
        .map_err(|e| format!("lock log_file: {e}"))?
        .clone();

    // The app log: clear it through the logger so the in-process write cursor is
    // reset in lock-step with the truncation.
    if target == app_log
        && let Some(handle) = state
            .log_handle
            .lock()
            .map_err(|e| format!("lock log_handle: {e}"))?
            .as_ref()
    {
        return handle
            .clear_log_file()
            .map_err(|e| format!("clear log file: {e}"));
    }

    // Any other file (or a missing logger handle): a plain truncate is enough,
    // since no in-process handle holds a stale cursor into it.
    match std::fs::OpenOptions::new().write(true).open(&target) {
        Ok(file) => file
            .set_len(0)
            .map_err(|e| format!("truncate log file: {e}")),
        // Nothing written yet → nothing to clear.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("open log file: {e}")),
    }
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
                .and_then(|_| platform::log_dir().map(|d| d.join("mpv.log")))
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

// ── Cache ──────────────────────────────────────────────────────────────────────

/// Sentinel error returned by [`clear_cache`] when the service is still running.
/// The frontend maps this to a localized "stop the service first" message.
pub const ERR_SERVICE_RUNNING: &str = "SERVICE_RUNNING";

/// Log files that count as clearable cache: the app log (`etlp.log`) and the
/// mpv log (`mpv.log`), both written under the `log/` sub-directory.
fn cache_log_paths(state: &GuiState) -> Result<Vec<PathBuf>, String> {
    let app_log = state
        .log_file
        .lock()
        .map_err(|e| format!("lock log_file: {e}"))?
        .clone();
    let mut paths = vec![app_log];
    if let Some(log) = platform::log_dir() {
        paths.push(log.join("mpv.log"));
    }
    Ok(paths)
}

/// Recursively sum the byte size of every regular file under `dir`, skipping
/// `exclude` (and anything beneath it).
///
/// Missing directories contribute zero so a fresh install reports no cache.
/// `exclude` keeps config backups out of the cache total even if they were ever
/// nested inside the cache tree.
fn dir_size(dir: &std::path::Path, exclude: Option<&std::path::Path>) -> u64 {
    let mut total = 0u64;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if exclude.is_some_and(|ex| path == ex) {
            continue;
        }
        match entry.file_type() {
            Ok(ft) if ft.is_dir() => total += dir_size(&path, exclude),
            Ok(ft) if ft.is_file() => {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
            _ => {}
        }
    }
    total
}

/// Total clearable cache size: the log files plus everything under `cache/`.
#[tauri::command]
pub async fn get_cache_size(state: State<'_, GuiState>) -> Result<u64, String> {
    let mut total: u64 = cache_log_paths(&state)?
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();
    if let Some(cache) = platform::cache_dir() {
        // Config backups are not cache — never count them.
        total += dir_size(&cache, platform::backup_dir().as_deref());
    }
    Ok(total)
}

/// Clear the cache: truncate the log files and delete everything under `cache/`.
///
/// Refuses while the service is running (returns [`ERR_SERVICE_RUNNING`]).
/// Logs are truncated rather than deleted — that is race-safe against a logger
/// that still holds the file open. The `cache/` tree (download cache, bangumi
/// subject cache, future per-feature caches) is removed entirely and recreated
/// empty, since the service is stopped and holds no handles into it.
/// Returns the number of bytes freed.
#[tauri::command]
pub async fn clear_cache(state: State<'_, GuiState>) -> Result<u64, String> {
    if state.running.load(Ordering::Acquire) {
        return Err(ERR_SERVICE_RUNNING.to_owned());
    }
    let mut freed = 0u64;
    let mut errors: Vec<String> = Vec::new();
    for path in cache_log_paths(&state)? {
        let size = match std::fs::metadata(&path) {
            Ok(meta) => meta.len(),
            Err(_) => continue, // missing file → nothing to clear
        };
        match std::fs::OpenOptions::new().write(true).open(&path) {
            Ok(file) => match file.set_len(0) {
                Ok(()) => freed += size,
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }
    if let Some(cache) = platform::cache_dir()
        && cache.is_dir()
    {
        // Clear the cache tree entry-by-entry, preserving config backups: they
        // are not cache and must survive a cache clear (they live under
        // `backup/`, but skip the dir explicitly so the guarantee holds even if
        // that ever moves under `cache/`).
        let backup = platform::backup_dir();
        freed += dir_size(&cache, backup.as_deref());
        if let Ok(entries) = std::fs::read_dir(&cache) {
            for entry in entries.flatten() {
                let path = entry.path();
                if backup.as_deref().is_some_and(|ex| path == ex) {
                    continue;
                }
                let is_dir =
                    entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                let res = if is_dir {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                };
                if let Err(e) = res {
                    errors.push(format!("{}: {e}", path.display()));
                }
            }
        }
    }
    if errors.is_empty() {
        info!(freed, "cache cleared");
        Ok(freed)
    } else {
        Err(errors.join("; "))
    }
}

// ── Config backup / restore / reset ──────────────────────────────────────────────

/// List existing config backups, newest first.
#[tauri::command]
pub async fn list_config_backups()
-> Result<Vec<crate::backup::BackupEntry>, String> {
    crate::backup::list_backups()
}

/// Create a timestamped backup of the current config; returns the new entry.
#[tauri::command]
pub async fn backup_config() -> Result<crate::backup::BackupEntry, String> {
    let entry = crate::backup::create_backup()?;
    info!(name = %entry.name, "config backed up");
    Ok(entry)
}

/// Restore the config from a backup archive at `path`, then reload the server.
#[tauri::command]
pub async fn restore_config(
    state: State<'_, GuiState>,
    path: String,
) -> Result<(), String> {
    crate::backup::restore_backup(&path)?;
    info!(path = %path, "config restored from backup");
    // Push the restored config into a running server, if any.
    let _ = reload_config(state).await;
    Ok(())
}

/// Delete a backup archive at `path`.
#[tauri::command]
pub async fn delete_config_backup(path: String) -> Result<(), String> {
    crate::backup::delete_backup(&path)?;
    info!(path = %path, "config backup deleted");
    Ok(())
}

/// Reveal a backup archive in the system file manager (selects the file).
#[tauri::command]
pub async fn reveal_config_backup(
    app: tauri::AppHandle,
    path: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| format!("reveal backup: {e}"))
}

/// Reset the config to the bundled default, then reload the server.
#[tauri::command]
pub async fn reset_config(state: State<'_, GuiState>) -> Result<(), String> {
    crate::backup::reset_config()?;
    info!("config reset to default");
    let _ = reload_config(state).await;
    Ok(())
}

// ── App info ───────────────────────────────────────────────────────────────────

/// Return the application version string from the Cargo manifest.
#[tauri::command]
pub fn get_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// GitHub repository releases are checked against, in `owner/repo` form.
const GITHUB_REPO: &str = "PiliPili-Team/etlp";

/// Result of an update check surfaced to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UpdateInfo {
    /// The running version.
    pub current: String,
    /// The latest release tag (without a leading `v`); empty when unknown.
    pub latest: String,
    /// Whether `latest` is newer than `current`.
    pub has_update: bool,
    /// The release page to open when the user chooses to update.
    pub url: String,
}

/// Whether dotted-numeric version `a` is strictly newer than `b`.
///
/// Compares component by component (`0.0.3` > `0.0.2`); non-numeric suffixes on
/// a component are ignored. Missing trailing components count as zero.
fn version_gt(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.split('.')
            .map(|p| {
                p.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse::<u64>()
                    .unwrap_or(0)
            })
            .collect()
    };
    let (av, bv) = (parse(a), parse(b));
    for i in 0..av.len().max(bv.len()) {
        let x = av.get(i).copied().unwrap_or(0);
        let y = bv.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

/// Check GitHub for the latest release and compare it to the running version.
///
/// Queries the public releases API (no auth needed). Network or parse failures
/// return an error string the frontend can surface; a malformed/empty tag fails
/// safe as "no update".
#[tauri::command]
pub async fn check_update() -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION").to_owned();
    let api =
        format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let fallback_url =
        format!("https://github.com/{GITHUB_REPO}/releases/latest");

    let client = reqwest::Client::builder()
        .user_agent(format!("etlp/{current}"))
        .build()
        .map_err(|e| format!("build client: {e}"))?;
    let resp = client
        .get(&api)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("update request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("github api status {}", resp.status().as_u16()));
    }
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("parse release: {e}"))?;
    let tag = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_owned();
    let url = json
        .get("html_url")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .unwrap_or(fallback_url);
    let latest = tag.trim_start_matches(['v', 'V']).to_owned();
    let has_update = !latest.is_empty() && version_gt(&latest, &current);
    info!(current, latest, has_update, "update check");
    Ok(UpdateInfo {
        current,
        latest,
        has_update,
        url,
    })
}

/// Font directories to scan, covering both system-wide and per-user locations.
///
/// User locations matter: fonts a user installs themselves (e.g. via Font Book
/// on macOS or the "Install for me" option on Windows) land under the home
/// directory and would otherwise be missed.
fn font_directories() -> Vec<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    // Only macOS/Linux derive per-user font paths from the home directory;
    // Windows uses %LOCALAPPDATA% instead, so binding it there is unused.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let home = dirs::home_dir();

    #[cfg(target_os = "macos")]
    {
        dirs.push("/System/Library/Fonts".into());
        dirs.push("/System/Library/Fonts/Supplemental".into());
        dirs.push("/Library/Fonts".into());
        if let Some(h) = &home {
            dirs.push(h.join("Library/Fonts"));
        }
    }
    #[cfg(target_os = "windows")]
    {
        dirs.push(r"C:\Windows\Fonts".into());
        // Per-user fonts (Windows 10 1809+): %LOCALAPPDATA%\Microsoft\Windows\Fonts.
        if let Some(local) = dirs::data_local_dir() {
            dirs.push(local.join("Microsoft").join("Windows").join("Fonts"));
        }
    }
    #[cfg(target_os = "linux")]
    {
        dirs.push("/usr/share/fonts".into());
        dirs.push("/usr/local/share/fonts".into());
        if let Some(h) = &home {
            dirs.push(h.join(".fonts"));
            dirs.push(h.join(".local/share/fonts"));
        }
    }

    dirs
}

/// Collect font files under `dir` recursively, up to `depth` levels deep.
///
/// Font directories (notably on Linux) nest by foundry/family, so a flat scan
/// would miss most files; the depth bound keeps the walk cheap and avoids
/// pathological recursion.
fn collect_font_files(
    dir: &std::path::Path,
    depth: u8,
    out: &mut Vec<std::path::PathBuf>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth > 0 {
                collect_font_files(&path, depth - 1, out);
            }
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "dfont") {
            out.push(path);
        }
    }
}

/// Read a font file and return its family name(s), preferring the typographic
/// family and a Latin-script name so the value matches what CSS `font-family`
/// expects. Falls back to nothing on parse failure (the caller keeps the stem).
fn font_family_names(path: &std::path::Path) -> Vec<String> {
    let Ok(data) = std::fs::read(path) else {
        return Vec::new();
    };
    let face_count = ttf_parser::fonts_in_collection(&data).unwrap_or(1).max(1);
    let mut names: Vec<String> = Vec::new();
    for index in 0..face_count {
        if let Ok(face) = ttf_parser::Face::parse(&data, index)
            && let Some(name) = best_family_name(&face)
        {
            names.push(name);
        }
    }
    names
}

/// Pick the best family name from a parsed face: the typographic family (name
/// id 16) wins over the legacy family (id 1), and a Latin-script entry wins over
/// a localized one so the picker shows e.g. "LXGW WenKai" rather than "霞鹜文楷".
fn best_family_name(face: &ttf_parser::Face) -> Option<String> {
    use ttf_parser::name_id;
    let mut best: Option<(u8, bool, String)> = None;
    for name in face.names() {
        let priority = match name.name_id {
            name_id::TYPOGRAPHIC_FAMILY => 0u8,
            name_id::FAMILY => 1,
            _ => continue,
        };
        let Some(text) = name.to_string() else {
            continue;
        };
        let text = text.trim().to_owned();
        if text.is_empty() {
            continue;
        }
        let is_latin = text.is_ascii();
        // Lower priority value and Latin script are preferred.
        let better = match &best {
            None => true,
            Some((bp, bl, _)) => {
                priority < *bp || (priority == *bp && is_latin && !*bl)
            }
        };
        if better {
            best = Some((priority, is_latin, text));
        }
    }
    best.map(|(_, _, text)| text)
}

/// List the available font families on the current system.
///
/// Scans the system and per-user font directories, reading each file's real
/// family name from its `name` table (so a font appears under the name CSS
/// expects, not its filename). Common cross-platform fonts are prepended so they
/// appear at the top of any picker.
#[tauri::command]
pub fn list_system_fonts() -> Vec<String> {
    let mut fonts: Vec<String> = Vec::new();

    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for dir in font_directories() {
        collect_font_files(&dir, 4, &mut files);
    }
    for file in &files {
        let parsed = font_family_names(file);
        if parsed.is_empty() {
            // Parse failed (e.g. a .dfont resource fork): fall back to the stem.
            if let Some(stem) = file.file_stem().and_then(|s| s.to_str()) {
                fonts.push(stem.to_owned());
            }
        } else {
            fonts.extend(parsed);
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
        Ok(c) => Ok(c),
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

    Ok(result)
}

pub(crate) fn write_default_config(
    path: &std::path::Path,
) -> Result<(), String> {
    let template = default_config_template();
    match etlp_config::write_config_str(path, &template) {
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

#[cfg(test)]
mod tests {
    use super::{read_lines_before, version_gt};
    use std::io::Write as _;

    #[test]
    fn version_gt_compares_numeric_components() {
        assert!(version_gt("0.0.3", "0.0.2"));
        assert!(version_gt("0.1.0", "0.0.9"));
        assert!(version_gt("1.0.0", "0.9.9"));
        assert!(!version_gt("0.0.2", "0.0.2"));
        assert!(!version_gt("0.0.2", "0.0.3"));
        // Missing trailing components count as zero.
        assert!(version_gt("1.2", "1.1.9"));
        assert!(!version_gt("1.2", "1.2.0"));
    }

    fn write_tmp(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().expect("tempfile");
        f.write_all(content.as_bytes()).expect("write");
        f.flush().expect("flush");
        f
    }

    #[test]
    fn tail_returns_last_n_lines() {
        let f = write_tmp("a\nb\nc\nd\ne\n");
        let len = f.path().metadata().unwrap().len();
        let (start, lines) = read_lines_before(f.path(), len, 2).unwrap();
        assert_eq!(lines, vec!["d".to_owned(), "e".to_owned()]);
        // "d\n" begins at byte 6 ("a\nb\nc\n" == 6 bytes).
        assert_eq!(start, 6);
    }

    #[test]
    fn tail_clamps_to_available_lines() {
        let f = write_tmp("only\ntwo\n");
        let len = f.path().metadata().unwrap().len();
        let (start, lines) = read_lines_before(f.path(), len, 10).unwrap();
        assert_eq!(lines, vec!["only".to_owned(), "two".to_owned()]);
        assert_eq!(start, 0); // file head reached
    }

    #[test]
    fn paging_before_offset_returns_older_lines() {
        let f = write_tmp("a\nb\nc\nd\ne\n");
        let len = f.path().metadata().unwrap().len();
        // First page: last 2 lines, starting at byte 6.
        let (start, _) = read_lines_before(f.path(), len, 2).unwrap();
        // Older page ending just before byte 6.
        let (older_start, older) =
            read_lines_before(f.path(), start, 2).unwrap();
        assert_eq!(older, vec!["b".to_owned(), "c".to_owned()]);
        assert_eq!(older_start, 2); // "b\n" begins at byte 2
    }

    #[test]
    fn crlf_endings_are_trimmed() {
        let f = write_tmp("x\r\ny\r\n");
        let len = f.path().metadata().unwrap().len();
        let (_, lines) = read_lines_before(f.path(), len, 5).unwrap();
        assert_eq!(lines, vec!["x".to_owned(), "y".to_owned()]);
    }
}
