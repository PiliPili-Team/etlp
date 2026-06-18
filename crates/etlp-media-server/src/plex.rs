//! Plex payload parsing.
//!
//! Plex sends a `MediaContainer.Metadata` array; each entry becomes one
//! [`PlaybackData`]. Unlike Emby, the connection details live in the
//! `playbackUrl` query (`X-Plex-Token` / `-Client-Identifier` / `-Version`) and
//! the netloc/scheme come from the URL itself. The function returns the full
//! list (the first entry is the played item, the rest form the playlist).

use etlp_config::matching::match_order;
use etlp_core::{PlaybackData, Server, Subtitle};
use serde::Deserialize;
use thiserror::Error;
use url::Url;

use crate::parse::ParseError;

/// Plex subtitle stream type.
const SUBTITLE_STREAM_TYPE: i64 = 3;
/// Duration sentinel (ms) when Plex omits it (`10^12`).
const DURATION_FALLBACK_MS: i64 = 1_000_000_000_000;

/// Errors specific to Plex parsing (wrapping the shared [`ParseError`]).
#[derive(Debug, Error)]
pub enum PlexError {
    /// A required field was missing or the payload was malformed.
    #[error(transparent)]
    Parse(#[from] ParseError),
}

/// The Plex userscript payload.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlexReceivedData {
    #[serde(rename = "mountDiskEnable", default)]
    pub mount_disk_enable: String,
    #[serde(rename = "playbackUrl", default)]
    pub playback_url: String,
    #[serde(rename = "playbackData", default)]
    pub playback_data: PlexPayload,
}

/// The `playbackData` wrapper.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlexPayload {
    #[serde(rename = "MediaContainer", default)]
    pub media_container: PlexMediaContainer,
}

/// The `MediaContainer`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlexMediaContainer {
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<PlexMeta>,
}

/// One metadata item (movie / episode).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlexMeta {
    #[serde(rename = "Media", default)]
    pub media: Vec<PlexMedia>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(rename = "viewOffset", default)]
    pub view_offset: Option<i64>,
    #[serde(rename = "ratingKey", default)]
    pub rating_key: String,
    #[serde(default)]
    pub index: Option<i64>,
    #[serde(rename = "type", default)]
    pub item_type: String,
}

/// A media version.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlexMedia {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(rename = "Part", default)]
    pub part: Vec<PlexPart>,
}

/// A media part (file).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlexPart {
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub key: String,
    #[serde(rename = "Stream", default)]
    pub stream: Vec<PlexStream>,
}

/// A media stream (video / audio / subtitle).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlexStream {
    #[serde(rename = "streamType", default)]
    pub stream_type: i64,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(rename = "displayTitle", default)]
    pub display_title: String,
}

/// Configuration consumed by [`parse_received_data_plex`].
#[derive(Debug, Clone, Default)]
pub struct PlexParseConfig {
    /// `dev.force_disk_mode_path` prefixes.
    pub force_disk_prefixes: Vec<String>,
    /// `dev.subtitle_priority` keywords.
    pub subtitle_priority: Vec<String>,
    /// `[src]`/`[dst]` path-translation pairs.
    pub path_pairs: Vec<(String, String)>,
}

/// The basename of a path.
fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// `os.path.splitdrive(p)[1]` with separators flattened to `__`.
fn fake_name(path: &str) -> String {
    let no_drive = match path.split_once(':') {
        Some((drive, rest)) if drive.len() == 1 => rest,
        _ => path,
    };
    no_drive.replace(['/', '\\'], "__")
}

/// Pick the subtitle key for a part: the selected track for the first item,
/// else the highest-priority track by `subtitle_priority`.
fn pick_subtitle_key(
    streams: &[PlexStream],
    is_first: bool,
    priority: &[String],
) -> Option<String> {
    let subs: Vec<&PlexStream> = streams
        .iter()
        .filter(|s| s.stream_type == SUBTITLE_STREAM_TYPE && s.key.is_some())
        .collect();
    if is_first {
        if let Some(sel) = subs.iter().find(|s| s.selected) {
            return sel.key.clone();
        }
    }
    // First item with no selection, or any later item: match by priority.
    subs.iter()
        .filter_map(|s| {
            let title = s.title.as_deref().unwrap_or("");
            let key = format!("{title},{}", s.display_title).to_lowercase();
            let order = match_order(&key, priority);
            if order == 0 {
                None
            } else {
                Some((order, s.key.clone()))
            }
        })
        .min_by_key(|(order, _)| *order)
        .and_then(|(_, key)| key)
}

