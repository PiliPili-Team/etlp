//! bgm.tv HTML scraper — subject search and detail pages.
//!
//! All network functions accept a `bgm_base_url` parameter so tests can point
//! them at a local mock server instead of the real bgm.tv.

use scraper::{Html, Selector};
use tracing::debug;

// ── Tunable constants ──────────────────────────────────────────────────────

/// Minimum Levenshtein title similarity to accept a subject in Round 2.
/// Scores below this threshold yield no match (warn + suggest manual mapping).
pub const BANGUMI_TITLE_MIN_SCORE: f64 = 0.6;

/// Episode / movie premiere date must be no earlier than the subject's
/// broadcast-start minus this many days. Upper bound is unconstrained
/// (covered by ep-range check).
pub const BANGUMI_DATE_WINDOW_DAYS: i64 = 5;

/// Minimum base-match score for a search candidate to pass pre-screening.
/// Keeps obviously unrelated results out of the expensive detail-page fetch.
pub(crate) const BANGUMI_CANDIDATE_PRESCREEN_SCORE: f64 = 0.3;

/// Default bgm.tv web base URL (parameterised in calls for testability).
pub const BGM_WEB_BASE_URL: &str = "https://bangumi.tv";

// ── Domain types ───────────────────────────────────────────────────────────

/// One episode entry within a [`SubjectDetail`].
#[derive(Debug, Clone)]
pub struct EpEntry {
    /// BGM sort number (global across arcs, e.g. 92-143 for arc 4).
    pub sort: u32,
    /// Human-readable episode title, with the "ep.N " prefix stripped.
    pub title: String,
    /// Broadcast date in "YYYY-MM-DD" format, when present on the subject page.
    pub airdate: Option<String>,
}

/// One subject hit from a bgm.tv search results page.
#[derive(Debug, Clone)]
pub struct SubjectCandidate {
    pub subject_id: u64,
    /// Localised (usually Chinese) name from `<a class="l">`.
    pub name: String,
    /// Japanese original name from `<small class="grey">`, if present.
    pub name_jp: Option<String>,
    /// First date in `<p class="info tip">`, "YYYY-MM-DD" (coarse pre-filter).
    pub search_date: Option<String>,
}

// ── Private helpers ────────────────────────────────────────────────────────

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

pub(crate) fn percent_encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'_' | b'.' | b'~')
        {
            out.push(byte as char);
        } else {
            let hi = byte >> 4;
            let lo = byte & 0xF;
            out.push('%');
            out.push(if hi < 10 {
                (b'0' + hi) as char
            } else {
                (b'A' + hi - 10) as char
            });
            out.push(if lo < 10 {
                (b'0' + lo) as char
            } else {
                (b'A' + lo - 10) as char
            });
        }
    }
    out
}

/// Parse the first "YYYY年M月D日" occurrence in `text` into "YYYY-MM-DD".
///
/// Returns `None` when no recognisable date pattern is found or when month /
/// day are outside their valid ranges.
pub(crate) fn parse_japanese_date(text: &str) -> Option<String> {
    let year_end = text.find('年')?;
    let year_start = text[..year_end]
        .char_indices()
        .rev()
        .find(|(_, c)| !c.is_ascii_digit())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let year: u32 = text.get(year_start..year_end)?.parse().ok()?;
    let rest = text.get(year_end + '年'.len_utf8()..)?;
    let month_end = rest.find('月')?;
    let month: u32 = rest.get(..month_end)?.trim().parse().ok()?;
    let rest = rest.get(month_end + '月'.len_utf8()..)?;
    let day_end = rest.find('日')?;
    let day: u32 = rest.get(..day_end)?.trim().parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

// ── HTML parsing ───────────────────────────────────────────────────────────

/// Parse a bgm.tv search results page into a list of subject candidates.
///
/// Each `<li class="item anime">` element yields one [`SubjectCandidate`]:
/// - `subject_id` — numeric ID from the `<a class="l">` href
/// - `name` — Chinese localised name (link text of `<a class="l">`)
/// - `name_jp` — Japanese original from `<small class="grey">` (optional)
/// - `search_date` — first date in `<p class="info tip">` (optional)
pub fn parse_search_page(html: &str) -> Vec<SubjectCandidate> {
    let doc = Html::parse_document(html);
    let Ok(sel_item) = Selector::parse("li.item.anime") else {
        return Vec::new();
    };
    let Ok(sel_name) = Selector::parse("a.l") else {
        return Vec::new();
    };
    let Ok(sel_jp) = Selector::parse("small.grey") else {
        return Vec::new();
    };
    let Ok(sel_date) = Selector::parse("p.info.tip") else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for item in doc.select(&sel_item) {
        let Some(name_el) = item.select(&sel_name).next() else {
            continue;
        };
        let name = name_el.text().collect::<String>().trim().to_owned();
        if name.is_empty() {
            continue;
        }
        let href = name_el.value().attr("href").unwrap_or("");
        let Some(subject_id) = href
            .strip_prefix("/subject/")
            .and_then(|s| s.split('/').next())
            .and_then(|s| s.parse::<u64>().ok())
        else {
            continue;
        };
        let name_jp = item
            .select(&sel_jp)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_owned())
            .filter(|s| !s.is_empty());
        let search_date = item
            .select(&sel_date)
            .next()
            .and_then(|el| parse_japanese_date(&el.text().collect::<String>()));
        out.push(SubjectCandidate {
            subject_id,
            name,
            name_jp,
            search_date,
        });
    }
    out
}

