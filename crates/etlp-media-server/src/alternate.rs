//! Alternate-shape (`AlternateMediaSources`) episode assembly.
//!
//! Newer Emby servers, when asked for the `AlternateMediaSources` field, return
//! one item per episode whose `MediaSources` array lists *every* version, each
//! carrying its own `ItemId`. This is a different shape from the legacy list
//! (one item per version) handled by [`crate::list_episodes`]; the two are kept
//! deliberately separate so a change to one cannot silently break the other.
//!
//! Assembly here is a per-episode *source selection* — pick the active version
//! for the current episode and a consistent version elsewhere — after which the
//! one-version-per-episode items flow through the shared
//! [`finalize_playlist`](crate::list_episodes::finalize_playlist) tail.

use etlp_core::PlaybackData;
use tracing::debug;

use crate::dto::{Item, MediaSource};
use crate::list_episodes::{ListContext, finalize_playlist};
use crate::version::select_version_index;

/// The final path component, splitting on both separators.
fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// The version-discriminating label of a source, lowercased for matching.
///
/// Prefers the source `Name` (the release label, e.g. `"WEB-DL.LINETV"`) and
/// falls back to the file basename when the server sends no name.
fn version_label(source: &MediaSource) -> String {
    if source.name.is_empty() {
        basename(&source.path).to_lowercase()
    } else {
        source.name.to_lowercase()
    }
}

/// Pick the one source to play for a single episode.
///
/// The currently playing source (matched by id) always wins, so the playlist
/// entry for the active episode is exactly the version the user selected. Every
/// other episode falls back to `preference` keywords (the active version's
/// label first, then `dev.version_prefer`), keeping one consistent release
/// across the season. Returns `None` only for an episode with no source.
fn pick_alt_source<'a>(
    item: &'a Item,
    current_source_id: &str,
    preference: &[String],
) -> Option<&'a MediaSource> {
    if !current_source_id.is_empty()
        && let Some(found) = item
            .media_sources
            .iter()
            .find(|s| s.id == current_source_id)
    {
        return Some(found);
    }
    if item.media_sources.is_empty() {
        return None;
    }
    let labels: Vec<String> =
        item.media_sources.iter().map(version_label).collect();
    let index = select_version_index(&labels, preference);
    item.media_sources.get(index)
}

/// Project the chosen source onto a single-version item.
///
/// The shared per-episode builder expects one item to mean one version, so the
/// chosen source's own `ItemId` becomes the item id (addressing the right Emby
/// item — the played version stays in sync with the selection) and its media
/// path becomes the item path, while episode metadata (indices, name, provider
/// ids, series) is inherited from the parent item.
fn version_item(parent: &Item, source: &MediaSource) -> Item {
    let id = source
        .item_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(&parent.id)
        .to_owned();
    Item {
        id,
        path: Some(source.path.clone()),
        media_sources: vec![source.clone()],
        ..parent.clone()
    }
}

/// Whether `items` carry the legacy one-item-per-version shape instead of the
/// collapsed alternate shape — detected when some episode (keyed by parent and
/// index number) appears more than once. A server that ignored the
/// `AlternateMediaSources` request returns this legacy shape, which the caller
/// must route to [`crate::list_episodes::assemble_episodes`] instead.
#[must_use]
pub fn looks_like_legacy_shape(items: &[Item]) -> bool {
    let mut seen: Vec<(Option<i64>, i64)> = Vec::new();
    for item in items {
        let Some(index) = item.index_number else {
            // Items with no index cannot be keyed; skip them for this signal.
            continue;
        };
        let key = (item.parent_index_number, index);
        if seen.contains(&key) {
            return true;
        }
        seen.push(key);
    }
    false
}

/// Assemble the playlist from the alternate (`AlternateMediaSources`) shape.
///
/// Each fetched item is already one episode whose `MediaSources` lists every
/// version. One source is selected per episode — the active version for the
/// current episode, the preferred version elsewhere — and the resulting
/// one-version-per-episode items are handed to the shared
/// [`finalize_playlist`] tail.
#[must_use]
pub fn assemble_episodes_alt(
    ctx: &ListContext,
    fetched: &[Item],
) -> Vec<PlaybackData> {
    let current_id = ctx.base.media_source_id.as_str();
    // The label of the version the user is watching takes top preference so the
    // rest of the season follows the same release whenever it is available.
    let mut preference: Vec<String> = Vec::new();
    if let Some(label) = fetched
        .iter()
        .flat_map(|item| &item.media_sources)
        .find(|s| s.id == current_id)
        .map(version_label)
    {
        preference.push(label);
    }
    preference.extend(ctx.config.version_prefer.iter().cloned());

    let mut episodes: Vec<Item> = fetched
        .iter()
        .filter_map(|item| {
            let source = pick_alt_source(item, current_id, &preference)?;
            Some(version_item(item, source))
        })
        .collect();
    episodes.sort_by_key(|item| (item.parent_index_number, item.index_number));

    debug!(
        "assemble_episodes_alt: selected {} of {} fetched episodes \
         (current_id={:?}, preference={:?})",
        episodes.len(),
        fetched.len(),
        current_id,
        preference
    );

    finalize_playlist(ctx, &episodes)
}

#[cfg(test)]
mod tests {
    use etlp_core::Server;

