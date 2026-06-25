//! Bangumi (bgm.tv) API client and watch-progress sync.
//!
//! Authentication is via a personal Bearer token (no OAuth flow required).
//! The primary sync entry-point is [`sync_episode_by_bangumi_id`], which
//! accepts a Bangumi subject ID taken directly from an Emby item's
//! `ProviderIds.Bangumi` field and marks the specified episodes as watched.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::{Result, SyncError};

/// Log label for this provider's HTTP send/retry/response lines.
const DOMAIN: &str = "bangumi";

/// BGM subject search page URL template; `%s` is replaced with the
/// percent-encoded keyword. `cat=2` filters to anime subjects.
const BGM_WEB_SEARCH_URL: &str = "https://bgm.tv/subject_search/%s?cat=2";

// ── Domain types ──────────────────────────────────────────────────────────────

/// User's relationship to a subject (collection state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CollectionState {
    /// 想看
    Wish = 1,
    /// 看过
    Watched = 2,
    /// 在看
    Watching = 3,
    /// 搁置
    OnHold = 4,
    /// 抛弃
    Dropped = 5,
}

/// A Bangumi subject (anime series, movie, etc.).
#[derive(Debug, Clone, Deserialize)]
pub struct BangumiSubject {
    pub id: u64,
    pub name: String,
    pub name_cn: String,
    /// Air date in `YYYY-MM-DD` format.
    pub date: Option<String>,
    /// Platform: `"TV"`, `"WEB"`, etc.
    pub platform: Option<String>,
}

/// A single episode entry from `GET /episodes`.
#[derive(Debug, Clone, Deserialize)]
pub struct BangumiEpisode {
    pub id: u64,
    /// Sort index (float because some specials use 0.5, etc.).
    pub sort: f64,
    /// Episode number within the season (0 = SP).
    pub ep: f64,
    /// Air date, may be absent for unreleased episodes.
    #[serde(alias = "airdate")]
    pub date: Option<String>,
}

/// Container returned by `GET /episodes`.
#[derive(Debug, Clone, Deserialize)]
pub struct BangumiEpisodeList {
    pub total: u64,
    pub data: Vec<BangumiEpisode>,
}

/// A related-subject entry (续集, 前传, etc.).
#[derive(Debug, Clone, Deserialize)]
pub struct BangumiRelated {
    pub id: u64,
    pub relation: String,
    pub name: String,
    pub name_cn: String,
}

/// Search result subject (lighter than `BangumiSubject`).
#[derive(Debug, Clone, Deserialize)]
pub struct BangumiSearchSubject {
    pub id: u64,
    pub name: String,
    pub name_cn: String,
    pub date: Option<String>,
    pub platform: Option<String>,
    pub rank: Option<u64>,
    pub rating: Option<serde_json::Value>,
}

/// Episode collection state for a single episode.
#[derive(Debug, Clone)]
pub struct EpCollectionState {
    pub ep_id: u64,
    /// `true` if the episode is marked as watched (type == 2).
    pub watched: bool,
    pub airdate: Option<String>,
}

/// One subject hit extracted from the bgm.tv HTML search page.
///
/// Used by the web-scrape fallback in [`BangumiApi::web_search_subjects`]
/// when the BGM API returns no candidates for a keyword search.
#[derive(Debug, Clone)]
pub struct BangumiWebHit {
    pub subject_id: u64,
    /// Localised (usually Chinese) name from `<a class="l">`.
    pub name: String,
    /// Japanese original name from `<small class="grey">`, if present.
    pub name_jp: Option<String>,
}

// ── Title & date matching helpers ───────────────────────────────────────────────

/// Case-insensitive similarity ratio in `[0, 1]` between two titles.
///
/// Uses a normalized Levenshtein distance over Unicode scalar values: identical
/// strings score `1.0`, fully disjoint ones `0.0`. Ranks Bangumi search results
/// against the media-server title to avoid binding to a same-named work.
fn title_similarity(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.trim().to_lowercase().chars().collect();
    let b: Vec<char> = b.trim().to_lowercase().chars().collect();
    let max = a.len().max(b.len());
    if max == 0 {
        return 1.0;
    }
    1.0 - levenshtein(&a, &b) as f64 / max as f64
}

/// Levenshtein edit distance between two character slices.
///
/// Two-row dynamic program written with `.get()`/`.last()` accessors so it
/// stays clear of the workspace `indexing_slicing` lint.
fn levenshtein(a: &[char], b: &[char]) -> usize {
    // `prev[j]` holds the distance between `a[..i]` and `b[..j]`.
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut curr: Vec<usize> = Vec::with_capacity(b.len() + 1);
        curr.push(i + 1);
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            let deletion = prev.get(j + 1).copied().unwrap_or(usize::MAX);
            let insertion = curr.last().copied().unwrap_or(usize::MAX);
            let substitution = prev.get(j).copied().unwrap_or(usize::MAX);
            curr.push(
                (deletion + 1).min(insertion + 1).min(substitution + cost),
            );
        }
        prev = curr;
    }
    prev.last().copied().unwrap_or(0)
}

/// Detect the season number stated in a subject's titles, if any.
///
/// Bangumi splits a franchise into one subject per season, and the season is
/// usually spelled out in the title — e.g. `4th season`, `Season 2`, `第四季`,
/// `第2期`, `二期`. Returns the parsed number, or `None` when no season marker
/// is present (which is the normal case for a first season). Both the native
/// (`name`) and Chinese (`name_cn`) titles are inspected.
fn season_from_title(name: &str, name_cn: &str) -> Option<u32> {
    english_season(&name.to_lowercase())
        .or_else(|| english_season(&name_cn.to_lowercase()))
        .or_else(|| cjk_season(name))
        .or_else(|| cjk_season(name_cn))
}

/// Parse an English season marker (`season 4`, `4th season`, `season4`).
fn english_season(lower: &str) -> Option<u32> {
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    for (i, w) in words.iter().enumerate() {
        // Compact form: "season4".
        if let Some(rest) = w.strip_prefix("season")
            && let Some(n) = leading_u32(rest)
        {
            return Some(n);
        }
        if *w == "season" {
            // "season 4"
            if let Some(next) = words.get(i + 1)
                && let Some(n) = leading_u32(next)
            {
                return Some(n);
            }
            // "4th season"
            if i > 0
                && let Some(prev) = words.get(i - 1)
                && let Some(n) = leading_u32(prev)
            {
                return Some(n);
            }
        }
    }
    None
}

/// Parse a CJK season marker: a numeral run immediately before `季`/`期`/`部`.
fn cjk_season(s: &str) -> Option<u32> {
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c != '季' && c != '期' && c != '部' {
            continue;
        }
        // Collect the numeral run ending just before this marker.
        let mut j = i;
        let mut run: Vec<char> = Vec::new();
        while j > 0 {
            let p = chars.get(j - 1).copied().unwrap_or(' ');
            if p.is_ascii_digit() || cjk_digit(p).is_some() {
                run.insert(0, p);
                j -= 1;
            } else {
                break;
            }
        }
        let parsed: String = run.into_iter().collect();
        if let Some(n) = parse_numeral(&parsed) {
            return Some(n);
        }
    }
    None
}

/// Leading ASCII digits of `s` parsed as a number (`"4th"` → `4`).
fn leading_u32(s: &str) -> Option<u32> {
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Map a single CJK digit character to its value (`一`→1 … `十`→10).
fn cjk_digit(c: char) -> Option<u32> {
    match c {
        '一' => Some(1),
        '二' | '两' => Some(2),
        '三' => Some(3),
        '四' => Some(4),
        '五' => Some(5),
        '六' => Some(6),
        '七' => Some(7),
        '八' => Some(8),
        '九' => Some(9),
        '十' => Some(10),
        _ => None,
    }
}

/// Parse an Arabic or CJK numeral string (1–99) into a number.
fn parse_numeral(s: &str) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    if let Ok(n) = s.parse::<u32>() {
        return Some(n);
    }
    let chars: Vec<char> = s.chars().collect();
    match chars.as_slice() {
        [a] => cjk_digit(*a),
        ['十', b] => cjk_digit(*b).map(|v| 10 + v),
        [a, '十'] => cjk_digit(*a).map(|v| v * 10),
        [a, '十', b] => match (cjk_digit(*a), cjk_digit(*b)) {
            (Some(va), Some(vb)) => Some(va * 10 + vb),
            _ => None,
        },
        _ => None,
    }
}

/// Score how well `keyword` matches a candidate's titles.
///
/// Builds on [`title_similarity`] but treats substring containment as a strong
/// signal: a season subject whose title embeds the base title (e.g.
/// `Re:ゼロ… 4th season` contains `Re:ゼロ…`) would otherwise be penalised by the
/// extra characters and fall below the acceptance threshold.
fn base_match_score(keyword: &str, name: &str, name_cn: &str) -> f64 {
    let mut best =
        title_similarity(keyword, name).max(title_similarity(keyword, name_cn));
    for cand in [name, name_cn] {
        if title_contains(cand, keyword) || title_contains(keyword, cand) {
            best = best.max(0.9);
        }
    }
    best
}

/// Whether `haystack` contains `needle` after lowercasing and removing spaces.
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

/// Convert a leading `YYYY-MM-DD` date into a day index for comparison.
///
/// Longer ISO strings (e.g. `2024-10-09T00:00:00Z`) are accepted by reading
/// only the first ten characters. Returns `None` for a malformed prefix.
fn date_to_days(s: &str) -> Option<i64> {
    let d = s.get(..10)?;
    let mut parts = d.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // days-from-civil (Howard Hinnant), valid across the Gregorian calendar.
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy =
        (153 * if month > 2 { month - 3 } else { month + 9 } + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// Absolute day difference between two date strings, if both parse.
fn date_diff_days(a: &str, b: &str) -> Option<i64> {
    Some((date_to_days(a)? - date_to_days(b)?).abs())
}

/// Resolve a target episode to a Bangumi episode ID.
///
/// Prefers the closest air-date match within `fuzzy_days` — robust against
/// per-subject numbering quirks such as recap or `.5` specials — then falls
/// back to an exact `sort` match when no usable air date is available.
fn pick_episode_id(
    episodes: &[BangumiEpisode],
    target_sort: u32,
    air_date: Option<&str>,
    fuzzy_days: i64,
) -> Option<u64> {
    if let Some(date) = air_date {
        let best = episodes
            .iter()
            .filter_map(|ep| {
                let diff = date_diff_days(ep.date.as_deref()?, date)?;
                (diff <= fuzzy_days).then_some((diff, ep.id))
            })
            .min_by_key(|(diff, _)| *diff);
        if let Some((_, id)) = best {
            return Some(id);
        }
    }
    episodes
        .iter()
        .find(|ep| ep.sort as u32 == target_sort)
        .map(|ep| ep.id)
}

// ── Web-search helpers ────────────────────────────────────────────────────────

/// Normalise a title for exact comparison: trim, remove all whitespace,
/// fold fullwidth Latin punctuation (U+FF01–U+FF5E) to halfwidth ASCII,
/// and fold to lowercase. Handles both ASCII and CJK text.
///
/// The fullwidth→halfwidth step makes "没有辣妹会对阿宅温柔！？" (TMDB) and
/// "没有辣妹会对阿宅温柔!?" (bgm.tv scraped) compare as equal.
pub fn normalize_title(s: &str) -> String {
    s.trim()
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| {
            // U+FF01 FULLWIDTH EXCLAMATION MARK … U+FF5E FULLWIDTH TILDE
            // map linearly to U+0021 … U+007E (printable ASCII).
            if ('\u{FF01}'..='\u{FF5E}').contains(&c) {
                char::from_u32(c as u32 - 0xFF01 + 0x0021).unwrap_or(c)
            } else {
                c
            }
        })
        .flat_map(char::to_lowercase)
        .collect()
}

/// Percent-encode every byte that is not an RFC 3986 unreserved character.
///
/// Used to embed a keyword in the BGM web-search path segment without
/// introducing any external dependencies.
fn percent_encode_path(s: &str) -> String {
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

/// Extract the bgm.tv subject ID from a block of HTML containing
/// `href="/subject/{id}"`.
fn extract_subject_id(block: &str) -> Option<u64> {
    const MARKER: &str = "href=\"/subject/";
    let start = block.find(MARKER)? + MARKER.len();
    let rest = &block[start..];
    let end = rest.find('"')?;
    rest[..end].parse().ok()
}

/// Extract the text between `start_marker` and `end_marker` within `s`.
fn extract_between<'a>(
    s: &'a str,
    start_marker: &str,
    end_marker: &str,
) -> Option<&'a str> {
    let start = s.find(start_marker)? + start_marker.len();
    let rest = &s[start..];
    let end = rest.find(end_marker)?;
    Some(&rest[..end])
}

