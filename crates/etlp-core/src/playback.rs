//! The normalized playback context.
//!
//! `PlaybackData` is the Rust counterpart of the Python `data` dict produced by
//! `data_parser`. Python copies this dict per playlist episode and overrides a
//! few keys, so the same struct also represents a playlist entry; the
//! playlist-only fields are grouped at the end and default to empty.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{IntroMarkers, Server, Subtitle};

/// Fraction of a media item's runtime required to treat playback as complete.
pub const PLAYBACK_COMPLETION_RATIO: f64 = 0.8;

/// Playback completion threshold expressed as a percentage.
pub const PLAYBACK_COMPLETION_PERCENT: f64 = PLAYBACK_COMPLETION_RATIO * 100.0;

/// A full playback context for one media item.
///
/// Connection/identity fields drive progress write-back; media fields drive
/// player launch; the trailing playlist fields are populated when the item is
/// part of a playlist.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PlaybackData {
    // ---- server / connection ----
    pub server: Server,
    pub scheme: String,
    pub netloc: String,
    pub api_key: String,
    /// Emby/Jellyfin device id.
    #[serde(default)]
    pub device_id: String,
    /// Plex client identifier.
    #[serde(default)]
    pub client_id: Option<String>,
    pub play_session_id: String,
    /// Extra request headers (Jellyfin auth / realtime reporting).
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub server_version: String,

    // ---- item identity ----
    pub item_id: String,
    #[serde(default)]
    pub media_source_id: String,
    /// Plex rating key (item id equivalent).
    #[serde(default)]
    pub rating_key: Option<String>,

    // ---- media / paths ----
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub source_path: String,
    #[serde(default)]
    pub basename: String,
    #[serde(default)]
    pub media_basename: String,
    #[serde(default)]
    pub stream_url: String,
    #[serde(default)]
    pub media_path: String,
    #[serde(default)]
    pub media_title: String,
    #[serde(default)]
    pub fake_name: String,

    // ---- playback position / size ----
    #[serde(default)]
    pub start_sec: i64,
    #[serde(default)]
    pub total_sec: i64,
    #[serde(default)]
    pub position: f64,
    #[serde(default)]
    pub size: i64,

    // ---- mode flags ----
    #[serde(default)]
    pub mount_disk_mode: bool,
    #[serde(default)]
    pub is_multiple_episodes: bool,
    #[serde(default)]
    pub is_strm: bool,
    #[serde(default)]
    pub strm_direct: bool,
    #[serde(default)]
    pub is_http_source: bool,
    #[serde(default)]
    pub is_http_direct_strm: bool,

    // ---- subtitle / intro ----
    #[serde(default)]
    pub sub: Subtitle,
    #[serde(default)]
    pub intro: IntroMarkers,

    // ---- sync metadata (Trakt / Bangumi) ----
    /// Emby/Jellyfin item type ("Episode", "Movie", …). Used for sync routing.
    #[serde(default)]
    pub item_type: String,
    /// External provider IDs keyed by provider name ("Imdb", "Tmdb", "Tvdb",
    /// "Bangumi", …). Populated from the server response; empty for Plex items
    /// that do not supply Guid metadata.
    #[serde(default)]
    pub provider_ids: BTreeMap<String, String>,
    /// Emby/Jellyfin series ID — needed by Bangumi sync to correlate subjects.
    #[serde(default)]
    pub series_id: String,
    /// Series display name (e.g. "Re:Zero"). Bangumi title-search keyword.
    #[serde(default)]
    pub series_name: String,
    /// Native-language title (Japanese for most anime). Preferred Bangumi
    /// search keyword when present, as it matches bgm's primary `name` field.
    #[serde(default)]
    pub original_title: String,
    /// Clean item display name (`Name`) reported by the media server. For a
    /// movie this is the localised film title; for an episode it is the single
    /// episode's title. Bangumi movie resolution uses it instead of the
    /// player-facing `media_title`, which carries year/quality/filename noise.
    #[serde(default)]
    pub item_name: String,
    /// Season number (`ParentIndexNumber`); drives walking bgm's sequel chain
    /// to the subject that represents this season.
    #[serde(default)]
    pub season_number: Option<i64>,
    /// Air date (`YYYY-MM-DD`) used to disambiguate the matched bgm episode.
    #[serde(default)]
    pub premiere_date: Option<String>,
    /// Genre tags; gate Bangumi title search to anime only.
    #[serde(default)]
    pub genres: Vec<String>,

    // ---- playlist-only (populated for playlist entries) ----
    /// Playlist position used for ordering (the Python `order` key).
    #[serde(default)]
    pub order: Option<i64>,
    /// Episode index number within the season.
    #[serde(default)]
    pub index: Option<i64>,
    /// Marks the currently playing entry in the playlist.
    #[serde(default)]
    pub is_start_file: bool,
    /// Cached resolved redirect URL for this entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect_url: Option<String>,
    /// Final reported stop second for this entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_sec: Option<i64>,
}

