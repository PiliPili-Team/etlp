//! Player orchestration layer.
//!
//! [`PlayerHandle`] is a type-erased enum over all supported player handles.
//! [`PlayerManager`] wraps a running player and coordinates:
//!   1. Waiting for the player to exit and collecting per-episode stop times.
//!   2. Writing progress back to the originating media server.
//!
//! Background loops ([`realtime_playing_feedback_loop`],
//! [`redirect_next_ep_loop`]) are spawned by [`PlayerManager::start_loops`]
//! immediately after playback begins. They are mpv-only; non-mpv players skip
//! them silently.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use etlp_core::PlaybackData;
use etlp_net::{HttpClient, PlaybackEvent, realtime_progress, update_progress};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{info, warn};

use crate::dandan::DanDanHandle;
use crate::mpc::MpcHandle;
use crate::mpv::MpvHandle;
use crate::pot::{PotHandle, stop_sec_pot};
use crate::transport::MpvClient;
use crate::vlc::VlcHandle;

// ── PlayerHandle ──────────────────────────────────────────────────────────────

/// A running player process, wrapping all supported player handle types.
///
/// Call [`PlayerHandle::stop_sec`] to block until the player exits and return
/// the last observed playback position in whole seconds.
pub enum PlayerHandle {
    Mpv(MpvHandle),
    Vlc(VlcHandle),
    Mpc(MpcHandle),
    Pot(PotHandle),
    DanDan(DanDanHandle),
    /// Test-only stub that returns `None` immediately without blocking.
    #[cfg(test)]
    Stub,
}

impl PlayerHandle {
    /// Block asynchronously until the player exits; return the last observed
    /// position in whole seconds, or `None` when it could not be determined.
    pub async fn stop_sec(&self) -> Option<i64> {
        match self {
            PlayerHandle::Mpv(h) => mpv_stop_sec(h).await,
            PlayerHandle::Vlc(h) => h.stop_sec().await,
            PlayerHandle::Mpc(h) => h.stop_sec().await,
            PlayerHandle::Pot(h) => stop_sec_pot(h.pid).await,
            PlayerHandle::DanDan(h) => h.stop_sec().await,
            #[cfg(test)]
            PlayerHandle::Stub => None,
        }
    }
}

