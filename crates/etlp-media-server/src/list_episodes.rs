//! Episode-list assembly, ported from the body of `data_parser.list_episodes`
//! (everything after the network fetch).
//!
//! [`assemble_episodes`] takes the raw season episodes (already fetched by the
//! caller via [`crate::emby::EmbyClient::episodes`]) plus the played context and
//! turns them into the ordered [`PlaybackData`] playlist: it drops broken
//! entries, syncs strm paths, runs [`version_filter`], parses each episode and
//! applies the per-episode `stream_redirect` / `stream_prefix` rewrites.
//!
//! Keeping the network fetch in the caller makes this a pure, fully testable
//! transformation. The movie short-circuit (no season) is handled before the
//! fetch and never reaches here.

use etlp_config::matching::{match_pair, matches};
use etlp_core::{PlaybackData, Server};

use crate::dto::Item;
use crate::episodes::{
    EpisodeContext, build_title_intro_maps, parse_episode_item,
};
use crate::parse::EmbyParseConfig;
use crate::version::match_version_range;
use crate::version_filter::{VersionFilterInput, version_filter};

/// Emby switched the static stream path name to `original` in this range.
const ORIGINAL_RANGE: &str = "4.8.0.40-9";

/// The played context plus config needed to assemble the playlist.
#[derive(Debug, Clone)]
pub struct ListContext<'a> {
    /// The currently-playing item (connection + identity inherited per entry).
    pub base: &'a PlaybackData,
    /// The userscript-provided season episodes, used for title/intro maps.
    pub episodes_info: &'a [Item],
    /// The season id whose episodes are being listed.
    pub season_id: &'a str,
    /// Whether this is a shuffle playlist (skips version filtering/titles).
    pub playlist: bool,
    /// Resolved configuration.
    pub config: &'a EmbyParseConfig,
}

/// Whether any fetched episode is missing a path or runtime (the Python
/// `eps_error`), plus the source ids of the path-less ones and whether any of
/// the broken ones also lacks an index number.
struct EpisodeErrors<'a> {
    any: bool,
    index_missing: bool,
    path_error_source_ids: Vec<&'a str>,
}

fn scan_errors(fetched: &[Item]) -> EpisodeErrors<'_> {
    let mut errors = EpisodeErrors {
        any: false,
        index_missing: false,
        path_error_source_ids: Vec::new(),
    };
    for item in fetched {
        let missing_path = item.path.is_none();
        let missing_runtime = item.run_time_ticks.is_none();
        if !(missing_path || missing_runtime) {
            continue;
        }
        errors.any = true;
        if item.index_number.is_none() {
            errors.index_missing = true;
        }
        if missing_path {
            if let Some(source) = item.media_sources.first() {
                errors.path_error_source_ids.push(source.id.as_str());
            }
        }
    }
    errors
}

/// Assemble the ordered playlist from the fetched season episodes.
///
/// Degrades to just the current episode (disabling the forward playlist) when
/// the data is too broken to build a reliable list.
#[must_use]
pub fn assemble_episodes(
    ctx: &ListContext,
    fetched: &[Item],
) -> Vec<PlaybackData> {
    let base = ctx.base;

    let errors = scan_errors(fetched);
    if errors.any {
        if errors.index_missing {
            return vec![base.clone()];
        }
        if errors
            .path_error_source_ids
            .iter()
            .any(|id| *id == base.media_source_id)
        {
            return vec![base.clone()];
        }
    }

    let mut episodes: Vec<Item> = fetched
        .iter()
        .filter(|i| i.path.is_some())
        .cloned()
        .collect();

    // strm_file_name_sync: local strm items expose their real path on the
    // first media source.
    if base.is_strm && !base.is_http_source {
        for ep in &mut episodes {
            if let Some(path) = ep.media_sources.first().map(|s| s.path.clone())
            {
                ep.path = Some(path);
            }
        }
    }

    if base.server == Server::Emby {
        let vf_input = VersionFilterInput {
            file_path: &base.file_path,
            playlist: ctx.playlist,
            version_filter_re: &ctx.config.version_filter,
            media_source_id: &base.media_source_id,
            version_prefer: &ctx.config.version_prefer,
            version_prefer_enabled: ctx.config.version_prefer_for_playlist,
        };
        episodes = version_filter(&vf_input, &episodes);
    }

    let (maps, title_fail) =
        build_title_intro_maps(ctx.episodes_info, ctx.season_id, ctx.playlist);
    let extra = if base.server == Server::Emby {
        "/emby"
    } else {
        ""
    };
    let stream_name =
        if match_version_range(&base.server_version, ORIGINAL_RANGE) {
            "original"
        } else {
            "stream"
        };
    let need_check_inner_sub = if base.sub.inner_index.is_some() {
        -1
    } else {
        -3
    };

    let ep_ctx = EpisodeContext {
        scheme: &base.scheme,
        netloc: &base.netloc,
        extra,
        stream_name,
        device_id: &base.device_id,
        play_session_id: &base.play_session_id,
        api_key: &base.api_key,
        is_strm: base.is_strm,
        strm_direct: base.strm_direct,
        is_http_direct_strm: base.is_http_direct_strm,
        mount_disk_mode: base.mount_disk_mode,
        pretty_title: ctx.config.pretty_title,
        main_ep_basename: &base.basename,
        need_check_inner_sub,
        subtitle_priority: &ctx.config.subtitle_priority,
        path_pairs: &ctx.config.path_pairs,
        maps: &maps,
    };

    let mut result: Vec<PlaybackData> = episodes
        .iter()
        .enumerate()
        .filter_map(|(order, item)| {
            parse_episode_item(&ep_ctx, base, item, order as i64)
        })
        .collect();

    if title_fail {
        for ep in &mut result {
            if ep.file_path == base.file_path {
                ep.media_title = base.media_title.clone();
            }
        }
    }

    apply_stream_rewrites(&mut result, base, ctx.config);
    result
}

