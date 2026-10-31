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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use etlp_core::PlaybackData;
use etlp_net::{
    HttpClient, PlaybackEvent, mark_as_played, realtime_progress,
    update_progress,
};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, info, warn};

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
    /// OS process id of the underlying player, when one is tracked.
    ///
    /// Used by the server layer to raise the player window to the foreground
    /// after launch. Returns `None` for players launched via a detached
    /// protocol handler (e.g. DandanPlay over HTTP on Windows) and for the
    /// test stub.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        match self {
            PlayerHandle::Mpv(h) => h.pid(),
            PlayerHandle::Vlc(h) => h.pid(),
            PlayerHandle::Mpc(h) => h.pid(),
            PlayerHandle::Pot(h) => Some(h.pid),
            PlayerHandle::DanDan(h) => h.pid(),
            #[cfg(test)]
            PlayerHandle::Stub => None,
        }
    }

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
            // `>= 0`: a seek back to the very start must overwrite a stale
            // later position, otherwise the old (possibly >80%) value wins.
            Ok(Some(pos)) if pos >= 0.0 => {
                last = Some(pos as i64);
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Ok(_) => {
                // When time-pos is unavailable because mpv reached EOF,
                // duration represents the true end of the video.
                if let Ok(Some(true)) = handle.eof_reached().await
                    && let Ok(Some(dur)) = handle.duration().await
                {
                    last = Some(dur as i64);
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
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
    /// Latest playback position of every episode seen while an mpv playlist
    /// advanced, keyed by media-title. Populated live by
    /// [`playlist_completion_loop`]; merged into `stop_times` once the player
    /// exits so continuously-played later episodes are written back and synced,
    /// not only the episode the user opened.
    playlist_stops: Arc<Mutex<HashMap<String, i64>>>,
    /// Episodes that were *left* (switched away from) at or past the completion
    /// threshold during this playlist session, keyed by media-title, with the
    /// position they were left at.
    ///
    /// Populated by [`playlist_completion_loop`] at the moment the playlist
    /// moves to another entry. When the user later revisits such an episode
    /// and quits at a low position, the remembered completed position is used
    /// instead, so the episode is still written back as played and synced to
    /// Trakt/Bangumi as watched (instead of being reset / scrobbled as 1 %).
    completed_episodes: Arc<Mutex<HashMap<String, i64>>>,
    /// Active episode currently playing in mpv, updated live by loops.
    active_episode: Arc<Mutex<Option<String>>>,
    /// Set to `true` by the realtime feedback loop when it successfully sends
    /// the initial `Sessions/Playing` (Start) event. `write_progress` reads
    /// this to avoid sending a redundant Start when the loop already ran.
    realtime_started: Arc<AtomicBool>,
    /// When `true`, skip realtime progress reports and `update_progress`; only
    /// call `mark_as_played` after the completion threshold is reached.
    pub disable_progress_report: bool,
}

impl PlayerManager {
    /// Create a new manager wrapping the given player handle and primary data.
    pub fn new(handle: PlayerHandle, data: PlaybackData) -> Self {
        let initial_active = Some(data.media_title.clone());
        Self {
            handle,
            data,
            playlist: HashMap::new(),
            stop_times: HashMap::new(),
            total_secs: HashMap::new(),
            playlist_stops: Arc::new(Mutex::new(HashMap::new())),
            completed_episodes: Arc::new(Mutex::new(HashMap::new())),
            active_episode: Arc::new(Mutex::new(initial_active)),
            realtime_started: Arc::new(AtomicBool::new(false)),
            disable_progress_report: false,
        }
    }

    /// Register playlist episode data for progress tracking.
    ///
    /// The key should be `media_title` (or `basename` for players that report
    /// the filename instead of the title).
    pub fn register_playlist(&mut self, key: String, ep: PlaybackData) {
        self.playlist.insert(key, ep);
    }

    /// Wait for the player to exit, then collect every watched episode's stop
    /// time.
    ///
    /// For mpv playlists, [`playlist_completion_loop`] records each episode's
    /// last position live; this method merges those: the opened (primary)
    /// episode is always included, and any *later* episode is included only when
    /// watched past [`PLAYBACK_COMPLETION_RATIO`] so skipped entries are ignored.
    /// For single-file playback (no playlist loop) it falls back to the precise
    /// final position reported by the player backend, keyed by the primary
    /// episode's `media_title`.
    pub async fn collect_stop_times(&mut self) {
        // Blocks until the player exits.
        let final_stop = self.handle.stop_sec().await;

        let active_title =
            self.active_episode.lock().ok().and_then(|m| m.clone());

        // Update active episode position with final_stop if reported so seeks
        // right before player exit are never lost to loop polling latency.
        if let (Some(title), Some(sec)) = (&active_title, final_stop) {
            if let Ok(mut stops) = self.playlist_stops.lock() {
                stops.insert(title.clone(), sec);
            }
            if let Some(ep) = self.playlist.get(title)
                && ep.is_complete_at(sec)
                && let Ok(mut comp) = self.completed_episodes.lock()
            {
                let entry = comp.entry(title.clone()).or_insert(sec);
                *entry = (*entry).max(sec);
            }
        }

        // Merge per-episode positions captured while the playlist advanced.
        let captured: Vec<(String, i64)> = self
            .playlist_stops
            .lock()
            .map(|m| m.iter().map(|(k, &v)| (k.clone(), v)).collect())
            .unwrap_or_default();

        let have_playlist_data = !captured.is_empty();
        let historically_completed = self
            .completed_episodes
            .lock()
            .map(|m| m.clone())
            .unwrap_or_default();
        for (title, pos) in captured {
            let ep = self.playlist.get(&title);
            // The opened episode. Matched by item id as well: for remote
            // `.strm` versions the playlist title can differ from the title
            // parsed from the click payload, and a title-only match would
            // drop the opened episode's progress entirely.
            let is_primary = title == self.data.media_title
                || ep.is_some_and(|e| e.item_id == self.data.item_id);
            let is_active_at_exit = active_title.as_deref() == Some(&title);
            let completed = ep.is_some_and(|e| e.is_complete_at(pos));
            let historical = historically_completed.get(&title).copied();
            if is_primary
                || is_active_at_exit
                || completed
                || historical.is_some()
            {
                let effective = effective_stop_sec(ep, pos, historical);
                self.stop_times.insert(title, effective);
            }
        }

        // Single-file playback: no playlist loop ran, so use the precise final
        // position for the opened episode.
        if !have_playlist_data && let Some(sec) = final_stop {
            self.stop_times.insert(self.data.media_title.clone(), sec);
        }
    }

    /// Write stop progress back to the originating media server for all
    /// episodes whose stop time was collected.
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

            if self.disable_progress_report {
                if ep.is_complete_at(stop_sec) {
                    // Watched past the completion threshold: mark as played.
                    match mark_as_played(http, ep).await {
                        Ok(()) => info!(
                            "marked as played: {:?} @ {stop_sec}s",
                            ep.basename
                        ),
                        Err(e) => warn!(
                            "mark_as_played failed for {:?}: {e}",
                            ep.basename
                        ),
                    }
                } else {
                    // Below the completion threshold: report final position.
                    match update_progress(http, ep, stop_sec, false).await {
                        Ok(()) => info!(
                            "progress written (dpr): {:?} @ {stop_sec}s",
                            ep.basename
                        ),
                        Err(e) => warn!(
                            "progress write failed for {:?}: {e}",
                            ep.basename
                        ),
                    }
                }
                continue;
            }

            let update_success = self.realtime_started.load(Ordering::Acquire);
            match update_progress(http, ep, stop_sec, update_success).await {
                Ok(()) => {
                    info!("progress written: {:?} @ {stop_sec}s", ep.basename)
                }
                Err(e) => {
                    warn!("progress write failed for {:?}: {e}", ep.basename)
                }
            }

            if ep.is_complete_at(stop_sec) {
                match mark_as_played(http, ep).await {
                    Ok(()) => info!(
                        "marked as played: {:?} @ {stop_sec}s",
                        ep.basename
                    ),
                    Err(e) => warn!(
                        "mark_as_played failed for {:?}: {e}",
                        ep.basename
                    ),
                }
            }
        }
    }
    /// Return a [`SyncEntry`] for every episode whose stop time was collected.
    ///
    /// Used by the Trakt/Bangumi sync layer after `write_progress`. Each entry
    /// carries the watched percentage and a `completed` flag so the
    /// sync layer can distinguish "watched" from merely "in-progress".
    #[must_use]
    pub fn completed_entries(&self) -> Vec<SyncEntry<'_>> {
        self.stop_times
            .iter()
            .map(|(key, &stop_sec)| {
                // Fall back to primary data for single-episode playback.
                let ep = self.playlist.get(key).unwrap_or(&self.data);
                let progress = ep.progress_percent(stop_sec);
                let is_historical_completed = self
                    .completed_episodes
                    .lock()
                    .map(|m| m.contains_key(key))
                    .unwrap_or(false);
                SyncEntry {
                    stop_sec,
                    progress,
                    completed: PlaybackData::is_complete_percent(progress)
                        || is_historical_completed,
                    data: ep,
                }
            })
            .collect()
    }
}

