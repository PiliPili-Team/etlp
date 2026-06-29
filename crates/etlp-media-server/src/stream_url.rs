//! Emby stream URL construction (`stream_url` / `stream_name` / `container`).

use crate::version::match_version_range;

/// Emby switched the static stream path name to `original` in this range.
const ORIGINAL_RANGE: &str = "4.8.0.40-9";

/// Inputs for building an Emby/Jellyfin static stream URL.
#[derive(Debug, Clone)]
pub struct StreamUrlInput<'a> {
    pub scheme: &'a str,
    pub netloc: &'a str,
    pub is_emby: bool,
    pub item_id: &'a str,
    pub device_id: &'a str,
    pub media_source_id: &'a str,
    pub play_session_id: &'a str,
    pub api_key: &'a str,
    /// The resolved file path (its extension becomes the URL container).
    pub file_path: &'a str,
    /// `media_source_info['Container']`.
    pub source_container: Option<&'a str>,
    /// `media_source_info['VideoType']`.
    pub video_type: Option<&'a str>,
    /// `ApiClient._serverVersion`.
    pub server_version: &'a str,
}

/// Convert a server-provided media URL to an absolute URL.
#[must_use]
pub fn absolute_media_url(
    scheme: &str,
    netloc: &str,
    candidate: Option<&str>,
) -> Option<String> {
    let raw = candidate?.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return Some(raw.to_owned());
    }
    if raw.starts_with('/') {
        return Some(format!("{scheme}://{netloc}{raw}"));
    }
    None
}

/// Prefer the media server's prepared stream endpoint for a source.
///
/// Emby may return `TranscodingUrl` or `DirectStreamUrl` in `PlaybackInfo`.
/// Those URLs carry the server's container/bitrate decision and are safer for
/// remote `.strm` sources than a locally constructed static stream URL.
#[must_use]
pub fn server_media_url(
    scheme: &str,
    netloc: &str,
    source: &crate::dto::MediaSource,
) -> Option<String> {
    absolute_media_url(scheme, netloc, source.transcoding_url.as_deref())
        .or_else(|| {
            absolute_media_url(
                scheme,
                netloc,
                source.direct_stream_url.as_deref(),
            )
        })
}

/// The file extension (including the dot) of a path, lowercased dot kept as-is.
fn extension_with_dot(file_path: &str) -> String {
    match file_path.rsplit_once('.') {
        Some((head, ext)) if !head.is_empty() || file_path.starts_with('.') => {
            format!(".{ext}")
        }
        _ => String::new(),
    }
}

/// Build the static stream URL plus the resolved `(stream_name, container)`.
#[must_use]
pub fn build_stream_url(input: &StreamUrlInput) -> String {
    let extra = if input.is_emby { "/emby" } else { "" };
    let mut stream_name =
        if match_version_range(input.server_version, ORIGINAL_RANGE) {
            "original"
        } else {
            "stream"
        };
    let mut container = extension_with_dot(input.file_path);

    if input.source_container == Some("bluray") {
        container = ".m2ts".to_owned();
    }
    if input.video_type == Some("BluRay") {
        stream_name = "main";
        container = ".m3u8".to_owned();
    }

    format!(
        "{}://{}{}/videos/{}/{}{}?DeviceId={}&MediaSourceId={}\
         &PlaySessionId={}&api_key={}&Static=true",
        input.scheme,
        input.netloc,
        extra,
        input.item_id,
        stream_name,
        container,
        input.device_id,
        input.media_source_id,
        input.play_session_id,
        input.api_key,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base<'a>() -> StreamUrlInput<'a> {
        StreamUrlInput {
            scheme: "https",
            netloc: "h:8096",
            is_emby: true,
            item_id: "100",
            device_id: "DEV",
            media_source_id: "src1",
            play_session_id: "ps",
            api_key: "KEY",
            file_path: "/m/a.mkv",
            source_container: None,
            video_type: None,
            server_version: "4.9.5.0",
        }
    }

    #[test]
    fn extension_helper() {
        assert_eq!(extension_with_dot("/m/a.mkv"), ".mkv");
        assert_eq!(extension_with_dot("/m/noext"), "");
        assert_eq!(extension_with_dot("/m/a.b.ts"), ".ts");
    }

    #[test]
    fn absolute_media_url_uses_absolute_input() {
        let url = absolute_media_url(
            "https",
            "h:8096",
            Some("https://cdn/media/master.m3u8"),
        );
        assert_eq!(url.as_deref(), Some("https://cdn/media/master.m3u8"));
    }

    #[test]
    fn absolute_media_url_expands_root_relative_input() {
        let url = absolute_media_url(
            "https",
            "h:8096",
            Some("/emby/videos/1/master.m3u8"),
        );
        assert_eq!(
            url.as_deref(),
            Some("https://h:8096/emby/videos/1/master.m3u8")
        );
    }

    #[test]
    fn absolute_media_url_rejects_empty_or_relative_path() {
        assert!(absolute_media_url("https", "h", Some("")).is_none());
        assert!(absolute_media_url("https", "h", Some("videos/1")).is_none());
        assert!(absolute_media_url("https", "h", None).is_none());
    }

    #[test]
    fn server_media_url_prefers_transcoding_url() {
        let source = crate::dto::MediaSource {
            direct_stream_url: Some("/videos/1/master.m3u8?direct=1".into()),
            transcoding_url: Some("/videos/1/master.m3u8?transcode=1".into()),
            ..crate::dto::MediaSource::default()
        };
        assert_eq!(
            server_media_url("https", "h", &source).as_deref(),
            Some("https://h/videos/1/master.m3u8?transcode=1")
        );
    }

    #[test]
    fn server_media_url_falls_back_to_direct_stream_url() {
        let source = crate::dto::MediaSource {
            direct_stream_url: Some("/videos/1/master.m3u8?direct=1".into()),
            ..crate::dto::MediaSource::default()
        };
        assert_eq!(
            server_media_url("https", "h", &source).as_deref(),
            Some("https://h/videos/1/master.m3u8?direct=1")
        );
    }

    #[test]
    fn modern_emby_uses_original_and_file_ext() {
        let url = build_stream_url(&base());
        assert_eq!(
            url,
            "https://h:8096/emby/videos/100/original.mkv?\
             DeviceId=DEV&MediaSourceId=src1&PlaySessionId=ps\
             &api_key=KEY&Static=true"
        );
    }

    #[test]
    fn older_emby_uses_stream_name() {
        let mut input = base();
        input.server_version = "4.7.0.0";
        let url = build_stream_url(&input);
        assert!(url.contains("/videos/100/stream.mkv?"));
    }

    #[test]
    fn jellyfin_has_no_emby_prefix() {
        let mut input = base();
        input.is_emby = false;
        let url = build_stream_url(&input);
        assert!(url.starts_with("https://h:8096/videos/100/"));
    }

    #[test]
    fn bluray_container_and_video_type_override() {
        let mut input = base();
        input.source_container = Some("bluray");
        assert!(build_stream_url(&input).contains("original.m2ts?"));

        let mut vt = base();
        vt.video_type = Some("BluRay");
        let url = build_stream_url(&vt);
        assert!(url.contains("/videos/100/main.m3u8?"));
    }
}