    use super::*;
    use crate::parse::EmbyParseConfig;

    /// One source in the alternate shape: id `mediasource_<item>` plus the
    /// matching `ItemId`, a release label and a media path carrying that label.
    fn source(item: &str, label: &str) -> MediaSource {
        MediaSource {
            id: format!("mediasource_{item}"),
            item_id: Some(item.to_owned()),
            name: label.to_owned(),
            path: format!("/mnt/media/S01E0x {label}.mkv"),
            run_time_ticks: Some(12_000_000_000),
            ..MediaSource::default()
        }
    }

    /// One collapsed episode with all its versions, ordered Baha/LINETV/CR.
    fn alt_episode(primary_item: &str, index: i64) -> Item {
        Item {
            id: primary_item.to_owned(),
            path: Some(format!("/mnt/strm/S01E0{index}.strm")),
            parent_index_number: Some(1),
            index_number: Some(index),
            media_sources: vec![
                source(&format!("{primary_item}0"), "WEB-DL.Baha"),
                source(&format!("{primary_item}1"), "WEB-DL.LINETV"),
                source(&format!("{primary_item}2"), "WEB-DL.CR"),
            ],
            ..Item::default()
        }
    }

    #[test]
    fn pick_alt_source_prefers_current_id() {
        let ep = alt_episode("29", 1);
        // Current selection is the LINETV source -> it wins over preference.
        let chosen =
            pick_alt_source(&ep, "mediasource_291", &[]).expect("a source");
        assert_eq!(chosen.id, "mediasource_291");
        assert_eq!(chosen.name, "WEB-DL.LINETV");
    }

    #[test]
    fn pick_alt_source_falls_back_to_preference_label() {
        let ep = alt_episode("30", 2);
        // Not the current episode (no id match); preference points to LINETV.
        let preference = vec!["web-dl.linetv".to_owned()];
        let chosen = pick_alt_source(&ep, "mediasource_291", &preference)
            .expect("a source");
        assert_eq!(chosen.name, "WEB-DL.LINETV");
    }

    #[test]
    fn pick_alt_source_defaults_to_first_without_preference() {
        let ep = alt_episode("31", 3);
        let chosen = pick_alt_source(&ep, "absent", &[]).expect("a source");
        assert_eq!(chosen.name, "WEB-DL.Baha");
    }

    #[test]
    fn version_item_addresses_the_versions_own_item() {
        let ep = alt_episode("29", 1);
        let src = ep.media_sources.get(1).expect("LINETV source, ItemId 291");
        let item = version_item(&ep, src);
        assert_eq!(item.id, "291", "must address the version's own item id");
        assert_eq!(item.path.as_deref(), Some(src.path.as_str()));
        assert_eq!(item.media_sources.len(), 1);
        // Episode metadata is inherited from the parent.
        assert_eq!(item.index_number, Some(1));
        assert_eq!(item.parent_index_number, Some(1));
    }

    fn base_playing(current_source_id: &str) -> PlaybackData {
        PlaybackData {
            server: Server::Emby,
            scheme: "https".into(),
            netloc: "h:8096".into(),
            api_key: "KEY".into(),
            device_id: "DEV".into(),
            play_session_id: "ps".into(),
            server_version: "4.9.5.0".into(),
            media_source_id: current_source_id.into(),
            ..PlaybackData::default()
        }
    }

    #[test]
    fn legacy_shape_detected_by_duplicate_episode_keys() {
        // Two items share key (1, 1) -> legacy one-item-per-version shape.
        let legacy = vec![
            Item {
                id: "a".into(),
                parent_index_number: Some(1),
                index_number: Some(1),
                ..Item::default()
            },
            Item {
                id: "b".into(),
                parent_index_number: Some(1),
                index_number: Some(1),
                ..Item::default()
            },
        ];
        assert!(looks_like_legacy_shape(&legacy));
        // The collapsed alternate shape has one item per episode.
        let collapsed = vec![alt_episode("29", 1), alt_episode("30", 2)];
        assert!(!looks_like_legacy_shape(&collapsed));
    }

    #[test]
    fn assemble_keeps_selected_version_and_stays_consistent() {
        // Watching episode 1's LINETV version (item 291). The playlist must
        // pin episode 1 to that exact version and pick LINETV for episode 2.
        let cfg = EmbyParseConfig::default();
        let played = base_playing("mediasource_291");
        let ctx = ListContext {
            base: &played,
            episodes_info: &[],
            season_id: "s1",
            playlist: false,
            config: &cfg,
        };
        let fetched = vec![alt_episode("29", 1), alt_episode("30", 2)];
        let result = assemble_episodes_alt(&ctx, &fetched);

        assert_eq!(result.len(), 2);
        let ep1 = result.first().expect("ep1");
        assert_eq!(
            ep1.item_id, "291",
            "current episode keeps selected version"
        );
        assert_eq!(ep1.media_source_id, "mediasource_291");
        assert!(ep1.stream_url.contains("/emby/videos/291/"));

        let ep2 = result.get(1).expect("ep2");
        assert_eq!(
            ep2.item_id, "301",
            "next episode follows same LINETV release"
        );
        assert_eq!(ep2.media_source_id, "mediasource_301");
    }
}