/// Parse a Plex payload into the playlist of [`PlaybackData`] entries.
///
/// The first entry is the played item; the remainder are the playlist. Returns
/// an error only when the URL or essential connection fields are missing.
pub fn parse_received_data_plex(
    received: &PlexReceivedData,
    config: &PlexParseConfig,
) -> Result<Vec<PlaybackData>, PlexError> {
    let url = Url::parse(&received.playback_url)
        .map_err(|e| ParseError::InvalidUrl(e.to_string()))?;
    let query: std::collections::BTreeMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let api_key = query
        .get("X-Plex-Token")
        .cloned()
        .ok_or(ParseError::MissingField("X-Plex-Token"))?;
    let client_id = query.get("X-Plex-Client-Identifier").cloned();
    let front_end_ver = query
        .get("X-Plex-Version")
        .map(String::as_str)
        .unwrap_or("");
    let scheme = url.scheme().to_owned();
    let netloc = match url.port() {
        Some(port) => format!("{}:{port}", url.host_str().unwrap_or("")),
        None => url.host_str().unwrap_or("").to_owned(),
    };
    let has_extras_prefix = query.contains_key("extrasPrefixCount");

    let metas = &received.playback_data.media_container.metadata;
    let first_file = metas
        .first()
        .and_then(|m| m.media.first())
        .and_then(|m| m.part.first())
        .map(|p| p.file.as_str())
        .unwrap_or("");
    let mut mount_disk_mode = received.mount_disk_enable == "true";
    if force_disk(first_file, &config.force_disk_prefixes) {
        mount_disk_mode = true;
    }

    let server_version = format!("?;front_end/{front_end_ver}");
    let mut result = Vec::with_capacity(metas.len());
    for (index, meta) in metas.iter().enumerate() {
        let Some(media) = meta.media.first() else {
            continue;
        };
        let Some(part) = media.part.first() else {
            continue;
        };
        let item_id = media.id.to_string();
        let duration = media
            .duration
            .filter(|d| *d > 0)
            .unwrap_or(DURATION_FALLBACK_MS);
        let file_path = part.file.clone();
        let stream_url = format!(
            "{scheme}://{netloc}{}?download=0&X-Plex-Token={api_key}",
            part.key,
        );

        let sub_key = pick_subtitle_key(
            &part.stream,
            index == 0,
            &config.subtitle_priority,
        );
        let sub_file = match (&sub_key, mount_disk_mode) {
            (Some(key), false) => Some(format!(
                "{scheme}://{netloc}{key}?download=0&X-Plex-Token={api_key}"
            )),
            _ => None,
        };

        let media_path = if mount_disk_mode {
            crate::path_map::translate_path(&file_path, &config.path_pairs)
        } else {
            stream_url.clone()
        };
        let base_name = basename(&file_path).to_owned();
        let media_basename = basename(&media_path).to_owned();
        let title = meta.title.clone().unwrap_or_else(|| base_name.clone());
        let media_title = if title == base_name {
            title
        } else {
            format!("{title} | {base_name}")
        };

        let start_sec = match meta.view_offset {
            Some(offset) if offset > 0 && !has_extras_prefix => offset / 1000,
            _ => 0,
        };
        let total_sec = duration / 1000;
        let position = if total_sec > 0 {
            start_sec as f64 / total_sec as f64
        } else {
            0.0
        };
        let entry_index = if meta.item_type == "episode" {
            meta.index
        } else {
            i64::try_from(index).ok()
        };

        result.push(PlaybackData {
            server: Server::Plex,
            scheme: scheme.clone(),
            netloc: netloc.clone(),
            api_key: api_key.clone(),
            client_id: client_id.clone(),
            server_version: server_version.clone(),
            mount_disk_mode,
            item_id,
            rating_key: Some(meta.rating_key.clone()),
            file_path: file_path.clone(),
            basename: base_name,
            media_basename,
            stream_url,
            media_path,
            media_title,
            fake_name: fake_name(&file_path),
            start_sec,
            total_sec,
            position,
            size: part.size,
            index: entry_index,
            order: i64::try_from(index).ok(),
            sub: Subtitle {
                external: sub_file,
                inner_index: None,
            },
            ..PlaybackData::default()
        });
    }

    if result.is_empty() {
        return Err(ParseError::NoMediaSource.into());
    }
    Ok(result)
}

