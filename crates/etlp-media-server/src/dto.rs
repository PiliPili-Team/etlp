//! JSON DTOs for the Emby/Jellyfin API responses.
//!
//! Only the fields etlp actually consumes are modeled; everything else is
//! ignored by serde. Field names use the server's PascalCase via `rename`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A chapter / marker entry (used for intro detection).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Chapter {
    #[serde(rename = "MarkerType", default)]
    pub marker_type: Option<String>,
    #[serde(rename = "StartPositionTicks", default)]
    pub start_position_ticks: i64,
}

/// One media stream (video / audio / subtitle) inside a media source.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct MediaStream {
    #[serde(rename = "Type", default)]
    pub stream_type: String,
    #[serde(rename = "Index", default)]
    pub index: Option<i64>,
    #[serde(rename = "IsExternal", default)]
    pub is_external: bool,
    #[serde(rename = "Title", default)]
    pub title: Option<String>,
    #[serde(rename = "DisplayTitle", default)]
    pub display_title: String,
    #[serde(rename = "Codec", default)]
    pub codec: Option<String>,
    #[serde(rename = "DeliveryUrl", default)]
    pub delivery_url: Option<String>,
    #[serde(rename = "Path", default)]
    pub path: Option<String>,
}

impl MediaStream {
    /// Whether this stream is a subtitle track.
    #[must_use]
    pub fn is_subtitle(&self) -> bool {
        self.stream_type == "Subtitle"
    }

    /// The lowercased `"{title},{display_title}"` key used for priority
    /// matching.
    #[must_use]
    pub fn priority_key(&self) -> String {
        let title = self.title.as_deref().unwrap_or("");
        format!("{title},{}", self.display_title).to_lowercase()
    }
}

/// One playable version (media source) of an item.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct MediaSource {
    #[serde(rename = "Id", default)]
    pub id: String,
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Path", default)]
    pub path: String,
    #[serde(rename = "Container", default)]
    pub container: Option<String>,
    #[serde(rename = "VideoType", default)]
    pub video_type: Option<String>,
    #[serde(rename = "RunTimeTicks", default)]
    pub run_time_ticks: Option<i64>,
    #[serde(rename = "Size", default)]
    pub size: Option<i64>,
    #[serde(rename = "DirectStreamUrl", default)]
    pub direct_stream_url: Option<String>,
    #[serde(rename = "TranscodingUrl", default)]
    pub transcoding_url: Option<String>,
    #[serde(rename = "SupportsDirectPlay", default)]
    pub supports_direct_play: Option<bool>,
    #[serde(rename = "SupportsDirectStream", default)]
    pub supports_direct_stream: Option<bool>,
    /// Present when the source belongs to a different item (multi-version).
    #[serde(rename = "ItemId", default)]
    pub item_id: Option<String>,
    #[serde(rename = "MediaStreams", default)]
    pub media_streams: Vec<MediaStream>,
}

/// A library item (episode / movie / video).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Item {
    #[serde(rename = "Id", default)]
    pub id: String,
    #[serde(rename = "Name", default)]
    pub name: Option<String>,
    #[serde(rename = "Path", default)]
    pub path: Option<String>,
    #[serde(rename = "Type", default)]
    pub item_type: Option<String>,
    #[serde(rename = "IndexNumber", default)]
    pub index_number: Option<i64>,
    #[serde(rename = "ParentIndexNumber", default)]
    pub parent_index_number: Option<i64>,
    #[serde(rename = "IndexNumberEnd", default)]
    pub index_number_end: Option<i64>,
    #[serde(rename = "SeriesId", default)]
    pub series_id: Option<String>,
    #[serde(rename = "SeasonId", default)]
    pub season_id: Option<String>,
    #[serde(rename = "SeriesName", default)]
    pub series_name: Option<String>,
    /// Native-language title (e.g. the Japanese title for anime). For a movie
    /// this is the film's native title; for an episode it is the single
    /// episode's native title (not the series name). Used as a Bangumi search
    /// keyword on the movie path only.
    #[serde(rename = "OriginalTitle", default)]
    pub original_title: Option<String>,
    /// Genre tags reported by the server; used to gate Bangumi title search to
    /// anime only.
    #[serde(rename = "Genres", default)]
    pub genres: Vec<String>,
    #[serde(rename = "ProductionYear", default)]
    pub production_year: Option<i64>,
    #[serde(rename = "ServerId", default)]
    pub server_id: Option<String>,
    #[serde(rename = "PremiereDate", default)]
    pub premiere_date: Option<String>,
    #[serde(rename = "RunTimeTicks", default)]
    pub run_time_ticks: Option<i64>,
    #[serde(rename = "MediaSources", default)]
    pub media_sources: Vec<MediaSource>,
    #[serde(rename = "Chapters", default)]
    pub chapters: Vec<Chapter>,
    #[serde(rename = "ProviderIds", default)]
    pub provider_ids: BTreeMap<String, String>,
    /// Per-user playback state; only populated when the request carries a
    /// `UserId` and requests the `UserData` field. Used to backfill earlier
    /// episodes the user already watched in the media-server client.
    #[serde(rename = "UserData", default)]
    pub user_data: Option<UserData>,
}

/// Per-user item state returned in the `UserData` field.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct UserData {
    /// Whether the user has marked the item as played/watched.
    #[serde(rename = "Played", default)]
    pub played: bool,
}

/// Response of `Items/{id}/PlaybackInfo`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct PlaybackInfo {
    #[serde(rename = "PlaySessionId", default)]
    pub play_session_id: Option<String>,
    #[serde(rename = "MediaSources", default)]
    pub media_sources: Vec<MediaSource>,
}

/// Response of an item list endpoint (e.g. Resume, Episodes).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ItemList {
    #[serde(rename = "Items", default)]
    pub items: Vec<Item>,
}
