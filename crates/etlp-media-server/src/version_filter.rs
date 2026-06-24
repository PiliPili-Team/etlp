//! Multi-version playlist filtering helpers.
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

use regex::RegexBuilder;
use tracing::debug;

use crate::dto::Item;
use crate::prefer::version_prefer_for_playlist;

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

/// Inputs for [`version_filter`], gathered from config and the played item.
#[derive(Debug, Clone)]
pub struct VersionFilterInput<'a> {
    /// The played file's path (`data['file_path']`).
    pub file_path: &'a str,
    /// Whether this is a shuffle playlist (`playlist_info`); skips filtering.
    pub playlist: bool,
    /// The `[playlist] version_filter` regex (already read from ini).
    pub version_filter_re: &'a str,
    /// The played item's media source id (fallback to locate the current ep).
    pub media_source_id: &'a str,
    /// `dev.version_prefer` keywords.
    pub version_prefer: &'a [String],
    /// Whether `dev.version_prefer_for_playlist` is enabled.
    pub version_prefer_enabled: bool,
}

/// Whether an item's path contains `rule`.
fn path_contains(item: &Item, rule: &str) -> bool {
    item.path.as_deref().is_some_and(|p| p.contains(rule))
}

/// Path comparison tolerant of platform separator differences.
///
/// Emby on Windows may use backslashes while the userscript sends the same
/// path with forward slashes. Normalising before comparing avoids a silent
/// miss that would cause `version_filter` to return all episodes unfiltered.
fn paths_equal(a: &str, b: &str) -> bool {
    a == b || a.replace('\\', "/") == b.replace('\\', "/")
}

/// Locate the currently playing episode by file path, falling back to the
/// media source id (mirrors the two lookups in `version_filter`).
fn find_current<'a>(
    episodes: &'a [Item],
    file_path: &str,
    media_source_id: &str,
) -> Option<&'a Item> {
    let by_path = episodes.iter().find(|ep| {
        ep.path
            .as_deref()
            .is_some_and(|p| paths_equal(p, file_path))
            || ep
                .media_sources
                .first()
                .is_some_and(|s| paths_equal(&s.path, file_path))
    });
    if let Some(found) = by_path {
        debug!(
            "find_current: matched by path id={:?} path={:?}",
            found.id,
            found.path.as_deref().unwrap_or("<none>")
        );
        return Some(found);
    }
    let by_id = episodes.iter().find(|ep| {
        ep.media_sources
            .first()
            .is_some_and(|s| s.id == media_source_id)
    });
    match &by_id {
        Some(found) => debug!(
            "find_current: matched by media_source_id={:?} id={:?}",
            media_source_id, found.id
        ),
        None => {
            let sample: Vec<&str> = episodes
                .iter()
                .take(6)
                .map(|e| e.path.as_deref().unwrap_or("<none>"))
                .collect();
            debug!(
                "find_current: no match file_path={:?} \
                 media_source_id={:?} ep_count={} \
                 first_paths={:?}",
                file_path,
                media_source_id,
                episodes.len(),
                sample
            );
        }
    }
    by_id
}

/// The episodes from `ep_current` onward whose keys stay in lock-step with
/// `cut_keys` — the Python `check_ep_cur_is_sequence`.
fn sequence_from_current(
    ep_data: &[Item],
    ep_current: &Item,
    cut_keys: &[String],
) -> Vec<Item> {
    if cut_keys.len() == 1 {
        return vec![ep_current.clone()];
    }
    let Some(start) = ep_data.iter().position(|e| e == ep_current) else {
        debug!(
            "sequence_from_current: ep_current id={:?} not found \
             in ep_data ({} items) — returning empty",
            ep_current.id,
            ep_data.len()
        );
        return Vec::new();
    };
    let tail = ep_data.get(start..).unwrap_or(&[]);
    tail.iter()
        .zip(cut_keys.iter())
        .filter(|(ep, key)| episode_key(ep).as_deref() == Some(key.as_str()))
        .map(|(ep, _)| ep.clone())
        .collect()
}

