//! Per-episode version preference for playlists.
//!
//! When a season exposes several versions per episode, the playlist must pick
//! one version for every episode key. The chosen version follows, in order:
//!
//! 1. a single available version — taken as-is;
//! 2. the version already selected by the ini regex filter (`ep_success_map`);
//! 3. for the currently playing key, the version matching the played file;
//! 4. otherwise the first `version_prefer` regex that matches the joined,
//!    lowercased basenames, falling back to the first version.
//!
//! Unlike [`crate::version::select_version_index`] (substring match for the
//! initial source pick), the rules here are full regular expressions, matching
//! the Python `re.compile(rule, re.I)` behaviour.

use regex::RegexBuilder;

use crate::dto::Item;

/// Separator used when joining per-version basenames, matching the Python
/// `'_|_'.join(...)`.
const JOIN: &str = "_|_";

/// Group `eps_data` by their parallel `cur_list` key, preserving first-seen key
/// order (the Python `defaultdict(list)` insertion order).
fn group_by_key<'a>(
    cur_list: &[String],
    eps_data: &'a [Item],
) -> Vec<(String, Vec<&'a Item>)> {
    let mut groups: Vec<(String, Vec<&'a Item>)> = Vec::new();
    for (key, item) in cur_list.iter().zip(eps_data.iter()) {
        match groups.iter_mut().find(|(k, _)| k == key) {
            Some((_, items)) => items.push(item),
            None => groups.push((key.clone(), vec![item])),
        }
    }
    groups
}

/// The lowercased basename of an item's `Path`.
fn path_basename(item: &Item) -> String {
    let path = item.path.as_deref().unwrap_or("");
    path.rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_lowercase()
}

/// Whether `file_path` identifies this version (its `Path` or first source).
fn is_played_version(item: &Item, file_path: &str) -> bool {
    if item.path.as_deref() == Some(file_path) {
        return true;
    }
    item.media_sources
        .first()
        .is_some_and(|s| s.path == file_path)
}

/// Pick the preferred version among `sources` by the ordered regex `rules`,
/// or `None` when no rule matches (the caller then keeps the first version).
fn pick_by_rules<'a>(
    sources: &[&'a Item],
    rules: &[String],
) -> Option<&'a Item> {
    let names: Vec<String> = sources.iter().map(|s| path_basename(s)).collect();
    let joined = names.join(JOIN);
    for rule in rules {
        let Ok(re) = RegexBuilder::new(rule).case_insensitive(true).build()
        else {
            continue;
        };
        if let Some(m) = re.find(&joined) {
            let prefix = joined.get(..m.start()).unwrap_or("");
            let index = prefix.matches(JOIN).count();
            if let Some(picked) = sources.get(index) {
                return Some(picked);
            }
        }
    }
    None
}

