//! Per-episode parsing for the playlist.
//!
//! [`build_title_intro_maps`] precomputes the pretty title and intro markers per
//! episode key, and [`parse_episode_item`] turns one season episode into a
//! [`PlaybackData`] entry, reusing the base item's connection fields. Both are
//! pure; the network fetch and the `version_filter` heuristic that select which
//! episodes reach here live in the caller.

use std::collections::BTreeMap;

use etlp_core::{PlaybackData, Subtitle};

use crate::dto::Item;
use crate::meta::{emby_title, intro_markers};
use crate::parse::apply_title_translate;
use crate::path_map::translate_path;
use crate::subtitle::subtitle_checker;

/// Ticks are 100ns; `10^7` per second.
const TICKS_PER_SEC: i64 = 10_000_000;
/// Runtime sentinel when the server reports none (24h).
const RUNTIME_FALLBACK_SEC: i64 = 86_400;

/// Pretty title and intro markers keyed by `"{parent}-{index}"`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TitleIntroMaps {
    /// Episode key → pretty display title.
    pub title: BTreeMap<String, String>,
    /// Episode key → intro start second.
    pub start: BTreeMap<String, i64>,
    /// Episode key → intro end second.
    pub end: BTreeMap<String, i64>,
}

/// The episode key `"{ParentIndexNumber}-{IndexNumber}"`, with `None` rendered
/// literally and a missing index defaulting to `0` (matching `parse_item`).
fn episode_key(item: &Item) -> String {
    let parent = match item.parent_index_number {
        Some(p) => p.to_string(),
        None => "None".to_owned(),
    };
    format!("{parent}-{}", item.index_number.unwrap_or(0))
}

/// Build the per-episode title/intro maps.
///
/// Returns the maps plus a `fail` flag: `true` when the episode list is empty
/// or an episode in the target season is missing its index numbers (the Python
/// `title_intro_map_fail`), which tells the caller to fall back to the played
/// item's own title.
#[must_use]
pub fn build_title_intro_maps(
    episodes_info: &[Item],
    season_id: &str,
    playlist: bool,
) -> (TitleIntroMaps, bool) {
    let mut maps = TitleIntroMaps::default();
    if playlist {
        return (maps, false);
    }
    let fail = episodes_info.is_empty();
    for ep in episodes_info {
        if ep.season_id.as_deref() != Some(season_id) {
            continue;
        }
        if ep.parent_index_number.is_none() || ep.index_number.is_none() {
            return (maps, true);
        }
        let key = episode_key(ep);
        maps.title.insert(key.clone(), emby_title(ep));
        let intro = intro_markers(ep);
        if let Some(start) = intro.start {
            maps.start.insert(key.clone(), start);
        }
        if let Some(end) = intro.end {
            maps.end.insert(key, end);
        }
    }
    (maps, fail)
}

/// Shared context for [`parse_episode_item`], gathered once per `list_episodes`
/// call. Mirrors the closed-over variables of the Python `parse_item`.
#[derive(Debug, Clone)]
pub struct EpisodeContext<'a> {
    pub scheme: &'a str,
    pub netloc: &'a str,
    /// `/emby` for Emby, empty for Jellyfin.
    pub extra: &'a str,
    /// `original` or `stream`, decided by the server version.
    pub stream_name: &'a str,
    pub device_id: &'a str,
    pub play_session_id: &'a str,
    pub api_key: &'a str,
    pub is_strm: bool,
    pub strm_direct: bool,
    pub is_http_direct_strm: bool,
    pub mount_disk_mode: bool,
    pub pretty_title: bool,
    /// The played item's basename; the current episode keeps the base position.
    pub main_ep_basename: &'a str,
    /// Subtitle probe index: `-1` when the current item had an inner subtitle,
    /// else `-3` (the Python `need_check_inner_sub`).
    pub need_check_inner_sub: i64,
    pub subtitle_priority: &'a [String],
    pub path_pairs: &'a [(String, String)],
    pub maps: &'a TitleIntroMaps,
    /// Pre-parsed character translation table from `dev.media_title_translate`.
    pub title_translate: &'a [(char, char)],
}

/// The file extension (with dot) of a path, or empty when none.
fn extension_with_dot(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((head, ext)) if !head.is_empty() => format!(".{ext}"),
        _ => String::new(),
    }
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