/// Parse the bgm.tv subject search HTML page into a list of hits.
///
/// Each `<li class="item …">` block is scanned for:
/// - subject ID (`href="/subject/NNN"`)
/// - localised name (`<a class="l">…</a>`)
/// - Japanese original name (`<small class="grey">…</small>`)
pub fn parse_bgm_search_html(html: &str) -> Vec<BangumiWebHit> {
    let mut hits = Vec::new();
    for block in html.split("<li class=\"item") {
        let Some(subject_id) = extract_subject_id(block) else {
            continue;
        };
        let Some(name) = extract_between(block, "class=\"l\">", "</a>")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
        else {
            continue;
        };
        let name_jp =
            extract_between(block, "<small class=\"grey\">", "</small>")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
        hits.push(BangumiWebHit {
            subject_id,
            name,
            name_jp,
        });
    }
    hits
}

// ── API client ────────────────────────────────────────────────────────────────

/// Bangumi (bgm.tv) REST API v0 client.
///
/// Constructed with a `base_url` so unit tests can point it at a mock server
/// without real network access.
pub struct BangumiApi {
    username: String,
    access_token: String,
    private: bool,
    base_url: String,
    http: reqwest::Client,
}

impl BangumiApi {
    /// The official bgm.tv API v0 base URL.
    ///
    /// The `/v0` suffix is required: without it every endpoint resolves to the
    /// legacy API and returns 404, which is the historic cause of the sync
    /// silently doing nothing.
    pub const DEFAULT_BASE_URL: &'static str = "https://api.bgm.tv/v0";

    /// Page shown to regenerate a personal access token when the current one is
    /// missing or expired.
    pub const TOKEN_PAGE_URL: &'static str =
        "https://next.bgm.tv/demo/access-token";

    /// Filename for the persisted `series:season → subject_id` resolution cache.
    pub const SUBJECT_CACHE_FILE: &'static str = "bangumi_subjects.json";

