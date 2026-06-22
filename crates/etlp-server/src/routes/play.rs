//! Main ToLocalPlayer route handlers.
//!
//! `POST /embyToLocalPlayer` and `POST /plexToLocalPlayer` are the primary
//! entry points called by the Tampermonkey userscript. Both respond with
//! HTTP 200 immediately and then drive the full play chain in a spawned task.

use std::sync::atomic::{AtomicUsize, Ordering};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use serde_json::{Value, json};
use tracing::{debug, info, warn};

use etlp_core::{PlaybackData, PlayerKind};
use etlp_media_server::{
    EmbyClient, EmbyParseConfig, ListContext, PlexParseConfig,
    PlexReceivedData, ReceivedData, assemble_episodes, assemble_episodes_alt,
    looks_like_legacy_shape, parse_received_data_emby,
    parse_received_data_plex,
};
use etlp_metrics::{PlayMetrics, Span};
use etlp_player::{
    DanDanConfig, DanDanHandle, LaunchArgs, LoadMode, LoadOptions, MpcHandle,
    MpvHandle, PlayerHandle, PlayerManager, PotHandle, SyncEntry, VlcHandle,
    resolve_player_executable,
};

use crate::state::SharedState;

/// Monotonically increasing session counter; each `run_player_chain` call gets
/// a unique ID so concurrent sessions write to distinct M3U8 files.
static SESSION_COUNTER: AtomicUsize = AtomicUsize::new(0);

// ── Public route handlers ─────────────────────────────────────────────────────

/// `POST /embyToLocalPlayer` – Emby and Jellyfin userscript entry point.
///
/// Responds 200 immediately; parsing and player launch run in a background
/// task so the browser is not left waiting.
pub async fn emby_to_local_player(
    State(state): State<SharedState>,
    Json(received): Json<ReceivedData>,
) -> (StatusCode, Json<Value>) {
    info!("POST /embyToLocalPlayer received");
    if received.show_task_manager {
        info!("show_task_manager requested (GUI not available)");
        return (
            StatusCode::OK,
            Json(json!({"msg": "task manager not available"})),
        );
    }

    reload_config(&state);

    let state2 = state.clone();
    tokio::spawn(async move {
        start_emby_play(state2, received).await;
    });

    (StatusCode::OK, Json(json!({"msg": "ok"})))
}

/// `POST /plexToLocalPlayer` – Plex userscript entry point.
pub async fn plex_to_local_player(
    State(state): State<SharedState>,
    Json(received): Json<PlexReceivedData>,
) -> (StatusCode, Json<Value>) {
    info!("POST /plexToLocalPlayer received");
    reload_config(&state);

    let state2 = state.clone();
    tokio::spawn(async move {
        start_plex_play(state2, received).await;
    });

    (StatusCode::OK, Json(json!({"msg": "ok"})))
}

// ── Config reload ─────────────────────────────────────────────────────────────

fn reload_config(state: &SharedState) {
    match state.config.write() {
        Ok(mut cfg) => match cfg.reload() {
            Ok(()) => debug!("config reloaded from {}", cfg.path().display()),
            Err(e) => warn!("config reload failed: {e}"),
        },
        Err(e) => warn!("config write lock poisoned: {e}"),
    }
}

// ── Player launch config ──────────────────────────────────────────────────────

struct LaunchCfg {
    player_exe: String,
    fullscreen: bool,
    disable_audio: bool,
    http_proxy: Option<String>,
    static_ipc: Option<String>,
    dandan: DanDanConfig,
    playlist_limit: usize,
    disable_progress_report: bool,
}

fn read_launch_cfg(state: &SharedState) -> Option<LaunchCfg> {
    let cfg = state.config.read().ok()?;
    let player = cfg.emby.player.clone();
    let player_exe = cfg
        .dev
        .player_path
        .clone()
        .unwrap_or_else(|| player.clone());
    let fullscreen = cfg.emby.fullscreen;
    let disable_audio = cfg.emby.disable_audio;
    let http_proxy = cfg.dev.http_proxy.clone();
    let static_ipc = cfg.dev.mpv_input_ipc_server.clone();
    let dandan = DanDanConfig {
        port: cfg.dandan.port,
        api_key: cfg.dandan.api_key.clone(),
    };
    // `item_limit == 0` means "no cap": append the whole season.
    let playlist_limit = match cfg.playlist.item_limit {
        0 => usize::MAX,
        n => n as usize,
    };
    let disable_progress_report = cfg.dev.disable_progress_report;
    Some(LaunchCfg {
        player_exe,
        fullscreen,
        disable_audio,
        http_proxy,
        static_ipc,
        dandan,
        playlist_limit,
        disable_progress_report,
    })
}

// ── Core play chain ───────────────────────────────────────────────────────────