/// A watched episode/movie surfaced to the third-party (Trakt/Bangumi) sync
/// layer after playback ends.
///
/// `completed` mirrors the `mark_as_played` threshold: when the viewer reached
/// the completion threshold the item is marked watched, otherwise it is
/// reported as in-progress ("watching" / "Up Next") only.
#[derive(Debug, Clone, Copy)]
pub struct SyncEntry<'a> {
    /// Final playback position in whole seconds.
    pub stop_sec: i64,
    /// Watched percentage in the range `0.0..=100.0`.
    pub progress: f64,
    /// Whether `progress` reached the completion threshold.
    pub completed: bool,
    /// The episode/movie metadata.
    pub data: &'a PlaybackData,
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
        let realtime_started = self.realtime_started.clone();
        let disable_progress_report = self.disable_progress_report;

        if !disable_progress_report {
            tokio::spawn(realtime_playing_feedback_loop(
                data,
                client.clone(),
                http2,
                playlist.clone(),
                cancel_tx,
                realtime_started,
                self.playlist_stops.clone(),
                self.completed_episodes.clone(),
            ));
        }
        tokio::spawn(force_playlist_title_loop(client.clone()));
        // Always track per-episode stop positions so continuously-played later
        // episodes are written back / synced, independent of progress reporting.
        tokio::spawn(playlist_completion_loop(
            client.clone(),
            self.playlist.clone(),
            self.playlist_stops.clone(),
            self.completed_episodes.clone(),
            self.active_episode.clone(),
        ));
        tokio::spawn(redirect_next_ep_loop(client, playlist, http));
    }
}

