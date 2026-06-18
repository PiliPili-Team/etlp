//! Input DTOs for the userscript POST payload (`received_data`).
//!
//! These model the JSON the Tampermonkey script sends to the local server when
//! the user clicks play. `parse_received_data_emby` / `_plex` (ported in a
//! later step) consume these to build [`etlp_core::PlaybackData`].

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::dto::{Item, MediaSource};

/// Deserializes a field that may be absent OR explicitly `null` in JSON.
/// Both cases produce `T::default()`.
fn null_as_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// The full payload posted by the userscript.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReceivedData {
    #[serde(rename = "extraData", default)]
    pub extra_data: ExtraData,
    #[serde(rename = "ApiClient", default)]
    pub api_client: ApiClient,
    #[serde(rename = "request", default)]
    pub request: RequestData,
    #[serde(rename = "playbackUrl", default)]
    pub playback_url: String,
    #[serde(rename = "playbackData", default)]
    pub playback_data: PlaybackPayload,
    /// `"true"` / `"false"` string, mirroring the JS payload.
    #[serde(rename = "mountDiskEnable", default, deserialize_with = "null_as_default")]
    pub mount_disk_enable: String,
    #[serde(rename = "showTaskManager", default)]
    pub show_task_manager: bool,
}

impl ReceivedData {
    /// Whether the user enabled read-from-disk mode (`mountDiskEnable`).
    #[must_use]
    pub fn mount_disk_mode(&self) -> bool {
        self.mount_disk_enable == "true"
    }
}

/// The `extraData` block: the main episode, the season episode list, and an
/// optional shuffle playlist.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExtraData {
    #[serde(rename = "mainEpInfo", default)]
    pub main_ep_info: Item,
    #[serde(rename = "episodesInfo", default)]
    pub episodes_info: Vec<Item>,
    #[serde(rename = "playlistInfo", default, deserialize_with = "null_as_default")]
    pub playlist_info: Vec<serde_json::Value>,
    #[serde(rename = "userAgent", default)]
    pub user_agent: Option<String>,
}

/// The `ApiClient` block.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ApiClient {
    #[serde(rename = "_serverAddress", default)]
    pub server_address: String,
    #[serde(rename = "_serverVersion", default)]
    pub server_version: String,
}

/// The `request` block (HTTP headers forwarded from the browser).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RequestData {
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

/// The `playbackData` block: the available versions and the play session id.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlaybackPayload {
    #[serde(rename = "MediaSources", default)]
    pub media_sources: Vec<MediaSource>,
    #[serde(rename = "PlaySessionId", default)]
    pub play_session_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "mountDiskEnable": "true",
        "playbackUrl": "https://h:8096/emby/Items/29907/PlaybackInfo?X-Emby-Token=k",
        "ApiClient": {"_serverAddress": "https://h:8096", "_serverVersion": "4.9.5.0"},
        "request": {"headers": {"X-Emby-Authorization": "MediaBrowser Token=\"k\""}},
        "playbackData": {
            "PlaySessionId": "ps-1",
            "MediaSources": [
                {"Id": "src1", "Name": "v1", "Path": "/m/a.mkv",
                 "RunTimeTicks": 12000000000, "MediaStreams": [
                    {"Type": "Subtitle", "Index": 2, "IsExternal": false,
                     "DisplayTitle": "简体中文"}
                 ]}
            ]
        },
        "extraData": {
            "userAgent": "Mozilla/5.0",
            "mainEpInfo": {"Id": "29907", "Name": "Ep1", "Path": "/m/a.mkv",
                "SeasonId": "s1", "IndexNumber": 1, "ParentIndexNumber": 1,
                "Chapters": [{"MarkerType": "IntroStart", "StartPositionTicks": 0},
                             {"MarkerType": "IntroEnd", "StartPositionTicks": 900000000}]},
            "episodesInfo": [{"Id": "29907", "IndexNumber": 1}],
            "playlistInfo": []
        }
    }"#;

    #[test]
    fn deserializes_full_payload() {
        let data: ReceivedData =
            serde_json::from_str(SAMPLE).expect("parse payload");
        assert!(data.mount_disk_mode());
        assert_eq!(data.api_client.server_version, "4.9.5.0");
        assert_eq!(data.playback_data.play_session_id, "ps-1");
        assert_eq!(data.playback_data.media_sources.len(), 1);

        let main = &data.extra_data.main_ep_info;
        assert_eq!(main.id, "29907");
        assert_eq!(main.season_id.as_deref(), Some("s1"));
        assert_eq!(main.chapters.len(), 2);
        assert_eq!(
            main.chapters.first().and_then(|c| c.marker_type.as_deref()),
            Some("IntroStart")
        );
        assert_eq!(data.extra_data.episodes_info.len(), 1);
    }

    #[test]
    fn missing_fields_use_defaults() {
        let data: ReceivedData =
            serde_json::from_str("{}").expect("parse empty");
        assert!(!data.mount_disk_mode());
        assert!(data.playback_url.is_empty());
        assert!(data.extra_data.episodes_info.is_empty());
    }

    #[test]
    fn null_fields_use_defaults() {
        // The userscript sends null for mountDiskEnable and playlistInfo when
        // the feature is disabled. Without null_as_default these would produce
        // a 422 from axum, making it look like mpv was never called.
        let json = r#"{
            "mountDiskEnable": null,
            "extraData": {
                "mainEpInfo": {},
                "episodesInfo": [],
                "playlistInfo": null
            }
        }"#;
        let data: ReceivedData =
            serde_json::from_str(json).expect("parse null fields");
        assert!(!data.mount_disk_mode());
        assert!(data.extra_data.playlist_info.is_empty());
    }
}
