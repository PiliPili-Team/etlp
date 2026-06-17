//! Multi-version playlist filtering helpers, ported from the nested functions
//! of `data_parser.list_episodes`.
//!
//! When several files back the same episode (e.g. `S01E01.mkv` and
//! `S01E01 - VCB.mkv`), the playlist must collapse them to one entry per
//! episode. This module ports the two *pure* building blocks used by the larger
//! `version_filter` heuristic:
//!
//! * [`shortest_episode`] — `multi_ver_find_sortest_ep`: among files for one
//!   episode, the one with the shortest basename, plus whether every sibling
//!   name starts with that file's stem (i.e. they are real derivations of it);
//! * [`filter_by_raw_name`] — `version_filter_by_raw_name`: keep one file per
//!   episode by that rule, stopping at the first episode that is not a clean
//!   derivation.
//!
//! The full `version_filter` orchestration (official-rule / clean-path / ini
//! regex passes) is ported separately.

use crate::dto::Item;

/// The playlist key `"{ParentIndexNumber}-{IndexNumber}"`, or `None` when either
/// index is missing (the Python `ep_to_key` would `KeyError`).
#[must_use]
pub fn episode_key(item: &Item) -> Option<String> {
    let parent = item.parent_index_number?;
    let index = item.index_number?;
    Some(format!("{parent}-{index}"))
}

/// The basename (final path component) of a path, splitting on both separators.
fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// The stem (basename without its final extension) of a path.
fn stem(path: &str) -> &str {
    let name = basename(path);
    match name.rsplit_once('.') {
        // A leading dot (dotfile) is part of the name, not an extension.
        Some((head, _)) if !head.is_empty() => head,
        _ => name,
    }
}

/// Among files for one episode, find the one with the shortest basename and
/// report whether every file's name starts with that file's stem.
///
/// Returns `None` only for an empty slice. Mirrors `multi_ver_find_sortest_ep`.
#[must_use]
pub fn shortest_episode(same_episode: &[Item]) -> Option<(&Item, bool)> {
    let shortest = same_episode.iter().min_by_key(|item| {
        basename(item.path.as_deref().unwrap_or("")).chars().count()
    })?;
    let shortest_stem = stem(shortest.path.as_deref().unwrap_or(""));
    let is_real_raw = same_episode.iter().all(|item| {
        basename(item.path.as_deref().unwrap_or("")).starts_with(shortest_stem)
    });
    Some((shortest, is_real_raw))
}

/// Keep one file per episode by the shortest-stem rule, stopping at the first
/// episode whose files are not all derivations of the shortest one.
///
/// Episodes are grouped by [`episode_key`] in first-seen order. Mirrors
/// `version_filter_by_raw_name`; an episode whose key cannot be built is
/// treated as its own singleton group (kept as-is).
#[must_use]
pub fn filter_by_raw_name(episodes: &[Item]) -> Vec<Item> {
    let mut groups: Vec<(Option<String>, Vec<&Item>)> = Vec::new();
    for ep in episodes {
        let key = episode_key(ep);
        match groups.iter_mut().find(|(k, _)| *k == key && key.is_some()) {
            Some((_, items)) => items.push(ep),
            None => groups.push((key, vec![ep])),
        }
    }

    let mut success = Vec::new();
    for (_, eps) in groups {
        if eps.len() == 1 {
            if let Some(first) = eps.first() {
                success.push((*first).clone());
            }
            continue;
        }
        let owned: Vec<Item> = eps.iter().map(|e| (*e).clone()).collect();
        match shortest_episode(&owned) {
            Some((ep, true)) => success.push(ep.clone()),
            _ => break,
        }
    }
    success
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(id: &str, parent: i64, index: i64, path: &str) -> Item {
        Item {
            id: id.to_owned(),
            path: Some(path.to_owned()),
            parent_index_number: Some(parent),
            index_number: Some(index),
            ..Item::default()
        }
    }

    #[test]
    fn episode_key_requires_both_indices() {
        assert_eq!(
            episode_key(&ep("1", 1, 2, "/m/x.mkv")).as_deref(),
            Some("1-2")
        );
        let mut missing = ep("1", 1, 2, "/m/x.mkv");
        missing.index_number = None;
        assert!(episode_key(&missing).is_none());
    }

    #[test]
    fn shortest_episode_detects_real_derivations() {
        // "S01E01.mkv" is the raw; "S01E01 - VCB.mkv" derives from its stem.
        let eps = vec![
            ep("a", 1, 1, "/m/S01E01 - VCB.mkv"),
            ep("b", 1, 1, "/m/S01E01.mkv"),
        ];
        let (chosen, is_real_raw) = shortest_episode(&eps).expect("non-empty");
        assert_eq!(chosen.id, "b");
        assert!(is_real_raw);
    }

    #[test]
    fn shortest_episode_flags_unrelated_names() {
        let eps = vec![
            ep("a", 1, 1, "/m/Apple.mkv"),
            ep("b", 1, 1, "/m/Banana.mkv"),
        ];
        let (chosen, is_real_raw) = shortest_episode(&eps).expect("non-empty");
        assert_eq!(chosen.id, "a"); // shorter basename
        assert!(!is_real_raw);
    }

    #[test]
    fn filter_by_raw_name_keeps_one_per_episode() {
        let eps = vec![
            ep("a1", 1, 1, "/m/S01E01.mkv"),
            ep("a2", 1, 1, "/m/S01E01 - VCB.mkv"),
            ep("b1", 1, 2, "/m/S01E02.mkv"),
        ];
        let res = filter_by_raw_name(&eps);
        let ids: Vec<&str> = res.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["a1", "b1"]);
    }

    #[test]
    fn filter_by_raw_name_stops_at_unrelated_episode() {
        let eps = vec![
            ep("a1", 1, 1, "/m/S01E01.mkv"),
            ep("a2", 1, 1, "/m/S01E01 - VCB.mkv"),
            ep("b1", 1, 2, "/m/Apple.mkv"),
            ep("b2", 1, 2, "/m/Banana.mkv"),
        ];
        let res = filter_by_raw_name(&eps);
        // Episode 1 collapses; episode 2 is not a clean derivation -> stop.
        let ids: Vec<&str> = res.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["a1"]);
    }
}
