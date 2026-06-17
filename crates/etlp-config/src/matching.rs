//! Pure string-matching helpers ported from the Python `Configs` methods.
//!
//! These operate on already-fetched ini option strings so they can be unit
//! tested without touching the filesystem. The Python original overloaded a
//! single `check_str_match` with many boolean flags; here it is split into
//! well-typed functions with the same underlying semantics:
//!
//! * a token "matches" when it is a **substring** of the input;
//! * the first token (in ini order) that matches wins;
//! * pair/next lookups only apply when the matched token sits at an even index
//!   (1-based odd position), so values stored as `key, value, key, value`
//!   resolve correctly.

/// Replace the full-width CJK punctuation accepted in configs with their ASCII
/// equivalents, matching the Python `.replace('：', ':')...` normalization.
#[must_use]
pub fn normalize(raw: &str) -> String {
    raw.replace('：', ":").replace('，', ",").replace('；', ";")
}

/// Split an ini option value into trimmed, non-empty tokens.
///
/// Mirrors `ini_str_split`: normalize full-width punctuation, split on
/// `split_by`, strip each token, and drop empties.
#[must_use]
pub fn split_list(raw: &str, split_by: char) -> Vec<String> {
    normalize(raw)
        .split(split_by)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Split into `(a, b)` pairs by taking tokens two at a time (the `zip_it`
/// path). A trailing odd token is dropped, matching `zip(ini[0::2], ini[1::2])`.
#[must_use]
pub fn split_pairs(raw: &str, split_by: char) -> Vec<(String, String)> {
    let tokens = split_list(raw, split_by);
    tokens
        .chunks_exact(2)
        .filter_map(|c| match c {
            [a, b] => Some((a.clone(), b.clone())),
            _ => None,
        })
        .collect()
}

/// Split into groups, then split each group again by `re_split_by` (the
/// `re_split_by` path), e.g. `"a,b; c,d"` with `';'`/`','`.
#[must_use]
pub fn split_groups(
    raw: &str,
    split_by: char,
    re_split_by: char,
) -> Vec<Vec<String>> {
    split_list(raw, split_by)
        .iter()
        .map(|group| {
            group
                .split(re_split_by)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .collect()
}

/// Find the first token (in list order) that is contained in `input`,
/// returning its zero-based index and the token slice.
#[must_use]
pub fn first_match<'a>(
    input: &str,
    ini_list: &'a [String],
) -> Option<(usize, &'a str)> {
    ini_list
        .iter()
        .enumerate()
        .find(|(_, tok)| input.contains(tok.as_str()))
        .map(|(i, tok)| (i, tok.as_str()))
}

/// Whether any token matches (the default `check_str_match` boolean result).
#[must_use]
pub fn matches(input: &str, ini_list: &[String]) -> bool {
    first_match(input, ini_list).is_some()
}

/// The matched token itself (the `return_value=True` path).
#[must_use]
pub fn matched_value<'a>(
    input: &str,
    ini_list: &'a [String],
) -> Option<&'a str> {
    first_match(input, ini_list).map(|(_, tok)| tok)
}

/// 1-based order of the matched token, or `0` when nothing matches
/// (the `order_only=True` path).
#[must_use]
pub fn match_order(input: &str, ini_list: &[String]) -> usize {
    first_match(input, ini_list).map_or(0, |(i, _)| i + 1)
}

/// The token immediately after the matched one, but only when the match sits
/// at an even index (the `get_next=True` path).
#[must_use]
pub fn match_next<'a>(input: &str, ini_list: &'a [String]) -> Option<&'a str> {
    let (i, _) = first_match(input, ini_list)?;
    if i % 2 != 0 {
        return None;
    }
    ini_list.get(i + 1).map(String::as_str)
}

/// The `(matched, next)` pair, only when the match sits at an even index
/// (the `get_pair=True` path).
#[must_use]
pub fn match_pair<'a>(
    input: &str,
    ini_list: &'a [String],
) -> Option<(&'a str, &'a str)> {
    let (i, _) = first_match(input, ini_list)?;
    if i % 2 != 0 {
        return None;
    }
    let cur = ini_list.get(i)?;
    let next = ini_list.get(i + 1)?;
    Some((cur.as_str(), next.as_str()))
}

/// Apply the first matching `from -> to` replacement to `input`
/// (the `string_replace_by_ini_pair` helper).
#[must_use]
pub fn replace_by_pair(input: &str, ini_list: &[String]) -> String {
    match match_pair(input, ini_list) {
        Some((from, to)) => input.replacen(from, to, 1),
        None => input.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn normalize_replaces_fullwidth_punctuation() {
        assert_eq!(normalize("a：b，c；d"), "a:b,c;d");
    }

    #[test]
    fn split_list_trims_and_drops_empty() {
        assert_eq!(
            split_list(" pili , bili , , local ", ','),
            list(&["pili", "bili", "local"])
        );
        // Full-width comma is normalized first.
        assert_eq!(split_list("a，b", ','), list(&["a", "b"]));
    }

    #[test]
    fn split_pairs_drops_trailing_odd() {
        assert_eq!(
            split_pairs("k1, v1, k2, v2, dangling", ','),
            vec![
                ("k1".to_string(), "v1".to_string()),
                ("k2".to_string(), "v2".to_string()),
            ]
        );
    }

    #[test]
    fn split_groups_two_levels() {
        assert_eq!(
            split_groups("name, /p1, /p2; other, /p3", ';', ','),
            vec![list(&["name", "/p1", "/p2"]), list(&["other", "/p3"]),]
        );
    }

    #[test]
    fn first_match_is_substring_in_list_order() {
        let l = list(&["192.168", "127.0", "local"]);
        assert_eq!(
            first_match("http://127.0.0.1:8096", &l),
            Some((1, "127.0"))
        );
        assert_eq!(first_match("ftp://example", &l), None);
    }

    #[test]
    fn boolean_and_value_matching() {
        let l = list(&["pili", "bili"]);
        assert!(matches("v.pili.app", &l));
        assert!(!matches("v.other.app", &l));
        assert_eq!(matched_value("x.bili.y", &l), Some("bili"));
        assert!(matched_value("none", &l).is_none());
        // Empty rule list never matches.
        assert!(!matches("anything", &[]));
    }

    #[test]
    fn order_is_one_based_zero_for_miss() {
        let l = list(&["xx", "yy", "zz"]);
        assert_eq!(match_order("has zz inside", &l), 3);
        assert_eq!(match_order("nothing", &l), 0);
    }

    #[test]
    fn next_and_pair_only_at_even_index() {
        // key/value layout: index0=key, index1=value, index2=key, ...
        let l = list(&["pili", "300", "bili", "600"]);
        // matched at index 0 (even) -> next/pair available
        assert_eq!(match_next("v.pili.app", &l), Some("300"));
        assert_eq!(match_pair("v.pili.app", &l), Some(("pili", "300")));
        // matching the value token (index 1, odd) yields nothing
        assert!(match_next("code 300 ok", &l).is_none());
        assert!(match_pair("code 300 ok", &l).is_none());
    }

    #[test]
    fn next_handles_out_of_bounds() {
        let l = list(&["only_key"]);
        assert!(match_next("only_key here", &l).is_none());
        assert!(match_pair("only_key here", &l).is_none());
    }

    #[test]
    fn replace_by_pair_applies_first_match() {
        let l = list(&["old.host", "new.host"]);
        assert_eq!(
            replace_by_pair("http://old.host/x", &l),
            "http://new.host/x"
        );
        assert_eq!(replace_by_pair("http://keep/x", &l), "http://keep/x");
    }
}