/// Spawn the player, manage the playlist, run progress loops, and write
/// stop-time back to the media server.
///
/// `episode_list` must include the currently playing episode (at any index).
/// `active_players` must be incremented by the caller before invoking; this
/// function always decrements it before returning.
/// `session_id` is generated by the caller and used for metrics correlation
/// and per-session M3U8 file naming.
async fn run_player_chain(
    state: SharedState,
    data: PlaybackData,
    episode_list: Vec<PlaybackData>,
    session_id: usize,
    mut metrics: PlayMetrics,
) {
    let chain_start = std::time::Instant::now();
    let playlist_m3u8 = format!("etlp_playlist_{session_id}.m3u8");

    let mut cfg = match read_launch_cfg(&state) {
        Some(c) => c,
        None => {
            warn!("run_player_chain: config lock poisoned");
            metrics.total_ms = Some(chain_start.elapsed().as_millis());
            metrics.report();
            state.active_players.fetch_sub(1, Ordering::Release);
            return;
        }
    };

    // A macOS `.app` is a directory; spawning it fails with "Permission denied"
    // (os error 13). Unwrap such a bundle to the executable inside it (e.g.
    // /Applications/IINA.app → …/Contents/MacOS/iina-cli) before launch.
    let resolved_exe = resolve_player_executable(&cfg.player_exe);
    if resolved_exe != cfg.player_exe {
        info!(
            from = %cfg.player_exe,
            to = %resolved_exe,
            "resolved app bundle to executable"
        );
        cfg.player_exe = resolved_exe;
    }

    let kind = PlayerKind::detect_from_path(&cfg.player_exe)
        .unwrap_or(PlayerKind::Mpv);
    let play_multiple = episode_list.len() > 1;
    debug!(
        player_kind = ?kind,
        exe = %cfg.player_exe,
        mount_disk_mode = data.mount_disk_mode,
        episode_count = episode_list.len(),
        "launching player",
    );

    // For mpv: write the full episode list as a playlist file before spawn so
    // that all entries (including those before the current episode) appear in
    // the playlist panel immediately, and titles come from #EXTINF rather than
    // a post-launch IPC write (which is silently discarded in mpv ≥0.38).
    let (launch_media_path, launch_playlist_start, launch_cur_idx) =
        if play_multiple && kind.is_mpv_family() {
            let cur_idx = episode_list
                .iter()
                .position(|e| {
                    e.item_id == data.item_id
                        || e.media_source_id == data.media_source_id
                })
                .unwrap_or_else(|| {
                    warn!(
                        item_id = %data.item_id,
                        media_source_id = %data.media_source_id,
                        "current episode not found; defaulting cur_idx=0"
                    );
                    0
                });
            debug!(cur_idx, "current episode index in assembled playlist");

            let m3u8_path = std::env::temp_dir().join(&playlist_m3u8);
            let mut m3u8 = String::from("#EXTM3U\n");
            for ep in &episode_list {
                let title = ep.media_title.replace(['\n', '\r'], " ");
                m3u8.push_str(&format!(
                    "#EXTINF:-1,{title}\n{}\n",
                    ep.stream_url
                ));
            }
            let m3u8_span = Span::new("m3u8_write").with_session(session_id);
            match std::fs::write(&m3u8_path, &m3u8) {
                Ok(()) => {
                    metrics.m3u8_write_ms = Some(m3u8_span.finish());
                    debug!(
                        path = %m3u8_path.display(),
                        entries = episode_list.len(),
                        cur_idx,
                        "M3U8 playlist written for launch"
                    );
                    (m3u8_path.display().to_string(), Some(cur_idx), cur_idx)
                }
                Err(e) => {
                    let _ = m3u8_span.finish();
                    warn!(
                        "M3U8 write failed ({e}); \
                         falling back to direct stream URL"
                    );
                    (data.stream_url.clone(), None, cur_idx)
                }
            }
        } else {
            (data.stream_url.clone(), None, 0)
        };

    // `stream_url` is the resolved play URL (HTTP stream or translated local path).
    let args = LaunchArgs {
        exe: cfg.player_exe.clone(),
        media_path: launch_media_path,
        media_title: data.media_title.clone(),
        start_sec: (data.start_sec > 0).then_some(data.start_sec as f64),
        sub: data.sub.clone(),
        is_multiple_episodes: play_multiple,
        mount_disk_mode: data.mount_disk_mode,
        intro: data.intro,
        fullscreen: cfg.fullscreen,
        disable_audio: cfg.disable_audio,
        http_proxy: cfg.http_proxy.clone(),
        static_ipc: cfg.static_ipc.clone(),
        event_handler: None,
        playlist_start: launch_playlist_start,
        mpv_log_file: kind.is_mpv_family().then(|| {
            crate::platform::log_dir_in(&state.working_dir).join("mpv.log")
        }),
    };

    let spawn_span = Span::new("player_spawn").with_session(session_id);
    let handle_result: Result<PlayerHandle, String> = match kind {
        PlayerKind::Mpv | PlayerKind::Iina => MpvHandle::spawn(args)
            .await
            .map(PlayerHandle::Mpv)
            .map_err(|e| format!("{e}")),

        PlayerKind::Vlc => VlcHandle::spawn(&args)
            .await
            .map(PlayerHandle::Vlc)
            .map_err(|e| format!("{e}")),

        PlayerKind::Mpc => MpcHandle::spawn(&args)
            .await
            .map(PlayerHandle::Mpc)
            .map_err(|e| format!("{e}")),

        PlayerKind::PotPlayer => PotHandle::spawn(&args)
            .map(PlayerHandle::Pot)
            .map_err(|e| format!("{e}")),

        PlayerKind::DandanPlay => DanDanHandle::spawn(&args, &cfg.dandan)
            .await
            .map(PlayerHandle::DanDan)
            .map_err(|e| format!("{e}")),
    };

    metrics.player_spawn_ms = Some(spawn_span.finish());
    let handle = match handle_result {
        Ok(h) => h,
        Err(e) => {
            warn!("player spawn failed ({kind:?}): {e}");
            metrics.total_ms = Some(chain_start.elapsed().as_millis());
            metrics.report();
            state.active_players.fetch_sub(1, Ordering::Release);
            return;
        }
    };

    // Raise the freshly launched player window to the foreground. On Windows
    // a process spawned from a background service often lands behind the
    // caller or minimised; `activate_window_by_pid` is a no-op elsewhere.
    if let Some(pid) = handle.pid() {
        crate::platform::activate_window_by_pid(pid);
    }

    let mut mgr = PlayerManager::new(handle, data.clone());
    mgr.disable_progress_report = cfg.disable_progress_report;

    // Playback-latency metric (mpv family): measure how long after the stream
    // URL is handed to mpv (spawn + IPC connected) the media is actually loaded
    // and ready to render. `time-pos` is `null` until mpv has opened the file,
    // so its first non-null read marks playback-ready. Runs detached so it does
    // not delay playlist setup; logs `playback_started` with both the URL→ready
    // latency and the overall click→playing time.
    if let PlayerHandle::Mpv(ref h) = mgr.handle {
        let client = h.client.clone();
        let sid = session_id;
        let spawn_ms = metrics.player_spawn_ms;
        let chain_start_for_task = chain_start;
        let spawn_done = std::time::Instant::now();
        tokio::spawn(async move {
            let deadline = spawn_done + std::time::Duration::from_secs(60);
            loop {
                if std::time::Instant::now() > deadline {
                    debug!(
                        session_id = sid,
                        "playback_started: timed out waiting for first frame"
                    );
                    break;
                }
                match client.command("get_property", &[json!("time-pos")]).await
                {
                    // First non-null position → media loaded and ready.
                    Ok(Some(_)) => {
                        let url_to_ready_ms = spawn_done.elapsed().as_millis();
                        let click_to_play_ms =
                            chain_start_for_task.elapsed().as_millis();
                        info!(
                            session_id = sid,
                            url_to_ready_ms,
                            click_to_play_ms,
                            player_spawn_ms = ?spawn_ms,
                            "playback_started"
                        );
                        break;
                    }
                    // Loaded but position not yet available — keep polling.
                    Ok(None) => {
                        tokio::time::sleep(std::time::Duration::from_millis(
                            50,
                        ))
                        .await;
                    }
                    // IPC closed (player exited before playing).
                    Err(_) => break,
                }
            }
        });
    }

    // For mpv / iina: verify the playlist loaded correctly then unpause.
    if play_multiple
        && kind.is_mpv_family()
        && let PlayerHandle::Mpv(ref h) = mgr.handle
    {
        if let Ok(Some(ver)) = h
            .client
            .command("get_property", &[json!("mpv-version")])
            .await
        {
            debug!("mpv version: {ver}");
        }

        // M3U8 write failed before spawn — fall back to appending after-episodes
        // via a post-launch loadlist. Titles will not show for earlier episodes.
        if launch_playlist_start.is_none() {
            let new_fmt = h.detect_new_loadfile_format().await;
            let after: Vec<&PlaybackData> = episode_list
                .iter()
                .skip(launch_cur_idx + 1)
                .take(cfg.playlist_limit)
                .collect();
            debug!(
                after_count = after.len(),
                "fallback: appending episodes via loadlist"
            );

            let m3u8_path = std::env::temp_dir().join(&playlist_m3u8);
            let mut m3u8 = String::from("#EXTM3U\n");
            for ep in &after {
                let title = ep.media_title.replace(['\n', '\r'], " ");
                m3u8.push_str(&format!(
                    "#EXTINF:-1,{title}\n{}\n",
                    ep.stream_url
                ));
            }
            let loaded_via_m3u8 = match std::fs::write(&m3u8_path, &m3u8) {
                Err(e) => {
                    warn!("M3U8 fallback write failed ({e})");
                    false
                }
                Ok(()) => {
                    let path_str = m3u8_path.display().to_string();
                    match h
                        .client
                        .command(
                            "loadlist",
                            &[json!(path_str), json!("append")],
                        )
                        .await
                    {
                        Ok(_) => {
                            debug!(
                                "M3U8 fallback loaded ({} entries)",
                                after.len()
                            );
                            true
                        }
                        Err(e) => {
                            warn!("loadlist fallback: {e}");
                            false
                        }
                    }
                }
            };
            if !loaded_via_m3u8 {
                for ep in &after {
                    let title_escaped =
                        ep.media_title.replace('"', "").replace(',', "\\,");
                    let opts = LoadOptions {
                        media_title: Some(title_escaped),
                        ..LoadOptions::default()
                    };
                    if let Err(e) = h
                        .loadfile(
                            &ep.stream_url,
                            LoadMode::Append,
                            &opts,
                            new_fmt,
                        )
                        .await
                    {
                        warn!("playlist append {:?}: {e}", ep.media_title);
                    }
                }
            }
        }

        let expected_count = episode_list.len() as i64;
        if let Ok(Some(cnt)) = h
            .client
            .command("get_property", &[json!("playlist-count")])
            .await
        {
            debug!("playlist-count: {cnt} (expected {expected_count})");
        }

        if let Err(e) = h.set_pause(false).await {
            warn!("mpv unpause: {e}");
        }
    }

    // For VLC: the launch URL plays the current episode; enqueue every later
    // episode through the HTTP control interface so playback continues across
    // the season. VLC advances the playlist on its own and `--play-and-exit`
    // quits after the final item. Resume (`:start-time`) stays on the current
    // episode; later episodes start from the beginning.
    if play_multiple && let PlayerHandle::Vlc(ref h) = mgr.handle {
        let cur_idx = episode_list
            .iter()
            .position(|e| {
                e.item_id == data.item_id
                    || e.media_source_id == data.media_source_id
            })
            .unwrap_or_else(|| {
                warn!(
                    item_id = %data.item_id,
                    "vlc: current episode not found; enqueueing from start"
                );
                0
            });
        let after: Vec<&PlaybackData> = episode_list
            .iter()
            .skip(cur_idx + 1)
            .take(cfg.playlist_limit)
            .collect();
        debug!(after_count = after.len(), "vlc: enqueueing later episodes");
        for ep in &after {
            if let Err(e) = h.playlist_add(&ep.stream_url).await {
                warn!("vlc enqueue {:?}: {e}", ep.media_title);
            }
        }
    }

    // Register all episodes for progress tracking.
    for ep in &episode_list {
        mgr.register_playlist(ep.media_title.clone(), ep.clone());
    }

    // Cancel channel: the feedback loop sends each outgoing episode's
    // download task-id so the receiver can cancel in-progress downloads.
    let (cancel_tx, mut cancel_rx) =
        tokio::sync::mpsc::unbounded_channel::<String>();
    let state_dl = state.clone();
    tokio::spawn(async move {
        while let Some(id) = cancel_rx.recv().await {
            state_dl.dl_manager.lock().await.cancel_only(&id).await;
        }
    });

    let http = state.http_client.clone();
    mgr.start_loops(http.clone(), Some(cancel_tx));
    mgr.collect_stop_times().await;
    mgr.write_progress(&http).await;

    let entries = mgr.completed_entries();
    sync_trakt(&state, &entries).await;
    sync_bangumi(&state, &entries).await;

    metrics.total_ms = Some(chain_start.elapsed().as_millis());
    metrics.report();
    state.active_players.fetch_sub(1, Ordering::Release);
}