/// The `official_rule` (text after the last `" - "`) and `clean_path` (text
/// after the first `E\d\d?`, dropped when too short) used by the builtin pass.
fn name_rules(file_path: &str) -> (Option<&str>, Option<String>) {
    let official = file_path.rsplit_once(" - ").map(|(_, tail)| tail);
    let clean = RegexBuilder::new(r"E\d\d?")
        .case_insensitive(true)
        .build()
        .ok()
        .and_then(|re| {
            let rest = re.splitn(file_path, 2).last().unwrap_or("").trim();
            if rest.chars().count() <= 5 {
                None
            } else {
                Some(rest.to_owned())
            }
        });
    (official, clean)
}

/// The builtin substring pass over `official_rule` / `clean_path`.
///
/// Returns `Ok(full_match)` when a rule selects exactly one file per key (the
/// Python early `return`), or `Err(seq_match)` carrying the best sequence
/// match found (possibly empty) for the caller to weigh against the ini pass.
fn builtin_pass(
    episodes: &[Item],
    eps_after: &[Item],
    ep_current: &Item,
    seq_keys: &[String],
    cut_keys: &[String],
    rules: (Option<&str>, Option<String>),
) -> Result<Vec<Item>, Vec<Item>> {
    let (official, clean) = rules;
    for (scope, data, keys) in
        [("full", episodes, seq_keys), ("tail", eps_after, cut_keys)]
    {
        for (rule_name, rule) in
            [("official", official), ("clean_path", clean.as_deref())]
                .into_iter()
                .filter_map(|(n, r)| r.map(|v| (n, v)))
        {
            let matched: Vec<Item> = data
                .iter()
                .filter(|i| path_contains(i, rule))
                .cloned()
                .collect();
            debug!(
                "builtin_pass: scope={} rule_name={} rule={:?} matched={}/{}",
                scope,
                rule_name,
                rule,
                matched.len(),
                keys.len()
            );
            if matched.len() == keys.len() && keys.len() > 1 {
                debug!(
                    "builtin_pass: exact full match via {} rule={:?} ({} episodes): {:?}",
                    rule_name,
                    rule,
                    matched.len(),
                    matched
                        .iter()
                        .map(|e| e.path.as_deref().unwrap_or("?"))
                        .collect::<Vec<_>>()
                );
                return Ok(matched);
            }
            let seq = sequence_from_current(&matched, ep_current, cut_keys);
            if seq.len() >= 2 {
                debug!(
                    "builtin_pass: partial sequence via {} rule={:?} ({} episodes from current)",
                    rule_name,
                    rule,
                    seq.len()
                );
                return Err(seq);
            }
        }
    }
    debug!("builtin_pass: no rule produced a usable match, returning empty");
    Err(Vec::new())
}