/// Parse one season episode into a [`PlaybackData`] playlist entry.
///
/// `base` is the currently-playing context whose connection/identity fields are
/// inherited; per-episode media fields are overridden. Returns `None` only when
/// the episode carries no media source.
#[must_use]
pub fn parse_episode_item(
    ctx: &EpisodeContext,
    base: &PlaybackData,
    item: &Item,
    order: i64,
) -> Option<PlaybackData> {
    let source = item.media_sources.first()?;
    let media_source_id = source.id.clone();
    let file_path = item.path.clone().unwrap_or_default();
    let source_path = source.path.clone();
    let item_id = item.id.clone();
    let container = extension_with_dot(&file_path);

    let mut stream_url = format!(
        "{}://{}{}/videos/{item_id}/{}{container}?DeviceId={}\
         &MediaSourceId={media_source_id}&PlaySessionId={}\
         &api_key={}&Static=true",
        ctx.scheme,
        ctx.netloc,
        ctx.extra,
        ctx.stream_name,
        ctx.device_id,
        ctx.play_session_id,
        ctx.api_key,
    );
    if ctx.is_http_direct_strm {
        stream_url = source_path.clone();
    }

    let media_path = if ctx.mount_disk_mode {
        if ctx.is_strm {
            if ctx.strm_direct {
                translate_path(&source_path, ctx.path_pairs)
            } else {
                stream_url.clone()
            }
        } else {
            translate_path(&file_path, ctx.path_pairs)
        }
    } else if ctx.is_strm && ctx.strm_direct && !ctx.is_http_direct_strm {
        source_path.clone()
    } else {
        stream_url.clone()
    };

    let base_name = basename(&file_path).to_owned();
    let index = item.index_number.unwrap_or(0);
    let unique_key = episode_key(item);
    let title = ctx.maps.title.get(&unique_key);
    let raw_title = match title {
        Some(t) if ctx.pretty_title && !t.is_empty() => {
            format!("{t}  |  {base_name}")
        }
        _ => base_name.clone(),
    }
    .replace('"', "\u{201d}");
    let media_title = apply_title_translate(&raw_title, ctx.title_translate);
    let media_basename = basename(&media_path).to_owned();
    let total_sec = match source.run_time_ticks {
        Some(ticks) if ticks > 0 => ticks / TICKS_PER_SEC,
        _ => RUNTIME_FALLBACK_SEC,
    };

    let selection = subtitle_checker(
        &source.media_streams,
        ctx.need_check_inner_sub,
        ctx.mount_disk_mode,
        ctx.subtitle_priority,
    );
    let sub_file = selection.selected.as_ref().map(|stream| {
        let ext = extension_with_dot(stream.path.as_deref().unwrap_or(""));
        format!(
            "{}://{}/Videos/{item_id}/{media_source_id}/Subtitles/{}/Stream{ext}",
            ctx.scheme,
            ctx.netloc,
            stream.index.unwrap_or(0),
        )
    });

    let is_current = base_name == ctx.main_ep_basename;
    let start_sec = if is_current { base.start_sec } else { 0 };
    let position = if total_sec > 0 {
        start_sec as f64 / total_sec as f64
    } else {
        0.0
    };

    Some(PlaybackData {
        basename: base_name,
        media_basename,
        item_id,
        media_source_id,
        file_path,
        source_path,
        stream_url,
        media_path,
        fake_name: fake_name(item.path.as_deref().unwrap_or("")),
        total_sec,
        start_sec,
        position,
        size: source.size.unwrap_or(0),
        media_title,
        index: Some(index),
        order: Some(order),
        is_start_file: false,
        intro: etlp_core::IntroMarkers {
            start: ctx.maps.start.get(&unique_key).copied(),
            end: ctx.maps.end.get(&unique_key).copied(),
        },
        sub: Subtitle {
            external: sub_file,
            inner_index: (selection.sub_inner_idx > 0)
                .then_some(selection.sub_inner_idx),
        },
        item_type: item.item_type.clone().unwrap_or_default(),
        provider_ids: item.provider_ids.clone(),
        series_id: item.series_id.clone().unwrap_or_default(),
        ..base.clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{Chapter, MediaSource, MediaStream};

    fn season_ep(parent: i64, index: i64, name: &str) -> Item {
        Item {
            name: Some(name.to_owned()),
            season_id: Some("s1".to_owned()),
            series_name: Some("Show".to_owned()),
            parent_index_number: Some(parent),
            index_number: Some(index),
            ..Item::default()
        }
    }

    #[test]
    fn title_map_built_per_episode() {
        let mut e1 = season_ep(1, 1, "Pilot");
        e1.chapters = vec![
            Chapter {
                marker_type: Some("IntroStart".to_owned()),
                start_position_ticks: 100_000_001,
            },
            Chapter {
                marker_type: Some("IntroEnd".to_owned()),
                start_position_ticks: 900_000_001,
            },
        ];
        let e2 = season_ep(1, 2, "Second");
        let (maps, fail) = build_title_intro_maps(&[e1, e2], "s1", false);
        assert!(!fail);
        assert_eq!(
            maps.title.get("1-1").map(String::as_str),
            Some("Show S1:E1 - Pilot")
        );
        assert_eq!(maps.start.get("1-1").copied(), Some(10));
        assert_eq!(maps.end.get("1-1").copied(), Some(90));
        // E2 has no chapters -> no intro entries.
        assert!(!maps.start.contains_key("1-2"));
    }

    #[test]
    fn missing_index_fails_the_map() {
        let mut bad = season_ep(1, 1, "X");
        bad.index_number = None;
        let (_, fail) = build_title_intro_maps(&[bad], "s1", false);
        assert!(fail);
    }

    #[test]
    fn empty_episodes_fail_flag() {
        let (_, fail) = build_title_intro_maps(&[], "s1", false);
        assert!(fail);
    }

    fn ctx<'a>(maps: &'a TitleIntroMaps) -> EpisodeContext<'a> {
        EpisodeContext {
            scheme: "https",
            netloc: "h:8096",
            extra: "/emby",
            stream_name: "original",
            device_id: "DEV",
            play_session_id: "ps",
            api_key: "KEY",
            is_strm: false,
            strm_direct: false,
            is_http_direct_strm: false,
            mount_disk_mode: false,
            pretty_title: true,
            main_ep_basename: "s01e01.mkv",
            need_check_inner_sub: -3,
            subtitle_priority: &[],
            path_pairs: &[],
            maps,
            title_translate: &[],
        }
    }

    fn ep_with_source(id: &str, index: i64, path: &str, ticks: i64) -> Item {
        Item {
            id: id.to_owned(),
            path: Some(path.to_owned()),
            parent_index_number: Some(1),
            index_number: Some(index),
            media_sources: vec![MediaSource {
                id: format!("src-{id}"),
                path: path.to_owned(),
                run_time_ticks: Some(ticks),
                size: Some(123),
                ..MediaSource::default()
            }],
            ..Item::default()
        }
    }

    #[test]
    fn parse_item_builds_network_entry() {
        let mut maps = TitleIntroMaps::default();
        maps.title
            .insert("1-2".to_owned(), "Show S1:E2 - Two".to_owned());
        let context = ctx(&maps);
        let base = PlaybackData {
            start_sec: 60,
            ..PlaybackData::default()
        };
        let item = ep_with_source("200", 2, "/m/s01e02.mkv", 12_000_000_000);
        let data =
            parse_episode_item(&context, &base, &item, 5).expect("entry");

        assert_eq!(data.item_id, "200");
        assert_eq!(data.media_source_id, "src-200");
        assert_eq!(data.basename, "s01e02.mkv");
        assert_eq!(data.order, Some(5));
        assert_eq!(data.index, Some(2));
        assert_eq!(data.total_sec, 1200);
        assert_eq!(data.size, 123);
        assert!(data.media_path.contains("/emby/videos/200/original.mkv"));
        assert_eq!(data.media_path, data.stream_url);
        assert_eq!(data.media_title, "Show S1:E2 - Two  |  s01e02.mkv");
        // Not the current episode -> position resets to 0.
        assert_eq!(data.start_sec, 0);
    }

    #[test]
    fn current_episode_keeps_base_position() {
        let maps = TitleIntroMaps::default();
        let context = ctx(&maps);
        let base = PlaybackData {
            start_sec: 60,
            ..PlaybackData::default()
        };
        let item = ep_with_source("100", 1, "/m/s01e01.mkv", 12_000_000_000);
        let data =
            parse_episode_item(&context, &base, &item, 0).expect("entry");
        assert_eq!(data.start_sec, 60);
        assert!((data.position - 0.05).abs() < 1e-9);
    }

    #[test]
    fn external_subtitle_builds_videos_url() {
        let maps = TitleIntroMaps::default();
        let mut context = ctx(&maps);
        let priority = vec!["chs".to_owned()];
        context.subtitle_priority = &priority;
        let base = PlaybackData::default();
        let mut item =
            ep_with_source("100", 1, "/m/s01e01.mkv", 12_000_000_000);
        if let Some(source) = item.media_sources.first_mut() {
            source.media_streams = vec![MediaStream {
                stream_type: "Subtitle".to_owned(),
                index: Some(3),
                is_external: true,
                display_title: "chs".to_owned(),
                path: Some("/m/s01e01.chs.ass".to_owned()),
                ..MediaStream::default()
            }];
        }
        let data =
            parse_episode_item(&context, &base, &item, 0).expect("entry");
        assert_eq!(
            data.sub.external.as_deref(),
            Some("https://h:8096/Videos/100/src-100/Subtitles/3/Stream.ass")
        );
    }
}