/// Whether `path` starts with any configured force-disk prefix.
fn force_disk(path: &str, prefixes: &[String]) -> bool {
    prefixes
        .iter()
        .any(|p| !p.is_empty() && path.starts_with(p.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "mountDiskEnable": "false",
        "playbackUrl": "https://plex.host:32400/x?X-Plex-Token=TOK&X-Plex-Client-Identifier=CID&X-Plex-Version=1.2.3",
        "playbackData": {"MediaContainer": {"Metadata": [
            {"title": "Pilot", "ratingKey": "555", "index": 1, "type": "episode",
             "viewOffset": 60000,
             "Media": [{"id": 900, "duration": 1200000, "Part": [{
                "file": "/data/Show/S01E01.mkv", "size": 123, "key": "/library/parts/1/file.mkv",
                "Stream": [
                    {"streamType": 3, "key": "/library/streams/9", "selected": true,
                     "displayTitle": "简体"}
                ]
             }]}]}
        ]}}
    }"#;

    fn config() -> PlexParseConfig {
        PlexParseConfig::default()
    }

    #[test]
    fn parses_plex_episode() {
        let received: PlexReceivedData =
            serde_json::from_str(SAMPLE).expect("payload");
        let list =
            parse_received_data_plex(&received, &config()).expect("parse");
        assert_eq!(list.len(), 1);
        let data = list.first().expect("entry");
        assert_eq!(data.server, Server::Plex);
        assert_eq!(data.scheme, "https");
        assert_eq!(data.netloc, "plex.host:32400");
        assert_eq!(data.api_key, "TOK");
        assert_eq!(data.client_id.as_deref(), Some("CID"));
        assert_eq!(data.rating_key.as_deref(), Some("555"));
        assert_eq!(data.item_id, "900");
        assert_eq!(data.start_sec, 60); // 60000 ms / 1000
        assert_eq!(data.total_sec, 1200);
        assert_eq!(data.size, 123);
        assert_eq!(data.index, Some(1));
        assert_eq!(data.media_title, "Pilot | S01E01.mkv");
        assert!(
            data.stream_url
                .contains("/library/parts/1/file.mkv?download=0")
        );
        // Selected subtitle on the first item -> external sub built.
        assert_eq!(
            data.sub.external.as_deref(),
            Some(
                "https://plex.host:32400/library/streams/9\
                 ?download=0&X-Plex-Token=TOK"
            )
        );
    }

    #[test]
    fn missing_token_is_error() {
        let mut received: PlexReceivedData =
            serde_json::from_str(SAMPLE).expect("payload");
        received.playback_url = "https://plex.host:32400/x".to_owned();
        let err = parse_received_data_plex(&received, &config()).unwrap_err();
        assert!(matches!(
            err,
            PlexError::Parse(ParseError::MissingField("X-Plex-Token"))
        ));
    }

    #[test]
    fn force_disk_path_enables_disk_mode() {
        let received: PlexReceivedData =
            serde_json::from_str(SAMPLE).expect("payload");
        let cfg = PlexParseConfig {
            force_disk_prefixes: vec!["/data".to_owned()],
            path_pairs: vec![("/data".to_owned(), "D:".to_owned())],
            ..PlexParseConfig::default()
        };
        let list = parse_received_data_plex(&received, &cfg).expect("parse");
        let data = list.first().expect("entry");
        assert!(data.mount_disk_mode);
        assert_eq!(data.media_path, "D:/Show/S01E01.mkv");
        // Disk mode -> no external subtitle URL.
        assert!(data.sub.external.is_none());
    }
}
