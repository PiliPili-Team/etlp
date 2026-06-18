//! Emby playback-data orchestration.
//!
//! Wires together the pure helpers (version selection, stream URL, resolve,
//! subtitle, title/intro) plus the network-dependent redirect lookup into a
//! single [`PlaybackData`]. Pure parts are tested in their own modules; this
//! module owns the glue and is exercised with fixture payloads.
//!
//! Config access is injected through [`EmbyParseConfig`] (extracted from the
//! ini once) so the orchestrator stays testable without touching the disk, and
//! the network redirect step is injected through [`HttpClient`] +
//! [`RedirectCache`].

use std::collections::BTreeMap;

use etlp_config::Config;
use etlp_config::matching::{matches, replace_by_pair};
use etlp_core::{PlaybackData, Server, Subtitle};
use etlp_net::{HttpClient, RedirectCache};
use thiserror::Error;
use url::Url;

use crate::dto::MediaSource;
use crate::meta::{emby_title, intro_markers};
use crate::received::ReceivedData;
use crate::resolve::{ResolveInput, classify_source, resolve_stream};
use crate::stream_url::{StreamUrlInput, build_stream_url};
use crate::subtitle::subtitle_checker;
use crate::version::select_version_index;

/// Ticks are 100ns; `10^7` per second.
const TICKS_PER_SEC: i64 = 10_000_000;
/// Runtime sentinel when the server reports none (24h).
const RUNTIME_FALLBACK_SEC: i64 = 86_400;

/// Errors raised while parsing an Emby/Jellyfin playback payload.
#[derive(Debug, Error)]
pub enum ParseError {
    /// `playbackUrl` was not a valid absolute URL.
    #[error("invalid playbackUrl: {0}")]
    InvalidUrl(String),