/// Position to write back / sync for an episode whose last observed position
/// is `pos`.
///
/// When the last visit itself reached the completion threshold, `pos` is used.
/// Otherwise, if an earlier visit in this session left the episode completed
/// (`historical`), that completed position wins, so revisiting a finished
/// episode and quitting near the start neither resets the media server's
/// played state nor downgrades the Trakt/Bangumi sync to "in progress".
fn effective_stop_sec(
    ep: Option<&PlaybackData>,
    pos: i64,
    historical: Option<i64>,
) -> i64 {
    let last_visit_completed = ep.is_some_and(|e| e.is_complete_at(pos));
    match historical {
        Some(done) if !last_visit_completed => done.max(pos),
        _ => pos,
    }
}

/// Tracks playlist entry switches for [`playlist_completion_loop`].
///
/// Each poll records the current entry's position into `stops`. When the
/// playing entry changes, the entry being left is judged on the position it
/// was *left at*: only if that reaches the completion threshold is it added to
/// `completed`. Watching past 80 % and then seeking back before leaving
/// therefore does not count, while leaving at 80 %+ is remembered even if the
/// episode is revisited later.
#[derive(Debug, Default)]
struct VisitTracker {
    current: Option<String>,
}

impl VisitTracker {
    fn observe(
        &mut self,
        title: &str,
        pos_sec: Option<i64>,
        playlist: &HashMap<String, PlaybackData>,
        stops: &Mutex<HashMap<String, i64>>,
        completed: &Mutex<HashMap<String, i64>>,
    ) {
        // Ignore the brief window before mpv reports a known title.
        if !playlist.contains_key(title) {
            return;
        }
        if self.current.as_deref() != Some(title) {
            if let Some(prev) = self.current.take() {
                Self::settle(&prev, playlist, stops, completed);
            }
            self.current = Some(title.to_owned());
        }
        if let Some(pos) = pos_sec.filter(|p| *p >= 0)
            && let Ok(mut map) = stops.lock()
        {
            map.insert(title.to_owned(), pos);
        }
    }

    /// Judge the entry that was left using its last recorded position.
    fn settle(
        title: &str,
        playlist: &HashMap<String, PlaybackData>,
        stops: &Mutex<HashMap<String, i64>>,
        completed: &Mutex<HashMap<String, i64>>,
    ) {
        let Some(left_at) =
            stops.lock().ok().and_then(|m| m.get(title).copied())
        else {
            return;
        };
        if playlist
            .get(title)
            .is_some_and(|ep| ep.is_complete_at(left_at))
            && let Ok(mut map) = completed.lock()
        {
            let entry = map.entry(title.to_owned()).or_insert(left_at);
            *entry = (*entry).max(left_at);
            debug!(title, left_at, "playlist: episode left completed");
        }
    }
}