// ── Emby / Jellyfin play ──────────────────────────────────────────────────────

async fn start_emby_play(state: SharedState, received: ReceivedData) {
    let session_id = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    debug!(
        session_id,
        mount_disk_enable = %received.mount_disk_enable,
        playlist_info_len = received.extra_data.playlist_info.len(),
        episodes_info_len = received.extra_data.episodes_info.len(),
        "emby play chain starting",
    );
    let mut metrics = PlayMetrics::new(session_id);

    let (parse_cfg, redirect_cache) = {
        let cfg = match state.config.read() {
            Ok(c) => c,
            Err(e) => {
                warn!("config read lock poisoned: {e}");
                return;
            }
        };
        (
            EmbyParseConfig::from_config(&cfg),
            state.redirect_cache.clone(),
        )
    };

    let parse_span = Span::new("emby_parse").with_session(session_id);
    let mut data = match parse_received_data_emby(
        &received,
        &parse_cfg,
        &state.http_client,
        &redirect_cache,
    )
    .await
    {
        Ok(d) => d,
        Err(e) => {
            warn!("parse_received_data_emby: {e}");
            let _ = parse_span.finish();
            return;
        }
    };
    metrics.parse_ms = Some(parse_span.finish());

    if data.device_id.is_empty() {
        data.device_id = state.device_id.clone();
    }

    state.active_players.fetch_add(1, Ordering::AcqRel);
    info!(
        session_id,
        "starting play: server={} item_id={} file={:?}",
        data.server.as_str(),
        data.item_id,
        data.file_path,
    );

    let fetch_span = Span::new("episode_fetch").with_session(session_id);
    let episode_list =
        build_emby_playlist(&state, &received, &data, &parse_cfg)
            .await
            .unwrap_or_else(|| vec![data.clone()]);
    metrics.episode_fetch_ms = Some(fetch_span.finish());

    run_player_chain(state, data, episode_list, session_id, metrics).await;
}