/// Poll mpv's `time-pos` property until IPC disconnects (player closed).
async fn mpv_stop_sec(handle: &MpvHandle) -> Option<i64> {
    let mut last: Option<i64> = None;
    loop {
        match handle.time_pos().await {
            Ok(Some(pos)) if pos > 0.0 => {
                last = Some(pos as i64);
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Ok(_) => {
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
            Err(_) => {
                info!("mpv IPC disconnected, last position: {last:?}s");
                return last;
            }
        }
    }
}

// ── PlayerManager ─────────────────────────────────────────────────────────────

/// Orchestrates a running player: collects stop times and writes progress back.
pub struct PlayerManager {
    /// The running player handle.
    pub handle: PlayerHandle,
    /// Primary episode data (the first file played).
    pub data: PlaybackData,
    /// Playlist: key (media_title) → episode data.
    ///
    /// Populated by the server layer via `register_playlist`.
    pub playlist: HashMap<String, PlaybackData>,
    /// Collected stop times after the player exits: key → seconds.
    pub stop_times: HashMap<String, i64>,
    /// Per-episode total durations reported by the player (mpv only for now).
    pub total_secs: HashMap<String, i64>,
}

impl PlayerManager {
    /// Create a new manager wrapping the given player handle and primary data.
    pub fn new(handle: PlayerHandle, data: PlaybackData) -> Self {
        Self {
            handle,
            data,
            playlist: HashMap::new(),
            stop_times: HashMap::new(),
            total_secs: HashMap::new(),
        }
    }

    /// Register playlist episode data for progress tracking.
    ///
    /// The key should be `media_title` (or `basename` for players that report
    /// the filename instead of the title).
    pub fn register_playlist(&mut self, key: String, ep: PlaybackData) {
        self.playlist.insert(key, ep);
    }

    /// Wait for the player to exit, collecting the stop time for the primary
    /// episode.
    ///
    /// For multi-episode playlists, per-episode times are populated by the
    /// individual player backends (stage 3.6). This method stores the result
    /// under the primary episode's `media_title` key.
    pub async fn collect_stop_times(&mut self) {
        let stop_sec = self.handle.stop_sec().await;
        if let Some(sec) = stop_sec {
            self.stop_times.insert(self.data.media_title.clone(), sec);
        }
    }

    /// Write stop progress back to the originating media server for all
    /// episodes whose stop time was collected.
    ///
    /// Skips entries whose `stop_sec` is too close to `start_sec` (< 20 s),
    /// preventing spurious "watched" marks from accidental play-and-quit.
    pub async fn write_progress(&self, http: &HttpClient) {
        for (key, &stop_sec) in &self.stop_times {
            let ep = match self.playlist.get(key) {
                Some(ep) => ep,
                None => {
                    if key == &self.data.media_title {
                        &self.data
                    } else {
                        warn!(
                            "write_progress: no episode found for key {key:?}"
                        );
                        continue;
                    }
                }
            };

            let start_sec = ep.start_sec;
            if (stop_sec - start_sec).unsigned_abs() < 20 {
                info!(
                    "skip progress write for {:?}: stop({stop_sec}) too close \
                     to start({start_sec})",
                    ep.basename,
                );
                continue;
            }

            match update_progress(http, ep, stop_sec, false).await {
                Ok(()) => {
                    info!("progress written: {:?} @ {stop_sec}s", ep.basename)
                }
                Err(e) => {
                    warn!("progress write failed for {:?}: {e}", ep.basename)
                }
            }
        }
    }
}

impl PlayerManager {
    /// Spawn the background feedback and redirect loops for mpv.
    ///
    /// `cancel_tx`: when provided, the feedback loop sends each outgoing
    /// episode's download task-id (= `media_source_id || item_id`) through
    /// this channel so the caller can cancel in-progress downloads.
    ///
    /// Safe to call for non-mpv players: the functions detect the wrong variant
    /// and return immediately, so the spawned tasks finish instantly.
    pub fn start_loops(
        &self,
        http: HttpClient,
        cancel_tx: Option<UnboundedSender<String>>,
    ) {
        let PlayerHandle::Mpv(ref handle) = self.handle else {
            return;
        };
        let client = handle.client.clone();
        let playlist = self.playlist.clone();
        let data = self.data.clone();
        let http2 = http.clone();

        tokio::spawn(realtime_playing_feedback_loop(
            data,
            client.clone(),
            http2,
            playlist.clone(),
            cancel_tx,
        ));
        tokio::spawn(redirect_next_ep_loop(client, playlist, http));
    }
}

// ── Background loops (mpv-only) ───────────────────────────────────────────────

/// Returns the download task-id for `ep`: `media_source_id` when non-empty,
/// otherwise `item_id`. Mirrors the scheme used by the `/gui` route handler.
fn dl_task_id(ep: &PlaybackData) -> &str {
    if !ep.media_source_id.is_empty() {
        &ep.media_source_id
    } else {
        &ep.item_id
    }
}

/// Interval between realtime progress heartbeats during normal playback.
const PROGRESS_INTERVAL_SECS: u64 = 10;

/// Interval between realtime progress heartbeats while the player is paused.
///
/// Emby / Jellyfin sessions time out if they receive no heartbeat; this slower
/// cadence keeps the session alive without flooding the server.
const PAUSED_INTERVAL_SECS: u64 = 30;

/// Report realtime playback position to Emby / Jellyfin.
///
/// Sends `Start` when the playing episode changes, `Playing` for periodic
/// heartbeats, and `End` when switching away from an episode.
///
/// Heartbeat cadence:
/// - Playing: every `PROGRESS_INTERVAL_SECS` (10 s).
/// - Paused:  every `PAUSED_INTERVAL_SECS` (30 s) — keeps the Emby session
///   alive without flooding the server.
/// - Pause / resume transition: one immediate report, then the cadence for
///   the new state.
///
/// Exits when mpv's IPC disconnects. Plex and STRM media are skipped.
pub async fn realtime_playing_feedback_loop(
    data: PlaybackData,
    client: MpvClient,
    http: HttpClient,
    playlist: HashMap<String, PlaybackData>,
    cancel_tx: Option<UnboundedSender<String>>,
) {
    // STRM sentinel (total_sec == 86400) and Plex are not supported.
    if data.runtime_missing() || data.server == etlp_core::Server::Plex {
        info!("realtime_feedback: skip (STRM or Plex)");
        return;
    }

    let interval = Duration::from_secs(PROGRESS_INTERVAL_SECS);
    let paused_interval = Duration::from_secs(PAUSED_INTERVAL_SECS);
    let mut last_key: Option<String> = None;
    let mut last_ep: Option<PlaybackData> = None;
    let mut req_sec: i64 = 0;
    let mut was_paused: bool = false;

    loop {
        // Poll mpv: exit when IPC disconnects.
        let title = match client
            .command("get_property", &[serde_json::json!("media-title")])
            .await
        {
            Ok(Some(v)) => v.as_str().map(str::to_owned).unwrap_or_default(),
            _ => break,
        };
        let pos_sec: i64 = match client
            .command("get_property", &[serde_json::json!("time-pos")])
            .await
        {
            Ok(Some(v)) => v.as_f64().unwrap_or(0.0) as i64,
            _ => break,
        };
        let paused: bool = match client
            .command("get_property", &[serde_json::json!("pause")])
            .await
        {
            Ok(Some(v)) => v.as_bool().unwrap_or(false),
            _ => break,
        };

        let pause_changed = paused != was_paused;

        let ep = playlist.get(&title).or_else(|| {
            if data.media_title == title {
                Some(&data)
            } else {
                None
            }
        });
        let Some(ep) = ep else {
            tokio::time::sleep(Duration::from_millis(500)).await;
            continue;
        };

        if last_key.as_deref() != Some(&title) {
            // Episode changed: cancel stale download, send End + Start.
            if let Some(prev_ep) = last_ep.take() {
                if let Some(tx) = &cancel_tx {
                    let id = dl_task_id(&prev_ep).to_owned();
                    let _ = tx.send(id);
                }
                let _ = realtime_progress(
                    &http,
                    &prev_ep,
                    req_sec,
                    PlaybackEvent::End,
                )
                .await;
            }
            let _ = realtime_progress(&http, ep, pos_sec, PlaybackEvent::Start)
                .await;
            last_key = Some(title.clone());
            last_ep = Some(ep.clone());
            req_sec = pos_sec;
            was_paused = paused;
            tokio::time::sleep(interval).await;
            continue;
        }

        // Pause/resume transition: report exactly once, then switch to the
        // cadence appropriate for the new state (30 s paused, 10 s playing).
        if pause_changed {
            let _ =
                realtime_progress(&http, ep, pos_sec, PlaybackEvent::Playing)
                    .await;
            req_sec = pos_sec;
            was_paused = paused;
            let next = if paused { paused_interval } else { interval };
            tokio::time::sleep(next).await;
            continue;
        }
        was_paused = paused;

        // While paused: report every PAUSED_INTERVAL_SECS to keep the
        // Emby / Jellyfin session alive.
        if paused {
            let _ =
                realtime_progress(&http, ep, pos_sec, PlaybackEvent::Playing)
                    .await;
            req_sec = pos_sec;
            tokio::time::sleep(paused_interval).await;
            continue;
        }

        // Playing: report every `PROGRESS_INTERVAL_SECS` seconds unconditionally.
        let _ =
            realtime_progress(&http, ep, pos_sec, PlaybackEvent::Playing).await;
        req_sec = pos_sec;
        tokio::time::sleep(interval).await;
    }

    // Send End for the last tracked episode.
    if let (Some(ep), Some(_)) = (last_ep, last_key) {
        let _ =
            realtime_progress(&http, &ep, req_sec, PlaybackEvent::End).await;
    }

    info!("realtime_feedback loop exited");
}

/// Watch the playlist for episodes that have reached 50 % completion and
/// pre-resolve their successor's redirect URL in mpv's internal playlist.
///
/// Exits when mpv's IPC disconnects or the playlist is exhausted. Only
/// handles HTTP sources (`media_path.starts_with("http")`); local files are
/// skipped. When appending the resolved URL, the episode title is included
/// so mpv shows it in the playlist UI instead of the bare URL.
pub async fn redirect_next_ep_loop(
    client: MpvClient,
    playlist: HashMap<String, PlaybackData>,
    http: HttpClient,
) {
    if playlist.len() <= 1 {
        return;
    }

    // Ordered key list preserves the playlist insertion order.
    let keys: Vec<String> = playlist.keys().cloned().collect();
    let mut done: HashSet<String> = HashSet::new();

    'outer: loop {
        // Poll mpv: exit when IPC disconnects.
        let title_val = match client
            .command("get_property", &[serde_json::json!("media-title")])
            .await
        {
            Ok(v) => v,
            Err(_) => break,
        };
        let cur_title = title_val.and_then(|v| v.as_str().map(str::to_owned));
        let pos_sec: f64 = match client
            .command("get_property", &[serde_json::json!("time-pos")])
            .await
        {
            Ok(Some(v)) => v.as_f64().unwrap_or(0.0),
            _ => {
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        for (ep_idx, key) in keys.iter().enumerate() {
            if done.contains(key) {
                continue;
            }
            let ep = match playlist.get(key) {
                Some(e) => e,
                None => continue,
            };

            // Only redirect HTTP sources.
            if !ep.media_path.starts_with("http") {
                return;
            }

            // Check if this episode's position has passed 50 %.
            let is_current = cur_title.as_deref() == Some(key.as_str());
            if is_current && ep.total_sec > 0 {
                let progress = pos_sec / ep.total_sec as f64;
                if progress < 0.5 {
                    continue;
                }
            } else if !is_current {
                continue;
            }

            // Last entry — nothing to redirect.
            let next_idx = ep_idx + 1;
            if next_idx >= keys.len() {
                break 'outer;
            }
            let next_key = match keys.get(next_idx) {
                Some(k) => k,
                None => break 'outer,
            };
            let next_ep = match playlist.get(next_key) {
                Some(e) => e,
                None => continue,
            };

            // Find the current entry's index in mpv's actual playlist.
            let mpv_playlist = match client
                .command("get_property", &[serde_json::json!("playlist")])
                .await
            {
                Ok(Some(v)) => v,
                _ => break 'outer,
            };
            let entries: Vec<serde_json::Value> =
                match serde_json::from_value(mpv_playlist) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

            let cu_url = &ep.stream_url;
            let cu_re_url = ep.redirect_url.as_deref().unwrap_or("");

            let cur_mpv_idx = entries.iter().position(|e| {
                let filename =
                    e.get("filename").and_then(|v| v.as_str()).unwrap_or("");
                (e.get("playing").and_then(|v| v.as_bool()).unwrap_or(false)
                    || e.get("current")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false))
                    && (filename == cu_url || filename == cu_re_url)
            });

            let Some(cur_mpv_idx) = cur_mpv_idx else {
                info!(
                    "redirect_next_ep: current entry not found in mpv \
                     playlist for {key:?}"
                );
                done.insert(key.clone());
                continue;
            };
            let next_mpv_idx = cur_mpv_idx + 1;
            if next_mpv_idx >= entries.len() {
                break 'outer;
            }
            let next_filename = entries
                .get(next_mpv_idx)
                .and_then(|e| e.get("filename"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if next_filename != next_ep.stream_url {
                info!("redirect_next_ep: next entry filename mismatch, skip");
                continue;
            }

            // Resolve the redirect URL for the next episode.
            let ne_re_url =
                match http.resolve_redirect(&next_ep.stream_url).await {
                    Ok(url) => url,
                    Err(e) => {
                        warn!("redirect_next_ep: resolve_redirect failed: {e}");
                        continue;
                    }
                };

            // Append the resolved URL. Commas must be escaped in the
            // option string since they are the option separator; double
            // quotes are dropped because mpv's option parser does not
            // strip them and they would appear in the title literally.
            let escaped_title =
                next_ep.media_title.replace('"', "").replace(',', "\\,");
            let title_opts = format!(
                "force-media-title={escaped_title},\
                 osd-playing-msg={escaped_title}"
            );
            let total = entries.len() as i64;
            let ni = next_mpv_idx as i64;
            if client
                .command(
                    "loadfile",
                    &[
                        serde_json::json!(ne_re_url),
                        serde_json::json!("append"),
                        serde_json::json!(title_opts),
                    ],
                )
                .await
                .is_err()
            {
                break 'outer;
            }
            // Set the playlist entry title for the just-appended item so
            // mpv's playlist UI shows the episode name instead of the URL
            // even before that entry starts playing.
            let _ = client
                .command(
                    "set_property",
                    &[
                        serde_json::json!(format!("playlist/{total}/title")),
                        serde_json::json!(next_ep.media_title),
                    ],
                )
                .await;
            // Move the newly appended entry to position ni, pushing the
            // original (unresolved) next entry to ni+1, then remove it.
            if client
                .command(
                    "playlist-move",
                    &[serde_json::json!(total), serde_json::json!(ni)],
                )
                .await
                .is_err()
            {
                break 'outer;
            }
            if client
                .command("playlist-remove", &[serde_json::json!(ni + 1)])
                .await
                .is_err()
            {
                break 'outer;
            }

            info!("redirect_next_ep: resolved {next_key:?} → {ne_re_url}");
            done.insert(key.clone());
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    info!("redirect_next_ep loop exited");
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use etlp_core::{IntroMarkers, PlaybackData, Server, Subtitle};
    use std::collections::BTreeMap;

    fn dummy_data(media_title: &str, start_sec: i64) -> PlaybackData {
        PlaybackData {
            server: Server::Emby,
            scheme: "http".into(),
            netloc: "localhost:8096".into(),
            api_key: "key".into(),
            device_id: "dev".into(),
            client_id: None,
            play_session_id: "session".into(),
            headers: BTreeMap::new(),
            user_id: "user".into(),
            server_version: "4.8.0".into(),
            item_id: "item1".into(),
            media_source_id: "src1".into(),
            rating_key: None,
            file_path: "/mnt/video.mkv".into(),
            source_path: "/mnt/video.mkv".into(),
            basename: "video.mkv".into(),
            media_basename: "video.mkv".into(),
            stream_url: "http://localhost:8096/videos/item1/stream".into(),
            media_path: "/mnt/video.mkv".into(),
            media_title: media_title.into(),
            fake_name: String::new(),
            start_sec,
            total_sec: 3600,
            position: 0.0,
            size: 0,
            mount_disk_mode: false,
            is_multiple_episodes: false,
            is_strm: false,
            strm_direct: false,
            is_http_source: false,
            is_http_direct_strm: false,
            sub: Subtitle::default(),
            intro: IntroMarkers::default(),
            order: None,
            index: None,
            is_start_file: true,
            redirect_url: None,
            stop_sec: None,
        }
    }

    fn make_mgr(data: PlaybackData) -> PlayerManager {
        PlayerManager::new(PlayerHandle::Stub, data)
    }

    // ── collect_stop_times (unit logic, no real player) ──────────────────────

    #[test]
    fn stop_times_populated_after_collect() {
        let data = dummy_data("Anime S01E01", 0);
        let mut mgr = make_mgr(data);
        mgr.stop_times.insert("Anime S01E01".into(), 1800);
        assert_eq!(mgr.stop_times.get("Anime S01E01"), Some(&1800));
    }

    #[test]
    fn register_playlist_stores_episode() {
        let data = dummy_data("Anime S01E01", 0);
        let ep2 = dummy_data("Anime S01E02", 0);
        let mut mgr = make_mgr(data);
        mgr.register_playlist("Anime S01E02".into(), ep2);
        assert!(mgr.playlist.contains_key("Anime S01E02"));
    }

    // ── write_progress skip logic ─────────────────────────────────────────────

    #[test]
    fn skip_when_stop_too_close_to_start() {
        let data = dummy_data("Anime S01E01", 500);
        let mut mgr = make_mgr(data);
        // stop_sec=510, start_sec=500 → diff=10 < 20 → should skip
        mgr.stop_times.insert("Anime S01E01".into(), 510);
        let stop_sec =
            *mgr.stop_times.get("Anime S01E01").expect("entry present");
        let ep = &mgr.data;
        let should_skip = (stop_sec - ep.start_sec).unsigned_abs() < 20;
        assert!(should_skip, "expected skip when diff < 20s");
    }

    #[test]
    fn do_not_skip_when_stop_far_from_start() {
        let data = dummy_data("Anime S01E01", 0);
        let mut mgr = make_mgr(data);
        mgr.stop_times.insert("Anime S01E01".into(), 1800);
        let stop_sec = *mgr.stop_times.get("Anime S01E01").expect("entry");
        let ep = &mgr.data;
        let should_skip = (stop_sec - ep.start_sec).unsigned_abs() < 20;
        assert!(!should_skip, "should not skip when diff >= 20s");
    }
}
