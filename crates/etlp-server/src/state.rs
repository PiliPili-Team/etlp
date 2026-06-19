//! Shared application state threaded through axum route handlers.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, RwLock};

use tokio::sync::Mutex;

use etlp_config::Config;
use etlp_download::DownloadManager;
use etlp_net::{HttpClient, RedirectCache};

/// All cross-request shared state.
///
/// Always accessed as `Arc<AppState>` via axum's `State<SharedState>`
/// extractor. Each field uses the minimal synchronisation primitive
/// required for its access pattern.
pub struct AppState {
    /// Counts currently active player subprocesses; zero means idle.
    pub active_players: AtomicUsize,
    /// Redirect URL cache (internally `Arc<RwLock<…>>`—cheap to clone).
    pub redirect_cache: RedirectCache,
    /// Maps `"{netloc}-{item_id}"` → override start-second.
    pub miss_runtime: RwLock<HashMap<String, i64>>,
    /// Download manager shared across all routes.
    pub dl_manager: Mutex<DownloadManager>,
    /// HTTP client for upstream media-server calls.
    pub http_client: HttpClient,
    /// Live configuration; reloaded on each incoming request.
    pub config: RwLock<Config>,
    /// Platform data directory — runtime files (logs, device_id, cache) live here.
    /// Follows XDG_DATA_HOME on Linux, ~/Library/… on macOS, %LOCALAPPDATA% on Windows.
    pub working_dir: PathBuf,
    /// Persistent device identifier; used as fallback when the request
    /// does not carry a DeviceId query parameter.
    pub device_id: String,
}

/// The shared handle used by every route handler.
pub type SharedState = Arc<AppState>;

impl AppState {
    /// Construct a new `AppState` from pre-built dependencies.
    pub fn new(
        config: Config,
        dl_manager: DownloadManager,
        http_client: HttpClient,
        working_dir: PathBuf,
    ) -> Self {
        Self {
            active_players: AtomicUsize::new(0),
            redirect_cache: RedirectCache::new(),
            miss_runtime: RwLock::new(HashMap::new()),
            dl_manager: Mutex::new(dl_manager),
            http_client,
            config: RwLock::new(config),
            working_dir,
            device_id: crate::platform::device_id::load_or_create(),
        }
    }
}

#[cfg(test)]
pub mod test_helpers {
    use etlp_config::Config;
    use etlp_download::{
        DEFAULT_MAX_CONCURRENT, DEFAULT_MAX_PER_DOMAIN, DownloadManager,
    };
    use etlp_net::HttpClientBuilder;
    use std::io::Write as _;
    use tempfile::TempDir;

    use super::{AppState, SharedState};
    use std::sync::Arc;

    /// Minimal TOML content sufficient for most route tests.
    pub const MINIMAL_TOML: &str = "\
[emby]\nplayer = \"mpv\"\n\
[dev]\nskip_certificate_verify = false\n\
[trakt]\nenable_host = \"\"\n\
";

    /// Build a test [`SharedState`] backed by a temp directory.
    ///
    /// Returns the state and the `TempDir` guard (dropped when test ends).
    pub fn test_state() -> (SharedState, TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let toml_path = dir.path().join("config.toml");
        {
            let mut f = std::fs::File::create(&toml_path).expect("create toml");
            f.write_all(MINIMAL_TOML.as_bytes()).expect("write toml");
        }
        let config = Config::load_file(&toml_path).expect("load config");
        let client =
            reqwest::Client::builder().build().expect("reqwest client");
        let dl_manager = DownloadManager::new(
            dir.path().to_path_buf(),
            0,
            DEFAULT_MAX_CONCURRENT,
            DEFAULT_MAX_PER_DOMAIN,
            client,
        );
        let http_client =
            HttpClientBuilder::new().build().expect("http client");
        let state = Arc::new(AppState::new(
            config,
            dl_manager,
            http_client,
            dir.path().to_path_buf(),
        ));
        (state, dir)
    }
}