    /// A field the payload must contain was absent.
    #[error("missing required field: {0}")]
    MissingField(&'static str),

    /// The payload carried no media sources to play.
    #[error("payload contained no media sources")]
    NoMediaSource,

    /// A redirect lookup failed at the transport layer.
    #[error(transparent)]
    Net(#[from] etlp_net::NetError),
}

/// Configuration values consumed by [`parse_received_data_emby`], extracted
/// from the ini once. Construct via [`EmbyParseConfig::from_config`], or build
/// directly in tests.
#[derive(Debug, Clone, Default)]
pub struct EmbyParseConfig {
    /// `dev.strm_direct_host` — hosts whose strm files play in place.
    pub strm_direct_hosts: Vec<String>,
    /// `dev.stream_redirect` — `from, to` replacement pairs for the URL.
    pub stream_redirect: Vec<String>,
    /// `dev.redirect_check_host` — hosts to probe for a 30x redirect.
    pub redirect_check_hosts: Vec<String>,
    /// `dev.stream_prefix` — a literal prefix to prepend to the stream URL.
    pub stream_prefix: Vec<String>,
    /// `dev.force_disk_mode_path` — path prefixes forced to read-from-disk.
    pub force_disk_prefixes: Vec<String>,
    /// `[src]`/`[dst]` path-translation pairs.
    pub path_pairs: Vec<(String, String)>,
    /// `dev.version_prefer` — ordered multi-version preference keywords.
    pub version_prefer: Vec<String>,
    /// `dev.subtitle_priority` — ordered subtitle preference keywords.
    pub subtitle_priority: Vec<String>,
    /// `dev.pretty_title` — prepend the Emby title to the file name.
    pub pretty_title: bool,
    /// `dev.last_ep_disable_playlist` — last episode disables the playlist.
    pub last_ep_disable_playlist: bool,
    /// `[playlist] version_filter` — regex selecting one version per episode.
    pub version_filter: String,
    /// `dev.version_prefer_for_playlist` — fill remaining episodes by
    /// preference.
    pub version_prefer_for_playlist: bool,
}

impl EmbyParseConfig {
    /// Read the relevant options out of a loaded [`Config`].
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self {
            strm_direct_hosts: config.dev.strm_direct_host.clone(),
            stream_redirect: config.dev.stream_redirect.clone(),
            redirect_check_hosts: config.dev.redirect_check_host.clone(),
            stream_prefix: config.dev.stream_prefix.clone(),
            force_disk_prefixes: config.dev.force_disk_mode_path.clone(),
            path_pairs: config.path_translation_pairs(),
            version_prefer: config.dev.version_prefer.clone(),
            subtitle_priority: config.dev.subtitle_priority.clone(),
            pretty_title: config.dev.pretty_title,
            last_ep_disable_playlist: config.dev.last_ep_disable_playlist,
            version_filter: config.playlist.version_filter.clone(),
            version_prefer_for_playlist: config.dev.version_prefer_for_playlist,
        }
    }
}

/// The query string of `playbackUrl`, as a key→value map.
fn query_map(url: &Url) -> BTreeMap<String, String> {
    url.query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

/// Parse a Jellyfin auth header (`MediaBrowser Token="x", DeviceId="y"`) into a
/// key→value map. Quotes and surrounding whitespace are stripped.
fn jellyfin_auth(raw: &str) -> BTreeMap<String, String> {
    raw.split(',')
        .filter_map(|part| {
            let cleaned = part.replace(['\'', '"'], "");
            let (k, v) = cleaned.trim().split_once('=')?;
            Some((k.trim().to_owned(), v.trim().to_owned()))
        })
        .collect()
}

/// The path segment after `Items/` in the playback URL.
fn item_id_from_path(path: &str) -> Option<String> {
    let segments: Vec<&str> = path.split('/').collect();
    let idx = segments.iter().position(|s| *s == "Items")?;
    segments.get(idx + 1).map(|s| (*s).to_owned())
}

/// `os.path.basename`, splitting on both separators.
fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// `os.path.splitdrive(p)[1]` with separators turned into `__`, used as a flat
/// cache file name.
fn fake_name(file_path: &str) -> String {
    let no_drive = match file_path.split_once(':') {
        // A single-letter drive (`C:...`) is stripped; URLs keep their scheme.
        Some((drive, rest)) if drive.len() == 1 => rest,
        _ => file_path,
    };
    no_drive.replace(['/', '\\'], "__")
}

/// The Jellyfin subtitle path infix (`{uuid}/`) or empty for Emby.
fn jellyfin_sub_prefix(item_id: &str, is_emby: bool) -> String {
    if is_emby {
        return String::new();
    }
    let g = |r| item_id.get(r).unwrap_or("");
    format!(
        "{}-{}-{}-{}-{}/",
        g(0..8),
        g(8..12),
        g(12..16),
        g(16..20),
        item_id.get(20..).unwrap_or(""),
    )
}

/// The `host[:port]` of a URL, or an empty string when unparseable.
fn url_netloc(url: &str) -> String {
    match Url::parse(url) {
        Ok(parsed) => {
            let host = parsed.host_str().unwrap_or("");
            match parsed.port() {
                Some(port) => format!("{host}:{port}"),
                None => host.to_owned(),
            }
        }
        Err(_) => String::new(),
    }
}

/// Pick the media source to play, returning `(index, media_source_id)`.
///
/// A concrete `MediaSourceId` from the query wins; otherwise multi-version Emby
/// uses [`select_version_index`] over the per-source names, and everything else
/// takes the first source (matching `version_prefer_emby`).
fn pick_source<'a>(
    sources: &'a [MediaSource],
    requested_id: Option<&str>,
    is_emby: bool,
    version_prefer: &[String],
) -> Result<(&'a MediaSource, String), ParseError> {
    if let Some(id) = requested_id
        && id != "undefined"
    {
        let found = sources
            .iter()
            .find(|s| s.id == id)
            .ok_or(ParseError::NoMediaSource)?;
        return Ok((found, id.to_owned()));
    }
    let index = if sources.len() > 1 && is_emby {
        let http_first =
            sources.first().is_some_and(|s| s.path.starts_with("http"));
        let names: Vec<String> = sources
            .iter()
            .map(|s| {
                let raw = if http_first { s.name.as_str() } else { &s.path };
                basename(raw).to_lowercase()
            })
            .collect();
        select_version_index(&names, version_prefer)
    } else {
        0
    };
    let source = sources.get(index).ok_or(ParseError::NoMediaSource)?;
    Ok((source, source.id.clone()))
}

/// Correct the file path for a multi-version http strm, where the source path
/// alone does not identify the chosen version. Mirrors the
/// `episodes_info`-based remap in `parse_received_data_emby`.
fn correct_multi_version_path(
    file_path: &str,
    received: &ReceivedData,
    sources: &[MediaSource],
    chosen: &MediaSource,
    media_source_id: &str,
) -> String {
    if chosen.name.is_empty() || file_path.contains(&chosen.name) {
        return file_path.to_owned();
    }
    for ep in &received.extra_data.episodes_info {
        for source in &ep.media_sources {
            if source.id == media_source_id
                && let Some(path) = ep.path.as_deref()
            {
                return path.to_owned();
            }
        }
    }
    let base = basename(file_path);
    for other in sources {
        if !other.name.is_empty() && base.contains(&other.name) {
            return file_path.replacen(&other.name, &chosen.name, 1);
        }
    }
    file_path.to_owned()
}

/// Parse the Emby/Jellyfin userscript payload into a [`PlaybackData`].
///
/// The network is touched only for the optional redirect probe (gated by
/// `dev.redirect_check_host`); everything else is computed from the payload and
/// the injected [`EmbyParseConfig`].
pub async fn parse_received_data_emby(
    received: &ReceivedData,
    config: &EmbyParseConfig,
    http: &HttpClient,
    redirect_cache: &RedirectCache,
) -> Result<PlaybackData, ParseError> {
    let url = Url::parse(&received.playback_url)
        .map_err(|e| ParseError::InvalidUrl(e.to_string()))?;
    let path = url.path().to_owned();
    let is_emby = path.contains("/emby/");
    let query = query_map(&url);
    let headers = &received.request.headers;

    let jf_auth = if is_emby {
        BTreeMap::new()
    } else {
        let raw = headers
            .get("X-Emby-Authorization")
            .or_else(|| headers.get("Authorization"))
            .map(String::as_str)
            .unwrap_or("");
        jellyfin_auth(raw)
    };

    let item_id =
        item_id_from_path(&path).ok_or(ParseError::MissingField("item_id"))?;
    let api_key = if is_emby {
        query.get("X-Emby-Token").cloned()
    } else {
        jf_auth.get("Token").cloned()
    }
    .ok_or(ParseError::MissingField("api_key"))?;
    let device_id = if is_emby {
        query.get("X-Emby-Device-Id").cloned()
    } else {
        jf_auth.get("DeviceId").cloned()
    }
    .unwrap_or_default();
    let user_id = query.get("UserId").cloned().unwrap_or_default();
    let sub_index: i64 = query
        .get("SubtitleStreamIndex")
        .and_then(|v| v.parse().ok())
        .unwrap_or(-1);

    let (scheme, netloc) = received
        .api_client
        .server_address
        .split_once("://")
        .ok_or(ParseError::MissingField("serverAddress"))?;

    let media_sources = &received.playback_data.media_sources;
    if media_sources.is_empty() {
        return Err(ParseError::NoMediaSource);
    }
    let play_session_id = &received.playback_data.play_session_id;
    let requested_id = query
        .get("MediaSourceId")
        .map(String::as_str)
        .filter(|v| !v.is_empty());
    let (source, media_source_id) = pick_source(
        media_sources,
        requested_id,
        is_emby,
        &config.version_prefer,
    )?;

    let source_path = source.path.clone();
    let main_ep = &received.extra_data.main_ep_info;
    let main_ep_path = main_ep.path.as_deref().unwrap_or("");
    let (mut file_path, is_strm, is_http_source) = classify_source(
        &source_path,
        main_ep_path,
        main_ep.item_type.as_deref(),
        source.container.as_deref(),
    );
    let strm_direct = matches(netloc, &config.strm_direct_hosts);

    if is_strm && is_http_source && media_sources.len() > 1 {
        file_path = correct_multi_version_path(
            &file_path,
            received,
            media_sources,
            source,
            &media_source_id,
        );
    }

    let server_version = &received.api_client.server_version;
    let mut stream_url = build_stream_url(&StreamUrlInput {
        scheme,
        netloc,
        is_emby,
        item_id: &item_id,
        device_id: &device_id,
        media_source_id: &media_source_id,
        play_session_id,
        api_key: &api_key,
        file_path: &file_path,
        source_container: source.container.as_deref(),
        video_type: source.video_type.as_deref(),
        server_version,
    });

    let is_http_direct_strm = is_strm && strm_direct && is_http_source;
    let mut stream_netloc = netloc.to_owned();
    if is_http_direct_strm {
        stream_url = source_path.clone();
        stream_netloc = url_netloc(&stream_url);
    }

    let mount_disk_enable = received.mount_disk_mode();
    if !mount_disk_enable || is_http_direct_strm {
        stream_url = replace_by_pair(&stream_url, &config.stream_redirect);
        if matches(&stream_netloc, &config.redirect_check_hosts) {
            let resolved = match redirect_cache.get(&stream_url) {
                Some(cached) => cached,
                None => {
                    let target = http.resolve_redirect(&stream_url).await?;
                    redirect_cache.insert(&stream_url, target.clone());
                    target
                }
            };
            if resolved != stream_url {
                stream_url = resolved;
            }
        }
        if matches(&stream_netloc, &config.stream_prefix)
            && let Some(prefix) = config.stream_prefix.first()
        {
            stream_url = format!("{}{stream_url}", prefix.trim_matches('/'));
        }
    }

    let resolution = resolve_stream(&ResolveInput {
        source_path: &source_path,
        main_ep_path,
        item_type: main_ep.item_type.as_deref(),
        container: source.container.as_deref(),
        mount_disk_enable,
        strm_direct,
        stream_url: &stream_url,
        force_disk_prefixes: &config.force_disk_prefixes,
        path_pairs: &config.path_pairs,
    });
    let mount_disk_mode = resolution.mount_disk_mode;
    let mut media_path = resolution.media_path;

    let selection = subtitle_checker(
        &source.media_streams,
        sub_index,
        mount_disk_mode,
        &config.subtitle_priority,
    );
    let sub_file = build_sub_file(
        &selection,
        scheme,
        netloc,
        &item_id,
        &media_source_id,
        &api_key,
        is_emby,
    );

    if file_path.contains(".m3u8") {
        media_path = file_path.clone();
        stream_url = file_path.clone();
    }

    let title = if received.extra_data.playlist_info.is_empty() {
        Some(emby_title(main_ep))
    } else {
        None
    };
    let base = basename(&file_path).to_owned();
    let media_title = match &title {
        Some(t) if config.pretty_title && !t.is_empty() => {
            format!("{t}  |  {base}")
        }
        _ => base.clone(),
    };

    let start_sec = query
        .get("StartTimeTicks")
        .and_then(|v| v.parse::<i64>().ok())
        .map_or(0, |ticks| ticks / TICKS_PER_SEC);
    let total_sec = match source.run_time_ticks {
        Some(ticks) if ticks > 0 => ticks / TICKS_PER_SEC,
        _ => RUNTIME_FALLBACK_SEC,
    };
    let position = if total_sec > 0 {
        start_sec as f64 / total_sec as f64
    } else {
        0.0
    };

    let episodes = &received.extra_data.episodes_info;
    let is_multiple_episodes = !(config.last_ep_disable_playlist
        && episodes.last().is_some_and(|last| {
            last.index_number.is_some()
                && last.index_number == main_ep.index_number
        }));

    let intro = intro_markers(main_ep);
    let server = if is_emby {
        Server::Emby
    } else {
        Server::Jellyfin
    };

    Ok(PlaybackData {
        server,
        scheme: scheme.to_owned(),
        netloc: netloc.to_owned(),
        api_key,
        device_id,
        client_id: None,
        play_session_id: play_session_id.clone(),
        headers: headers.clone(),
        user_id,
        server_version: server_version.clone(),
        item_id,
        media_source_id,
        rating_key: None,
        file_path: file_path.clone(),
        source_path,
        basename: base,
        media_basename: basename(&media_path).to_owned(),
        stream_url,
        media_path,
        media_title,
        fake_name: fake_name(&file_path),
        start_sec,
        total_sec,
        position,
        size: source.size.unwrap_or(0),
        mount_disk_mode,
        is_multiple_episodes,
        is_strm,
        strm_direct,
        is_http_source,
        is_http_direct_strm: resolution.is_http_direct_strm,
        sub: Subtitle {
            external: sub_file,
            inner_index: (selection.sub_inner_idx > 0)
                .then_some(selection.sub_inner_idx),
        },
        intro,
        ..PlaybackData::default()
    })
}

/// Build the external subtitle URL for the selected stream, or `None`.
///
/// Mirrors the `sub_delivery_url` logic: a non-`sup` `DeliveryUrl` is used
/// verbatim, otherwise a `Subtitles/{idx}/0/Stream.{codec}` fallback is built.
/// (The `sub_via_other_media_version` extraction fallback is not yet ported.)
fn build_sub_file(
    selection: &crate::subtitle::SubtitleSelection,
    scheme: &str,
    netloc: &str,
    item_id: &str,
    media_source_id: &str,
    api_key: &str,
    is_emby: bool,
) -> Option<String> {
    let stream = selection.selected.as_ref()?;
    let extra = if is_emby { "/emby" } else { "" };
    let codec = stream.codec.as_deref().unwrap_or("");
    let delivery = stream
        .delivery_url
        .as_deref()
        .filter(|url| codec != "sup" && !url.is_empty());
    let sub_path = match delivery {
        Some(url) => url.to_owned(),
        None => {
            let jf = jellyfin_sub_prefix(item_id, is_emby);
            let emby_part = if is_emby {
                format!("/{media_source_id}")
            } else {
                String::new()
            };
            format!(
                "{extra}/videos/{jf}{item_id}{emby_part}/Subtitles\
                 /{}/0/Stream.{codec}?api_key={api_key}",
                selection.sub_index,
            )
        }
    };
    Some(format!("{scheme}://{netloc}{sub_path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "mountDiskEnable": "false",
        "playbackUrl": "https://h:8096/emby/Items/29907/PlaybackInfo?X-Emby-Token=KEY&X-Emby-Device-Id=DEV&UserId=U1&StartTimeTicks=600000000&SubtitleStreamIndex=2",
        "ApiClient": {"_serverAddress": "https://h:8096", "_serverVersion": "4.9.5.0"},
        "request": {"headers": {}},
        "playbackData": {
            "PlaySessionId": "ps-1",
            "MediaSources": [
                {"Id": "src1", "Name": "v1", "Path": "/m/a.mkv",
                 "RunTimeTicks": 12000000000, "Size": 999, "MediaStreams": [
                    {"Type": "Video", "Index": 0},
                    {"Type": "Audio", "Index": 1},
                    {"Type": "Subtitle", "Index": 2, "IsExternal": false,
                     "DisplayTitle": "简体中文"}
                 ]}
            ]
        },
        "extraData": {
            "userAgent": "Mozilla/5.0",
            "mainEpInfo": {"Id": "29907", "Name": "Ep1", "Path": "/m/a.mkv",
                "Type": "Episode", "SeasonId": "s1", "SeriesName": "Show",
                "IndexNumber": 1, "ParentIndexNumber": 1,
                "Chapters": [{"MarkerType": "IntroStart", "StartPositionTicks": 100000001},
                             {"MarkerType": "IntroEnd", "StartPositionTicks": 900000001}]},
            "episodesInfo": [{"Id": "29907", "IndexNumber": 1},
                             {"Id": "29908", "IndexNumber": 2}],
            "playlistInfo": []
        }
    }"#;

    fn parse_sample(received: &ReceivedData) -> PlaybackData {
        let config = EmbyParseConfig {
            subtitle_priority: vec!["简".to_owned()],
            pretty_title: true,
            last_ep_disable_playlist: true,
            ..EmbyParseConfig::default()
        };
        let http = HttpClient::new().expect("client");
        let cache = RedirectCache::new();
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        rt.block_on(parse_received_data_emby(received, &config, &http, &cache))
            .expect("parse")
    }

    #[test]
    fn parses_emby_network_play() {
        let received: ReceivedData =
            serde_json::from_str(SAMPLE).expect("payload");
        let data = parse_sample(&received);

        assert_eq!(data.server, Server::Emby);
        assert_eq!(data.scheme, "https");
        assert_eq!(data.netloc, "h:8096");
        assert_eq!(data.api_key, "KEY");
        assert_eq!(data.device_id, "DEV");
        assert_eq!(data.user_id, "U1");
        assert_eq!(data.item_id, "29907");
        assert_eq!(data.media_source_id, "src1");
        assert!(!data.mount_disk_mode);
        // start = 600000000 / 1e7 = 60s; total = 12e9 / 1e7 = 1200s.
        assert_eq!(data.start_sec, 60);
        assert_eq!(data.total_sec, 1200);
        assert!((data.position - 0.05).abs() < 1e-9);
        assert_eq!(data.size, 999);
        // Network play -> media_path is the static stream url.
        assert!(data.media_path.contains("/emby/videos/29907/original.mkv"));
        assert_eq!(data.media_path, data.stream_url);
        // Pretty title joins emby title and basename.
        assert_eq!(data.media_title, "Show S1:E1 - Ep1  |  a.mkv");
        assert_eq!(data.basename, "a.mkv");
        assert_eq!(data.fake_name, "__m__a.mkv");
        // Intro markers extracted (100000001 ticks -> 10s, 900000001 -> 90s).
        assert_eq!(data.intro.start, Some(10));
        assert_eq!(data.intro.end, Some(90));
        // Explicit embedded subtitle index 2 is the only embedded sub -> sid 1.
        assert_eq!(data.sub.inner_index, Some(1));
        assert!(data.sub.external.is_none());
        // Two episodes, current is the first -> playlist stays enabled.
        assert!(data.is_multiple_episodes);
    }

    #[test]
    fn last_episode_disables_playlist() {
        let mut received: ReceivedData =
            serde_json::from_str(SAMPLE).expect("payload");
        // Make the current (main) episode the last in the list.
        received.extra_data.main_ep_info.index_number = Some(2);
        let data = parse_sample(&received);
        assert!(!data.is_multiple_episodes);
    }

    #[test]
    fn missing_runtime_falls_back_to_24h() {
        let mut received: ReceivedData =
            serde_json::from_str(SAMPLE).expect("payload");
        if let Some(source) = received.playback_data.media_sources.first_mut() {
            source.run_time_ticks = None;
        }
        let data = parse_sample(&received);
        assert_eq!(data.total_sec, RUNTIME_FALLBACK_SEC);
        assert!(data.runtime_missing());
    }

    #[test]
    fn invalid_url_is_reported() {
        let mut received: ReceivedData =
            serde_json::from_str(SAMPLE).expect("payload");
        received.playback_url = "not a url".to_owned();
        let config = EmbyParseConfig::default();
        let http = HttpClient::new().expect("client");
        let cache = RedirectCache::new();
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        let err = rt
            .block_on(parse_received_data_emby(
                &received, &config, &http, &cache,
            ))
            .unwrap_err();
        assert!(matches!(err, ParseError::InvalidUrl(_)));
    }

    #[test]
    fn jellyfin_auth_header_is_parsed() {
        let raw =
            "MediaBrowser Client=\"web\", DeviceId=\"dev-1\", Token=\"abc\"";
        let auth = jellyfin_auth(raw);
        assert_eq!(auth.get("Token").map(String::as_str), Some("abc"));
        assert_eq!(auth.get("DeviceId").map(String::as_str), Some("dev-1"));
    }

    #[test]
    fn fake_name_strips_drive_and_separators() {
        assert_eq!(fake_name("C:\\media\\a.mkv"), "__media__a.mkv");
        assert_eq!(fake_name("/m/a.mkv"), "__m__a.mkv");
    }
}