    /// Create a new client.
    ///
    /// `base_url` is normally [`Self::DEFAULT_BASE_URL`]. Pass the address of a
    /// local mock server in tests. `private` controls whether new collection
    /// entries are hidden from the user's public profile.
    pub fn new(
        username: impl Into<String>,
        access_token: impl Into<String>,
        private: bool,
        base_url: impl Into<String>,
    ) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(etlp_core::UA_ETLP)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(SyncError::Http)?;
        Ok(Self {
            username: username.into(),
            access_token: access_token.into(),
            private,
            base_url: base_url.into(),
            http,
        })
    }

    /// Shared HTTP client — usable by web-scrape callers in the same crate.
    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// Verify the access token by calling `GET /me`.
    ///
    /// Returns `Ok(())` when the token is accepted. A `401`/`403` response maps
    /// to [`SyncError::Unauthorized`] so callers can prompt the user to
    /// regenerate the token.
    pub async fn verify_token(&self) -> Result<()> {
        debug!(user = %self.username, "bangumi: GET /me (verify token)");
        let resp = crate::curl::send_logged(
            DOMAIN,
            self.http.get(self.url("me")).headers(self.auth_headers()),
        )
        .await?;
        // /me response contains PII (email, nickname, reg_time, …) that the
        // masker does not cover; log only the status code and discard the body.
        let (status, body) =
            crate::curl::read_status_only(DOMAIN, resp).await?;
        let status = status.as_u16();
        if (200..300).contains(&status) {
            return Ok(());
        }
        if status == 401 || status == 403 {
            return Err(SyncError::Unauthorized);
        }
        Err(SyncError::Api { status, body })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url.trim_end_matches('/'), path)
    }

    fn auth_headers(&self) -> reqwest::header::HeaderMap {
        use reqwest::header::{
            ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue,
        };
        let mut map = HeaderMap::new();
        let _ =
            map.insert(ACCEPT, HeaderValue::from_static("application/json"));
        let _ = map
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let val = format!("Bearer {}", self.access_token);
        if let Ok(v) = HeaderValue::from_str(&val) {
            let _ = map.insert(AUTHORIZATION, v);
        }
        map
    }

    // ── Subject search ────────────────────────────────────────────────────────

    /// Search for anime subjects by keyword.
    ///
    /// The BGM search API frequently returns empty results when an `air_date`
    /// filter is included, so date narrowing is done client-side after fetching
    /// candidates (see [`BangumiApi::resolve_subject_id`]).
    pub async fn search_subjects(
        &self,
        keyword: &str,
        limit: u32,
    ) -> Result<Vec<BangumiSearchSubject>> {
        let filter = serde_json::json!({
            "type": [2],
            "nsfw": true,
        });
        let body = serde_json::json!({
            "keyword": keyword,
            "filter": filter,
        });
        let resp = crate::curl::send_logged(
            DOMAIN,
            self.http
                .post(self.url("search/subjects"))
                .headers(self.auth_headers())
                .query(&[("limit", limit)])
                .json(&body),
        )
        .await?;

        let raw: serde_json::Value =
            crate::curl::json_logged(DOMAIN, resp).await?;
        let data = raw
            .get("data")
            .and_then(|d| d.as_array())
            .cloned()
            .unwrap_or_default();
        let subjects = data
            .into_iter()
            .filter_map(|v| {
                serde_json::from_value::<BangumiSearchSubject>(v).ok()
            })
            .collect();
        Ok(subjects)
    }

    // ── Subject & episode info ────────────────────────────────────────────────

    /// Fetch subject metadata by ID.
    pub async fn get_subject(&self, subject_id: u64) -> Result<BangumiSubject> {
        let resp = crate::curl::send_logged(
            DOMAIN,
            self.http
                .get(self.url(&format!("subjects/{subject_id}")))
                .headers(self.auth_headers()),
        )
        .await?;
        crate::curl::json_logged(DOMAIN, resp).await
    }

    /// Fetch all episodes for a subject (type 0 = main episodes).
    pub async fn get_episodes(
        &self,
        subject_id: u64,
    ) -> Result<BangumiEpisodeList> {
        debug!(subject_id, "bangumi: GET /episodes");
        let resp = crate::curl::send_logged(
            DOMAIN,
            self.http
                .get(self.url("episodes"))
                .headers(self.auth_headers())
                .query(&[("subject_id", subject_id), ("type", 0)]),
        )
        .await?;
        crate::curl::json_logged(DOMAIN, resp).await
    }

    /// Fetch subjects related to `subject_id` (e.g. sequels `続集`).
    pub async fn get_related_subjects(
        &self,
        subject_id: u64,
    ) -> Result<Vec<BangumiRelated>> {
        let resp = crate::curl::send_logged(
            DOMAIN,
            self.http
                .get(self.url(&format!("subjects/{subject_id}/subjects")))
                .headers(self.auth_headers()),
        )
        .await?;
        crate::curl::json_logged(DOMAIN, resp).await
    }

    // ── Web-search fallback ───────────────────────────────────────────────────

    /// Scrape the bgm.tv anime search page for `keyword` and return the hits.
    ///
    /// Uses [`BGM_WEB_SEARCH_URL`] with the keyword percent-encoded into the
    /// path. Returns an empty vec on any network or parse error; errors are
    /// logged at `debug` level so they do not surface as noisy warnings when
    /// the network is temporarily unavailable.
    ///
    /// The results are intended for exact-title matching against a set of
    /// known alternate names; see [`parse_bgm_search_html`].
    pub async fn web_search_subjects(
        &self,
        keyword: &str,
    ) -> Vec<BangumiWebHit> {
        let url =
            BGM_WEB_SEARCH_URL.replace("%s", &percent_encode_path(keyword));
        let resp = match self.http.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                debug!(keyword, "bangumi: web search request failed: {e}");
                return Vec::new();
            }
        };
        let html = match resp.text().await {
            Ok(t) => t,
            Err(e) => {
                debug!("bangumi: web search response unreadable: {e}");
                return Vec::new();
            }
        };
        let hits = parse_bgm_search_html(&html);
        debug!(count = hits.len(), keyword, "bangumi: web search hits");
        hits
    }

    // ── Subject resolution by title ───────────────────────────────────────────

    /// Resolve a Bangumi subject ID for an episode using title search and an
    /// optional air-date hint.
    ///
    /// When `premiere_date` (`"YYYY-MM-DD"` or an ISO-8601 prefix) is supplied,
    /// the search is filtered to subjects whose air date is on or after
    /// `premiere_date − 30 days`, narrowing the candidate set to the right
    /// season or arc.  Among the filtered candidates the method picks the one
    /// whose subject `date` is **closest** to `premiere_date`; title similarity
    /// serves as a tiebreaker.  When no unique best candidate can be identified,
    /// a warning is emitted and `None` is returned so the caller can prompt the
    /// user to add an ID mapping.
    ///
    /// When `premiere_date` is absent the method falls back to the original
    /// behaviour: score candidates by title similarity only and walk the `续集`
    /// sequel chain to reach `season`.
    pub async fn resolve_subject_id(
        &self,
        keywords: &[&str],
        season: u32,
        min_score: f64,
        premiere_date: Option<&str>,
    ) -> Result<Option<u64>> {
        // Gather candidates from every keyword, de-duplicated by subject id.
        // Date narrowing is applied client-side below; the BGM search API
        // returns empty results too often when an air_date filter is sent.
        let mut candidates: Vec<BangumiSearchSubject> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for keyword in keywords.iter().filter(|k| !k.trim().is_empty()) {
            for candidate in self.search_subjects(keyword, 10).await? {
                if seen.insert(candidate.id) {
                    candidates.push(candidate);
                }
            }
        }
        if candidates.is_empty() {
            if premiere_date.is_some() {
                warn!(
                    keywords = ?keywords,
                    premiere_date,
                    "bangumi: no subject found for air date — \
                     add an ID mapping in Settings → Bangumi"
                );
            } else {
                debug!("bangumi: title search returned no candidates");
            }
            return Ok(None);
        }

        let score_of = |c: &BangumiSearchSubject| -> f64 {
            keywords
                .iter()
                .filter(|k| !k.trim().is_empty())
                .map(|k| base_match_score(k, &c.name, &c.name_cn))
                .fold(0.0_f64, f64::max)
        };

        // ── Date-guided selection (when premiere_date is supplied) ────────────
        //
        // Rank candidates by absolute date difference from `premiere_date`, then
        // break ties by title similarity.  Warn and return `None` when two or
        // more candidates share the same (diff, score) pair — forcing the user
        // to add an explicit mapping rather than guessing.
        if let Some(ref target_date) = premiere_date.map(str::to_owned) {
            let target_days = date_to_days(target_date);

            // (date_diff_days, -score_millionths, subject_id)
            // Sorting ascending puts best candidates first.
            let mut scored: Vec<(i64, i64, u64)> = candidates
                .iter()
                .filter(|c| score_of(c) >= min_score)
                .map(|c| {
                    let diff = c.date.as_deref().and_then(|d| {
                        target_days.and_then(|td| {
                            date_to_days(d).map(|cd| (cd - td).abs())
                        })
                    });
                    let diff = diff.unwrap_or(i64::MAX);
                    // Negate score so that higher score sorts before lower.
                    let neg_score = -(score_of(c) * 1_000_000.0) as i64;
                    (diff, neg_score, c.id)
                })
                .collect();

            if scored.is_empty() {
                warn!(
                    keywords = ?keywords,
                    premiere_date = target_date.as_str(),
                    "bangumi: no subject candidate cleared min_score — \
                     add an ID mapping in Settings → Bangumi"
                );
                return Ok(None);
            }

            scored.sort_unstable_by_key(|(d, s, _)| (*d, *s));
            let Some(&(best_diff, best_neg_score, _)) = scored.first() else {
                return Ok(None);
            };
            let finalists: Vec<u64> = scored
                .iter()
                .filter(|(d, s, _)| *d == best_diff && *s == best_neg_score)
                .map(|(_, _, id)| *id)
                .collect();

            return match finalists.as_slice() {
                [id] => {
                    debug!(
                        subject_id = id,
                        date_diff = best_diff,
                        "bangumi: resolved subject by air date"
                    );
                    Ok(Some(*id))
                }
                _ => {
                    warn!(
                        keywords = ?keywords,
                        premiere_date = target_date.as_str(),
                        finalists = ?finalists,
                        "bangumi: multiple subjects equally match air date — \
                         add an ID mapping in Settings → Bangumi"
                    );
                    Ok(None)
                }
            };
        }

        // ── Fallback: title-only resolution (no premiere_date) ────────────────

        // Direct hit: a candidate whose own title states the target season and
        // whose base title matches. This handles franchises with continuous
        // episode numbering, where the sequel-chain heuristics are unreliable.
        if season > 1 {
            let mut direct: Option<(f64, u64)> = None;
            for c in &candidates {
                let score = score_of(c);
                if score >= min_score
                    && season_from_title(&c.name, &c.name_cn) == Some(season)
                    && direct.is_none_or(|(b, _)| score > b)
                {
                    direct = Some((score, c.id));
                }
            }
            if let Some((score, id)) = direct {
                debug!(
                    id,
                    score, season, "bangumi: resolved season subject by title"
                );
                return Ok(Some(id));
            }
        }

        // Otherwise pick the franchise root: the best base-title match, biased
        // toward a first-season title so the sequel walk starts at the root.
        let mut root: Option<(f64, u64)> = None;
        for c in &candidates {
            let score = score_of(c);
            if score < min_score {
                continue;
            }
            let is_root =
                season_from_title(&c.name, &c.name_cn).is_none_or(|s| s <= 1);
            let rank = if is_root { score + 1.0 } else { score };
            if root.is_none_or(|(b, _)| rank > b) {
                root = Some((rank, c.id));
            }
        }
        let Some((rank, root_id)) = root else {
            debug!("bangumi: no subject candidate cleared min_score");
            return Ok(None);
        };
        debug!(root_id, rank, season, "bangumi: resolved franchise root");
        self.season_subject_id(root_id, season).await
    }

    /// Walk the `续集` (sequel) chain to the subject representing `season`.
    ///
    /// A `season` of 0 or 1 returns `root` unchanged. Each sequel that looks
    /// like a full season (more than three episodes, first `sort` ≤ 1) advances
    /// the counter; interstitial entries (OVAs, recap films) are traversed but
    /// not counted. A visited-set guards against relation cycles.
    async fn season_subject_id(
        &self,
        root: u64,
        season: u32,
    ) -> Result<Option<u64>> {
        // season == 0 means TMDB "Specials" — there is no reliable Bangumi
        // equivalent reachable via the sequel chain; let the caller decide.
        if season == 0 {
            return Ok(None);
        }
        if season == 1 {
            return Ok(Some(root));
        }
        let mut current = root;
        let mut counter = 1u32;
        let mut visited = std::collections::HashSet::new();
        while visited.insert(current) {
            let related = self.get_related_subjects(current).await?;
            let Some(sequel) =
                related.into_iter().find(|r| r.relation == "续集")
            else {
                debug!(current, "bangumi: sequel chain ended before season");
                return Ok(None);
            };
            // Prefer the season stated in the sequel's own title: it is exact
            // even when episodes are numbered continuously across the franchise
            // (where the episode-count heuristic below cannot tell a new season
            // from a split-cour continuation).
            if let Some(s) = season_from_title(&sequel.name, &sequel.name_cn) {
                if s == season {
                    return Ok(Some(sequel.id));
                }
                if s > season {
                    debug!(current, "bangumi: sequel chain overshot season");
                    return Ok(None);
                }
                counter = s;
                current = sequel.id;
                continue;
            }
            // Fallback when the title carries no season marker: treat a sequel
            // that restarts numbering (first sort ≤ 1, more than three episodes)
            // as the next season.
            let episodes = self.get_episodes(sequel.id).await?;
            let is_season = episodes.total > 3
                && episodes.data.first().is_some_and(|e| e.sort <= 1.0);
            current = sequel.id;
            if is_season {
                counter += 1;
                if counter == season {
                    return Ok(Some(current));
                }
            }
        }
        debug!(
            root,
            season, "bangumi: sequel chain cycle, season not found"
        );
        Ok(None)
    }

    // ── User collection ───────────────────────────────────────────────────────

    /// Get the user's collection entry for a subject, or `None` if uncollected.
    pub async fn get_subject_collection(
        &self,
        subject_id: u64,
    ) -> Result<Option<serde_json::Value>> {
        debug!(
            subject_id,
            user = %self.username,
            "bangumi: GET subject collection"
        );
        let resp = crate::curl::send_logged(
            DOMAIN,
            self.http
                .get(self.url(&format!(
                    "users/{}/collections/{}",
                    self.username, subject_id
                )))
                .headers(self.auth_headers()),
        )
        .await?;
        let (status, body) = crate::curl::read_logged(DOMAIN, resp).await?;
        match status.as_u16() {
            404 => Ok(None),
            200 => {
                Ok(Some(serde_json::from_str(&body).map_err(SyncError::Json)?))
            }
            status => Err(SyncError::Api { status, body }),
        }
    }

    /// Add or update the user's collection entry for a subject.
    ///
    /// Uses `POST /users/-/collections/{subject_id}` so it works for both
    /// adding new and updating existing entries.
    pub async fn add_collection_subject(
        &self,
        subject_id: u64,
        state: CollectionState,
    ) -> Result<()> {
        debug!(
            subject_id,
            state = state as u8,
            private = self.private,
            "bangumi: POST add/update collection"
        );
        let body = serde_json::json!({
            "type":    state as u8,
            "private": self.private,
        });
        let resp = crate::curl::send_logged(
            DOMAIN,
            self.http
                .post(self.url(&format!("users/-/collections/{subject_id}")))
                .headers(self.auth_headers())
                .json(&body),
        )
        .await?;
        let (status, body) = crate::curl::read_logged(DOMAIN, resp).await?;
        // 200/202/204 all count as success.
        if status.is_success() {
            return Ok(());
        }
        Err(SyncError::Api {
            status: status.as_u16(),
            body,
        })
    }

    /// Ensure the subject sits in the user's collection, adding it as
    /// `Watching` when absent.
    ///
    /// Used when the viewer has only partially watched an episode (< 90 %): the
    /// series should appear as in-progress without marking any episode watched.
    /// An existing collection state (including `Watched`) is left untouched so a
    /// partial replay never downgrades a finished subject.
    pub async fn ensure_collected_watching(
        &self,
        subject_id: u64,
    ) -> Result<()> {
        if self.get_subject_collection(subject_id).await?.is_none() {
            info!(
                "bangumi: subject {subject_id} not collected, \
                 adding as Watching"
            );
            self.add_collection_subject(subject_id, CollectionState::Watching)
                .await?;
        }
        Ok(())
    }

    /// Mark one or more episodes of a subject as watched.
    ///
    /// Uses `PATCH /users/-/collections/{subject_id}/episodes` for bulk
    /// updates (multiple IDs) or `PUT /users/-/collections/-/episodes/{ep_id}`
    /// for a single episode.
    pub async fn mark_episodes_watched(
        &self,
        subject_id: u64,
        ep_ids: &[u64],
    ) -> Result<()> {
        if ep_ids.is_empty() {
            return Ok(());
        }
        debug!(
            subject_id,
            ep_ids = ?ep_ids,
            "bangumi: marking episodes watched"
        );

        if ep_ids.len() == 1 {
            let ep_id = ep_ids
                .first()
                .copied()
                .ok_or(SyncError::MissingField { field: "ep_id" })?;
            let body =
                serde_json::json!({ "type": CollectionState::Watched as u8 });
            let resp = crate::curl::send_logged(
                DOMAIN,
                self.http
                    .put(self.url(&format!(
                        "users/-/collections/-/episodes/{ep_id}"
                    )))
                    .headers(self.auth_headers())
                    .json(&body),
            )
            .await?;
            let (status, text) = crate::curl::read_logged(DOMAIN, resp).await?;
            if status.is_success() {
                return Ok(());
            }
            return Err(SyncError::Api {
                status: status.as_u16(),
                body: text,
            });
        }

        // Bulk update.
        let body = serde_json::json!({
            "episode_id": ep_ids,
            "type":        CollectionState::Watched as u8,
        });
        let resp =
            crate::curl::send_logged(
                DOMAIN,
                self.http
                    .patch(self.url(&format!(
                        "users/-/collections/{subject_id}/episodes"
                    )))
                    .headers(self.auth_headers())
                    .json(&body),
            )
            .await?;
        let (status, text) = crate::curl::read_logged(DOMAIN, resp).await?;
        if status.is_success() {
            return Ok(());
        }
        Err(SyncError::Api {
            status: status.as_u16(),
            body: text,
        })
    }

    /// Return a map of `sort_number → EpCollectionState` for a subject.
    pub async fn get_user_eps_collection(
        &self,
        subject_id: u64,
    ) -> Result<HashMap<u64, EpCollectionState>> {
        let resp =
            crate::curl::send_logged(
                DOMAIN,
                self.http
                    .get(self.url(&format!(
                        "users/-/collections/{subject_id}/episodes"
                    )))
                    .headers(self.auth_headers()),
            )
            .await?;
        let raw: serde_json::Value =
            crate::curl::json_logged(DOMAIN, resp).await?;
        let mut map = HashMap::new();
        if let Some(arr) = raw.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                let ep = item.get("episode");
                let sort = ep
                    .and_then(|e| e.get("sort"))
                    .and_then(|s| s.as_f64())
                    .unwrap_or(0.0) as u64;
                let ep_num = ep
                    .and_then(|e| e.get("ep"))
                    .and_then(|e| e.as_f64())
                    .unwrap_or(0.0);
                if ep_num == 0.0 {
                    continue; // skip specials
                }
                let ep_id = ep
                    .and_then(|e| e.get("id"))
                    .and_then(|i| i.as_u64())
                    .unwrap_or(0);
                let watched = item
                    .get("type")
                    .and_then(|t| t.as_u64())
                    .map(|t| t == 2)
                    .unwrap_or(false);
                let airdate = ep
                    .and_then(|e| e.get("date").or_else(|| e.get("airdate")))
                    .and_then(|d| d.as_str())
                    .map(str::to_owned);
                map.insert(
                    sort,
                    EpCollectionState {
                        ep_id,
                        watched,
                        airdate,
                    },
                );
            }
        }
        Ok(map)
    }
}