/// Record the latest playback position of every episode an mpv playlist visits,
/// keyed by media-title, into `stops`, and remember episodes that were left at
/// or past the completion threshold in `completed`.
///
/// Runs for multi-episode playlists only and polls until mpv's IPC disconnects.
/// Unlike [`realtime_playing_feedback_loop`] it sends nothing to the media
/// server, so it runs even when progress reporting is disabled. The captured
/// positions are merged into `stop_times` by [`PlayerManager::collect_stop_times`]
/// after the player exits.
pub async fn playlist_completion_loop(
    client: MpvClient,
    playlist: HashMap<String, PlaybackData>,
    stops: Arc<Mutex<HashMap<String, i64>>>,
    completed: Arc<Mutex<HashMap<String, i64>>>,
    active_episode: Arc<Mutex<Option<String>>>,
) {
    use serde_json::json;

    if playlist.len() <= 1 {
        return;
    }

    let mut tracker = VisitTracker::default();
    loop {
        let title = match client
            .command("get_property", &[json!("media-title")])
            .await
        {
            Ok(Some(v)) => v.as_str().map(str::to_owned).unwrap_or_default(),
            Ok(None) => String::new(),
            Err(_) => break,
        };
        // `None` while mpv has no position yet (file still loading); a real
        // `0` (seeked back to the start) is kept so it overwrites stale values.
        let mut pos_sec: Option<i64> =
            match client.command("get_property", &[json!("time-pos")]).await {
                Ok(Some(v)) => v.as_f64().map(|p| p as i64),
                Ok(None) => None,
                Err(_) => break,
            };

        if pos_sec.is_none() {
            let eof = client
                .command("get_property", &[json!("eof-reached")])
                .await
                .ok()
                .flatten()
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if eof && let Some(ep) = playlist.get(&title) {
                pos_sec = Some(ep.total_sec);
            }
        }

        if !title.is_empty()
            && playlist.contains_key(&title)
            && let Ok(mut act) = active_episode.lock()
        {
            *act = Some(title.clone());
        }

        tracker.observe(&title, pos_sec, &playlist, &stops, &completed);

        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    if let Some(last) = tracker.current.take() {
        VisitTracker::settle(&last, &playlist, &stops, &completed);
    }
}

/// Force the current playlist entry's title to win over the file's embedded
/// `title` metadata.
///
/// mpv resolves `media-title` as `force-media-title` → embedded metadata title →
/// playlist entry title → filename. When a stream carries its own `title` tag it
/// therefore overrides the episode name we put in the `#EXTINF` line, so the OSD
/// and window show the wrong title and the progress loop (which keys episodes by
/// `media-title`) fails to match. This loop watches `playlist-pos` and, on every
/// entry change, copies that entry's playlist title into `force-media-title`,
/// guaranteeing the playlist name is what mpv displays and reports.
///
/// Exits when mpv's IPC disconnects.
pub async fn force_playlist_title_loop(client: MpvClient) {
    use serde_json::json;

    let mut last_pos: i64 = -1;
    loop {
        let pos = match client
            .command("get_property", &[json!("playlist-pos")])
            .await
        {
            Ok(Some(v)) => v.as_i64().unwrap_or(-1),
            Ok(None) => -1,
            Err(_) => break,
        };

        if pos >= 0 && pos != last_pos {
            // Read the title mpv parsed from the playlist's #EXTINF line.
            let title = client
                .command(
                    "get_property",
                    &[json!(format!("playlist/{pos}/title"))],
                )
                .await
                .ok()
                .flatten()
                .and_then(|v| v.as_str().map(str::to_owned))
                .filter(|s| !s.is_empty());

            if let Some(title) = title {
                let _ = client
                    .command(
                        "set_property",
                        &[json!("force-media-title"), json!(title)],
                    )
                    .await;
            }
            last_pos = pos;
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
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

/// Maximum continuous pause duration before heartbeats are suppressed.
///
/// After this many seconds without resuming, the user is considered AFK and
/// the server is no longer notified until playback resumes.
const PAUSE_TIMEOUT_SECS: u64 = 10 * 60;

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
/// - Long pause: after `PAUSE_TIMEOUT_SECS` (10 min) of continuous pause,
///   heartbeats are suppressed entirely. mpv is still polled at the paused
///   cadence so resume is detected promptly; reporting resumes on the next
///   play event.
///
/// Exits when mpv's IPC disconnects. Plex and STRM media are skipped.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::collapsible_if)]
pub async fn realtime_playing_feedback_loop(
    data: PlaybackData,
    client: MpvClient,
    http: HttpClient,
    playlist: HashMap<String, PlaybackData>,
    cancel_tx: Option<UnboundedSender<String>>,
    realtime_started: Arc<AtomicBool>,
    stops: Arc<Mutex<HashMap<String, i64>>>,
    completed_eps: Arc<Mutex<HashMap<String, i64>>>,
) {
    // STRM sentinel (total_sec == 86400) and Plex are not supported.
    if data.runtime_missing() || data.server == etlp_core::Server::Plex {
        info!("realtime_feedback: skip (STRM or Plex)");
        return;
    }

    let interval = Duration::from_secs(PROGRESS_INTERVAL_SECS);
    let paused_interval = Duration::from_secs(PAUSED_INTERVAL_SECS);
    let poll_interval = Duration::from_millis(500);
    let mut last_key: Option<String> = None;
    let mut last_ep: Option<PlaybackData> = None;
    let mut req_sec: i64 = 0;
    let mut was_paused: bool = false;
    let mut last_heartbeat = std::time::Instant::now();
    // Tracks when the current pause started; None while playing.
    let mut pause_started_at: Option<std::time::Instant> = None;

    loop {
        // Poll mpv: only Err means IPC disconnected. Ok(None) means the
        // property is not yet available (mpv still loading) — treat as a
        // safe default and retry rather than exiting the loop prematurely.
        let title = match client
            .command("get_property", &[serde_json::json!("media-title")])
            .await
        {
            Ok(Some(v)) => v.as_str().map(str::to_owned).unwrap_or_default(),
            Ok(None) => String::new(),
            Err(_) => break,
        };
        let pos_sec: i64 = match client
            .command("get_property", &[serde_json::json!("time-pos")])
            .await
        {
            Ok(Some(v)) => v.as_f64().unwrap_or(0.0) as i64,
            Ok(None) => 0,
            Err(_) => break,
        };
        let paused: bool = match client
            .command("get_property", &[serde_json::json!("pause")])
            .await
        {
            Ok(Some(v)) => v.as_bool().unwrap_or(false),
            Ok(None) => false,
            Err(_) => break,
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
            tokio::time::sleep(poll_interval).await;
            continue;
        };

        if last_key.as_deref() != Some(&title) {
            // Episode changed: cancel stale download, send End + Start.
            if let Some(prev_ep) = last_ep.take() {
                if let Some(tx) = &cancel_tx {
                    let id = dl_task_id(&prev_ep).to_owned();
                    let _ = tx.send(id);
                }
                let mut final_req_sec = req_sec;
                if let Some(prev_title) = &last_key {
                    // Prefer the 250 ms-resolution position from the completion
                    // loop over the last heartbeat.
                    if let Some(saved_pos) = stops
                        .lock()
                        .ok()
                        .and_then(|m| m.get(prev_title).copied())
                    {
                        final_req_sec = saved_pos;
                    }
                    // Leaving a revisited episode that an earlier visit
                    // finished: do not reset its played state on the server.
                    let historical = completed_eps
                        .lock()
                        .ok()
                        .and_then(|m| m.get(prev_title).copied());
                    final_req_sec = effective_stop_sec(
                        Some(&prev_ep),
                        final_req_sec,
                        historical,
                    );
                }
                let _ = realtime_progress(
                    &http,
                    &prev_ep,
                    final_req_sec,
                    PlaybackEvent::End,
                )
                .await;
                if prev_ep.is_complete_at(final_req_sec) {
                    let _ = mark_as_played(&http, &prev_ep).await;
                }
            }
            let _ = realtime_progress(&http, ep, pos_sec, PlaybackEvent::Start)
                .await;
            realtime_started.store(true, Ordering::Release);
            last_key = Some(title.clone());
            last_ep = Some(ep.clone());
            req_sec = pos_sec;
            was_paused = paused;
            last_heartbeat = std::time::Instant::now();
            tokio::time::sleep(poll_interval).await;
            continue;
        }

        // Pause/resume transition: report exactly once, then switch cadence.
        if pause_changed {
            if paused {
                pause_started_at = Some(std::time::Instant::now());
            } else {
                pause_started_at = None;
            }
            let _ =
                realtime_progress(&http, ep, pos_sec, PlaybackEvent::Playing)
                    .await;
            req_sec = pos_sec;
            was_paused = paused;
            last_heartbeat = std::time::Instant::now();
            tokio::time::sleep(poll_interval).await;
            continue;
        }
        was_paused = paused;

        // While paused: report every PAUSED_INTERVAL_SECS unless the pause
        // has lasted longer than PAUSE_TIMEOUT_SECS, in which case heartbeats
        // are suppressed until the user resumes playback.
        if paused {
            let timed_out = pause_started_at
                .map(|t| t.elapsed().as_secs() >= PAUSE_TIMEOUT_SECS)
                .unwrap_or(false);
            if !timed_out && last_heartbeat.elapsed() >= paused_interval {
                let _ = realtime_progress(
                    &http,
                    ep,
                    pos_sec,
                    PlaybackEvent::Playing,
                )
                .await;
                req_sec = pos_sec;
                last_heartbeat = std::time::Instant::now();
            }
            tokio::time::sleep(poll_interval).await;
            continue;
        }

        // Playing: detect seek (> 3s jump) or periodic heartbeat cadence.
        let seeked = (pos_sec - req_sec).abs() > 3;
        let heartbeat_due = last_heartbeat.elapsed() >= interval;
        if seeked || heartbeat_due {
            let _ =
                realtime_progress(&http, ep, pos_sec, PlaybackEvent::Playing)
                    .await;
            req_sec = pos_sec;
            last_heartbeat = std::time::Instant::now();
        }
        tokio::time::sleep(poll_interval).await;
    }

    // The final Sessions/Playing/Stopped is sent by write_progress, which
    // uses the position captured by mpv_stop_sec (polled at 500 ms) for
    // higher accuracy than the last heartbeat position here.
    let _ = last_ep;
    let _ = last_key;

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

    // Sort keys by PlaybackData::order so the sequence matches the mpv
    // playlist order. HashMap iteration order is non-deterministic; using
    // it directly causes next-episode lookups to pick the wrong entry,
    // leading to a permanent "filename mismatch" on every 5-second poll.
    let mut keys: Vec<String> = playlist.keys().cloned().collect();
    keys.sort_by_key(|k| {
        playlist.get(k).and_then(|e| e.order).unwrap_or(i64::MAX)
    });
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
                debug!(
                    next_key = ?next_key,
                    expected = %next_ep.stream_url,
                    got = %next_filename,
                    "redirect_next_ep: next entry filename mismatch, skip"
                );
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
            item_id: format!("item-{media_title}"),
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
            item_type: String::new(),
            provider_ids: BTreeMap::new(),
            series_id: String::new(),
            series_name: String::new(),
            original_title: String::new(),
            item_name: String::new(),
            season_number: None,
            premiere_date: None,
            genres: Vec::new(),
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
    fn stub_handle_reports_no_pid() {
        let data = dummy_data("Anime S01E01", 0);
        let mgr = make_mgr(data);
        // The test stub tracks no OS process, so window activation is skipped.
        assert_eq!(mgr.handle.pid(), None);
    }

    #[test]
    fn register_playlist_stores_episode() {
        let data = dummy_data("Anime S01E01", 0);
        let ep2 = dummy_data("Anime S01E02", 0);
        let mut mgr = make_mgr(data);
        mgr.register_playlist("Anime S01E02".into(), ep2);
        assert!(mgr.playlist.contains_key("Anime S01E02"));
    }

    // ── collect_stop_times playlist merge ─────────────────────────────────────

    #[tokio::test]
    async fn collect_merges_completed_playlist_episodes() {
        let completion_sec =
            (3600.0 * etlp_core::PLAYBACK_COMPLETION_RATIO).round() as i64;
        let mut mgr = make_mgr(dummy_data("Anime S01E01", 0));
        mgr.register_playlist(
            "Anime S01E01".into(),
            dummy_data("Anime S01E01", 0),
        );
        mgr.register_playlist(
            "Anime S01E02".into(),
            dummy_data("Anime S01E02", 0),
        );
        mgr.register_playlist(
            "Anime S01E03".into(),
            dummy_data("Anime S01E03", 0),
        );

        {
            let mut stops = mgr.playlist_stops.lock().unwrap();
            stops.insert("Anime S01E01".into(), 1000); // primary: kept regardless
            stops.insert("Anime S01E02".into(), completion_sec);
            stops.insert("Anime S01E03".into(), completion_sec - 1);
        }

        // The Stub handle returns None from stop_sec(), so only the merged
        // playlist data drives the result.
        mgr.collect_stop_times().await;

        assert_eq!(mgr.stop_times.get("Anime S01E01"), Some(&1000));
        assert_eq!(mgr.stop_times.get("Anime S01E02"), Some(&completion_sec));
        assert!(
            !mgr.stop_times.contains_key("Anime S01E03"),
            "a later episode below the threshold must not be marked"
        );
    }

    // ── completed_entries completion flag ─────────────────────────────────────

    #[test]
    fn completed_entries_flags_watched_vs_in_progress() {
        let completion_sec =
            (3600.0 * etlp_core::PLAYBACK_COMPLETION_RATIO).round() as i64;
        let mut mgr = make_mgr(dummy_data("Anime S01E01", 0));
        mgr.register_playlist(
            "Anime S01E01".into(),
            dummy_data("Anime S01E01", 0),
        );
        mgr.register_playlist(
            "Anime S01E02".into(),
            dummy_data("Anime S01E02", 0),
        );
        mgr.stop_times.insert("Anime S01E01".into(), 1800); // 50 %: watching
        mgr.stop_times.insert("Anime S01E02".into(), completion_sec);

        let entries = mgr.completed_entries();
        let by_title = |title: &str| {
            entries
                .iter()
                .find(|e| e.data.media_title == title)
                .copied()
                .unwrap()
        };

        let e1 = by_title("Anime S01E01");
        assert!(!e1.completed, "50 % must report as in-progress");
        assert!((e1.progress - 50.0).abs() < 0.001);

        let e2 = by_title("Anime S01E02");
        assert!(e2.completed, "completion threshold must report as watched");
        assert!(
            (e2.progress - etlp_core::PLAYBACK_COMPLETION_PERCENT).abs()
                < 0.001
        );
    }

    #[test]
    fn completed_entries_progress_zero_when_duration_unknown() {
        let mut data = dummy_data("Movie", 0);
        data.total_sec = 0; // unknown runtime
        let mut mgr = make_mgr(data);
        mgr.stop_times.insert("Movie".into(), 1200);

        let entries = mgr.completed_entries();
        let e = entries.first().unwrap();
        assert!(!e.completed);
        assert_eq!(e.progress, 0.0);
    }

    // ── multi-visit completion tracking ───────────────────────────────────────

    const EP1: &str = "Anime S01E01";
    const EP2: &str = "Anime S01E02";
    const EP3: &str = "Anime S01E03";
    /// 85 % of the 3600 s fixture runtime.
    const DONE: i64 = 3060;

    fn playlist_mgr() -> PlayerManager {
        let mut mgr = make_mgr(dummy_data(EP1, 0));
        for t in [EP1, EP2, EP3] {
            mgr.register_playlist(t.into(), dummy_data(t, 0));
        }
        mgr
    }

    /// Feed a sequence of `(title, pos)` mpv polls through the tracker.
    fn simulate(mgr: &PlayerManager, polls: &[(&str, Option<i64>)]) {
        let mut tracker = VisitTracker::default();
        for (title, pos) in polls {
            tracker.observe(
                title,
                *pos,
                &mgr.playlist,
                &mgr.playlist_stops,
                &mgr.completed_episodes,
            );
        }
    }

    fn entry<'a>(entries: &'a [SyncEntry<'a>], title: &str) -> SyncEntry<'a> {
        *entries
            .iter()
            .find(|e| e.data.media_title == title)
            .unwrap_or_else(|| panic!("no entry for {title}"))
    }

    /// 漏标: EP1 finished, moved on, came back to EP1 and quit near the start.
    /// EP1 must still be written back at its completed position and synced
    /// as watched (Trakt history / Bangumi), not scrobbled as in-progress.
    #[tokio::test]
    async fn revisited_completed_episode_stays_watched() {
        let mut mgr = playlist_mgr();
        simulate(
            &mgr,
            &[
                (EP1, Some(100)),
                (EP1, Some(DONE)),
                (EP2, None), // loading
                (EP2, Some(30)),
                (EP1, Some(0)),
                (EP1, Some(5)),
            ],
        );
        mgr.collect_stop_times().await;

        assert_eq!(mgr.stop_times.get(EP1), Some(&DONE));
        let entries = mgr.completed_entries();
        assert!(entry(&entries, EP1).completed, "EP1 must sync as watched");
        // EP2 was only sampled (30 s) and is not the opened episode.
        assert!(!mgr.stop_times.contains_key(EP2));
    }

    /// 错标: seeking past 80 % and back to the start *before* leaving the
    /// episode must not mark it watched.
    #[tokio::test]
    async fn seek_past_threshold_then_back_is_not_completed() {
        let mut mgr = playlist_mgr();
        simulate(
            &mgr,
            &[
                (EP2, Some(10)),
                (EP2, Some(DONE)),
                (EP2, Some(0)), // dragged back to 0 s
                (EP3, Some(20)),
            ],
        );
        mgr.collect_stop_times().await;

        assert!(
            mgr.completed_episodes.lock().unwrap().is_empty(),
            "EP2 was left at 0 s"
        );
        assert!(!mgr.stop_times.contains_key(EP2));
        assert!(mgr.completed_entries().iter().all(|e| !e.completed));
    }

    /// Seeking the opened episode back to 0 s must record 0 (reset), not the
    /// stale later position.
    #[tokio::test]
    async fn seek_to_zero_overwrites_stale_position() {
        let mut mgr = playlist_mgr();
        simulate(&mgr, &[(EP1, Some(DONE)), (EP1, Some(0))]);
        mgr.collect_stop_times().await;

        assert_eq!(mgr.stop_times.get(EP1), Some(&0));
        assert!(!entry(&mgr.completed_entries(), EP1).completed);
    }

    /// Episodes played straight through (auto-advance) are all synced.
    #[tokio::test]
    async fn continuous_playback_marks_each_finished_episode() {
        let mut mgr = playlist_mgr();
        simulate(
            &mgr,
            &[
                (EP1, Some(3590)),
                (EP2, Some(0)),
                (EP2, Some(3595)),
                (EP3, Some(100)),
            ],
        );
        mgr.collect_stop_times().await;

        let entries = mgr.completed_entries();
        assert!(entry(&entries, EP1).completed);
        assert!(entry(&entries, EP2).completed);
        assert!(!mgr.stop_times.contains_key(EP3));
    }

    /// The opened episode is matched by item id when its playlist title
    /// differs from the click-payload title (remote `.strm` versions), so its
    /// progress is not silently dropped.
    #[tokio::test]
    async fn primary_matched_by_item_id_when_title_differs() {
        let mut data = dummy_data("Show S01E54.strm", 0);
        data.item_id = "item-Show S01E54.mp4".into();
        let mut mgr = make_mgr(data);
        for t in ["Show S01E54.mp4", "Show S01E55.mp4"] {
            mgr.register_playlist(t.into(), dummy_data(t, 0));
        }
        simulate(&mgr, &[("Show S01E54.mp4", Some(1050))]);
        mgr.collect_stop_times().await;

        assert_eq!(mgr.stop_times.get("Show S01E54.mp4"), Some(&1050));
        assert_eq!(mgr.completed_entries().len(), 1);
    }

    #[test]
    fn effective_stop_prefers_latest_completed_visit() {
        let ep = dummy_data(EP1, 0);
        assert_eq!(effective_stop_sec(Some(&ep), 5, None), 5);
        assert_eq!(effective_stop_sec(Some(&ep), 5, Some(DONE)), DONE);
        // Latest visit itself completed: keep the real position.
        assert_eq!(effective_stop_sec(Some(&ep), 3500, Some(DONE)), 3500);
    }

    /// Without the old 20 s guard a stop close to the start is still kept.
    #[tokio::test]
    async fn short_session_progress_is_kept() {
        let mut mgr = playlist_mgr();
        mgr.data.start_sec = 500;
        simulate(&mgr, &[(EP1, Some(500)), (EP1, Some(505))]);
        mgr.collect_stop_times().await;
        assert_eq!(mgr.stop_times.get(EP1), Some(&505));
    }

    /// Exiting on a later episode saves its progress as active-at-exit.
    #[tokio::test]
    async fn exit_on_later_episode_saves_active_progress() {
        let mut mgr = playlist_mgr();
        *mgr.active_episode.lock().unwrap() = Some(EP2.into());
        {
            let mut stops = mgr.playlist_stops.lock().unwrap();
            stops.insert(EP1.into(), DONE);
            stops.insert(EP2.into(), 500); // 500s in EP2 (< 80%)
        }
        mgr.completed_episodes
            .lock()
            .unwrap()
            .insert(EP1.into(), DONE);
        mgr.collect_stop_times().await;

        assert_eq!(mgr.stop_times.get(EP1), Some(&DONE));
        assert_eq!(mgr.stop_times.get(EP2), Some(&500));
        let entries = mgr.completed_entries();
        assert!(entry(&entries, EP1).completed);
        assert!(!entry(&entries, EP2).completed);
    }
}