/// Fetch the season episode list from Emby and assemble the playlist.
///
/// Returns `None` when multi-episode is not applicable or the fetch fails,
/// allowing the caller to fall back to single-episode playback.
async fn build_emby_playlist(
    state: &SharedState,
    received: &ReceivedData,
    data: &PlaybackData,
    parse_cfg: &EmbyParseConfig,
) -> Option<Vec<PlaybackData>> {
    debug!(
        is_multiple_episodes = data.is_multiple_episodes,
        "build_emby_playlist: entry",
    );
    // Always attempt to build the playlist for series episodes regardless of
    // is_multiple_episodes — the userscript may set it false for the last
    // episode, but we still want the full season visible in the playlist panel.
    let main = &received.extra_data.main_ep_info;
    debug!(
        series_id = ?main.series_id,
        season_id = ?main.season_id,
        index_number = ?main.index_number,
        "build_emby_playlist: series ids",
    );
    let show_id = match main.series_id.as_deref().filter(|s| !s.is_empty()) {
        Some(id) => id,
        None => {
            debug!("build_emby_playlist: skip (series_id missing or empty)");
            return None;
        }
    };
    let season_id = main.season_id.as_deref();
    let base_url = format!("{}://{}", data.scheme, data.netloc);
    let emby = EmbyClient::new(
        state.http_client.clone(),
        &base_url,
        &data.api_key,
        &data.user_id,
    );
    let ctx = ListContext {
        base: data,
        episodes_info: &received.extra_data.episodes_info,
        season_id: season_id.unwrap_or(""),
        playlist: !received.extra_data.playlist_info.is_empty(),
        config: parse_cfg,
    };
    // Probe the AlternateMediaSources interface first: a server that supports
    // it returns the collapsed shape (one item per episode, every version in
    // MediaSources). A server that ignores the field returns the legacy
    // one-item-per-version shape, so fall back to the plain interface and the
    // legacy assembler. No config switch — the response shape decides.
    let episodes = match emby.episodes(show_id, season_id, true).await {
        Ok(list)
            if !list.items.is_empty()
                && !looks_like_legacy_shape(&list.items) =>
        {
            debug!(
                fetched_count = list.items.len(),
                "build_emby_playlist: using AlternateMediaSources shape",
            );
            assemble_episodes_alt(&ctx, &list.items)
        }
        probe => {
            if let Err(e) = &probe {
                debug!("AlternateMediaSources probe failed ({e}); using plain");
            }
            let fetched = match emby.episodes(show_id, season_id, false).await {
                Ok(list) => list,
                Err(e) => {
                    warn!("fetch episodes for {show_id}: {e}");
                    return None;
                }
            };
            debug!(
                fetched_count = fetched.items.len(),
                "build_emby_playlist: using plain (legacy) shape",
            );
            assemble_episodes(&ctx, &fetched.items)
        }
    };
    debug!(
        assembled_count = episodes.len(),
        "build_emby_playlist: assembled playlist",
    );
    if episodes.is_empty() {
        None
    } else {
        Some(episodes)
    }
}

// ── Plex play ─────────────────────────────────────────────────────────────────

async fn start_plex_play(state: SharedState, received: PlexReceivedData) {
    let session_id = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut metrics = PlayMetrics::new(session_id);

    let plex_cfg = {
        let cfg = match state.config.read() {
            Ok(c) => c,
            Err(e) => {
                warn!("config read lock poisoned: {e}");
                return;
            }
        };
        PlexParseConfig {
            force_disk_prefixes: cfg.dev.force_disk_mode_path.clone(),
            subtitle_priority: cfg.dev.subtitle_priority.clone(),
            path_pairs: cfg.path_translation_pairs(),
        }
    };

    let parse_span = Span::new("plex_parse").with_session(session_id);
    let items = match parse_received_data_plex(&received, &plex_cfg) {
        Ok(v) => v,
        Err(e) => {
            warn!("parse_received_data_plex: {e}");
            let _ = parse_span.finish();
            return;
        }
    };
    metrics.parse_ms = Some(parse_span.finish());

    let mut items_iter = items.into_iter();
    let Some(data) = items_iter.next() else {
        warn!("plex payload contained no items");
        return;
    };
    let episode_list: Vec<PlaybackData> =
        std::iter::once(data.clone()).chain(items_iter).collect();

    state.active_players.fetch_add(1, Ordering::AcqRel);
    info!(
        session_id,
        "starting plex play: item_id={} file={:?}",
        data.item_id,
        data.file_path,
    );

    run_player_chain(state, data, episode_list, session_id, metrics).await;
}

// ── Trakt / Bangumi sync helpers ──────────────────────────────────────────────

/// One episode of the current season that the media server reports as watched.
struct PlayedEpisode {
    /// Episode number within the season (`IndexNumber`).
    index: i64,
    /// Air date, used as the primary key for Bangumi episode matching.
    premiere_date: Option<String>,
    /// External ids (Imdb/Tmdb/Tvdb), used to build Trakt history items.
    provider_ids: std::collections::BTreeMap<String, String>,
}

/// Episodes of `data`'s season the user already watched, up to and including the
/// current episode, for "backfill" marking.
///
/// The first time a user syncs a season, every earlier episode they had already
/// finished in the Emby/Jellyfin client should also be marked watched on the
/// third-party service — not just the episode that triggered this sync. This
/// queries the season's episodes with per-user `Played` status and returns the
/// current episode plus every earlier one Emby marks played.
///
/// Returns an empty vec for non-Emby servers, when the series/episode cannot be
/// addressed, or when the request fails; callers then fall back to syncing only
/// the just-watched entry.
async fn emby_played_backfill(
    state: &SharedState,
    data: &PlaybackData,
) -> Vec<PlayedEpisode> {
    if !data.server.is_emby_like()
        || data.series_id.is_empty()
        || data.index.is_none()
    {
        return Vec::new();
    }
    let cur_index = data.index.unwrap_or_default();
    let base_url = format!("{}://{}", data.scheme, data.netloc);
    let emby = EmbyClient::new(
        state.http_client.clone(),
        &base_url,
        &data.api_key,
        &data.user_id,
    );
    let list = match emby.episodes_with_status(&data.series_id, None).await {
        Ok(l) => l,
        Err(e) => {
            debug!("backfill: fetch episodes failed: {e}");
            return Vec::new();
        }
    };

    let mut out: Vec<PlayedEpisode> = Vec::new();
    let mut seen: std::collections::HashSet<i64> =
        std::collections::HashSet::new();
    for item in list.items {
        let Some(idx) = item.index_number else {
            continue;
        };
        if idx > cur_index || !seen.insert(idx) {
            continue;
        }
        // Restrict to the same season when both numbers are known.
        if let (Some(season), Some(parent)) =
            (data.season_number, item.parent_index_number)
            && season != parent
        {
            seen.remove(&idx);
            continue;
        }
        // The current episode was just watched; earlier ones must be flagged
        // played by the server.
        let played = item.user_data.as_ref().map(|u| u.played).unwrap_or(false);
        if idx != cur_index && !played {
            continue;
        }
        out.push(PlayedEpisode {
            index: idx,
            premiere_date: item.premiere_date.clone(),
            provider_ids: item.provider_ids.clone(),
        });
    }
    debug!(
        series_id = %data.series_id,
        cur_index,
        backfill = out.len(),
        "backfill: resolved played episodes"
    );
    out
}