// ── Web-scrape resolution algorithm ────────────────────────────────────────────

/// Input to [`resolve_by_web_scrape`] — distinguishes TV episodes from movies.
pub enum WebResolveTarget<'a> {
    /// TV episode: match by ep_range, then optionally score episode title.
    Episode {
        episode: u32,
        premiere_date: Option<&'a str>,
        episode_title: Option<&'a str>,
    },
    /// Movie: no ep_range check; match by date + title similarity only.
    Movie { premiere_date: Option<&'a str> },
}

/// Bundles all parameters for [`resolve_by_web_scrape`] to keep arg count low.
pub struct WebScrapeReq<'a> {
    pub series: &'a str,
    pub season: u32,
    /// Primary search keywords (deduplicated original_title + series_name).
    pub keywords: &'a [&'a str],
    /// TMDB alternative titles — used in Round 1 exact matching only.
    pub alt_titles: &'a [String],
    pub target: WebResolveTarget<'a>,
}

/// Episode-specific parameters for [`resolve_episode_matching`].
struct EpisodeMatchInput<'a> {
    episode: u32,
    premiere_date: Option<&'a str>,
    episode_title: Option<&'a str>,
}

/// `true` when `premiere_date` is within the date window of `start_date`.
///
/// Passes when either date is absent (cannot reject what cannot be verified).
fn date_window_ok(
    premiere_date: Option<&str>,
    start_date: Option<&str>,
) -> bool {
    match (premiere_date, start_date) {
        (Some(ep), Some(start)) => {
            match (date_to_days(ep), date_to_days(start)) {
                (Some(e), Some(s)) => {
                    e >= s - crate::bangumi_web::BANGUMI_DATE_WINDOW_DAYS
                }
                _ => true,
            }
        }
        _ => true,
    }
}

/// `true` when any of `titles` is an exact normalised match for `detail`'s
/// name or Japanese name.
fn is_exact_title_match(
    titles: &[&str],
    detail: &crate::bangumi_web::SubjectDetail,
) -> bool {
    let name_n = normalize_title(&detail.name);
    let jp_n = detail.name_jp.as_deref().map(normalize_title);
    titles.iter().any(|t| {
        let t_n = normalize_title(t);
        t_n == name_n || jp_n.as_deref() == Some(t_n.as_str())
    })
}

/// Highest title similarity between any `keyword` and `detail`'s name/name_jp.
fn best_subject_score(
    keywords: &[&str],
    detail: &crate::bangumi_web::SubjectDetail,
) -> f64 {
    keywords
        .iter()
        .map(|k| {
            crate::bangumi_web::base_match_score(
                k,
                &detail.name,
                detail.name_jp.as_deref().unwrap_or(""),
            )
        })
        .fold(0.0_f64, f64::max)
}

/// Collect [`SubjectDetail`]s for all candidates found by `keywords`.
///
/// Uses `scrape_cache` to avoid duplicate searches and detail fetches within
/// the same sync pass.
async fn collect_details(
    http: &reqwest::Client,
    bgm_base_url: &str,
    keywords: &[&str],
    scrape_cache: &mut crate::bangumi_web::ScrapeCache,
) -> Vec<crate::bangumi_web::SubjectDetail> {
    use crate::bangumi_web;
    use std::collections::hash_map::Entry;

    let mut seen: std::collections::HashSet<u64> =
        std::collections::HashSet::new();
    let mut details = Vec::new();

    for keyword in keywords.iter().filter(|k| !k.trim().is_empty()) {
        let key = (*keyword).to_owned();
        if let Entry::Vacant(e) = scrape_cache.search_results.entry(key.clone())
        {
            let results =
                bangumi_web::web_search_all_pages(http, bgm_base_url, keyword)
                    .await;
            e.insert(results);
        }
        let candidates = scrape_cache
            .search_results
            .get(&key)
            .map(|v| v.to_vec())
            .unwrap_or_default();

        for candidate in candidates {
            let id = candidate.subject_id;
            if !seen.insert(id) {
                continue;
            }
            if let Entry::Vacant(e) = scrape_cache.subject_details.entry(id)
                && let Some(d) = bangumi_web::fetch_subject_detail(
                    http,
                    bgm_base_url,
                    &candidate,
                )
                .await
            {
                e.insert(d);
            }
            if let Some(d) = scrape_cache.subject_details.get(&id).cloned() {
                details.push(d);
            }
        }
    }
    details
}

/// Resolve a Bangumi subject by scraping bgm.tv search + detail pages.
///
/// Implements the Priority-3 resolution path described in the spec:
/// - Episode: ep-range filter → Round 1 exact title → Round 2 combined score
/// - Movie: Round 1 exact title + date → Round 2 title similarity + date
///
/// `scrape_cache` is shared across all per-episode calls in one `sync_bangumi`
/// pass so the same search / detail page is never fetched twice.
pub async fn resolve_by_web_scrape(
    http: &reqwest::Client,
    bgm_base_url: &str,
    req: &WebScrapeReq<'_>,
    scrape_cache: &mut crate::bangumi_web::ScrapeCache,
) -> Option<u64> {
    let episode_for_log = match &req.target {
        WebResolveTarget::Episode { episode, .. } => *episode,
        WebResolveTarget::Movie { .. } => 0,
    };
    debug!(
        series = req.series,
        season = req.season,
        episode = episode_for_log,
        keywords = ?req.keywords,
        "bangumi: resolve_subject"
    );

    let details =
        collect_details(http, bgm_base_url, req.keywords, scrape_cache).await;

    let all_titles: Vec<&str> = req
        .keywords
        .iter()
        .copied()
        .chain(req.alt_titles.iter().map(|s| s.as_str()))
        .collect();

    match &req.target {
        WebResolveTarget::Episode {
            episode,
            premiere_date,
            episode_title,
        } => resolve_episode_matching(
            req.series,
            req.season,
            &EpisodeMatchInput {
                episode: *episode,
                premiere_date: *premiere_date,
                episode_title: *episode_title,
            },
            req.keywords,
            &all_titles,
            &details,
        ),
        WebResolveTarget::Movie { premiere_date } => resolve_movie_matching(
            req.series,
            *premiere_date,
            req.keywords,
            &all_titles,
            &details,
        ),
    }
}

fn pick_top(
    scored: &[(f64, u64)],
    min_score: f64,
    series: &str,
) -> Option<u64> {
    let &(best, _) = scored.first()?;
    if best < min_score {
        return None;
    }
    let top: Vec<u64> = scored
        .iter()
        .filter(|(s, _)| (s - best).abs() < 1e-9)
        .map(|(_, id)| *id)
        .collect();
    match top.as_slice() {
        [id] => Some(*id),
        _ => {
            warn!(
                series,
                tied = ?top,
                "bangumi: subject resolution tied — \
                 add an ID mapping in Settings → Bangumi"
            );
            None
        }
    }
}

