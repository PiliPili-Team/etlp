//! Main ToLocalPlayer route handlers.
//!
//! `POST /embyToLocalPlayer` and `POST /plexToLocalPlayer` are the primary
//! entry points called by the Tampermonkey userscript. Both respond with
//! HTTP 200 immediately and then drive the full play chain in a spawned task.

use std::sync::atomic::Ordering;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use serde_json::{Value, json};
use tracing::{debug, info, warn};

use etlp_core::{PlaybackData, PlayerKind};
use etlp_media_server::{
    EmbyClient, EmbyParseConfig, ListContext, PlexParseConfig,
    PlexReceivedData, ReceivedData, assemble_episodes,
    parse_received_data_emby, parse_received_data_plex,
};
use etlp_player::{
    DanDanConfig, DanDanHandle, LaunchArgs, LoadMode, LoadOptions, MpcHandle,
    MpvHandle, PlayerHandle, PlayerManager, PotHandle, VlcHandle,
};

use crate::state::SharedState;

/// Temporary M3U8 file used to pass episode titles to mpv via `loadlist`.
/// `set_property playlist/N/title` is silently discarded by mpv (IPC bug);
/// M3U8 `#EXTINF` is the only reliable path for pre-play playlist titles.
const PLAYLIST_M3U8: &str = "etlp_playlist.m3u8";

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
        Ok(mut cfg) => {
            if let Err(e) = cfg.reload() {
                warn!("config reload failed: {e}");
            }
        }
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
    let playlist_limit = cfg.playlist.item_limit as usize;
    Some(LaunchCfg {
        player_exe,
        fullscreen,
        disable_audio,
        http_proxy,
        static_ipc,
        dandan,
        playlist_limit,
    })
}

// ── Core play chain ───────────────────────────────────────────────────────────

/// Spawn the player, manage the playlist, run progress loops, and write
/// stop-time back to the media server.
///
/// `episode_list` must include the currently playing episode (at any index).
/// `player_running` must be `true` before calling; this function always
/// resets it to `false` before returning.
async fn run_player_chain(
    state: SharedState,
    data: PlaybackData,
    episode_list: Vec<PlaybackData>,
) {
    let cfg = match read_launch_cfg(&state) {
        Some(c) => c,
        None => {
            warn!("run_player_chain: config lock poisoned");
            state.player_running.store(false, Ordering::Release);
            return;
        }
    };

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

            let m3u8_path = std::env::temp_dir().join(PLAYLIST_M3U8);
            let mut m3u8 = String::from("#EXTM3U\n");
            for ep in &episode_list {
                let title = ep.media_title.replace(['\n', '\r'], " ");
                m3u8.push_str(&format!(
                    "#EXTINF:-1,{title}\n{}\n",
                    ep.stream_url
                ));
            }
            match std::fs::write(&m3u8_path, &m3u8) {
                Ok(()) => {
                    debug!(
                        path = %m3u8_path.display(),
                        entries = episode_list.len(),
                        cur_idx,
                        "M3U8 playlist written for launch"
                    );
                    (m3u8_path.display().to_string(), Some(cur_idx), cur_idx)
                }
                Err(e) => {
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
        mpv_log_file: kind
            .is_mpv_family()
            .then(|| state.working_dir.join("mpv.log")),
    };

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

    let handle = match handle_result {
        Ok(h) => h,
        Err(e) => {
            warn!("player spawn failed ({kind:?}): {e}");
            state.player_running.store(false, Ordering::Release);
            return;
        }
    };

    let mut mgr = PlayerManager::new(handle, data.clone());

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

            let m3u8_path = std::env::temp_dir().join(PLAYLIST_M3U8);
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

    state.player_running.store(false, Ordering::Release);
}

// ── Emby / Jellyfin play ──────────────────────────────────────────────────────

async fn start_emby_play(state: SharedState, received: ReceivedData) {
    debug!(
        mount_disk_enable = %received.mount_disk_enable,
        playlist_info_len = received.extra_data.playlist_info.len(),
        episodes_info_len = received.extra_data.episodes_info.len(),
        "emby play chain starting",
    );
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
            return;
        }
    };

    if data.device_id.is_empty() {
        data.device_id = state.device_id.clone();
    }

    if state
        .player_running
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        warn!(
            "player already running (one_instance_mode); \
             skipping item_id={}",
            data.item_id
        );
        return;
    }

    info!(
        "starting play: server={} item_id={} file={:?}",
        data.server.as_str(),
        data.item_id,
        data.file_path,
    );

    let episode_list =
        build_emby_playlist(&state, &received, &data, &parse_cfg)
            .await
            .unwrap_or_else(|| vec![data.clone()]);

    run_player_chain(state, data, episode_list).await;
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
    let fetched = match emby.episodes(show_id, season_id).await {
        Ok(list) => list,
        Err(e) => {
            warn!("fetch episodes for {show_id}: {e}");
            return None;
        }
    };
    debug!(
        fetched_count = fetched.items.len(),
        "build_emby_playlist: fetched from server",
    );
    let ctx = ListContext {
        base: data,
        episodes_info: &received.extra_data.episodes_info,
        season_id: season_id.unwrap_or(""),
        playlist: !received.extra_data.playlist_info.is_empty(),
        config: parse_cfg,
    };
    let episodes = assemble_episodes(&ctx, &fetched.items);
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

    let items = match parse_received_data_plex(&received, &plex_cfg) {
        Ok(v) => v,
        Err(e) => {
            warn!("parse_received_data_plex: {e}");
            return;
        }
    };

    let mut items_iter = items.into_iter();
    let Some(data) = items_iter.next() else {
        warn!("plex payload contained no items");
        return;
    };
    let episode_list: Vec<PlaybackData> =
        std::iter::once(data.clone()).chain(items_iter).collect();

    if state
        .player_running
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        warn!(
            "player already running; skipping plex item_id={}",
            data.item_id
        );
        return;
    }

    info!(
        "starting plex play: item_id={} file={:?}",
        data.item_id, data.file_path,
    );

    run_player_chain(state, data, episode_list).await;
}