/// Extract Trakt provider ids from a media-server `ProviderIds` map.
fn trakt_ids_from_provider_map(
    provider_ids: &std::collections::BTreeMap<String, String>,
) -> etlp_sync::TraktIds {
    etlp_sync::TraktIds {
        imdb: provider_ids.get("Imdb").cloned(),
        tmdb: provider_ids.get("Tmdb").and_then(|v| v.parse().ok()),
        tvdb: provider_ids.get("Tvdb").and_then(|v| v.parse().ok()),
        ..etlp_sync::TraktIds::default()
    }
}

/// Whether any external id is present.
fn has_any_trakt_id(ids: &etlp_sync::TraktIds) -> bool {
    ids.imdb.is_some() || ids.tmdb.is_some() || ids.tvdb.is_some()
}

/// Build a [`TraktHistoryItem`] from an episode/movie's external ids, or `None`
/// when no usable id is present.
fn trakt_item_from_ids(
    kind: etlp_sync::TraktItemKind,
    provider_ids: &std::collections::BTreeMap<String, String>,
) -> Option<etlp_sync::TraktHistoryItem> {
    let ids = trakt_ids_from_provider_map(provider_ids);
    if !has_any_trakt_id(&ids) {
        return None;
    }
    Some(etlp_sync::TraktHistoryItem {
        kind,
        ids,
        episode: None,
        watched_at: None,
    })
}

/// Build the best-matching Trakt item for the just-watched entry.
///
/// For an Emby/Jellyfin episode it prefers the **show's** ids plus the
/// season/episode number — the match Trakt resolves most reliably — by fetching
/// the series' `ProviderIds` from the media server. It falls back to the item's
/// own ids (movies, or when the show ids / season / episode number are missing).
/// Returns `None` when no usable id can be found, so the caller skips the item
/// instead of sending a request that would 404.
async fn build_trakt_item(
    state: &SharedState,
    kind: etlp_sync::TraktItemKind,
    data: &PlaybackData,
) -> Option<etlp_sync::TraktHistoryItem> {
    if kind == etlp_sync::TraktItemKind::Episode
        && data.server.is_emby_like()
        && !data.series_id.is_empty()
        && let (Some(season), Some(number)) = (data.season_number, data.index)
        && let (Ok(season), Ok(number)) =
            (u32::try_from(season), u32::try_from(number))
        && let Some(show_ids) = fetch_series_trakt_ids(state, data).await
    {
        debug!(
            item = %data.media_title,
            season, number,
            "trakt: matched episode by show ids + season/number"
        );
        return Some(etlp_sync::TraktHistoryItem {
            kind,
            ids: show_ids,
            episode: Some(etlp_sync::TraktEpisode { season, number }),
            watched_at: None,
        });
    }
    let item = trakt_item_from_ids(kind, &data.provider_ids);
    match &item {
        Some(_) => debug!(
            item = %data.media_title,
            "trakt: matched by the item's own provider ids"
        ),
        None => warn!(
            item = %data.media_title,
            item_type = %data.item_type,
            "trakt: no usable tmdb/imdb/tvdb id, cannot sync this item"
        ),
    }
    item
}

/// Fetch the series' Trakt ids from the media server, or `None` when the series
/// item cannot be read or carries no usable external id.
async fn fetch_series_trakt_ids(
    state: &SharedState,
    data: &PlaybackData,
) -> Option<etlp_sync::TraktIds> {
    let base_url = format!("{}://{}", data.scheme, data.netloc);
    let emby = EmbyClient::new(
        state.http_client.clone(),
        &base_url,
        &data.api_key,
        &data.user_id,
    );
    let series = emby.item(&data.series_id).await.ok()?;
    let ids = trakt_ids_from_provider_map(&series.provider_ids);
    has_any_trakt_id(&ids).then_some(ids)
}