fn sort_scored(scored: &mut [(f64, u64)]) {
    scored.sort_by(|(a, _), (b, _)| {
        b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn resolve_episode_matching(
    series: &str,
    season: u32,
    input: &EpisodeMatchInput<'_>,
    scoring_keywords: &[&str],
    all_titles: &[&str],
    details: &[crate::bangumi_web::SubjectDetail],
) -> Option<u64> {
    let episode = input.episode;
    let premiere_date = input.premiere_date;
    let episode_title = input.episode_title;

    let in_range: Vec<&crate::bangumi_web::SubjectDetail> = details
        .iter()
        .filter(|d| {
            d.ep_range
                .is_some_and(|(min, max)| min <= episode && episode <= max)
        })
        .collect();

    debug!(
        count = in_range.len(),
        episode, "bangumi: episode_range_filter"
    );

    // Round 1: exact title match + date window + in range
    let r1: Vec<_> = in_range
        .iter()
        .filter(|d| {
            let exact = is_exact_title_match(all_titles, d);
            let date_ok =
                date_window_ok(premiere_date, d.start_date.as_deref());
            debug!(
                subject_id = d.subject_id,
                name = %d.name,
                exact_match = exact,
                in_range = true,
                date_ok,
                "bangumi: round1_check"
            );
            exact && date_ok
        })
        .copied()
        .collect();

    debug!(count = r1.len(), "bangumi: round1_result");

    if let [single] = r1.as_slice() {
        let id = single.subject_id;
        debug!(subject_id = id, via = "round1", "bangumi: resolve_result");
        return Some(id);
    }

    // Round 2 candidates: in_range + date_window
    let r2: Vec<_> = in_range
        .iter()
        .filter(|d| date_window_ok(premiere_date, d.start_date.as_deref()))
        .copied()
        .collect();

    if r2.is_empty() {
        warn!(
            series,
            season,
            episode,
            "bangumi: no subject matched for {series} S{season}E{episode} — \
             add an ID mapping in Settings → Bangumi"
        );
        return None;
    }

    let mut scored: Vec<(f64, u64)> = r2
        .iter()
        .map(|d| {
            let ep_title_score = episode_title
                .and_then(|et| {
                    d.episodes.iter().find(|(n, _)| *n == episode).map(
                        |(_, t)| crate::bangumi_web::title_similarity(et, t),
                    )
                })
                .unwrap_or(0.0);
            let subj_title_score = best_subject_score(scoring_keywords, d);
            let combined = if episode_title.is_some() {
                0.6 * ep_title_score + 0.4 * subj_title_score
            } else {
                subj_title_score
            };
            debug!(
                subject_id = d.subject_id,
                ep_title = ep_title_score,
                subj_title = subj_title_score,
                combined,
                "bangumi: round2_scoring"
            );
            (combined, d.subject_id)
        })
        .collect();

    sort_scored(&mut scored);

    match pick_top(&scored, crate::bangumi_web::BANGUMI_TITLE_MIN_SCORE, series)
    {
        None => {
            warn!(
                series,
                season,
                episode,
                "bangumi: no subject matched for {series} S{season}E{episode} \
                 — add an ID mapping in Settings → Bangumi"
            );
            None
        }
        Some(id) => {
            let best = scored.first().map(|(s, _)| *s).unwrap_or(0.0);
            debug!(
                subject_id = id,
                score = best,
                via = "round2",
                "bangumi: resolve_result"
            );
            Some(id)
        }
    }
}

fn resolve_movie_matching(
    series: &str,
    premiere_date: Option<&str>,
    scoring_keywords: &[&str],
    all_titles: &[&str],
    details: &[crate::bangumi_web::SubjectDetail],
) -> Option<u64> {
    // Round 1: exact title + date window
    let r1: Vec<_> = details
        .iter()
        .filter(|d| {
            is_exact_title_match(all_titles, d)
                && date_window_ok(premiere_date, d.start_date.as_deref())
        })
        .collect();

    debug!(count = r1.len(), "bangumi: round1_result (movie)");

    if let [single] = r1.as_slice() {
        let id = single.subject_id;
        debug!(subject_id = id, via = "round1", "bangumi: resolve_result");
        return Some(id);
    }

    // Round 2: date window filter + title similarity
    let r2: Vec<_> = details
        .iter()
        .filter(|d| date_window_ok(premiere_date, d.start_date.as_deref()))
        .collect();

    let mut scored: Vec<(f64, u64)> = r2
        .iter()
        .map(|d| (best_subject_score(scoring_keywords, d), d.subject_id))
        .collect();

    sort_scored(&mut scored);

    match pick_top(&scored, crate::bangumi_web::BANGUMI_TITLE_MIN_SCORE, series)
    {
        None => {
            warn!(
                series,
                "bangumi: no movie subject matched for {series} — \
                 add an ID mapping in Settings → Bangumi"
            );
            None
        }
        Some(id) => {
            let best = scored.first().map(|(s, _)| *s).unwrap_or(0.0);
            debug!(
                subject_id = id,
                score = best,
                via = "round2",
                "bangumi: resolve_result (movie)"
            );
            Some(id)
        }
    }
}

// ── Subject ID cache ────────────────────────────────────────────────────────────

/// On-disk cache mapping `series_id:season:episode` to a resolved Bangumi
/// subject ID.
///
/// The key is episode-granular so that different episodes within the same TMDB
/// season can map to different Bangumi subjects (e.g. when a franchise is split
/// into separate arc subjects on bgm.tv). Title search is fuzzy and costs
/// several requests, so a resolved mapping is persisted and reused on the next
/// playback of the same episode. The cache is advisory: any read/parse failure
/// degrades to an empty cache rather than aborting the sync.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SubjectCache {
    #[serde(flatten)]
    map: std::collections::BTreeMap<String, u64>,
}

impl SubjectCache {
    /// Build the cache key for a series/season/episode triple.
    fn key(series_id: &str, season: i64, episode: i64) -> String {
        format!("{series_id}:{season}:{episode}")
    }

    /// Load the cache from `path`, returning an empty cache when the file is
    /// absent or cannot be parsed.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Look up a cached subject ID for a series/season/episode triple.
    #[must_use]
    pub fn get(
        &self,
        series_id: &str,
        season: i64,
        episode: i64,
    ) -> Option<u64> {
        self.map
            .get(&Self::key(series_id, season, episode))
            .copied()
    }

    /// Insert a mapping and persist the whole cache to `path`.
    pub fn insert(
        &mut self,
        series_id: &str,
        season: i64,
        episode: i64,
        subject_id: u64,
        path: &Path,
    ) -> Result<()> {
        let _ = self
            .map
            .insert(Self::key(series_id, season, episode), subject_id);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

// ── Sync orchestration ────────────────────────────────────────────────────────

/// Sync episode watch status to Bangumi using a subject ID obtained directly
/// from the Emby item's `ProviderIds.Bangumi` field.
///
/// Thin wrapper over [`sync_episodes`] that matches purely by `sort` number
/// (no air-date hints), preserving the behaviour of the provider-id path.
pub async fn sync_episode_by_bangumi_id(
    api: &BangumiApi,
    subject_id: u64,
    ep_sorts: &[u32],
) -> Result<Vec<u64>> {
    let eps: Vec<(u32, Option<String>)> =
        ep_sorts.iter().map(|&sort| (sort, None)).collect();
    sync_episodes(api, subject_id, &eps).await
}

/// Mark episodes of a known subject as watched.
///
/// 1. Ensures the subject is in the user's collection (adds as Watching if not;
///    skips entirely when it is already marked Watched).
/// 2. Resolves each `(sort, air_date)` pair to a Bangumi episode ID, preferring
///    the air-date match and falling back to the sort number.
/// 3. Marks the resolved episodes as watched.
///
/// Returns the list of Bangumi episode IDs that were marked.
pub async fn sync_episodes(
    api: &BangumiApi,
    subject_id: u64,
    eps: &[(u32, Option<String>)],
) -> Result<Vec<u64>> {
    if eps.is_empty() {
        return Ok(Vec::new());
    }
    debug!(
        subject_id,
        count = eps.len(),
        "bangumi: sync_episodes start"
    );

    // Ensure the subject is collected.
    match api.get_subject_collection(subject_id).await? {
        None => {
            info!(
                "bangumi: subject {subject_id} not collected, adding as Watching"
            );
            api.add_collection_subject(subject_id, CollectionState::Watching)
                .await?;
        }
        Some(ref c) if c.get("type").and_then(|t| t.as_u64()) == Some(2) => {
            info!("bangumi: subject {subject_id} already marked Watched, skip");
            return Ok(Vec::new());
        }
        Some(_) => {}
    }

    // Resolve each (sort, air_date) pair to a Bangumi episode ID.
    let ep_list = api.get_episodes(subject_id).await?;
    let ep_ids: Vec<u64> = eps
        .iter()
        .filter_map(|(sort, air_date)| {
            pick_episode_id(&ep_list.data, *sort, air_date.as_deref(), 2)
        })
        .collect();
    debug!(
        subject_id,
        total = ep_list.total,
        resolved = ?ep_ids,
        "bangumi: resolved episodes to ids"
    );

    if ep_ids.is_empty() {
        info!(
            "bangumi: no matching episodes found in subject {subject_id} \
             for {eps:?}"
        );
        return Ok(Vec::new());
    }

    api.mark_episodes_watched(subject_id, &ep_ids).await?;
    info!(
        "bangumi: marked {} episodes watched in subject {subject_id}",
        ep_ids.len()
    );
    Ok(ep_ids)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn make_api(server: &MockServer) -> BangumiApi {
        BangumiApi::new("testuser", "tok123", true, server.uri()).unwrap()
    }

    // ── search_subjects ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn search_subjects_parses_data_array() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 1,
                    "data": [{
                        "id":      301,
                        "name":    "テスト",
                        "name_cn": "测试",
                        "date":    "2024-04-01",
                        "platform": "TV",
                        "rank": 100,
                    }],
                }),
            ))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let results = api.search_subjects("テスト", 5).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results.first().map(|s| s.id), Some(301));
    }

    #[tokio::test]
    async fn search_subjects_returns_empty_on_empty_data() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({ "total": 0, "data": [] }),
                ),
            )
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let results = api.search_subjects("missing", 5).await.unwrap();
        assert!(results.is_empty());
    }

    // ── get_subject ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn get_subject_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/subjects/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "id": 42,
                    "name": "進撃の巨人",
                    "name_cn": "进击的巨人",
                    "date": "2013-04-07",
                    "platform": "TV",
                }),
            ))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let sub = api.get_subject(42).await.unwrap();
        assert_eq!(sub.id, 42);
        assert_eq!(sub.name_cn, "进击的巨人");
    }

    // ── get_episodes ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn get_episodes_parses_list() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 2,
                    "data": [
                        { "id": 1001, "sort": 1.0, "ep": 1.0, "date": "2024-01-07" },
                        { "id": 1002, "sort": 2.0, "ep": 2.0, "date": "2024-01-14" },
                    ],
                }),
            ))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let eps = api.get_episodes(42).await.unwrap();
        assert_eq!(eps.total, 2);
        assert_eq!(eps.data.len(), 2);
        assert_eq!(eps.data.first().map(|e| e.id), Some(1001));
    }

    // ── get_related_subjects ──────────────────────────────────────────────────

    #[tokio::test]
    async fn get_related_subjects_returns_sequels() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/subjects/42/subjects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([
                    { "id": 43, "relation": "续集",
                      "name": "S2", "name_cn": "第2季" }
                ]),
            ))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let related = api.get_related_subjects(42).await.unwrap();
        assert_eq!(related.len(), 1);
        assert_eq!(related.first().map(|r| r.id), Some(43));
        assert_eq!(related.first().map(|r| r.relation.as_str()), Some("续集"));
    }

    // ── get_subject_collection ────────────────────────────────────────────────

    #[tokio::test]
    async fn get_subject_collection_returns_none_on_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/42"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let res = api.get_subject_collection(42).await.unwrap();
        assert!(res.is_none());
    }

    #[tokio::test]
    async fn get_subject_collection_returns_some_on_200() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "type": 3, "subject_id": 42 }),
            ))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let res = api.get_subject_collection(42).await.unwrap();
        assert!(res.is_some());
        assert_eq!(res.as_ref().and_then(|v| v["type"].as_u64()), Some(3));
    }

    // ── add_collection_subject ────────────────────────────────────────────────

    #[tokio::test]
    async fn add_collection_subject_posts_correct_state() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/users/-/collections/42"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        api.add_collection_subject(42, CollectionState::Watching)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn add_collection_subject_sends_private_flag() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/users/-/collections/42"))
            .and(wiremock::matchers::body_partial_json(
                serde_json::json!({ "private": true }),
            ))
            .respond_with(ResponseTemplate::new(202))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        api.add_collection_subject(42, CollectionState::Watching)
            .await
            .unwrap();
    }

    // ── ensure_collected_watching ─────────────────────────────────────────────

    #[tokio::test]
    async fn ensure_watching_adds_when_not_collected() {
        let server = MockServer::start().await;
        // Not collected yet → 404.
        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/42"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        // ...so it must be added as Watching (type 3).
        Mock::given(method("POST"))
            .and(path("/users/-/collections/42"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "type": 3,
            })))
            .respond_with(ResponseTemplate::new(202))
            .expect(1)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        api.ensure_collected_watching(42).await.unwrap();
    }

    #[tokio::test]
    async fn ensure_watching_leaves_existing_collection_untouched() {
        let server = MockServer::start().await;
        // Already collected → must not POST any collection update.
        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "type": 2, "subject_id": 42 }),
            ))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/users/-/collections/42"))
            .respond_with(ResponseTemplate::new(202))
            .expect(0)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        api.ensure_collected_watching(42).await.unwrap();
    }

    // ── verify_token ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn verify_token_ok_on_200() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({ "username": "testuser" }),
                ),
            )
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        api.verify_token().await.unwrap();
    }

    #[tokio::test]
    async fn requests_use_unified_user_agent() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me"))
            .and(wiremock::matchers::header("user-agent", etlp_core::UA_ETLP))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        // The mock only matches when the request carried `User-Agent: etlp`,
        // so a non-error result proves the unified agent was sent.
        assert!(api.verify_token().await.is_ok());
    }

    #[tokio::test]
    async fn verify_token_maps_401_to_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let err = api.verify_token().await.unwrap_err();
        assert!(matches!(err, SyncError::Unauthorized));
    }

    // ── mark_episodes_watched ─────────────────────────────────────────────────

    #[tokio::test]
    async fn mark_single_episode_watched_uses_put() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/users/-/collections/-/episodes/1001"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        api.mark_episodes_watched(42, &[1001]).await.unwrap();
    }

    #[tokio::test]
    async fn mark_multiple_episodes_watched_uses_patch() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/users/-/collections/42/episodes"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        api.mark_episodes_watched(42, &[1001, 1002]).await.unwrap();
    }

    // ── sync_episode_by_bangumi_id ────────────────────────────────────────────

    #[tokio::test]
    async fn sync_episode_by_bangumi_id_full_flow() {
        let server = MockServer::start().await;

        // Collection check: not collected yet.
        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/42"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        // Add to collection as Watching.
        Mock::given(method("POST"))
            .and(path("/users/-/collections/42"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&server)
            .await;

        // Episode list.
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 2,
                    "data": [
                        { "id": 1001, "sort": 1.0, "ep": 1.0, "date": "2024-01-07" },
                        { "id": 1002, "sort": 2.0, "ep": 2.0, "date": "2024-01-14" },
                    ],
                }),
            ))
            .mount(&server)
            .await;

        // Bulk mark watched.
        Mock::given(method("PATCH"))
            .and(path("/users/-/collections/42/episodes"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let marked =
            sync_episode_by_bangumi_id(&api, 42, &[1, 2]).await.unwrap();
        assert_eq!(marked.len(), 2);
        assert!(marked.contains(&1001));
        assert!(marked.contains(&1002));
    }

    #[tokio::test]
    async fn sync_episode_skips_already_watched_subject() {
        let server = MockServer::start().await;

        // Collection entry with type=2 (Watched).
        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "type": 2, "subject_id": 42 }),
            ))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let marked = sync_episode_by_bangumi_id(&api, 42, &[1]).await.unwrap();
        assert!(marked.is_empty());
    }

    #[tokio::test]
    async fn sync_episode_returns_empty_for_empty_sorts() {
        let server = MockServer::start().await;
        let api = make_api(&server).await;
        let marked = sync_episode_by_bangumi_id(&api, 42, &[]).await.unwrap();
        assert!(marked.is_empty());
    }

    // ── normalize_title ───────────────────────────────────────────────────────

    #[test]
    fn normalize_title_strips_whitespace_and_lowercases() {
        assert_eq!(
            normalize_title("  魔法姐妹露露特莉莉  "),
            "魔法姐妹露露特莉莉"
        );
        assert_eq!(normalize_title("Re:Zero"), "re:zero");
        assert_eq!(normalize_title("A B C"), "abc");
        // Full-width space is whitespace and must be stripped.
        assert_eq!(normalize_title("魔法　姐妹"), "魔法姐妹");
    }

    #[test]
    fn normalize_title_equalizes_fullwidth_and_halfwidth_punctuation() {
        // bgm.tv scrapes "!?" (halfwidth); TMDB returns "！？" (fullwidth).
        // Both must normalise to the same string so the web-fallback matches.
        let bgm = normalize_title("没有辣妹会对阿宅温柔!?");
        let tmdb = normalize_title("没有辣妹会对阿宅温柔！？");
        assert_eq!(bgm, tmdb);

        // Verify the entire fullwidth range maps correctly for common chars.
        assert_eq!(normalize_title("ＡＢＣＤ"), "abcd");
        assert_eq!(normalize_title("１２３"), "123");
        assert_eq!(normalize_title("（test）"), "(test)");
    }

    // ── parse_bgm_search_html ─────────────────────────────────────────────────

    /// Minimal HTML fragment that mirrors the structure bgm.tv actually serves.
    fn bgm_html_fixture() -> &'static str {
        r#"<ul class="browserList">