/// Apply the per-episode `stream_redirect` replacement and `stream_prefix`
/// prepend, keeping `media_path` in sync for network play.
fn apply_stream_rewrites(
    result: &mut [PlaybackData],
    base: &PlaybackData,
    config: &EmbyParseConfig,
) {
    let pair = result.first().and_then(|ep| {
        match_pair(&ep.stream_url, &config.stream_redirect)
            .map(|(from, to)| (from.to_owned(), to.to_owned()))
    });
    if let Some((from, to)) = pair {
        for ep in result.iter_mut() {
            ep.stream_url = ep.stream_url.replace(&from, &to);
            if !base.mount_disk_mode {
                ep.media_path = ep.stream_url.clone();
            }
        }
    }

    if matches(&base.netloc, &config.stream_prefix) {
        if let Some(prefix) = config.stream_prefix.first() {
            let prefix = prefix.trim_matches('/');
            for ep in result.iter_mut() {
                if ep.stream_url.starts_with(prefix) {
                    continue;
                }
                ep.stream_url = format!("{prefix}{}", ep.stream_url);
                if !base.mount_disk_mode {
                    ep.media_path = ep.stream_url.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::MediaSource;

    fn base() -> PlaybackData {
        PlaybackData {
            server: Server::Emby,
            scheme: "https".into(),
            netloc: "h:8096".into(),
            api_key: "KEY".into(),
            device_id: "DEV".into(),
            play_session_id: "ps".into(),
            server_version: "4.9.5.0".into(),
            file_path: "/m/s01e01.mkv".into(),
            basename: "s01e01.mkv".into(),
            media_source_id: "src-100".into(),
            ..PlaybackData::default()
        }
    }

    fn episode(id: &str, index: i64, path: &str) -> Item {
        Item {
            id: id.to_owned(),
            path: Some(path.to_owned()),
            parent_index_number: Some(1),
            index_number: Some(index),
            media_sources: vec![MediaSource {
                id: format!("src-{id}"),
                path: path.to_owned(),
                run_time_ticks: Some(12_000_000_000),
                ..MediaSource::default()
            }],
            ..Item::default()
        }
    }

    fn config() -> EmbyParseConfig {
        EmbyParseConfig::default()
    }

    #[test]
    fn assembles_ordered_playlist() {
        let b = base();
        let cfg = config();
        let ctx = ListContext {
            base: &b,
            episodes_info: &[],
            season_id: "s1",
            playlist: false,
            config: &cfg,
        };
        let fetched = vec![
            episode("100", 1, "/m/s01e01.mkv"),
            episode("101", 2, "/m/s01e02.mkv"),
        ];
        let res = assemble_episodes(&ctx, &fetched);
        assert_eq!(res.len(), 2);
        assert_eq!(res.first().map(|e| e.item_id.as_str()), Some("100"));
        assert_eq!(res.first().and_then(|e| e.order), Some(0));
        assert_eq!(res.get(1).and_then(|e| e.order), Some(1));
        assert!(
            res.first()
                .map(|e| e.media_path.contains("/emby/videos/100/"))
                .unwrap_or(false)
        );
    }

    #[test]
    fn broken_current_source_disables_playlist() {
        let b = base();
        let cfg = config();
        let ctx = ListContext {
            base: &b,
            episodes_info: &[],
            season_id: "s1",
            playlist: false,
            config: &cfg,
        };
        // The current source has no Path -> playlist disabled to [base].
        let mut broken = episode("100", 1, "/m/s01e01.mkv");
        broken.path = None;
        let fetched = vec![broken, episode("101", 2, "/m/s01e02.mkv")];
        let res = assemble_episodes(&ctx, &fetched);
        assert_eq!(res.len(), 1);
        assert_eq!(
            res.first().map(|e| e.basename.as_str()),
            Some("s01e01.mkv")
        );
    }

    #[test]
    fn stream_prefix_is_prepended() {
        let b = base();
        let cfg = EmbyParseConfig {
            stream_prefix: vec![
                "https://proxy/".to_owned(),
                "h:8096".to_owned(),
            ],
            ..EmbyParseConfig::default()
        };
        let ctx = ListContext {
            base: &b,
            episodes_info: &[],
            season_id: "s1",
            playlist: false,
            config: &cfg,
        };
        let fetched = vec![episode("100", 1, "/m/s01e01.mkv")];
        let res = assemble_episodes(&ctx, &fetched);
        assert!(
            res.first()
                .map(|e| e.stream_url.starts_with("https://proxy"))
                .unwrap_or(false)
        );
    }
}
