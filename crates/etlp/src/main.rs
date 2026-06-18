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
use tracing::info;

use etlp_config::Config;
use etlp_download::{
    DEFAULT_MAX_CONCURRENT, DEFAULT_MAX_PER_DOMAIN, DownloadManager,
};
use etlp_logging::{Masker, init as init_logging};
use etlp_net::HttpClientBuilder;
use etlp_server::{AppState, build_router};

use cli::{Cli, Commands};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let working_dir =
        cli.config_dir.clone().unwrap_or_else(|| PathBuf::from("."));

    if let Some(cmd) = cli.command {
        if let Err(e) = run_command(cmd, &working_dir).await {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        return;
    }

    // Load config first so logging can be configured from it.
    let config = match Config::load_from_dir(&working_dir) {
        Ok(c) => c,
        Err(e) => {
            // Init logging with safe defaults before printing the error.
            let _ = init_logging(Masker::new(true), "info", None);
            eprintln!(
                "failed to load config from {}: {e}",
                working_dir.display()
            );
            std::process::exit(1);
        }
    };

    // `dev.mix_log = false` disables log masking; absent keeps the default true.
    let mix_log = config.dev.mix_log;
    let log_level = config.dev.log_level.clone();
    let log_file = config.dev.log_file.clone();

    let _ = init_logging(Masker::new(mix_log), &log_level, log_file.as_deref());

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

    let proxy = config.dev.proxy.clone();
    let cert_verify = !config.dev.skip_certificate_verify;
    let http_client = match HttpClientBuilder::new()
        .proxy(proxy)
        .cert_verify(cert_verify)
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
        .unwrap_or_else(|| working_dir.join("cache"));
    let speed_limit: u64 = config.gui.speed_limit_mb * 1024 * 1024;

    let dl_client = match reqwest::Client::builder().build() {
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
        Arc::new(AppState::new(config, dl_manager, http_client, working_dir));

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

    let app = build_router(state);
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
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

    let client_id = std::env::var("TRAKT_CLIENT_ID")
        .map_err(|_| "TRAKT_CLIENT_ID not set")?;
    let client_secret = std::env::var("TRAKT_CLIENT_SECRET")
        .map_err(|_| "TRAKT_CLIENT_SECRET not set")?;

    let token_path = working_dir.join("trakt_token.json");
    let mut api = TraktApi::new(
        &client_id,
        &client_secret,
        "",
        &token_path,
        "https://api.trakt.tv",
    )?;

    let code_resp = api.request_device_code().await?;
    println!(
        "Visit {} and enter code: {}",
        code_resp.verification_url, code_resp.user_code
    );

    let token = api
        .poll_device_token(
            &code_resp.device_code,
            code_resp.expires_in,
            code_resp.interval,
        )
        .await?;
    api.save_token(&token)?;
    println!("Trakt authentication successful.");
    Ok(())
}

/// Mark an episode as watched on bgm.tv.
async fn run_bgm_mark_played(
    working_dir: &std::path::Path,
    subject_id: u64,
    ep: u32,
    token_override: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use etlp_sync::{BangumiApi, sync_episode_by_bangumi_id};

    let config = Config::load_from_dir(working_dir).ok();
    let token = token_override
        .or_else(|| {
            config.as_ref().and_then(|c| c.bangumi.access_token.clone())
        })
        .ok_or("bangumi access_token required (--token or config)")?;

    let api = BangumiApi::new("", &token, "https://api.bgm.tv")?;
    sync_episode_by_bangumi_id(&api, subject_id, &[ep]).await?;
    println!("bgm.tv: episode {ep} of subject {subject_id} marked as watched.");
    Ok(())
}