<li class="item anime" id="subject_501796">
<div class="inner">
<h3>
<span class="ico_subject_type subject_type_2 ll"></span>
<a href="/subject/501796" class="l">魔法姐妹露露特莉莉</a>
<small class="grey">魔法の姉妹ルルットリリィ</small>
</h3>
<p class="info tip">
2026年4月5日 / 道解慎太郎 / スタジオぴえろ
</p>
</div>
</li>
<li class="item anime" id="subject_99999">
<div class="inner">
<h3>
<span class="ico_subject_type subject_type_2 ll"></span>
<a href="/subject/99999" class="l">另一部动画</a>
<small class="grey">もう一つのアニメ</small>
</h3>
<p class="info tip">2025年1月7日</p>
</div>
</li>
</ul>"#
    }

    #[test]
    fn parse_bgm_search_html_extracts_subject_id_and_names() {
        let hits = parse_bgm_search_html(bgm_html_fixture());
        assert_eq!(hits.len(), 2);

        let first = hits.first().unwrap();
        assert_eq!(first.subject_id, 501796);
        assert_eq!(first.name, "魔法姐妹露露特莉莉");
        assert_eq!(first.name_jp.as_deref(), Some("魔法の姉妹ルルットリリィ"));

        let second = hits.get(1).unwrap();
        assert_eq!(second.subject_id, 99999);
        assert_eq!(second.name, "另一部动画");
    }

    #[test]
    fn parse_bgm_search_html_returns_empty_for_no_items() {
        let hits =
            parse_bgm_search_html("<html><body>no results</body></html>");
        assert!(hits.is_empty());
    }

    #[test]
    fn normalize_title_matches_bgm_hit_by_cn_name() {
        let hits = parse_bgm_search_html(bgm_html_fixture());
        // Simulate the match set built in the fallback: Emby alt title matches
        // the BGM Chinese name after normalisation.
        let candidates = [
            "魔法的姐妹露露和莉莉".to_owned(),
            "魔法姐妹露露特莉莉".to_owned(),
        ];
        let found = candidates.iter().find_map(|c| {
            let norm = normalize_title(c);
            hits.iter()
                .find(|h| {
                    normalize_title(&h.name) == norm
                        || h.name_jp
                            .as_ref()
                            .is_some_and(|jp| normalize_title(jp) == norm)
                })
                .map(|h| h.subject_id)
        });
        assert_eq!(found, Some(501796));
    }

    #[test]
    fn normalize_title_matches_bgm_hit_by_jp_name() {
        let hits = parse_bgm_search_html(bgm_html_fixture());
        // Japanese original title from TMDB alternates matches the BGM jp name.
        let candidate = "魔法の姉妹ルルットリリィ";
        let norm = normalize_title(candidate);
        let found = hits.iter().find(|h| {
            normalize_title(&h.name) == norm
                || h.name_jp
                    .as_ref()
                    .is_some_and(|jp| normalize_title(jp) == norm)
        });
        assert_eq!(found.map(|h| h.subject_id), Some(501796));
    }

    // ── title_similarity ──────────────────────────────────────────────────────

    #[test]
    fn title_similarity_scores_identity_and_difference() {
        assert!((title_similarity("Re:Zero", "Re:Zero") - 1.0).abs() < 1e-9);
        // Case-insensitive.
        assert!((title_similarity("re:zero", "RE:ZERO") - 1.0).abs() < 1e-9);
        // Two empty strings are trivially equal.
        assert!((title_similarity("", "") - 1.0).abs() < 1e-9);
        // Disjoint strings score low.
        assert!(title_similarity("abcdef", "zzzzzz") < 0.2);
        // Near matches score high.
        assert!(title_similarity("進撃の巨人", "進撃の巨人 2") > 0.7);
    }

    // ── season_from_title ─────────────────────────────────────────────────────

    #[test]
    fn season_from_title_parses_english_and_cjk() {
        // English forms.
        assert_eq!(
            season_from_title("Re:Zero 4th season 喪失編 TV", ""),
            Some(4)
        );
        assert_eq!(season_from_title("Attack on Titan Season 2", ""), Some(2));
        assert_eq!(season_from_title("Show season3", ""), Some(3));
        // CJK forms.
        assert_eq!(season_from_title("", "从零开始 第四季 丧失篇"), Some(4));
        assert_eq!(season_from_title("", "进击的巨人 第2期"), Some(2));
        assert_eq!(season_from_title("", "某番 二期"), Some(2));
        // No marker → None (a first season).
        assert_eq!(season_from_title("Re:Zero", "从零开始"), None);
    }

    #[test]
    fn base_match_score_rewards_containment() {
        // The longer season title embeds the base keyword, so it must clear a
        // typical acceptance threshold despite the extra characters.
        let score = base_match_score(
            "Re:ゼロから始める異世界生活",
            "Re:ゼロから始める異世界生活 4th season 喪失編 TV",
            "Re：从零开始的异世界生活 第四季 丧失篇",
        );
        assert!(score >= 0.9, "containment should score high, got {score}");
    }

    #[tokio::test]
    async fn resolve_subject_picks_season_directly_by_title() {
        // Re:Zero S4 case: the season subject is in the search results and its
        // title states "4th season", so it is selected without walking the
        // sequel chain (which would fail on continuous episode numbering).
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 2,
                    "data": [
                        { "id": 100, "name": "Re:ゼロから始める異世界生活",
                          "name_cn": "Re：从零开始的异世界生活" },
                        { "id": 547888,
                          "name": "Re:ゼロから始める異世界生活 4th season 喪失編 TV",
                          "name_cn": "Re：从零开始的异世界生活 第四季 丧失篇" }
                    ],
                }),
            ))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let id = api
            .resolve_subject_id(
                &["Re:ゼロから始める異世界生活", "Re：从零开始的异世界生活"],
                4,
                0.6,
                None,
            )
            .await
            .unwrap();
        assert_eq!(id, Some(547888));
    }

    // ── date helpers ──────────────────────────────────────────────────────────

    #[test]
    fn date_diff_handles_iso_prefix_and_gap() {
        // Same day, one with a time component.
        assert_eq!(
            date_diff_days("2024-10-09T00:00:00Z", "2024-10-09"),
            Some(0)
        );
        // One week apart.
        assert_eq!(date_diff_days("2024-10-16", "2024-10-09"), Some(7));
        // Across a month boundary.
        assert_eq!(date_diff_days("2024-11-01", "2024-10-30"), Some(2));
        // Malformed input yields None.
        assert_eq!(date_diff_days("not-a-date", "2024-10-09"), None);
    }

    // ── pick_episode_id ───────────────────────────────────────────────────────

    fn ep(id: u64, sort: f64, date: Option<&str>) -> BangumiEpisode {
        BangumiEpisode {
            id,
            sort,
            ep: sort,
            date: date.map(str::to_owned),
        }
    }

    #[test]
    fn pick_episode_prefers_air_date_over_sort() {
        // A recap special has shifted the sort numbers: the episode that aired
        // on the target date is sort 3, not sort 2. Air-date wins.
        let eps = vec![
            ep(10, 1.0, Some("2024-10-02")),
            ep(11, 2.0, Some("2024-10-09")),
            ep(12, 3.0, Some("2024-10-16")),
        ];
        let id = pick_episode_id(&eps, 99, Some("2024-10-16T00:00:00Z"), 2);
        assert_eq!(id, Some(12));
    }

    #[test]
    fn pick_episode_falls_back_to_sort() {
        let eps = vec![ep(10, 1.0, None), ep(11, 2.0, None)];
        // No usable air date -> match by sort number.
        assert_eq!(pick_episode_id(&eps, 2, None, 2), Some(11));
        // Air date supplied but no episode dates -> still falls back to sort.
        assert_eq!(pick_episode_id(&eps, 1, Some("2024-10-09"), 2), Some(10));
    }

    #[test]
    fn pick_episode_returns_none_when_unmatched() {
        let eps = vec![ep(10, 1.0, Some("2020-01-01"))];
        assert_eq!(pick_episode_id(&eps, 5, Some("2024-10-09"), 2), None);
    }

    // ── resolve_subject_id ────────────────────────────────────────────────────

    #[tokio::test]
    async fn resolve_subject_returns_root_for_season_one() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 1,
                    "data": [{ "id": 100, "name": "Show", "name_cn": "节目" }],
                }),
            ))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let id = api
            .resolve_subject_id(&["Show"], 1, 0.5, None)
            .await
            .unwrap();
        assert_eq!(id, Some(100));
    }

    #[tokio::test]
    async fn resolve_subject_walks_sequel_chain_to_target_season() {
        let server = MockServer::start().await;
        // Title search finds the franchise root (season 1).
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 1,
                    "data": [{
                        "id": 100, "name": "Re:Zero", "name_cn": "从零开始",
                    }],
                }),
            ))
            .mount(&server)
            .await;
        // Root's sequel is subject 200.
        Mock::given(method("GET"))
            .and(path("/subjects/100/subjects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([
                    { "id": 200, "relation": "续集",
                      "name": "Re:Zero 2", "name_cn": "从零开始 第二季" }
                ]),
            ))
            .mount(&server)
            .await;
        // Subject 200 is a full season (>3 eps, first sort 1).
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 12,
                    "data": [{ "id": 2001, "sort": 1.0, "ep": 1.0,
                               "airdate": "2020-01-01" }],
                }),
            ))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let id = api
            .resolve_subject_id(&["Re:Zero"], 2, 0.5, None)
            .await
            .unwrap();
        assert_eq!(id, Some(200));
    }

    #[tokio::test]
    async fn resolve_subject_rejects_low_score_candidate() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 1,
                    "data": [{ "id": 100, "name": "Totally Different",
                               "name_cn": "完全不同的作品" }],
                }),
            ))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let id = api
            .resolve_subject_id(&["Re:Zero"], 1, 0.9, None)
            .await
            .unwrap();
        assert_eq!(id, None);
    }

    // ── sync_episodes (air-date aware) ────────────────────────────────────────

    #[tokio::test]
    async fn sync_episodes_marks_by_air_date() {
        let server = MockServer::start().await;
        // Not yet collected -> POST add, then GET episodes, then PUT mark.
        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/300"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/users/-/collections/300"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 3,
                    "data": [
                        { "id": 3001, "sort": 1.0, "ep": 1.0,
                          "airdate": "2024-10-02" },
                        { "id": 3002, "sort": 2.0, "ep": 2.0,
                          "airdate": "2024-10-09" },
                    ],
                }),
            ))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/users/-/collections/-/episodes/3002"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let eps = vec![(99u32, Some("2024-10-09T00:00:00Z".to_owned()))];
        let marked = sync_episodes(&api, 300, &eps).await.unwrap();
        assert_eq!(marked, vec![3002]);
    }

    // ── SubjectCache ──────────────────────────────────────────────────────────

    #[test]
    fn subject_cache_roundtrips_through_disk() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path();

        let mut cache = SubjectCache::load(path);
        assert_eq!(cache.get("series-1", 4, 1), None);

        cache.insert("series-1", 4, 1, 12345, path).unwrap();
        cache.insert("series-1", 1, 5, 678, path).unwrap();

        // A fresh load sees the persisted entries.
        let reloaded = SubjectCache::load(path);
        assert_eq!(reloaded.get("series-1", 4, 1), Some(12345));
        assert_eq!(reloaded.get("series-1", 1, 5), Some(678));
        // Different episode → different cache entry.
        assert_eq!(reloaded.get("series-1", 4, 2), None);
        assert_eq!(reloaded.get("series-1", 2, 1), None);
    }

    #[test]
    fn subject_cache_load_missing_file_is_empty() {
        let path = std::path::Path::new("/nonexistent/etlp/bgm_cache.json");
        let cache = SubjectCache::load(path);
        assert_eq!(cache.get("x", 1, 1), None);
    }

    // ── T4: web-scrape resolution algorithm ───────────────────────────────────

    fn web_search_html(candidates: &[(u64, &str, Option<&str>)]) -> String {
        let items: String = candidates
            .iter()
            .map(|(id, name, jp)| {
                let jp_html = jp
                    .map(|j| format!(r#"<small class="grey">{j}</small>"#))
                    .unwrap_or_default();
                format!(
                    r#"<li class="item anime"><a class="l" href="/subject/{id}">{name}</a>{jp_html}</li>"#
                )
            })
            .collect();
        format!("<ul>{items}</ul>")
    }

    fn subject_main_html(start_date_jp: Option<&str>) -> String {
        match start_date_jp {
            Some(d) => format!(
                r#"<ul><li><span class="tip">放送开始: </span>{d}</li></ul>"#
            ),
            None => "<html></html>".to_owned(),
        }
    }

    fn ep_page_html(episodes: &[(u32, &str)]) -> String {
        let items: String = episodes
            .iter()
            .map(|(num, title)| {
                format!(
                    r#"<a class="load-epinfo" title="ep.{num} {title}">{num}</a>"#
                )
            })
            .collect();
        format!("<html>{items}</html>")
    }

    async fn mount_search(
        server: &MockServer,
        path_seg: &str,
        candidates: Vec<(u64, &str, Option<&str>)>,
    ) {
        use wiremock::matchers::query_param;
        Mock::given(method("GET"))
            .and(path(path_seg))
            .and(query_param("cat", "2"))
            .and(query_param("page", "1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(web_search_html(&candidates)),
            )
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path(path_seg))
            .and(query_param("cat", "2"))
            .and(query_param("page", "2"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string("<html></html>"),
            )
            .mount(server)
            .await;
    }

    async fn mount_detail(
        server: &MockServer,
        id: u64,
        start_jp: Option<&str>,
        eps: Vec<(u32, &str)>,
    ) {
        use wiremock::matchers::query_param;
        Mock::given(method("GET"))
            .and(path(format!("/subject/{id}")))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(subject_main_html(start_jp)),
            )
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/subject/{id}/ep")))
            .and(query_param("page", "1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(ep_page_html(&eps)),
            )
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/subject/{id}/ep")))
            .and(query_param("page", "2"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string("<html></html>"),
            )
            .mount(server)
            .await;
    }

    fn http() -> reqwest::Client {
        reqwest::Client::new()
    }

    // ── date_window tests (pure) ──────────────────────────────────────────────

    #[test]
    fn date_window_accepts_episode_on_start_date() {
        assert!(date_window_ok(Some("2024-04-01"), Some("2024-04-01")));
    }

    #[test]
    fn date_window_accepts_episode_5_days_before_start() {
        assert!(date_window_ok(Some("2024-03-27"), Some("2024-04-01")));
    }

    #[test]
    fn date_window_rejects_episode_6_days_before_start() {
        assert!(!date_window_ok(Some("2024-03-26"), Some("2024-04-01")));
    }

    #[test]
    fn date_window_skips_check_when_dates_unavailable() {
        assert!(date_window_ok(None, None));
        assert!(date_window_ok(Some("2024-01-01"), None));
        assert!(date_window_ok(None, Some("2024-01-01")));
    }

    // ── Episode Round 1 ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn resolve_episode_round1_exact_title_in_range_selected() {
        let server = MockServer::start().await;
        mount_search(
            &server,
            "/subject_search/AnimeA",
            vec![(100, "AnimeA", None)],
        )
        .await;
        mount_detail(
            &server,
            100,
            None,
            vec![(1, "Ep1"), (2, "Ep2"), (3, "Ep3")],
        )
        .await;

        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeA"];
        let req = WebScrapeReq {
            series: "AnimeA",
            season: 1,
            keywords: kws,
            alt_titles: &[],
            target: WebResolveTarget::Episode {
                episode: 2,
                premiere_date: None,
                episode_title: None,
            },
        };
        let id =
            resolve_by_web_scrape(&http(), &server.uri(), &req, &mut cache)
                .await;
        assert_eq!(id, Some(100));
    }

    #[tokio::test]
    async fn resolve_episode_round1_exact_title_out_of_range_skipped() {
        let server = MockServer::start().await;
        mount_search(
            &server,
            "/subject_search/AnimeA",
            vec![(100, "AnimeA", None)],
        )
        .await;
        mount_detail(
            &server,
            100,
            None,
            vec![(1, "Ep1"), (2, "Ep2"), (3, "Ep3")],
        )
        .await;

        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeA"];
        let req = WebScrapeReq {
            series: "AnimeA",
            season: 1,
            keywords: kws,
            alt_titles: &[],
            target: WebResolveTarget::Episode {
                episode: 5,
                premiere_date: None,
                episode_title: None,
            },
        };
        let id =
            resolve_by_web_scrape(&http(), &server.uri(), &req, &mut cache)
                .await;
        assert_eq!(id, None);
    }

    #[tokio::test]
    async fn resolve_episode_round1_multiple_exact_matches_falls_to_round2() {
        // Two subjects both named "AnimeA" both covering episode 7.
        // Round 1 yields 2 matches → falls to Round 2.
        // Subject 100 has ep 7 titled "EpSeven"; episode_title="EpSeven" → higher score.
        let server = MockServer::start().await;
        mount_search(
            &server,
            "/subject_search/AnimeA",
            vec![(100, "AnimeA", None), (200, "AnimeA", None)],
        )
        .await;
        mount_detail(
            &server,
            100,
            None,
            vec![(1, ""), (7, "EpSeven"), (10, "")],
        )
        .await;
        mount_detail(
            &server,
            200,
            None,
            vec![(5, ""), (7, "Different"), (15, "")],
        )
        .await;

        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeA"];
        let req = WebScrapeReq {
            series: "AnimeA",
            season: 1,
            keywords: kws,
            alt_titles: &[],
            target: WebResolveTarget::Episode {
                episode: 7,
                premiere_date: None,
                episode_title: Some("EpSeven"),
            },
        };
        let id =
            resolve_by_web_scrape(&http(), &server.uri(), &req, &mut cache)
                .await;
        assert_eq!(id, Some(100));
    }

    #[tokio::test]
    async fn resolve_episode_round1_date_outside_window_skipped() {
        // Premiere is 30 days before start_date: outside the 5-day window.
        let server = MockServer::start().await;
        mount_search(
            &server,
            "/subject_search/AnimeA",
            vec![(100, "AnimeA", None)],
        )
        .await;
        // start date = 2024-04-01 in Japanese kanji
        mount_detail(
            &server,
            100,
            Some("2024年4月1日"),
            vec![(1, ""), (2, ""), (3, "")],
        )
        .await;

        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeA"];
        let req = WebScrapeReq {
            series: "AnimeA",
            season: 1,
            keywords: kws,
            alt_titles: &[],
            target: WebResolveTarget::Episode {
                episode: 2,
                premiere_date: Some("2024-03-01"), // 31 days before 2024-04-01
                episode_title: None,
            },
        };
        let id =
            resolve_by_web_scrape(&http(), &server.uri(), &req, &mut cache)
                .await;
        assert_eq!(id, None);
    }

    // ── Episode Round 2 ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn resolve_episode_round2_picks_highest_combined_score() {
        // AnimeABC covers ep 1-10 (higher combined score),
        // AnimeDEF covers ep 1-10 (lower combined score).
        // keyword = "AnimeABC" → subject 100 wins.
        let server = MockServer::start().await;
        mount_search(
            &server,
            "/subject_search/AnimeABC",
            vec![(100, "AnimeABC", None), (200, "AnimeDEF", None)],
        )
        .await;
        mount_detail(
            &server,
            100,
            None,
            (1..=10).map(|n| (n, "")).collect::<Vec<_>>(),
        )
        .await;
        mount_detail(
            &server,
            200,
            None,
            (1..=10).map(|n| (n, "")).collect::<Vec<_>>(),
        )
        .await;

        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeABC"];
        let req = WebScrapeReq {
            series: "AnimeABC",
            season: 1,
            keywords: kws,
            alt_titles: &[],
            target: WebResolveTarget::Episode {
                episode: 5,
                premiere_date: None,
                episode_title: None,
            },
        };
        let id =
            resolve_by_web_scrape(&http(), &server.uri(), &req, &mut cache)
                .await;
        assert_eq!(id, Some(100));
    }

    #[tokio::test]
    async fn resolve_episode_round2_returns_none_on_tied_scores() {
        // Both subjects have the same name → same score → tied → None.
        let server = MockServer::start().await;
        mount_search(
            &server,
            "/subject_search/AnimeA",
            vec![(100, "AnimeA", None), (200, "AnimeA", None)],
        )
        .await;
        mount_detail(&server, 100, None, vec![(1, ""), (2, "")]).await;
        mount_detail(&server, 200, None, vec![(1, ""), (2, "")]).await;

        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeA"];
        let req = WebScrapeReq {
            series: "AnimeA",
            season: 1,
            keywords: kws,
            alt_titles: &[],
            target: WebResolveTarget::Episode {
                episode: 1,
                premiere_date: None,
                episode_title: None,
            },
        };
        let id =
            resolve_by_web_scrape(&http(), &server.uri(), &req, &mut cache)
                .await;
        assert_eq!(id, None);
    }

    #[tokio::test]
    async fn resolve_episode_round2_returns_none_below_threshold() {
        // Subject "ZZZZZ" has near-zero similarity to keyword "AnimeA".
        let server = MockServer::start().await;
        mount_search(
            &server,
            "/subject_search/AnimeA",
            vec![(100, "ZZZZZ", None)],
        )
        .await;
        mount_detail(
            &server,
            100,
            None,
            (1..=5).map(|n| (n, "")).collect::<Vec<_>>(),
        )
        .await;

        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeA"];
        let req = WebScrapeReq {
            series: "AnimeA",
            season: 1,
            keywords: kws,
            alt_titles: &[],
            target: WebResolveTarget::Episode {
                episode: 3,
                premiere_date: None,
                episode_title: None,
            },
        };
        let id =
            resolve_by_web_scrape(&http(), &server.uri(), &req, &mut cache)
                .await;
        assert_eq!(id, None);
    }

    #[tokio::test]
    async fn resolve_episode_round2_degrades_gracefully_without_episode_title()
    {
        // No episode_title → score = subj_title_score only → still picks best.
        let server = MockServer::start().await;
        mount_search(
            &server,
            "/subject_search/AnimeABC",
            vec![(100, "AnimeABC", None), (200, "AnimeXYZ", None)],
        )
        .await;
        mount_detail(
            &server,
            100,
            None,
            (1..=5).map(|n| (n, "")).collect::<Vec<_>>(),
        )
        .await;
        mount_detail(
            &server,
            200,
            None,
            (1..=5).map(|n| (n, "")).collect::<Vec<_>>(),
        )
        .await;

        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeABC"];
        let req = WebScrapeReq {
            series: "AnimeABC",
            season: 1,
            keywords: kws,
            alt_titles: &[],
            target: WebResolveTarget::Episode {
                episode: 3,
                premiere_date: None,
                episode_title: None,
            },
        };
        let id =
            resolve_by_web_scrape(&http(), &server.uri(), &req, &mut cache)
                .await;
        assert_eq!(id, Some(100));
    }

    // ── Movie tests ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn resolve_movie_round1_exact_match_selected() {
        let server = MockServer::start().await;
        mount_search(
            &server,
            "/subject_search/MovieA",
            vec![(300, "MovieA", None)],
        )
        .await;
        mount_detail(&server, 300, Some("2024年6月1日"), vec![]).await;

        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["MovieA"];
        let req = WebScrapeReq {
            series: "MovieA",
            season: 1,
            keywords: kws,
            alt_titles: &[],
            target: WebResolveTarget::Movie {
                premiere_date: Some("2024-07-01"),
            },
        };
        let id =
            resolve_by_web_scrape(&http(), &server.uri(), &req, &mut cache)
                .await;
        assert_eq!(id, Some(300));
    }

    #[tokio::test]
    async fn resolve_movie_round2_picks_highest_levenshtein() {
        let server = MockServer::start().await;
        mount_search(
            &server,
            "/subject_search/MovieABC",
            vec![(300, "MovieABC", None), (400, "MovieXYZ", None)],
        )
        .await;
        mount_detail(&server, 300, None, vec![]).await;
        mount_detail(&server, 400, None, vec![]).await;

        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["MovieABC"];
        let req = WebScrapeReq {
            series: "MovieABC",
            season: 1,
            keywords: kws,
            alt_titles: &[],
            target: WebResolveTarget::Movie {
                premiere_date: None,
            },
        };
        let id =
            resolve_by_web_scrape(&http(), &server.uri(), &req, &mut cache)
                .await;
        assert_eq!(id, Some(300));
    }

    #[tokio::test]
    async fn resolve_movie_round2_returns_none_below_threshold() {
        let server = MockServer::start().await;
        mount_search(
            &server,
            "/subject_search/MovieABC",
            vec![(300, "ZZZZZZZ", None)],
        )
        .await;
        mount_detail(&server, 300, None, vec![]).await;

        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["MovieABC"];
        let req = WebScrapeReq {
            series: "MovieABC",
            season: 1,
            keywords: kws,
            alt_titles: &[],
            target: WebResolveTarget::Movie {
                premiere_date: None,
            },
        };
        let id =
            resolve_by_web_scrape(&http(), &server.uri(), &req, &mut cache)
                .await;
        assert_eq!(id, None);
    }

    // ── Concrete scenario tests ───────────────────────────────────────────────

    #[tokio::test]
    async fn scenario_shixiong_s1e127_selects_nianfan2_subject() {
        // 师兄啊师兄 S1E127:
        // subject A  (id=501): "师兄啊师兄",       ep_range 1..91  → E127 not in range
        // subject B  (id=502): "师兄啊师兄 年番2",  ep_range 92..143 → E127 in range, wins
        let server = MockServer::start().await;
        let kw = "师兄啊师兄";
        let encoded = super::percent_encode_path(kw);
        let search_path = format!("/subject_search/{encoded}");

        mount_search(
            &server,
            &search_path,
            vec![(501, "师兄啊师兄", None), (502, "师兄啊师兄 年番2", None)],
        )
        .await;
        mount_detail(
            &server,
            501,
            None,
            (1u32..=91).map(|n| (n, "")).collect::<Vec<_>>(),
        )
        .await;
        mount_detail(
            &server,
            502,
            None,
            (92u32..=143).map(|n| (n, "")).collect::<Vec<_>>(),
        )
        .await;

        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &[kw];
        let req = WebScrapeReq {
            series: kw,
            season: 1,
            keywords: kws,
            alt_titles: &[],
            target: WebResolveTarget::Episode {
                episode: 127,
                premiere_date: None,
                episode_title: None,
            },
        };
        let id =
            resolve_by_web_scrape(&http(), &server.uri(), &req, &mut cache)
                .await;
        assert_eq!(id, Some(502));
    }

    #[tokio::test]
    async fn scenario_doupo_s5e135_selects_zhongzhou_subject() {
        // 斗破苍穹 S5E135:
        // subject 601: "斗破苍穹 中州风云志", ep_range 106..157 → contains 135 → wins
        let server = MockServer::start().await;
        let kw = "斗破苍穹";
        let encoded = super::percent_encode_path(kw);
        let search_path = format!("/subject_search/{encoded}");

        mount_search(
            &server,
            &search_path,
            vec![(601, "斗破苍穹 中州风云志", None)],
        )
        .await;
        mount_detail(
            &server,
            601,
            None,
            (106u32..=157).map(|n| (n, "")).collect::<Vec<_>>(),
        )
        .await;

        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &[kw];
        let req = WebScrapeReq {
            series: kw,
            season: 5,
            keywords: kws,
            alt_titles: &[],
            target: WebResolveTarget::Episode {
                episode: 135,
                premiere_date: None,
                episode_title: None,
            },
        };
        let id =
            resolve_by_web_scrape(&http(), &server.uri(), &req, &mut cache)
                .await;
        assert_eq!(id, Some(601));
    }
}
