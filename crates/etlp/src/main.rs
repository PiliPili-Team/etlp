//! Binary entry point for `etlp`.
//!
//! Startup sequence:
//! 1. Parse CLI args.
//! 2. If a subcommand is given, run it and exit.
//! 3. Load configuration.
//! 4. Init logging (level and masking from config).
//! 5. Build `AppState`.
//! 6. Optionally kill previous player instances.
//! 7. Start background loops and serve the axum HTTP server.

mod cli;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser as _;
use tokio::net::TcpListener;
use tower::Layer as _;
use tower_http::normalize_path::NormalizePathLayer;
use tracing::info;

use etlp_config::Config;
use etlp_download::{
    DEFAULT_MAX_CONCURRENT, DEFAULT_MAX_PER_DOMAIN, DownloadManager,
};
use etlp_logging::{LogRotation, Masker, init as init_logging};
use etlp_net::HttpClientBuilder;
use etlp_server::{AppState, build_router, platform};

use cli::{Cli, Commands};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Data directory priority: --data-dir flag > XDG / platform default > cwd.
    let data_dir = cli
        .data_dir
        .clone()
        .or_else(platform::data_dir)
        .unwrap_or_else(|| PathBuf::from("."));

    if let Some(cmd) = cli.command {
        // For sub-commands, use the explicit config dir or the XDG config dir.
        let cmd_working_dir = cli
            .config_dir
            .clone()
            .or_else(platform::config_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        if let Err(e) = run_command(cmd, &cmd_working_dir).await {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        return;
    }

    // Resolve the config file, searching XDG dirs and generating a default if
    // none exists.
    let config_path = resolve_config(&cli);
    let config = match Config::load_file(&config_path) {
        Ok(c) => c,
        Err(e) => {
            let _ = init_logging(
                Masker::new(true),
                "info",
                None,
                LogRotation::default(),
            );
            eprintln!("failed to load config {}: {e}", config_path.display());
            std::process::exit(1);
        }
    };

    // `dev.mix_log = false` disables log masking; absent keeps the default true.
    let mix_log = config.dev.mix_log;
    let log_level = config.dev.log_level.clone();
    // Relocate any legacy flat-layout files into the log/ and cache/ dirs.
    let _ = std::fs::create_dir_all(&data_dir);
    platform::migrate_layout(&data_dir);
    let log_dir = platform::log_dir_in(&data_dir);
    let _ = std::fs::create_dir_all(&log_dir);
    // Use the configured log file, or default to log/etlp.log.
    let log_file = config
        .dev
        .log_file
        .clone()
        .or_else(|| Some(log_dir.join("etlp.log")));

    let rotation = LogRotation::from_mb(
        config.dev.log_max_size_mb,
        config.dev.log_max_files,
    );
    let masker = Masker::new(mix_log);
    // Redact the configured Bangumi / Trakt account names from on-disk logs
    // when masking is on, so a shared log never carries the user's identity —
    // whether it surfaces as a `user=` field, a `user_name=` field, or a
    // `users/<name>/…` URL path in a logged curl line.
    masker.add_user(&config.bangumi.username);
    masker.add_user(&config.trakt.user_name);
    let _ = init_logging(masker, &log_level, log_file.as_deref(), rotation);

    info!(
        "etlp {} starting (config={})",
        env!("CARGO_PKG_VERSION"),
        config.path().display(),
    );

    if config.dev.kill_process_at_start {
        let _ = etlp_server::platform::kill_matching_processes(
            r"(embyToLocalPlayer\.py|autohotkey_tool|mpv.*exe|mpc-.*exe|\
              vlc\.exe|PotPlayer.*exe|/IINA|/VLC|/mpv)",
            r"(tmux|greasyfork|github)",
        );
    }

    let cert_verify = !config.dev.skip_certificate_verify;
    let http_client = match HttpClientBuilder::new()
        .proxy_http(config.dev.proxy_http.clone())
        .proxy_https(config.dev.proxy_https.clone())
        .proxy_enabled(config.dev.proxy_enabled)
        .cert_verify(cert_verify)
        .user_agent(config.dev.user_agent.clone())
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to build http client: {e}");
            std::process::exit(1);
        }
    };

    let cache_path = config
        .gui
        .server_cache_path
        .clone()
        .unwrap_or_else(|| data_dir.join("cache"));
    let speed_limit: u64 = config.gui.speed_limit_mb * 1024 * 1024;

    let dl_client = match etlp_net::build_media_download_client(
        config.dev.proxy_http.clone(),
        config.dev.proxy_https.clone(),
        config.dev.proxy_enabled,
        cert_verify,
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to build download client: {e}");
            std::process::exit(1);
        }
    };
    let dl_manager = DownloadManager::new(
        cache_path,
        speed_limit,
        DEFAULT_MAX_CONCURRENT,
        DEFAULT_MAX_PER_DOMAIN,
        dl_client,
    );
    dl_manager.start_update_db_loop(30);

    let state =
        Arc::new(AppState::new(config, dl_manager, http_client, data_dir));

    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port)
        .parse()
        .unwrap_or(SocketAddr::from(([127, 0, 0, 1], 58000)));

    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("failed to bind {addr}: {e}");
            std::process::exit(1);
        }
    };

    info!("serving at http://{addr}");

    let router = build_router(state);
    // NormalizePathLayer must wrap the entire Router as the outermost service
    // so it strips trailing slashes BEFORE axum's routing logic runs.
    // Router::layer() only wraps matched route handlers, so it cannot affect
    // routing decisions; this external wrapping is the correct placement.
    let app = NormalizePathLayer::trim_trailing_slash().layer(router);
    if let Err(e) = axum::serve(listener, tower::make::Shared::new(app)).await {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
}

