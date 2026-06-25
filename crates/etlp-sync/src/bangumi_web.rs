//! Bangumi subject-resolution types and scoring helpers.
//!
//! The HTML scraping layer has been removed; all network operations now use
//! the authenticated BGM JSON API (see [`super::bangumi::BangumiApi`]).

use std::collections::{HashMap, HashSet};

// ── Tunable constants ──────────────────────────────────────────────────────────

/// Minimum Levenshtein title similarity to accept a subject in Round 2.
/// Scores below this threshold yield no match (warn + suggest manual mapping).
pub const BANGUMI_TITLE_MIN_SCORE: f64 = 0.6;

/// Episode / movie premiere date must be no earlier than the subject's
/// broadcast-start minus this many days. Upper bound is unconstrained
/// (covered by ep-range check).
pub const BANGUMI_DATE_WINDOW_DAYS: i64 = 5;

/// Minimum base-match score for a search candidate to pass pre-screening.
/// Keeps obviously unrelated results out of the expensive detail fetch.
pub(crate) const BANGUMI_CANDIDATE_PRESCREEN_SCORE: f64 = 0.3;

// ── Domain types ───────────────────────────────────────────────────────────────

/// One episode entry within a [`SubjectDetail`].
#[derive(Debug, Clone)]
pub struct EpEntry {
    /// BGM sort number (global across arcs, e.g. 92–143 for arc 2).
    pub sort: u32,
    /// Human-readable episode title.
    pub title: String,
    /// Broadcast date in `"YYYY-MM-DD"` format, when present.
    pub airdate: Option<String>,
}

/// One subject hit returned by the BGM JSON search API.
#[derive(Debug, Clone)]
pub struct SubjectCandidate {
    pub subject_id: u64,
    /// Localised (usually Chinese) name.
    pub name: String,
    /// Japanese original name, if the localised name differs.
    pub name_jp: Option<String>,
}

/// Full detail for one BGM subject, assembled from the JSON API.
#[derive(Debug, Clone)]
pub struct SubjectDetail {
    pub subject_id: u64,
    /// Localised (usually Chinese) name.
    pub name: String,
    /// Japanese original name.
    pub name_jp: Option<String>,
    /// Broadcast-start date (`"YYYY-MM-DD"`).
    pub start_date: Option<String>,
    /// Full episode list ordered by sort number.
    pub episodes: Vec<EpEntry>,
    /// `(min_sort, max_sort)` across all episodes; `None` when empty.
    pub ep_range: Option<(u32, u32)>,
}

/// In-pass resolution cache — reuses API responses within one sync call.
///
/// Not persisted; lives only for the duration of one `sync_bangumi` pass.
#[derive(Debug, Default)]
pub struct ScrapeCache {
    pub search_results: HashMap<String, Vec<SubjectCandidate>>,
    pub subject_details: HashMap<u64, SubjectDetail>,
    /// Subject IDs whose 前传/续集 relations have been fetched this pass.
    pub chain_walked: HashSet<u64>,
}

// ── Scoring helpers ────────────────────────────────────────────────────────────

fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut curr = Vec::with_capacity(b.len() + 1);
        curr.push(i + 1);
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            let del = prev.get(j + 1).copied().unwrap_or(usize::MAX);
            let ins = curr.last().copied().unwrap_or(usize::MAX);
            let sub = prev.get(j).copied().unwrap_or(usize::MAX);
            curr.push((del + 1).min(ins + 1).min(sub + cost));
        }
        prev = curr;
    }
    prev.last().copied().unwrap_or(0)
}

pub(crate) fn title_similarity(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.trim().to_lowercase().chars().collect();
    let b: Vec<char> = b.trim().to_lowercase().chars().collect();
    let max = a.len().max(b.len());
    if max == 0 {
        return 1.0;
    }
    1.0 - levenshtein(&a, &b) as f64 / max as f64
}

fn title_contains(haystack: &str, needle: &str) -> bool {
    let norm = |s: &str| -> String {
        s.to_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    };
    let n = norm(needle);
    !n.is_empty() && norm(haystack).contains(&n)
}

pub(crate) fn base_match_score(
    keyword: &str,
    name: &str,
    name_jp: &str,
) -> f64 {
    let mut best =
        title_similarity(keyword, name).max(title_similarity(keyword, name_jp));
    for cand in [name, name_jp] {
        if title_contains(cand, keyword) || title_contains(keyword, cand) {
            best = best.max(0.9);
        }
    }
    best
}

/// `true` when any episode in `episodes` has an `airdate` within
/// `window_days` of `premiere_date`.
pub(crate) fn airdate_matches_premiere(
    premiere_date: &str,
    episodes: &[EpEntry],
    window_days: i64,
) -> bool {
    let target_days = crate::bangumi::date_to_days_pub(premiere_date);
    let Some(t) = target_days else {
        return false;
    };
    episodes.iter().any(|e| {
        e.airdate
            .as_deref()
            .and_then(crate::bangumi::date_to_days_pub)
            .is_some_and(|ep_d| (ep_d - t).abs() <= window_days)
    })
}

/// Compute `(min, max)` sort number across a non-empty episode list.
pub(crate) fn ep_range(episodes: &[EpEntry]) -> Option<(u32, u32)> {
    let min = episodes.iter().map(|e| e.sort).min()?;
    let max = episodes.iter().map(|e| e.sort).max()?;
    Some((min, max))
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ep(sort: u32, airdate: Option<&str>) -> EpEntry {
        EpEntry {
            sort,
            title: String::new(),
            airdate: airdate.map(str::to_owned),
        }
    }

    #[test]
    fn airdate_matches_premiere_returns_true_within_window() {
        let eps = vec![
            make_ep(1, Some("2024-08-04")),
            make_ep(2, Some("2024-08-11")),
            make_ep(135, Some("2027-03-19")),
        ];
        assert!(airdate_matches_premiere("2024-08-04", &eps, 5));
        assert!(airdate_matches_premiere("2024-08-08", &eps, 5));
        // 2025-01-01 is far from all three episode airdates
        assert!(!airdate_matches_premiere("2025-01-01", &eps, 5));
    }

    #[test]
    fn ep_range_returns_min_and_max() {
        let eps =
            vec![make_ep(92, None), make_ep(100, None), make_ep(143, None)];
        assert_eq!(ep_range(&eps), Some((92, 143)));
    }

    #[test]
    fn ep_range_returns_none_for_empty() {
        assert_eq!(ep_range(&[]), None);
    }
}