// ── Network search ─────────────────────────────────────────────────────────

/// Fetch all pages of bgm.tv anime search results for `keyword`.
///
/// Starting from page 1, pages are fetched until a page returns zero hits.
/// Each candidate is pre-screened: those whose [`base_match_score`] against
/// `keyword` is below [`BANGUMI_CANDIDATE_PRESCREEN_SCORE`] are discarded to
/// reduce subsequent detail-page requests.
///
/// `bgm_base_url` is normally [`BGM_WEB_BASE_URL`]; tests may pass the
/// address of a local mock server.
pub async fn web_search_all_pages(
    http: &reqwest::Client,
    bgm_base_url: &str,
    keyword: &str,
) -> Vec<SubjectCandidate> {
    let encoded = percent_encode_path(keyword);
    let base = bgm_base_url.trim_end_matches('/');
    let mut all = Vec::new();
    let mut page = 1u32;

    loop {
        let url = format!("{base}/subject_search/{encoded}?cat=2&page={page}");
        let html = match http.get(&url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    debug!(
                        keyword,
                        page, "bangumi: web search body error: {e}"
                    );
                    break;
                }
            },
            Err(e) => {
                debug!(
                    keyword,
                    page, "bangumi: web search request failed: {e}"
                );
                break;
            }
        };
        let hits = parse_search_page(&html);
        debug!(keyword, page, hits = hits.len(), "bangumi: web_search");
        if hits.is_empty() {
            break;
        }
        for candidate in hits {
            let score = base_match_score(
                keyword,
                &candidate.name,
                candidate.name_jp.as_deref().unwrap_or(""),
            );
            let pass = score >= BANGUMI_CANDIDATE_PRESCREEN_SCORE;
            debug!(
                subject_id = candidate.subject_id,
                name = %candidate.name,
                score,
                result = if pass { "pass" } else { "skip" },
                "bangumi: candidate_prescreen"
            );
            if pass {
                all.push(candidate);
            }
        }
        page += 1;
    }
    all
}

// ── Subject detail ─────────────────────────────────────────────────────────

/// Full detail for one bgm.tv subject, assembled from the subject page.
#[derive(Debug, Clone)]
pub struct SubjectDetail {
    pub subject_id: u64,
    /// Localised (usually Chinese) name, carried over from search.
    pub name: String,
    /// Japanese original name, carried over from search.
    pub name_jp: Option<String>,
    /// Broadcast-start date ("YYYY-MM-DD"), parsed from the detail page.
    pub start_date: Option<String>,
    /// Full episode list ordered by sort number.
    pub episodes: Vec<EpEntry>,
    /// `(min_ep_num, max_ep_num)` across all episodes; `None` when empty.
    pub ep_range: Option<(u32, u32)>,
}