/// Choose one version per episode key for the playlist.
///
/// Returns `None` when the feature is disabled or no `version_prefer` rules are
/// configured. `ep_success_map` maps an
/// episode key to the version already chosen by the ini regex filter;
/// `current_key`/`file_path` identify the episode being played now.
#[must_use]
pub fn version_prefer_for_playlist(
    cur_list: &[String],
    eps_data: &[Item],
    ep_success_map: &[(String, Item)],
    current_key: &str,
    file_path: &str,
    rules: &[String],
    enabled: bool,
) -> Option<Vec<Item>> {
    if !enabled || rules.is_empty() {
        return None;
    }
    let groups = group_by_key(cur_list, eps_data);
    let mut result = Vec::with_capacity(groups.len());
    for (key, sources) in groups {
        let Some(first) = sources.first() else {
            continue;
        };
        if sources.len() == 1 {
            result.push((*first).clone());
            continue;
        }
        if let Some((_, ep)) = ep_success_map.iter().find(|(k, _)| *k == key) {
            result.push(ep.clone());
            continue;
        }
        if key == current_key {
            let played = sources
                .iter()
                .find(|s| is_played_version(s, file_path))
                .copied()
                .unwrap_or(*first);
            result.push(played.clone());
            continue;
        }
        let picked = pick_by_rules(&sources, rules).unwrap_or(*first);
        result.push(picked.clone());
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::MediaSource;

    fn item(id: &str, path: &str) -> Item {
        Item {
            id: id.to_owned(),
            path: Some(path.to_owned()),
            ..Item::default()
        }
    }

    #[test]
    fn disabled_or_no_rules_returns_none() {
        assert!(
            version_prefer_for_playlist(
                &[],
                &[],
                &[],
                "1-1",
                "x",
                &["vcb".to_owned()],
                false,
            )
            .is_none()
        );
        assert!(
            version_prefer_for_playlist(&[], &[], &[], "1-1", "x", &[], true)
                .is_none()
        );
    }

    #[test]
    fn single_version_taken_as_is() {
        let eps = vec![item("a", "/m/s01e01.mkv")];
        let keys = vec!["1-1".to_owned()];
        let res = version_prefer_for_playlist(
            &keys,
            &eps,
            &[],
            "1-1",
            "/m/s01e01.mkv",
            &["vcb".to_owned()],
            true,
        )
        .expect("some");
        assert_eq!(res.len(), 1);
        assert_eq!(res.first().expect("entry").id, "a");
    }

    #[test]
    fn current_key_uses_played_file() {
        // Two versions for the current key; the played file_path wins over the
        // version_prefer rule.
        let eps = vec![
            item("vcb", "/m/s01e01.VCB.mkv"),
            item("baha", "/m/s01e01.Baha.mkv"),
        ];
        let keys = vec!["1-1".to_owned(), "1-1".to_owned()];
        let res = version_prefer_for_playlist(
            &keys,
            &eps,
            &[],
            "1-1",
            "/m/s01e01.Baha.mkv",
            &["vcb".to_owned()],
            true,
        )
        .expect("some");
        assert_eq!(res.len(), 1);
        assert_eq!(res.first().expect("entry").id, "baha");
    }

    #[test]
    fn other_key_uses_regex_preference() {
        // A non-current key picks by the version_prefer regex (vcb wins).
        let eps = vec![
            item("baha", "/m/s01e02.Baha.mkv"),
            item("vcb", "/m/s01e02.VCB.mkv"),
        ];
        let keys = vec!["1-2".to_owned(), "1-2".to_owned()];
        let res = version_prefer_for_playlist(
            &keys,
            &eps,
            &[],
            "1-1",
            "/m/s01e01.x.mkv",
            &["vcb".to_owned()],
            true,
        )
        .expect("some");
        assert_eq!(res.len(), 1);
        assert_eq!(res.first().expect("entry").id, "vcb");
    }

    #[test]
    fn ini_filter_success_wins_over_rule() {
        let eps = vec![
            item("vcb", "/m/s01e02.VCB.mkv"),
            item("baha", "/m/s01e02.Baha.mkv"),
        ];
        let keys = vec!["1-2".to_owned(), "1-2".to_owned()];
        // ep_success_map forces the baha version for key 1-2.
        let chosen =
            vec![("1-2".to_owned(), item("baha", "/m/s01e02.Baha.mkv"))];
        let res = version_prefer_for_playlist(
            &keys,
            &eps,
            &chosen,
            "1-1",
            "/m/s01e01.x.mkv",
            &["vcb".to_owned()],
            true,
        )
        .expect("some");
        assert_eq!(res.first().expect("entry").id, "baha");
    }

    #[test]
    fn matches_via_first_media_source_path() {
        let mut current = item("baha", "/other/display/path.mkv");
        current.media_sources = vec![MediaSource {
            id: "src".to_owned(),
            path: "/m/s01e01.Baha.mkv".to_owned(),
            ..MediaSource::default()
        }];
        let eps = vec![item("vcb", "/m/s01e01.VCB.mkv"), current];
        let keys = vec!["1-1".to_owned(), "1-1".to_owned()];
        let res = version_prefer_for_playlist(
            &keys,
            &eps,
            &[],
            "1-1",
            "/m/s01e01.Baha.mkv",
            &["vcb".to_owned()],
            true,
        )
        .expect("some");
        assert_eq!(res.first().expect("entry").id, "baha");
    }
}