/// Sync all completed entries to Trakt.tv when configured.
///
/// Reads `trakt.enable_host` and `trakt.client_id` from the config; silently
/// skips when either is absent or the netloc does not match `enable_host`.
async fn sync_trakt(state: &SharedState, entries: &[SyncEntry<'_>]) {
    use etlp_sync::{
        ScrobbleAction, TraktApi, TraktHistoryItem, TraktItemKind, sync_history,
    };

    if entries.is_empty() {
        debug!("trakt: no watched entries to sync, skip");
        return;
    }

    let (
        client_id,
        client_secret,
        user_name,
        redirect_uri,
        enable_host,
        allow_duplicate,
    ) = {
        let Ok(cfg) = state.config.read() else {
            warn!("trakt: config lock poisoned, skip");
            return;
        };
        if cfg.trakt.client_id.is_empty() {
            debug!("trakt: client_id not configured, skip");
            return;
        }
        (
            cfg.trakt.client_id.clone(),
            cfg.trakt.client_secret.clone(),
            cfg.trakt.user_name.clone(),
            cfg.trakt.redirect_uri.clone(),
            cfg.trakt.enable_host.clone(),
            cfg.trakt.allow_duplicate,
        )
    };

    debug!(
        entries = entries.len(),
        enable_host = %enable_host,
        user_name = %user_name,
        "trakt: sync start"
    );

    // Skip early when nothing targets an enabled host.
    if !entries
        .iter()
        .any(|e| host_enabled(&e.data.netloc, &enable_host))
    {
        debug!("trakt: no entry matches enable_host, skipping");
        return;
    }

    let token_path = state.working_dir.join(TraktApi::TOKEN_FILE_NAME);
    let Ok(mut api) = TraktApi::new(
        &client_id,
        &client_secret,
        &user_name,
        &token_path,
        TraktApi::DEFAULT_BASE_URL,
    ) else {
        return;
    };

    match api.ensure_auth().await {
        Ok(true) => {}
        Ok(false) => {
            // No token yet (or refresh failed): open the OAuth authorize page
            // so the user can grant access. The /trakt_auth callback then
            // exchanges the code; the next playback will scrobble.
            warn!("trakt: no valid token, opening authorization page");
            let _ =
                crate::platform::open_url(&api.authorize_url(&redirect_uri));
            return;
        }
        Err(e) => {
            warn!("trakt auth failed: {e}");
            return;
        }
    }

    // Two batches, both added via `/sync/history` (no scrobble):
    // - `current_items`: the just-watched items, added directly with **no**
    //   history de-dup, so re-watching an episode records a fresh entry. The
    //   throttle above is the only gate (bypassed by `allow_duplicate`).
    // - `backfill_items`: earlier episodes the server already marks played,
    //   added through `sync_history` which de-duplicates against the existing
    //   history so they are never re-marked.
    let mut current_items: Vec<TraktHistoryItem> = Vec::new();
    let mut backfill_items: Vec<TraktHistoryItem> = Vec::new();
    for entry in entries {
        let data = entry.data;
        if !host_enabled(&data.netloc, &enable_host) {
            continue;
        }
        let kind = if data.item_type.eq_ignore_ascii_case("movie") {
            TraktItemKind::Movie
        } else if data.item_type.eq_ignore_ascii_case("episode") {
            TraktItemKind::Episode
        } else {
            continue;
        };

        // Throttle a finished item: re-watching it within the window is skipped
        // unless `allow_duplicate` is on (then every completion is reported
        // again, so re-watching e.g. episode 6 re-marks it).
        let key = format!("trakt:{}:{}", data.netloc, data.item_id);
        if entry.completed && !allow_duplicate && state.sync_recently_done(&key)
        {
            debug!(item = %data.media_title, "trakt: throttled, skip");
            continue;
        }

        // Two APIs, split by progress so each does its own job and a watch is
        // never recorded twice:
        // - Finished (≥ the completion threshold): add to `/sync/history`, the
        //   reliable "watched" mark (matches the reference implementation).
        // - Not finished: a single `scrobble/stop` at the real progress, which
        //   `/sync/history` cannot express — Trakt still scrobbles it watched
        //   at ≥ 80 %, or saves the resume position between 1 % and 79 %.
        //   Progress is floored to 1 % so Trakt does not reject it (HTTP 422).
        // build_trakt_item logs the reason when it yields None.
        if entry.completed {
            if let Some(item) = build_trakt_item(state, kind, data).await {
                info!(
                    item = %data.media_title,
                    "trakt: queued watched item for history"
                );
                current_items.push(item);
            }
        } else if let Some(mut item) = build_trakt_item(state, kind, data).await
        {
            if item.episode.is_none() && item.ids.trakt.is_none() {
                item.ids.trakt = api.resolve_trakt_id(kind, &item.ids).await;
            }
            let progress = entry.progress.max(1.0);
            match api.scrobble(ScrobbleAction::Stop, &item, progress).await {
                Ok(_) => info!(
                    item = %data.media_title,
                    progress,
                    "trakt: scrobbled stop (in-progress)"
                ),
                Err(e) => warn!("trakt scrobble stop error: {e}"),
            }
        }

        // Backfill earlier episodes the server already marks played, so the
        // first sync of a season records everything finished in the client.
        if kind == TraktItemKind::Episode {
            let cur_index = data.index.unwrap_or_default();
            for ep in &emby_played_backfill(state, data).await {
                if ep.index == cur_index {
                    continue;
                }
                let Some(mut item) = trakt_item_from_ids(
                    TraktItemKind::Episode,
                    &ep.provider_ids,
                ) else {
                    continue;
                };
                item.ids.trakt = api
                    .resolve_trakt_id(TraktItemKind::Episode, &item.ids)
                    .await;
                backfill_items.push(item);
            }
        }
    }

    // De-duplicate the backfill batch (a binged playlist can queue the same
    // earlier episode from several entries). Items with no resolved Trakt id are
    // kept; `sync_history` then de-duplicates them against the history.
    let mut seen: std::collections::HashSet<(bool, u64)> =
        std::collections::HashSet::new();
    backfill_items.retain(|item| match item.ids.trakt {
        Some(id) => seen.insert((item.kind == TraktItemKind::Movie, id)),
        None => true,
    });

    debug!(
        current = current_items.len(),
        backfill = backfill_items.len(),
        "trakt: built history items"
    );

    // Current items: added directly, no de-dup, so a re-watch records again.
    if !current_items.is_empty() {
        match api.add_to_history(&current_items).await {
            Ok(_) => {
                info!("trakt: added {} watched item(s)", current_items.len())
            }
            Err(e) => warn!("trakt history add error: {e}"),
        }
    }

    // Backfill: de-duplicated against the existing history before adding.
    if !backfill_items.is_empty() {
        match sync_history(&api, backfill_items).await {
            Ok(n) => info!("trakt: backfilled {n} item(s)"),
            Err(e) => warn!("trakt sync error: {e}"),
        }
    }
}

/// Minimum watched duration, in seconds, for the current episode to count as a
/// "real watch" worth syncing to a third-party service.
///
/// Mirrors the guard `update_progress` applies before writing back progress
/// (`|stop_sec - start_sec| >= 20`), so a momentary open-and-quit is treated as
/// noise: Bangumi does not mark such an episode watched, and Trakt does not even
/// report it as in-progress. Shared by both sync paths so the floor is identical.
const MIN_REAL_WATCH_SECS: u64 = 20;

/// Sync all completed entries to Bangumi (bgm.tv) when configured.
///
/// Reads `[bangumi]` from the config. Silently skips when `access_token` is
/// empty or the media-server host does not match any `enable_host` keyword.
/// The subject ID comes from `provider_ids["Bangumi"]`; when that is absent and
/// `title_search_fallback` is enabled, it is resolved by searching bgm by title
/// and walking the sequel chain to the season given by `season_number`. The
/// episode is matched by air date (`premiere_date`) with `index` as fallback.
/// When the token is rejected, the bgm.tv token page is opened so the user can
/// regenerate it.
async fn sync_bangumi(state: &SharedState, entries: &[SyncEntry<'_>]) {
    use etlp_sync::{BangumiApi, SubjectCache, SyncError, sync_episodes};

    if entries.is_empty() {
        return;
    }

    let (
        username,
        access_token,
        private,
        enable_host,
        genres,
        title_fallback,
        subject_map,
    ) = {
        let Ok(cfg) = state.config.read() else {
            return;
        };
        if cfg.bangumi.access_token.is_empty() {
            return;
        }
        (
            cfg.bangumi.username.clone(),
            cfg.bangumi.access_token.clone(),
            cfg.bangumi.private,
            cfg.bangumi.enable_host.clone(),
            cfg.bangumi.genres.clone(),
            cfg.bangumi.title_search_fallback,
            cfg.bangumi.subject_map.clone(),
        )
    };
    // User-pinned subject mappings take priority over auto-resolution.
    let mappings = etlp_sync::parse_mappings(&subject_map);

    debug!(
        entries = entries.len(),
        enable_host = %enable_host,
        username = %username,
        private,
        title_fallback,
        "bangumi: sync start"
    );

    // Skip early when no completed entry targets an enabled host.
    if !entries
        .iter()
        .any(|e| host_enabled(&e.data.netloc, &enable_host))
    {
        debug!("bangumi: no entry matches enable_host, skipping");
        return;
    }

    let Ok(api) = BangumiApi::new(
        &username,
        &access_token,
        private,
        BangumiApi::DEFAULT_BASE_URL,
    ) else {
        return;
    };

    // Validate the token once. On rejection, open the regeneration page so the
    // user can mint a fresh token instead of the sync silently failing.
    if let Err(e) = api.verify_token().await {
        if matches!(e, SyncError::Unauthorized) {
            warn!("bangumi: access token rejected; opening token page");
            let _ = crate::platform::open_url(BangumiApi::TOKEN_PAGE_URL);
        } else {
            warn!("bangumi: token verification failed: {e}");
        }
        return;
    }

    let bangumi_cache_dir =
        crate::platform::cache_subdir_in(&state.working_dir, "bangumi");
    let _ = std::fs::create_dir_all(&bangumi_cache_dir);
    let cache_path = bangumi_cache_dir.join(BangumiApi::SUBJECT_CACHE_FILE);
    let mut cache = SubjectCache::load(&cache_path);

    for entry in entries {
        let data = entry.data;
        if !host_enabled(&data.netloc, &enable_host) {
            continue;
        }
        // The episode index is required by both paths to address an episode.
        let Some(ep_index) = data.index else {
            debug!(item = %data.media_title, "bangumi: no episode index, skip");
            continue;
        };
        if u32::try_from(ep_index).is_err() {
            continue;
        }

        // Throttle only once the episode is finished; while it is still in
        // progress, keep reporting so newer progress always reaches Bangumi.
        let key = format!("bangumi:{}:{}", data.netloc, data.item_id);
        if entry.completed && state.sync_recently_done(&key) {
            debug!(item = %data.media_title, "bangumi: throttled, skip");
            continue;
        }

        let Some(target) = resolve_bangumi_subject(
            &api,
            data,
            title_fallback,
            &genres,
            &mappings,
            &mut cache,
            &cache_path,
        )
        .await
        else {
            continue;
        };
        let subject_id = target.subject_id;

        // Translate a local episode index to a Bangumi sort number, applying
        // the mapping's per-season offset; drops indices that fall out of range.
        let to_bgm_sort = |index: i64| -> Option<u32> {
            u32::try_from(index + target.ep_offset)
                .ok()
                .filter(|s| *s > 0)
        };

        // Bangumi has no per-episode progress, so it cannot represent a partial
        // view of an episode. Unlike Trakt (which pauses at the real progress),
        // any *real watch* of the current episode — one past the same floor the
        // progress write-back uses — is marked watched here, even below the 90 %
        // completion threshold.
        let current_watched_secs =
            (entry.stop_sec - data.start_sec).unsigned_abs();
        let current_real_watch = current_watched_secs >= MIN_REAL_WATCH_SECS;
        if !current_real_watch {
            info!(
                item = %data.media_title,
                watched_secs = current_watched_secs,
                "bangumi: watched too short (< {MIN_REAL_WATCH_SECS}s), \
                 current episode not marked"
            );
        }

        // Collect the episodes to mark watched: every earlier one the client
        // already finished, plus the current episode when it was a real watch.
        // A momentary open of the current episode is dropped so it is not marked.
        let backfill = emby_played_backfill(state, data).await;
        let mut eps: Vec<(u32, Option<String>)> = backfill
            .iter()
            .filter(|p| current_real_watch || p.index != ep_index)
            .filter_map(|p| {
                to_bgm_sort(p.index).map(|s| (s, p.premiere_date.clone()))
            })
            .collect();
        // Non-Emby servers report no backfill; fall back to the current episode
        // when it was a real watch.
        if eps.is_empty()
            && current_real_watch
            && let Some(s) = to_bgm_sort(ep_index)
        {
            eps.push((s, data.premiere_date.clone()));
        }
        eps.sort_by_key(|(s, _)| *s);
        eps.dedup_by_key(|(s, _)| *s);

        // Nothing watched to mark (only a momentary open of the current episode
        // and no earlier history): just register the subject as "watching" so
        // the season still shows up as in-progress on Bangumi.
        if eps.is_empty() {
            match api.ensure_collected_watching(subject_id).await {
                Ok(()) => info!(
                    "bangumi: subject {subject_id} marked watching ({}%)",
                    entry.progress as i64
                ),
                Err(e) => {
                    warn!("bangumi watching mark failed for {subject_id}: {e}")
                }
            }
            continue;
        }

        debug!(
            subject_id,
            ep_offset = target.ep_offset,
            count = eps.len(),
            "bangumi: syncing entry"
        );
        match sync_episodes(&api, subject_id, &eps).await {
            Ok(ids) => {
                info!(
                    "bangumi: marked {} episode(s) for subject {subject_id}",
                    ids.len()
                )
            }
            Err(e) => warn!("bangumi sync error for subject {subject_id}: {e}"),
        }
    }
}

/// A resolved Bangumi subject plus the per-season episode offset to apply when
/// marking episodes. The offset is non-zero only for user-mapping matches.
struct BangumiTarget {
    subject_id: u64,
    ep_offset: i64,
}

impl BangumiTarget {
    /// A target resolved through auto-detection (no episode offset).
    fn auto(subject_id: u64) -> Self {
        Self {
            subject_id,
            ep_offset: 0,
        }
    }
}

/// Resolve the Bangumi subject for one entry.
///
/// Resolution order: a user-pinned `subject_map` mapping (highest priority,
/// carries an episode offset) → `provider_ids["Bangumi"]` → cache → title search
/// (gated to anime by `genres`). Returns `None` when the subject cannot be
/// determined.
async fn resolve_bangumi_subject(
    api: &etlp_sync::BangumiApi,
    data: &PlaybackData,
    title_fallback: bool,
    genres: &str,
    mappings: &[etlp_sync::SubjectMapping],
    cache: &mut etlp_sync::SubjectCache,
    cache_path: &std::path::Path,
) -> Option<BangumiTarget> {
    use etlp_sync::{MapProvider, match_mapping};

    // Highest priority: an explicit user mapping pinned to this item by its
    // tmdb/imdb/tvdb id (and season for TV).
    let is_movie = data.item_type.eq_ignore_ascii_case("movie");
    let season_u32 = data.season_number.and_then(|s| u32::try_from(s).ok());
    let ids: Vec<(MapProvider, &str)> = [
        ("Tmdb", MapProvider::Tmdb),
        ("Imdb", MapProvider::Imdb),
        ("Tvdb", MapProvider::Tvdb),
    ]
    .into_iter()
    .filter_map(|(key, prov)| {
        data.provider_ids
            .get(key)
            .filter(|v| !v.is_empty())
            .map(|v| (prov, v.as_str()))
    })
    .collect();
    if let Some(m) = match_mapping(mappings, &ids, is_movie, season_u32) {
        // For TV, the offset must land the current episode on a positive
        // Bangumi sort. A non-positive result (e.g. `E-59` while watching ep 1)
        // means the mapping cannot serve this episode: skip it and fall back to
        // id/title resolution rather than erroring.
        if m.yields_positive_episode(data.index) {
            info!(
                subject_id = m.subject_id,
                ep_offset = m.ep_offset,
                "bangumi: subject from user mapping"
            );
            return Some(BangumiTarget {
                subject_id: m.subject_id,
                ep_offset: m.ep_offset,
            });
        }
        warn!(
            subject_id = m.subject_id,
            ep_offset = m.ep_offset,
            episode = data.index.unwrap_or_default(),
            "bangumi: mapping offset yields a non-positive episode; \
             skipping mapping and falling back to query"
        );
    }

    if let Some(id_str) = data.provider_ids.get("Bangumi") {
        match id_str.parse::<u64>() {
            Ok(id) => return Some(BangumiTarget::auto(id)),
            Err(_) => {
                warn!("bangumi: invalid subject id {id_str:?}, skipping");
                return None;
            }
        }
    }

    if !title_fallback {
        debug!(item = %data.media_title, "bangumi: no provider id, fallback off");
        return None;
    }
    // Only anime should reach title search; bgm's keyword index is anime-only.
    if !genres_match(&data.genres, genres) {
        debug!(item = %data.media_title, "bangumi: genres gate, skip fallback");
        return None;
    }

    let season = data.season_number.unwrap_or(1);
    if !data.series_id.is_empty()
        && let Some(id) = cache.get(&data.series_id, season)
    {
        debug!(id, "bangumi: subject from cache");
        return Some(BangumiTarget::auto(id));
    }

    let keywords: Vec<&str> =
        [data.original_title.as_str(), data.series_name.as_str()]
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .collect();
    if keywords.is_empty() {
        debug!("bangumi: no title keywords for fallback, skip");
        return None;
    }
    let target_season = u32::try_from(season.max(1)).unwrap_or(1);

    match api
        .resolve_subject_id(&keywords, target_season, BANGUMI_TITLE_MIN_SCORE)
        .await
    {
        Ok(Some(id)) => {
            info!(
                subject_id = id,
                series = %data.series_name,
                season,
                "bangumi: resolved subject via title search"
            );
            if !data.series_id.is_empty()
                && let Err(e) =
                    cache.insert(&data.series_id, season, id, cache_path)
            {
                warn!("bangumi: subject cache write failed: {e}");
            }
            Some(BangumiTarget::auto(id))
        }
        Ok(None) => {
            warn!(
                series = %data.series_name,
                season,
                "bangumi: title search found no subject"
            );
            None
        }
        Err(e) => {
            warn!("bangumi: title search failed: {e}");
            None
        }
    }
}

/// Minimum title-similarity score for accepting a Bangumi search candidate.
const BANGUMI_TITLE_MIN_SCORE: f64 = 0.6;

/// Whether any of `item_genres` matches the `|`-separated `pattern`.
///
/// Matching is case-insensitive substring on each alternative. An empty pattern
/// allows everything; an item with no genres is allowed (the genre data is
/// simply unavailable, not a negative signal).
fn genres_match(item_genres: &[String], pattern: &str) -> bool {
    let alternatives: Vec<String> = pattern
        .split('|')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if alternatives.is_empty() || item_genres.is_empty() {
        return true;
    }
    item_genres.iter().any(|g| {
        let g = g.to_lowercase();
        alternatives.iter().any(|a| g.contains(a))
    })
}

/// Returns `true` when `netloc` matches the comma-separated `enable_host`
/// keyword list. An empty list disables the feature (returns `false`).
///
/// A standalone `.` token is a wildcard meaning "every host", checked first so
/// its presence anywhere in the list short-circuits to enabled regardless of
/// the other keywords. Otherwise each non-empty keyword is matched as a
/// substring of `netloc`.
fn host_enabled(netloc: &str, enable_host: &str) -> bool {
    let mut keywords = enable_host
        .split(',')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .peekable();
    // Empty list → feature disabled.
    if keywords.peek().is_none() {
        return false;
    }
    // Wildcard has priority: any standalone "." enables every host.
    let keywords: Vec<&str> = keywords.collect();
    if keywords.contains(&".") {
        return true;
    }
    keywords.iter().any(|&k| netloc.contains(k))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tower::ServiceExt as _;

    use crate::router::build_router;
    use crate::state::test_helpers::test_state;

    #[test]
    fn host_enabled_matches_keywords() {
        // Empty list disables the feature.
        assert!(!super::host_enabled("emby.example.com:8096", ""));
        assert!(!super::host_enabled("emby.example.com:8096", "  ,  "));
        // A lone dot enables everything.
        assert!(super::host_enabled("anything", "."));
        // The dot wildcard has priority even mixed with non-matching keywords.
        assert!(super::host_enabled("10.0.0.1:8096", "localhost, ., foo"));
        // A keyword that merely contains a dot is NOT the wildcard.
        assert!(!super::host_enabled("10.0.0.1:8096", "192.168."));
        // Comma-separated keywords match as substrings, ignoring whitespace.
        assert!(super::host_enabled(
            "192.168.1.10:8096",
            "localhost, 192.168."
        ));
        assert!(!super::host_enabled("10.0.0.1:8096", "localhost, 192.168."));
    }

    #[test]
    fn genres_match_gates_title_fallback() {
        let anime = vec!["动画".to_owned(), "奇幻".to_owned()];
        let live = vec!["剧情".to_owned(), "犯罪".to_owned()];
        // Default anime pattern matches an anime item.
        assert!(super::genres_match(&anime, "动画|anime"));
        // Case-insensitive substring on each alternative.
        assert!(super::genres_match(&["Anime".to_owned()], "动画|anime"));
        // Non-anime genres are rejected.
        assert!(!super::genres_match(&live, "动画|anime"));
        // Empty pattern allows everything.
        assert!(super::genres_match(&live, ""));
        // Missing genre data is not a negative signal -> allowed.
        assert!(super::genres_match(&[], "动画|anime"));
    }

    #[tokio::test]
    async fn emby_route_returns_200() {
        let (state, _dir) = test_state();
        let app = build_router(state);
        let body = serde_json::json!({
            "playbackUrl":
                "http://emby:8096/emby/Items/1/PlaybackInfo?X-Emby-Token=tok",
            "ApiClient": {
                "_serverAddress": "http://emby:8096",
                "_serverVersion": "4.9"
            },
            "request": {"headers": {}},
            "playbackData": {"PlaySessionId": "s1", "MediaSources": []},
            "extraData": {
                "mainEpInfo": {"Id": "1"},
                "episodesInfo": [],
                "playlistInfo": []
            },
            "mountDiskEnable": "false"
        });
        let req = Request::builder()
            .method(Method::POST)
            .uri("/embyToLocalPlayer")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn plex_route_returns_200() {
        let (state, _dir) = test_state();
        let app = build_router(state);
        let body = serde_json::json!({
            "playbackUrl":
                "http://plex:32400/library/metadata/42?X-Plex-Token=t",
            "mountDiskEnable": "false",
            "playbackData": {"MediaContainer": {"Metadata": []}}
        });
        let req = Request::builder()
            .method(Method::POST)
            .uri("/plexToLocalPlayer")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn show_task_manager_returns_200() {
        let (state, _dir) = test_state();
        let app = build_router(state);
        let body = serde_json::json!({
            "showTaskManager": true,
            "playbackUrl": "",
            "playbackData": {"PlaySessionId": "", "MediaSources": []},
            "extraData": {},
            "mountDiskEnable": "false"
        });
        let req = Request::builder()
            .method(Method::POST)
            .uri("/embyToLocalPlayer")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