/// Filter a season's episodes down to one file per episode for the playlist.
///
/// Ports `data_parser.version_filter`. Every branch degrades safely: when the
/// heuristics cannot confidently build a sequence it returns just the current
/// episode (disabling the forward playlist) or the unfiltered list, never
/// panicking. Returns the chosen episode list.
#[must_use]
pub fn version_filter(
    input: &VersionFilterInput,
    episodes: &[Item],
) -> Vec<Item> {
    if input.playlist {
        debug!(
            "version_filter: playlist mode, passing all {} episodes through",
            episodes.len()
        );
        return episodes.to_vec();
    }

    let ver_re = input.version_filter_re.trim().trim_matches('|');

    // Build per-episode keys early so multi-version detection works even when
    // ver_re is empty (needed to apply version_prefer without a regex).
    let Some(keys): Option<Vec<String>> =
        episodes.iter().map(episode_key).collect()
    else {
        debug!(
            "version_filter: some episodes have missing indices, \
             returning all {} episodes unchanged",
            episodes.len()
        );
        return episodes.to_vec();
    };
    let seq_keys = dedup_keys(&keys);
    let ep_num = seq_keys.len();

    if ep_num == episodes.len() {
        // No multi-version episodes — nothing to filter.
        debug!(
            "version_filter: {} episodes, no multi-version detected, \
             returning all unchanged",
            episodes.len()
        );
        return episodes.to_vec();
    }

    debug!(
        "version_filter: {} raw episodes / {} unique episode keys \
         (multi-version present); ver_re={:?}, file_path={:?}",
        episodes.len(),
        ep_num,
        ver_re,
        input.file_path
    );

    // When no regex is configured, applying the ini pass would produce garbage
    // (an empty pattern matches everywhere).  Try version_prefer directly as
    // the sole version selector; if that's also disabled, return all.
    if ver_re.is_empty() {
        let Some(ep_current) =
            find_current(episodes, input.file_path, input.media_source_id)
        else {
            debug!(
                "version_filter: no regex and cannot locate current episode \
                 (file_path={:?}), returning all",
                input.file_path
            );
            return episodes.to_vec();
        };
        let Some(current_key) = episode_key(ep_current) else {
            return episodes.to_vec();
        };
        debug!(
            "version_filter: no regex — trying version_prefer \
             (enabled={}, {} rules, current_key={:?})",
            input.version_prefer_enabled,
            input.version_prefer.len(),
            current_key
        );
        let prefer = version_prefer_for_playlist(
            &keys,
            episodes,
            &[],
            &current_key,
            input.file_path,
            input.version_prefer,
            input.version_prefer_enabled,
        );
        return match prefer {
            Some(ref p) => {
                debug!(
                    "version_filter: version_prefer selected {} episodes: {:?}",
                    p.len(),
                    p.iter()
                        .map(|e| e.path.as_deref().unwrap_or("?"))
                        .collect::<Vec<_>>()
                );
                p.clone()
            }
            None => {
                debug!(
                    "version_filter: version_prefer disabled/no rules, \
                     returning all {} episodes",
                    episodes.len()
                );
                episodes.to_vec()
            }
        };
    }

    // ── regex-based path (ver_re is non-empty) ────────────────────────────────

    let Some(ep_current) =
        find_current(episodes, input.file_path, input.media_source_id)
    else {
        debug!(
            "version_filter: cannot locate current episode \
             (file_path={:?} media_source_id={:?}) \
             in multi-version path — returning all {} unchanged",
            input.file_path,
            input.media_source_id,
            episodes.len()
        );
        return episodes.to_vec();
    };
    let Some(current_key) = episode_key(ep_current) else {
        return episodes.to_vec();
    };
    let curr_count = keys.iter().filter(|k| **k == current_key).count();
    let curr_raw_index =
        keys.iter().position(|k| *k == current_key).unwrap_or(0);

    debug!(
        "version_filter: current_key={:?}, curr_count={}, curr_raw_index={}",
        current_key, curr_count, curr_raw_index
    );

    // Only the first episode is multi-version: collapse and we are done.
    if curr_count > 1 {
        let mut trimmed = Vec::with_capacity(episodes.len());
        for (i, ep) in episodes.iter().enumerate() {
            if i > curr_raw_index && i < curr_raw_index + curr_count {
                continue;
            }
            if i == curr_raw_index {
                trimmed.push(ep_current.clone());
            } else {
                trimmed.push(ep.clone());
            }
        }
        if ep_num == trimmed.len() {
            debug!(
                "version_filter: single multi-version episode collapsed, \
                 returning {} episodes",
                trimmed.len()
            );
            return trimmed;
        }
    }

    let cut_start =
        seq_keys.iter().position(|k| *k == current_key).unwrap_or(0);
    let cut_keys: Vec<String> =
        seq_keys.get(cut_start..).unwrap_or(&[]).to_vec();
    let eps_after: Vec<Item> = episodes
        .iter()
        .filter(|i| episode_key(i).is_some_and(|k| cut_keys.contains(&k)))
        .cloned()
        .collect();

    // Files derived from a raw original (S01E01.mkv -> S01E01 - ver.mkv).
    if curr_count > 1 {
        let group = episodes.get(curr_raw_index..curr_raw_index + curr_count);
        if let Some((shortest, true)) = group.and_then(shortest_episode)
            && shortest.path.as_deref() == Some(input.file_path)
        {
            let raw = filter_by_raw_name(episodes);
            if let Some(idx) = raw.iter().position(|e| e == ep_current)
                && raw.get(idx + 1..).is_some_and(|r| !r.is_empty())
            {
                debug!(
                    "version_filter: raw-name derivation rule selected {} episodes",
                    raw.len()
                );
                return raw;
            }
        }
    }

    let rules = name_rules(input.file_path);
    let builtin_res = match builtin_pass(
        episodes,
        &eps_after,
        ep_current,
        &seq_keys,
        &cut_keys,
        (rules.0, rules.1.clone()),
    ) {
        Ok(full) => {
            debug!(
                "version_filter: builtin pass (full match) selected {} episodes",
                full.len()
            );
            return full;
        }
        Err(seq) => seq,
    };

    debug!(
        "version_filter: builtin partial sequence len={}, proceeding to ini-regex pass",
        builtin_res.len()
    );

    // ini regex pass: derive tokens from the played path, keep episodes whose
    // path yields the same number of token matches.
    let single_line: String = ver_re.split('\n').collect();
    let Ok(outer) = RegexBuilder::new(&single_line)
        .case_insensitive(true)
        .build()
    else {
        debug!(
            "version_filter: invalid ini regex {:?}, falling back",
            ver_re
        );
        return fallback(builtin_res, ep_current);
    };
    let ini_tokens: Vec<String> = outer
        .find_iter(input.file_path)
        .map(|m| m.as_str().to_owned())
        .collect();
    debug!(
        "version_filter: ini-regex {:?} extracted tokens {:?} from {:?}",
        ver_re, ini_tokens, input.file_path
    );
    let combined = ini_tokens.join("|");
    let Ok(inner) = RegexBuilder::new(&combined).case_insensitive(true).build()
    else {
        return fallback(builtin_res, ep_current);
    };
    let token_count = ini_tokens.len();
    let ep_data: Vec<Item> = episodes
        .iter()
        .filter(|i| {
            let path = i.path.as_deref().unwrap_or("");
            let count = inner.find_iter(path).count();
            let matched = count == token_count;
            debug!(
                "version_filter: ini-regex {:?} tokens={} got={} matched={} path={:?}",
                ini_tokens, token_count, count, matched, path
            );
            matched
        })
        .cloned()
        .collect();
    debug!(
        "version_filter: ini-regex result: {}/{} episodes matched token_count={} \
         (need {} unique for exact)",
        ep_data.len(),
        episodes.len(),
        token_count,
        ep_num
    );
    if ep_data.len() == ep_num {
        debug!(
            "version_filter: [DECISION] ini-regex exact match, \
             locked {} episodes via tokens {:?}",
            ep_data.len(),
            ini_tokens
        );
        return ep_data;
    }

    let success_map: Vec<(String, Item)> = ep_data
        .iter()
        .filter_map(|i| episode_key(i).map(|k| (k, i.clone())))
        .collect();
    let prefer = version_prefer_for_playlist(
        &keys,
        episodes,
        &success_map,
        &current_key,
        input.file_path,
        input.version_prefer,
        input.version_prefer_enabled,
    );
    let has_prefer = prefer.as_ref().is_some_and(|p| !p.is_empty());
    debug!(
        "version_filter: version_prefer has_prefer={}, ep_data={}, builtin_res={}",
        has_prefer,
        ep_data.len(),
        builtin_res.len()
    );

    let mut ini_res: Vec<Item> = Vec::new();
    if ep_data.is_empty() {
        if !has_prefer {
            return fallback(builtin_res, ep_current);
        }
    } else {
        let seq = sequence_from_current(&ep_data, ep_current, &cut_keys);
        let success = seq.len() >= 2;
        if !has_prefer {
            if success {
                return if builtin_res.len() > seq.len() {
                    builtin_res
                } else {
                    seq
                };
            }
            return fallback(builtin_res, ep_current);
        }
        ini_res = seq;
    }

    let prefer = prefer.unwrap_or_default();
    let filter_res = if ini_res.len() > builtin_res.len() {
        ini_res
    } else {
        builtin_res
    };
    if filter_res.len() <= 1 {
        debug!(
            "version_filter: filter_res too small, returning version_prefer \
             result ({} episodes)",
            prefer.len()
        );
        return prefer;
    }
    let merged = merge_prefer(&prefer, &filter_res, &seq_keys);
    debug!(
        "version_filter: [DECISION] merged filter({} eps)+prefer -> {} episodes: {:?}",
        filter_res.len(),
        merged.len(),
        merged
            .iter()
            .map(|e| e.path.as_deref().unwrap_or("?"))
            .collect::<Vec<_>>()
    );
    merged
}