/// Extract the broadcast-start date from a subject HTML page.
///
/// Looks for `<span class="tip">放送开始: </span>` and reads the date text
/// from its parent `<li>`.
fn parse_start_date(html: &str) -> Option<String> {
    let doc = Html::parse_document(html);
    let Ok(sel_tip) = Selector::parse("span.tip") else {
        return None;
    };
    for span in doc.select(&sel_tip) {
        let text = span.text().collect::<String>();
        if !text.contains("放送开始") {
            continue;
        }
        let Some(parent_node) = span.parent() else {
            continue;
        };
        let Some(parent_el) = scraper::ElementRef::wrap(parent_node) else {
            continue;
        };
        let full = parent_el.text().collect::<String>();
        if let Some(date) = parse_japanese_date(&full) {
            return Some(date);
        }
    }
    None
}

/// Strip the `"ep.N "` prefix from a bgm.tv episode title attribute.
///
/// The `title` attribute of `<a class="load-epinfo">` looks like
/// `"ep.135 第一百三十五集 魂元果"`. This removes everything up to and
/// including the first space, leaving `"第一百三十五集 魂元果"`.
fn strip_ep_prefix(title: &str) -> String {
    title
        .split_once(' ')
        .map(|(_, rest)| rest.trim())
        .unwrap_or(title)
        .to_owned()
}

/// Extract a `YYYY-MM-DD` date from the text content of a `prginfo_*` popup.
///
/// The popup's `<span class="tip">` looks like `"首播: 2026-05-28时长: ..."`.
/// Finds the `首播:` marker and reads the 10-character date that follows.
fn extract_popup_airdate(text: &str) -> Option<String> {
    let marker = "首播:";
    let pos = text.find(marker)?;
    let after = text[pos + marker.len()..].trim_start();
    let candidate = after.get(..10)?;
    let b = candidate.as_bytes();
    if b.len() == 10 && b[4] == b'-' && b[7] == b'-' {
        Some(candidate.to_owned())
    } else {
        None
    }
}

/// Build a map from `prginfo_*` div ID → broadcast date for all popup divs in
/// the parsed document.
fn popup_date_map(
    doc: &Html,
) -> std::collections::HashMap<String, String> {
    let Ok(sel) = Selector::parse("div.prg_popup") else {
        return std::collections::HashMap::new();
    };
    let mut map = std::collections::HashMap::new();
    for div in doc.select(&sel) {
        let id = div.value().attr("id").unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let text = div.text().collect::<String>();
        if let Some(date) = extract_popup_airdate(&text) {
            map.insert(id.to_owned(), date);
        }
    }
    map
}

/// Parse all `<a class="load-epinfo">` entries from an HTML page into
/// [`EpEntry`] values.
///
/// Airdate is read from the companion `<div class="prg_popup">` referenced by
/// the `rel="#prginfo_*"` attribute — present on the subject main page but
/// absent on `/ep` sub-pages.
fn parse_ep_entries(html: &str) -> Vec<EpEntry> {
    let doc = Html::parse_document(html);
    let Ok(sel_ep) = Selector::parse("a.load-epinfo") else {
        return Vec::new();
    };
    let dates = popup_date_map(&doc);
    let mut out = Vec::new();
    for a in doc.select(&sel_ep) {
        let num_text = a.text().collect::<String>();
        let Ok(sort) = num_text.trim().parse::<u32>() else {
            continue;
        };
        let title = a
            .value()
            .attr("title")
            .map(strip_ep_prefix)
            .unwrap_or_default();
        let airdate = a
            .value()
            .attr("rel")
            .and_then(|rel| rel.strip_prefix('#'))
            .and_then(|id| dates.get(id))
            .cloned();
        out.push(EpEntry { sort, title, airdate });
    }
    out
}

/// Compute `(min, max)` sort number across a non-empty episode list.
fn ep_range(episodes: &[EpEntry]) -> Option<(u32, u32)> {
    let min = episodes.iter().map(|e| e.sort).min()?;
    let max = episodes.iter().map(|e| e.sort).max()?;
    Some((min, max))
}

/// `true` when any episode in `episodes` has an `airdate` within
/// `window_days` of `premiere_date`.
///
/// Used by [`resolve_episode_matching`] as a subject-level candidate filter
/// that is robust to Emby local vs. global episode numbering: instead of
/// checking whether `ep_index` falls in the BGM sort range, we check whether
/// the episode actually aired during this subject's broadcast window.
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
            .and_then(|d| crate::bangumi::date_to_days_pub(d))
            .is_some_and(|ep_d| (ep_d - t).abs() <= window_days)
    })
}