impl PlaybackData {
    /// The runtime is missing when Emby/Jellyfin could not report it; the
    /// Python sentinel is `total_sec == 86400` (24h). Plex uses a different
    /// large sentinel and is excluded.
    #[must_use]
    pub fn runtime_missing(&self) -> bool {
        self.server != Server::Plex && self.total_sec == 86_400
    }

    /// Watched fraction in `[0, 1]`, guarding against a zero runtime.
    #[must_use]
    pub fn progress_fraction(&self, stop_sec: i64) -> f64 {
        if self.total_sec <= 0 {
            0.0
        } else {
            stop_sec as f64 / self.total_sec as f64
        }
    }

    /// Watched percentage in `[0, 100]`, guarding against a zero runtime.
    #[must_use]
    pub fn progress_percent(&self, stop_sec: i64) -> f64 {
        (self.progress_fraction(stop_sec) * 100.0).clamp(0.0, 100.0)
    }

    /// Whether `stop_sec` reaches the shared completion threshold.
    #[must_use]
    pub fn is_complete_at(&self, stop_sec: i64) -> bool {
        self.progress_fraction(stop_sec) >= PLAYBACK_COMPLETION_RATIO
    }

    /// Whether a watched percentage reaches the shared completion threshold.
    #[must_use]
    pub fn is_complete_percent(progress: f64) -> bool {
        progress >= PLAYBACK_COMPLETION_PERCENT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_missing_only_for_emby_sentinel() {
        let mut d = PlaybackData {
            server: Server::Emby,
            total_sec: 86_400,
            ..PlaybackData::default()
        };
        assert!(d.runtime_missing());
        d.total_sec = 1200;
        assert!(!d.runtime_missing());
        d.server = Server::Plex;
        d.total_sec = 86_400;
        assert!(!d.runtime_missing());
    }

    #[test]
    fn progress_fraction_guards_zero_runtime() {
        let mut d = PlaybackData {
            total_sec: 0,
            ..PlaybackData::default()
        };
        assert!((d.progress_fraction(100) - 0.0).abs() < 1e-9);
        d.total_sec = 200;
        assert!((d.progress_fraction(100) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn completion_threshold_is_eighty_percent() {
        let d = PlaybackData {
            total_sec: 100,
            ..PlaybackData::default()
        };
        assert!(!d.is_complete_at(79));
        assert!(d.is_complete_at(80));
        assert_eq!(PLAYBACK_COMPLETION_PERCENT, 80.0);
        assert!(!PlaybackData::is_complete_percent(79.9));
        assert!(PlaybackData::is_complete_percent(80.0));
    }

    #[test]
    fn serde_roundtrip_preserves_core_fields() {
        let d = PlaybackData {
            server: Server::Jellyfin,
            item_id: "42".into(),
            total_sec: 1200,
            order: Some(3),
            ..PlaybackData::default()
        };
        let json = serde_json::to_string(&d).unwrap_or_default();
        let back: PlaybackData =
            serde_json::from_str(&json).unwrap_or_default();
        assert_eq!(back.item_id, "42");
        assert_eq!(back.server, Server::Jellyfin);
        assert_eq!(back.order, Some(3));
    }
}