/// Locate the config file, searching XDG dirs and generating a default if absent.
fn resolve_config(cli: &Cli) -> PathBuf {
    // Explicit --config-file / --config-dir flags take priority.
    if let Some(file) = &cli.config_file {
        return file.clone();
    }
    if let Some(dir) = &cli.config_dir
        && let Ok(c) = Config::load_from_dir(dir)
    {
        return c.path().to_path_buf();
    }

    // Search XDG / platform config dir.
    if let Some(cfg_dir) = platform::config_dir() {
        if let Ok(c) = Config::load_from_dir(&cfg_dir) {
            return c.path().to_path_buf();
        }
        // Nothing found — generate a default config.
        let default = cfg_dir.join("config.toml");
        match Config::write_default(&default) {
            Ok(()) => {
                eprintln!(
                    "no config found; created default at {}",
                    default.display()
                );
                return default;
            }
            Err(e) => {
                eprintln!("could not write default config: {e}");
            }
        }
    }

    // Last resort: current directory.
    PathBuf::from("config.toml")
}

async fn run_command(
    cmd: Commands,
    working_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Commands::TraktAuth => run_trakt_auth(working_dir).await,
        Commands::BgmMarkPlayed {
            subject_id,
            ep,
            token,
        } => run_bgm_mark_played(working_dir, subject_id, ep, token).await,
    }
}

/// Trigger Trakt Device Flow: display the user_code and poll for the token.
async fn run_trakt_auth(
    working_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use etlp_sync::TraktApi;

    let config = Config::load_from_dir(working_dir)
        .or_else(|_| {
            platform::config_dir()
                .map(|d| Config::load_from_dir(&d))
                .unwrap_or_else(|| {
                    Err(etlp_config::ConfigError::NotFound(
                        working_dir.to_path_buf(),
                    ))
                })
        })
        .or_else(|_| Config::load_from_dir(std::path::Path::new(".")))
        .ok();

    let client_id = config
        .as_ref()
        .map(|c| c.trakt.client_id.trim().to_owned())
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("TRAKT_CLIENT_ID").ok())
        .ok_or(
            "trakt client_id required (set in config [trakt].client_id or \
             TRAKT_CLIENT_ID env var)",
        )?;

    let client_secret = config
        .as_ref()
        .map(|c| c.trakt.client_secret.trim().to_owned())
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("TRAKT_CLIENT_SECRET").ok())
        .ok_or(
            "trakt client_secret required (set in config [trakt].client_secret \
             or TRAKT_CLIENT_SECRET env var)",
        )?;

    let user_name = config
        .as_ref()
        .map(|c| c.trakt.user_name.clone())
        .unwrap_or_default();

    let sync_proxy = config
        .as_ref()
        .map(|c| {
            etlp_sync::SyncProxy::new(
                c.dev.proxy_http.clone(),
                c.dev.proxy_https.clone(),
                c.dev.proxy_enabled,
            )
        })
        .unwrap_or_default();

    // Prefer data_dir for the token file; fall back to working_dir.
    let token_dir =
        platform::data_dir().unwrap_or_else(|| working_dir.to_path_buf());
    let _ = std::fs::create_dir_all(&token_dir);
    let token_path = token_dir.join(TraktApi::TOKEN_FILE_NAME);
    let mut api = TraktApi::new(
        &client_id,
        &client_secret,
        &user_name,
        &token_path,
        TraktApi::DEFAULT_BASE_URL,
        sync_proxy,
    )?;

    let code_resp = api.request_device_code().await?;
    println!(
        "Visit {} and enter code: {}",
        code_resp.verification_url, code_resp.user_code
    );

    // Automatically open the verification URL in the system's default browser.
    let _ = platform::open_url(&code_resp.verification_url);
    println!("Waiting for authorization in browser...");

    let token = api
        .poll_device_token(
            &code_resp.device_code,
            code_resp.interval,
            code_resp.expires_in,
        )
        .await?;
    api.save_token(&token)?;
    println!(
        "Trakt authentication successful. Token saved to {}",
        token_path.display()
    );
    Ok(())
}

/// Mark an episode as watched on bgm.tv.
async fn run_bgm_mark_played(
    working_dir: &std::path::Path,
    subject_id: u64,
    ep: u32,
    token_override: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use etlp_sync::{
        BangumiApi, new_bgm_read_cache, sync_episode_by_bangumi_id,
    };

    let config = Config::load_from_dir(working_dir).ok();
    let token = token_override
        .or_else(|| {
            config
                .as_ref()
                .map(|c| c.bangumi.access_token.clone())
                .filter(|t| !t.is_empty())
        })
        .ok_or("bangumi access_token required (--token or config)")?;

    let (username, private) = config
        .as_ref()
        .map(|c| (c.bangumi.username.clone(), c.bangumi.private))
        .unwrap_or_else(|| (String::new(), true));

    let api = BangumiApi::new(
        username,
        &token,
        private,
        BangumiApi::DEFAULT_BASE_URL,
        new_bgm_read_cache(),
        etlp_sync::SyncProxy::default(),
    )?;
    sync_episode_by_bangumi_id(&api, subject_id, &[ep]).await?;
    println!("bgm.tv: episode {ep} of subject {subject_id} marked as watched.");
    Ok(())
}
