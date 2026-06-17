//! Version selection and comparison, ported from `tools.py`.
//!
//! * [`select_version_index`] reproduces `version_prefer_emby`: given the
//!   per-source file names and the user's ordered preference keywords, pick the
//!   index of the source matching the highest-priority keyword.
//! * [`match_version_range`] reproduces `match_version_range`: whether a dotted
//!   version string falls within an inclusive `min-max` range.

/// Separator used to join source names before keyword search, matching the
/// Python `'_|_'.join(...)`.
const JOIN: &str = "_|_";

/// Pick the source index preferred by the ordered `rules` keywords.
///
/// `names` are the per-source basenames, already lowercased. Each rule is
/// matched (case-insensitively) against the joined names; the first rule that
/// appears wins, and its source index is the count of separators before its
/// first occurrence. Returns `0` when `rules` is empty or nothing matches
/// (mirroring the Python fallback to `sources[0]`).
#[must_use]
pub fn select_version_index(names: &[String], rules: &[String]) -> usize {
    if rules.is_empty() || names.is_empty() {
        return 0;
    }
    let joined = names.join(JOIN);
    for rule in rules {
        let needle = rule.to_lowercase();
        if needle.is_empty() {
            continue;
        }
        if let Some(pos) = joined.find(&needle) {
            let prefix = joined.get(..pos).unwrap_or("");
            return prefix.matches(JOIN).count();
        }
    }
    0
}

/// Parse a dotted version into numeric components, ignoring non-numeric parts.
fn parse_version(version: &str) -> Vec<i64> {
    version
        .split('.')
        .map(|part| part.trim().parse::<i64>().unwrap_or(0))
        .collect()
}

/// Compare two dotted versions component-wise (shorter is zero-padded).
fn compare_version(a: &str, b: &str) -> std::cmp::Ordering {
    let (va, vb) = (parse_version(a), parse_version(b));
    let max = va.len().max(vb.len());
    for i in 0..max {
        let x = va.get(i).copied().unwrap_or(0);
        let y = vb.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

/// Whether `version` is within the inclusive `min-max` range
/// (e.g. `"4.6.7.0-4.7.14.0"`). Malformed ranges yield `false`.
#[must_use]
pub fn match_version_range(version: &str, range: &str) -> bool {
    let Some((min, max)) = range.trim().split_once('-') else {
        return false;
    };
    compare_version(version, min.trim()).is_ge()
        && compare_version(version, max.trim()).is_le()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_lowercase()).collect()
    }

    fn rules(items: &[&str]) -> Vec<String> {
        items.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn empty_rules_pick_first() {
        let n = names(&["a.vcb.mkv", "b.baha.mkv"]);
        assert_eq!(select_version_index(&n, &[]), 0);
    }

    #[test]
    fn picks_highest_priority_rule_source() {
        // Priority order: vcb before baha; baha source is index 1.
        let n = names(&["ep01.Baha.mkv", "ep01.VCB.mkv"]);
        let r = rules(&["vcb", "baha"]);
        assert_eq!(select_version_index(&n, &r), 1);
    }

    #[test]
    fn falls_back_to_first_on_no_match() {
        let n = names(&["ep01.GroupA.mkv", "ep01.GroupB.mkv"]);
        let r = rules(&["nonexistent"]);
        assert_eq!(select_version_index(&n, &r), 0);
    }

    #[test]
    fn earlier_rule_wins_over_later_even_if_later_is_index_zero() {
        // "baha" is index 0, "vcb" is index 1, but rule order prefers vcb.
        let n = names(&["ep.Baha.mkv", "ep.VCB.mkv"]);
        let r = rules(&["vcb", "baha"]);
        assert_eq!(select_version_index(&n, &r), 1);
    }

    #[test]
    fn version_range_inclusive_bounds() {
        assert!(match_version_range("4.7.0.0", "4.6.7.0-4.7.14.0"));
        assert!(match_version_range("4.6.7.0", "4.6.7.0-4.7.14.0"));
        assert!(match_version_range("4.7.14.0", "4.6.7.0-4.7.14.0"));
        assert!(!match_version_range("4.8.0.0", "4.6.7.0-4.7.14.0"));
        assert!(!match_version_range("4.6.6.9", "4.6.7.0-4.7.14.0"));
    }

    #[test]
    fn version_range_zero_pads_shorter() {
        // Shorter side is zero-padded: "4.8" == "4.8.0.0" < "4.8.0.40".
        assert!(match_version_range("4.8.0.40", "4.8-9.9.9.9"));
        assert!(!match_version_range("4.7", "4.8-9.9.9.9"));
        assert!(!match_version_range("4.8", "4.8.0.40-9.9.9.9"));
    }

    #[test]
    fn malformed_range_is_false() {
        assert!(!match_version_range("4.7", "not-a-range-x"));
        assert!(!match_version_range("4.7", "noseparator"));
    }
}