/// Fetch and assemble complete detail for one bgm.tv subject.
///
/// 1. Fetches the main subject page for the broadcast-start date and initial
///    episode list.
/// 2. Always fetches the `/ep` sub-page (page 1, 2, …) to get the full
///    episode list for long series where the main page is truncated.
/// 3. Merges and deduplicates episodes by sort number.
///
/// Returns `None` when the main page cannot be fetched.
pub async fn fetch_subject_detail(
    http: &reqwest::Client,
    bgm_base_url: &str,
    candidate: &SubjectCandidate,
) -> Option<SubjectDetail> {
    let base = bgm_base_url.trim_end_matches('/');
    let id = candidate.subject_id;

    // ── Main subject page ──────────────────────────────────────────────────
    let main_url = format!("{base}/subject/{id}");
    let main_html = http.get(&main_url).send().await.ok()?.text().await.ok()?;

    let start_date = parse_start_date(&main_html);
    let mut episodes = parse_ep_entries(&main_html);

    // ── /ep sub-page pagination ────────────────────────────────────────────
    let mut ep_page = 1u32;
    loop {
        let ep_url = format!("{base}/subject/{id}/ep?page={ep_page}");
        let ep_html = match http.get(&ep_url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(t) => t,
                Err(_) => break,
            },
            Err(_) => break,
        };
        let new_eps = parse_ep_entries(&ep_html);
        if new_eps.is_empty() {
            break;
        }
        for entry in new_eps {
            if !episodes.iter().any(|e| e.sort == entry.sort) {
                episodes.push(entry);
            }
        }
        ep_page += 1;
    }

    episodes.sort_by_key(|e| e.sort);
    let ep_range = ep_range(&episodes);

    debug!(
        subject_id = id,
        start_date = start_date.as_deref().unwrap_or(""),
        ep_count = episodes.len(),
        ep_range = ?ep_range,
        "bangumi: detail_fetch"
    );

    Some(SubjectDetail {
        subject_id: id,
        name: candidate.name.clone(),
        name_jp: candidate.name_jp.clone(),
        start_date,
        episodes,
        ep_range,
    })
}

/// In-pass scrape cache: reuse HTML fetch results within a single sync call.
///
/// Keyed by keyword for search results, by subject_id for detail pages.
/// Not persisted — lives only for the duration of one `sync_bangumi` call.
#[derive(Debug, Default)]
pub struct ScrapeCache {
    pub search_results:
        std::collections::HashMap<String, Vec<SubjectCandidate>>,
    pub subject_details: std::collections::HashMap<u64, SubjectDetail>,
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn search_fixture() -> &'static str {
        r#"<ul class="browserList">
<li class="item anime" id="subject_501796">
<div class="inner">
<h3>
<a href="/subject/501796" class="l">魔法姐妹露露特莉莉</a>
<small class="grey">魔法の姉妹ルルットリリィ</small>
</h3>
<p class="info tip">2026年4月5日 / 道解慎太郎 / スタジオぴえろ</p>
</div>
</li>
<li class="item anime" id="subject_99999">
<div class="inner">
<h3>
<a href="/subject/99999" class="l">某动画无日文名</a>
</h3>
<p class="info tip">2025年10月15日</p>
</div>
</li>
</ul>"#
    }

    #[test]
    fn parse_search_page_extracts_subject_id_name_name_jp() {
        let candidates = parse_search_page(search_fixture());
        assert_eq!(candidates.len(), 2);

        let first = candidates.first().unwrap();
        assert_eq!(first.subject_id, 501796);
        assert_eq!(first.name, "魔法姐妹露露特莉莉");
        assert_eq!(first.name_jp.as_deref(), Some("魔法の姉妹ルルットリリィ"));
    }

    #[test]
    fn parse_search_page_extracts_start_date_from_info_tip() {
        let candidates = parse_search_page(search_fixture());

        let first = candidates.first().unwrap();
        assert_eq!(first.search_date.as_deref(), Some("2026-04-05"));

        let second = candidates.get(1).unwrap();
        assert_eq!(second.search_date.as_deref(), Some("2025-10-15"));
    }

    #[test]
    fn parse_search_page_returns_empty_on_no_results() {
        let candidates =
            parse_search_page("<html><body>暂无结果</body></html>");
        assert!(candidates.is_empty());
    }

    #[test]
    fn parse_search_page_handles_missing_name_jp() {
        let candidates = parse_search_page(search_fixture());
        let second = candidates.get(1).unwrap();
        assert_eq!(second.subject_id, 99999);
        assert_eq!(second.name, "某动画无日文名");
        assert!(second.name_jp.is_none());
    }

    // ── subject detail page tests ──────────────────────────────────────────

    fn detail_fixture() -> &'static str {
        r##"<html><body>