// ── Trakt / Bangumi sync helpers ──────────────────────────────────────────────

/// Sync all completed entries to Trakt.tv when configured.
///
/// Reads `trakt.enable_host` and `trakt.client_id` from the config; silently
/// skips when either is absent or the netloc does not match `enable_host`.
async fn sync_trakt(state: &SharedState, entries: &[(i64, &PlaybackData)]) {
    use etlp_sync::{
        TraktApi, TraktHistoryItem, TraktIds, TraktItemKind, sync_history,
    };

    if entries.is_empty() {
        return;
    }

    let (client_id, client_secret, enable_host) = {
        let Ok(cfg) = state.config.read() else {
            return;
        };
        if cfg.trakt.client_id.is_empty() {
            return;
        }
        (
            cfg.trakt.client_id.clone(),
            cfg.trakt.client_secret.clone(),
            cfg.trakt.enable_host.clone(),
        )
    };

    let token_path = state.working_dir.join("trakt_token.json");
    let Ok(mut api) = TraktApi::new(
        &client_id,
        &client_secret,
        "",
        &token_path,
        "https://api.trakt.tv",
    ) else {
        return;
    };

    if let Err(e) = api.ensure_auth().await {
        warn!("trakt auth failed: {e}");
        return;
    }

    let items: Vec<TraktHistoryItem> = entries
        .iter()
        .filter(|(_, data)| {
            !enable_host.is_empty() && data.netloc.contains(&enable_host)
        })
        .filter_map(|(_, data)| {
            let kind = if data.item_type.eq_ignore_ascii_case("movie") {
                TraktItemKind::Movie
            } else if data.item_type.eq_ignore_ascii_case("episode") {
                TraktItemKind::Episode
            } else {
                return None;
            };
            let ids = TraktIds {
                imdb: data.provider_ids.get("Imdb").cloned(),
                tmdb: data
                    .provider_ids
                    .get("Tmdb")
                    .and_then(|v| v.parse().ok()),
                tvdb: data
                    .provider_ids
                    .get("Tvdb")
                    .and_then(|v| v.parse().ok()),
                ..TraktIds::default()
            };
            Some(TraktHistoryItem {
                kind,
                ids,
                watched_at: None,
            })
        })
        .collect();

    if items.is_empty() {
        return;
    }

    match sync_history(&api, items).await {
        Ok(n) => info!("trakt: synced {n} item(s)"),
        Err(e) => warn!("trakt sync error: {e}"),
    }
}

/// Sync all completed entries to Bangumi (bgm.tv) when configured.
///
/// Reads `bangumi.access_token` from the config; silently skips when absent.
/// Uses `provider_ids["Bangumi"]` as the subject ID and `index` as episode
/// sort number.
async fn sync_bangumi(state: &SharedState, entries: &[(i64, &PlaybackData)]) {
    use etlp_sync::{BangumiApi, sync_episode_by_bangumi_id};

    if entries.is_empty() {
        return;
    }

    let access_token = {
        let Ok(cfg) = state.config.read() else {
            return;
        };
        match cfg.bangumi.access_token.clone() {
            Some(t) if !t.is_empty() => t,
            _ => return,
        }
    };

    let Ok(api) = BangumiApi::new("", &access_token, "https://api.bgm.tv")
    else {
        return;
    };

    for (_, data) in entries {
        let Some(subject_id_str) = data.provider_ids.get("Bangumi") else {
            continue;
        };
        let Ok(subject_id) = subject_id_str.parse::<u64>() else {
            continue;
        };
        let Some(ep_index) = data.index else {
            continue;
        };
        let Ok(sort) = u32::try_from(ep_index) else {
            continue;
        };
        match sync_episode_by_bangumi_id(&api, subject_id, &[sort]).await {
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tower::ServiceExt as _;

    use crate::router::build_router;
    use crate::state::test_helpers::test_state;

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