/// Degrade safely: the best builtin sequence if any, else the current episode
/// alone (disabling the forward playlist).
fn fallback(builtin_res: Vec<Item>, ep_current: &Item) -> Vec<Item> {
    if builtin_res.is_empty() {
        debug!(
            "version_filter: [DECISION] fallback to current episode only: {:?}",
            ep_current.path.as_deref().unwrap_or("?")
        );
        vec![ep_current.clone()]
    } else {
        debug!(
            "version_filter: [DECISION] fallback to builtin sequence ({} episodes)",
            builtin_res.len()
        );
        builtin_res
    }
}

/// Splice `filter_res` into `prefer` between the prefer entries before its
/// first key and after its last key (the Python tail combination).
fn merge_prefer(
    prefer: &[Item],
    filter_res: &[Item],
    seq_keys: &[String],
) -> Vec<Item> {
    let first_key = filter_res.first().and_then(episode_key);
    let last_key = filter_res.last().and_then(episode_key);
    let first_index = first_key
        .as_deref()
        .and_then(|k| seq_keys.iter().position(|s| s.as_str() == k))
        .unwrap_or(0);
    let last_index = last_key
        .as_deref()
        .and_then(|k| seq_keys.iter().position(|s| s.as_str() == k))
        .unwrap_or(0);
    debug!(
        "merge_prefer: prefer={} filter_res={} seq_keys={} \
         first_key={:?} last_key={:?} first_index={} last_index={}",
        prefer.len(),
        filter_res.len(),
        seq_keys.len(),
        first_key,
        last_key,
        first_index,
        last_index
    );
    let head = prefer.get(..first_index).unwrap_or(&[]);
    let tail = prefer.get(last_index + 1..).unwrap_or(&[]);
    let mut res = head.to_vec();
    res.extend(filter_res.iter().cloned());
    res.extend(tail.iter().cloned());
    res
}