<div id="subjectInfoBox">
<ul>
<li class=""><span class="tip">放送开始: </span>2024年8月4日</li>
<li class=""><span class="tip">话数: </span>39</li>
</ul>
</div>
<div id="prg_list">
<ul id="prg_list">
<li><a href="/ep/1001" id="prg_1001"
       class="load-epinfo epBtnAir"
       title="ep.1 第一集 开始"
       rel="#prginfo_1001">1</a></li>
<li><a href="/ep/1002" id="prg_1002"
       class="load-epinfo epBtnAir"
       title="ep.2 第二集 出发"
       rel="#prginfo_1002">2</a></li>
<li><a href="/ep/1135" id="prg_1135"
       class="load-epinfo epBtnAir"
       title="ep.135 第一百三十五集 魂元果"
       rel="#prginfo_1135">135</a></li>
</ul>
</div>
<div id="subject_prg_content">
<div id="prginfo_1001" class="prg_popup">
  <span class="tip">首播: 2024-08-04时长: 00:21:00</span>
</div>
<div id="prginfo_1002" class="prg_popup">
  <span class="tip">首播: 2024-08-11时长: 00:21:00</span>
</div>
<div id="prginfo_1135" class="prg_popup">
  <span class="tip">首播: 2027-03-19时长: 00:21:00</span>
</div>
</div>
</body></html>"##
    }

    #[test]
    fn parse_subject_detail_extracts_broadcast_start_date() {
        let date = parse_start_date(detail_fixture());
        assert_eq!(date.as_deref(), Some("2024-08-04"));
    }

    #[test]
    fn parse_subject_detail_extracts_prg_list_ep_num_and_title() {
        let eps = parse_ep_entries(detail_fixture());
        assert_eq!(eps.len(), 3);
        assert_eq!(eps.first().map(|e| e.sort), Some(1));
        assert_eq!(eps.get(2).map(|e| e.sort), Some(135));
    }

    #[test]
    fn parse_subject_detail_strips_ep_prefix_from_title() {
        let eps = parse_ep_entries(detail_fixture());
        assert_eq!(
            eps.get(2).map(|e| e.title.as_str()),
            Some("第一百三十五集 魂元果")
        );
        assert_eq!(
            eps.first().map(|e| e.title.as_str()),
            Some("第一集 开始")
        );
    }

    #[test]
    fn parse_subject_detail_extracts_airdate_from_popup() {
        let eps = parse_ep_entries(detail_fixture());
        assert_eq!(
            eps.first().and_then(|e| e.airdate.as_deref()),
            Some("2024-08-04")
        );
        assert_eq!(
            eps.get(2).and_then(|e| e.airdate.as_deref()),
            Some("2027-03-19")
        );
    }

    #[test]
    fn airdate_matches_premiere_returns_true_within_window() {
        let eps = parse_ep_entries(detail_fixture());
        // ep sort=1 airdate=2024-08-04, sort=2 airdate=2024-08-11
        assert!(airdate_matches_premiere("2024-08-04", &eps, 5));
        assert!(airdate_matches_premiere("2024-08-08", &eps, 5));
        // 2025-01-01 is far from all three episode airdates
        assert!(!airdate_matches_premiere("2025-01-01", &eps, 5));
    }

    #[test]
    fn parse_subject_detail_computes_ep_range_min_max() {
        let eps = parse_ep_entries(detail_fixture());
        let range = ep_range(&eps);
        assert_eq!(range, Some((1, 135)));
    }

    #[test]
    fn parse_subject_detail_handles_empty_prg_list() {
        let eps = parse_ep_entries("<html><body>no episodes</body></html>");
        assert!(eps.is_empty());
        assert_eq!(ep_range(&eps), None);
    }
}