/// Deduplicate keys preserving first-seen order (Python `dict.fromkeys`).
fn dedup_keys(keys: &[String]) -> Vec<String> {
    let mut seen = Vec::new();
    for key in keys {
        if !seen.contains(key) {
            seen.push(key.clone());
        }
    }
    seen
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

    fn input<'a>(
        file_path: &'a str,
        ver_re: &'a str,
    ) -> VersionFilterInput<'a> {
        VersionFilterInput {
            file_path,
            playlist: false,
            version_filter_re: ver_re,
            media_source_id: "",
            version_prefer: &[],
            version_prefer_enabled: false,
        }
    }

    #[test]
    fn no_multi_version_passes_through() {
        let eps = vec![
            ep("a", 1, 1, "/m/S01E01.mkv"),
            ep("b", 1, 2, "/m/S01E02.mkv"),
        ];
        let res = version_filter(&input("/m/S01E01.mkv", "1080p|720p"), &eps);
        assert_eq!(res.len(), 2);
    }

    #[test]
    fn official_rule_selects_one_per_episode() {
        let eps = vec![
            ep("a1", 1, 1, "/m/Show S01E01 - 1080p.mkv"),
            ep("a2", 1, 1, "/m/Show S01E01 - 720p.mkv"),
            ep("b1", 1, 2, "/m/Show S01E02 - 1080p.mkv"),
            ep("b2", 1, 2, "/m/Show S01E02 - 720p.mkv"),
        ];
        let res = version_filter(
            &input("/m/Show S01E01 - 1080p.mkv", "1080p|720p"),
            &eps,
        );
        let ids: Vec<&str> = res.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["a1", "b1"]);
    }

    #[test]
    fn unmatched_disables_playlist_to_current_only() {
        let eps = vec![
            ep("a1", 1, 1, "/m/x1.mkv"),
            ep("b1", 1, 2, "/m/y1.mkv"),
            ep("b2", 1, 2, "/m/y2.mkv"),
        ];
        let res = version_filter(&input("/m/x1.mkv", "1080p"), &eps);
        let ids: Vec<&str> = res.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["a1"]);
    }

    #[test]
    fn empty_filter_or_playlist_passes_through() {
        let eps = vec![ep("a", 1, 1, "/m/a.mkv")];
        assert_eq!(version_filter(&input("/m/a.mkv", ""), &eps).len(), 1);
        let mut pl = input("/m/a.mkv", "1080p");
        pl.playlist = true;
        assert_eq!(version_filter(&pl, &eps).len(), 1);
    }
}
