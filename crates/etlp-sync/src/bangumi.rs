//! Bangumi (bgm.tv) API client and watch-progress sync.
//!
//! Authentication is via a personal Bearer token (no OAuth flow required).
//! The primary sync entry-point is [`sync_episode_by_bangumi_id`], which
//! accepts a Bangumi subject ID taken directly from an Emby item's
//! `ProviderIds.Bangumi` field and marks the specified episodes as watched.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::{Result, SyncError};

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

    /// Verify the access token by calling `GET /me`.
    ///
    /// Returns `Ok(())` when the token is accepted. A `401`/`403` response maps
    /// to [`SyncError::Unauthorized`] so callers can prompt the user to
    /// regenerate the token.
    pub async fn verify_token(&self) -> Result<()> {
        debug!(user = %self.username, "bangumi: GET /me (verify token)");
        let resp = crate::curl::send_logged(
            self.http.get(self.url("me")).headers(self.auth_headers()),
        )
        .await?;
        let status = resp.status().as_u16();
        debug!(status, "bangumi: /me response");
        if resp.status().is_success() {
            return Ok(());
        }
        if status == 401 || status == 403 {
            return Err(SyncError::Unauthorized);
        }
        let body = resp.text().await.unwrap_or_default();
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

    /// Search for anime subjects by keyword, optionally bounded by air date.
    ///
    /// When both `start_date` and `end_date` (`YYYY-MM-DD`) are supplied the
    /// search is narrowed to that window; pass `None` for both to search by
    /// keyword alone. The latter is needed for season resolution, where the
    /// only date in hand is an episode's air date — too far from the franchise
    /// root's premiere to use as a filter.
    pub async fn search_subjects(
        &self,
        keyword: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
        limit: u32,
    ) -> Result<Vec<BangumiSearchSubject>> {
        let mut filter = serde_json::json!({
            "type": [2],
            "nsfw": true,
        });
        if let (Some(start), Some(end), Some(map)) =
            (start_date, end_date, filter.as_object_mut())
        {
            let _ = map.insert(
                "air_date".to_owned(),
                serde_json::json!([format!(">={start}"), format!("<{end}")]),
            );
        }
        let body = serde_json::json!({
            "keyword": keyword,
            "filter": filter,
        });
        let resp = crate::curl::send_logged(
            self.http
                .post(self.url("search/subjects"))
                .headers(self.auth_headers())
                .query(&[("limit", limit)])
                .json(&body),
        )
        .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(SyncError::Api { status, body });
        }
        let raw: serde_json::Value = resp.json().await?;
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
            self.http
                .get(self.url(&format!("subjects/{subject_id}")))
                .headers(self.auth_headers()),
        )
        .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(SyncError::Api { status, body });
        }
        Ok(resp.json().await?)
    }

    /// Fetch all episodes for a subject (type 0 = main episodes).
    pub async fn get_episodes(
        &self,
        subject_id: u64,
    ) -> Result<BangumiEpisodeList> {
        debug!(subject_id, "bangumi: GET /episodes");
        let resp = crate::curl::send_logged(
            self.http
                .get(self.url("episodes"))
                .headers(self.auth_headers())
                .query(&[("subject_id", subject_id), ("type", 0)]),
        )
        .await?;
        debug!(
            subject_id,
            status = resp.status().as_u16(),
            "bangumi: /episodes response"
        );
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(SyncError::Api { status, body });
        }
        Ok(resp.json().await?)
    }

    /// Fetch subjects related to `subject_id` (e.g. sequels `続集`).
    pub async fn get_related_subjects(
        &self,
        subject_id: u64,
    ) -> Result<Vec<BangumiRelated>> {
        let resp = crate::curl::send_logged(
            self.http
                .get(self.url(&format!("subjects/{subject_id}/subjects")))
                .headers(self.auth_headers()),
        )
        .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(SyncError::Api { status, body });
        }
        Ok(resp.json().await?)
    }

    // ── Subject resolution by title ───────────────────────────────────────────

    /// Resolve a Bangumi subject ID for a season using title search.
    ///
    /// Used when the media item lacks a `ProviderIds.Bangumi`. Tries each
    /// keyword in order (native title first, then series name), scores every
    /// candidate against all keywords, and walks the `续集` (sequel) chain from
    /// the best-scoring franchise root to the subject for `season`. Returns
    /// `None` when no candidate clears `min_score` or the season is unreachable.
    pub async fn resolve_subject_id(
        &self,
        keywords: &[&str],
        season: u32,
        min_score: f64,
    ) -> Result<Option<u64>> {
        // Gather candidates from every keyword, de-duplicated by subject id.
        let mut candidates: Vec<BangumiSearchSubject> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for keyword in keywords.iter().filter(|k| !k.trim().is_empty()) {
            for candidate in
                self.search_subjects(keyword, None, None, 10).await?
            {
                if seen.insert(candidate.id) {
                    candidates.push(candidate);
                }
            }
        }
        if candidates.is_empty() {
            debug!("bangumi: title search returned no candidates");
            return Ok(None);
        }

        let score_of = |c: &BangumiSearchSubject| -> f64 {
            keywords
                .iter()
                .filter(|k| !k.trim().is_empty())
                .map(|k| base_match_score(k, &c.name, &c.name_cn))
                .fold(0.0_f64, f64::max)
        };

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
        if season <= 1 {
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
            self.http
                .get(self.url(&format!(
                    "users/{}/collections/{}",
                    self.username, subject_id
                )))
                .headers(self.auth_headers()),
        )
        .await?;
        debug!(
            subject_id,
            status = resp.status().as_u16(),
            "bangumi: subject collection response"
        );
        match resp.status().as_u16() {
            404 => Ok(None),
            200 => Ok(Some(resp.json().await?)),
            status => {
                let body = resp.text().await.unwrap_or_default();
                Err(SyncError::Api { status, body })
            }
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
            self.http
                .post(self.url(&format!("users/-/collections/{subject_id}")))
                .headers(self.auth_headers())
                .json(&body),
        )
        .await?;
        debug!(
            subject_id,
            status = resp.status().as_u16(),
            "bangumi: add/update collection response"
        );
        // 200/202/204 all count as success.
        if resp.status().is_success() || resp.status().as_u16() == 204 {
            return Ok(());
        }
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        Err(SyncError::Api { status, body })
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
                self.http
                    .put(self.url(&format!(
                        "users/-/collections/-/episodes/{ep_id}"
                    )))
                    .headers(self.auth_headers())
                    .json(&body),
            )
            .await?;
            if resp.status().is_success() || resp.status().as_u16() == 204 {
                return Ok(());
            }
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(SyncError::Api { status, body: text });
        }

        // Bulk update.
        let body = serde_json::json!({
            "episode_id": ep_ids,
            "type":        CollectionState::Watched as u8,
        });
        let resp =
            crate::curl::send_logged(
                self.http
                    .patch(self.url(&format!(
                        "users/-/collections/{subject_id}/episodes"
                    )))
                    .headers(self.auth_headers())
                    .json(&body),
            )
            .await?;
        if resp.status().is_success() || resp.status().as_u16() == 204 {
            return Ok(());
        }
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        Err(SyncError::Api { status, body: text })
    }

    /// Return a map of `sort_number → EpCollectionState` for a subject.
    pub async fn get_user_eps_collection(
        &self,
        subject_id: u64,
    ) -> Result<HashMap<u64, EpCollectionState>> {
        let resp =
            crate::curl::send_logged(
                self.http
                    .get(self.url(&format!(
                        "users/-/collections/{subject_id}/episodes"
                    )))
                    .headers(self.auth_headers()),
            )
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(SyncError::Api { status, body });
        }
        let raw: serde_json::Value = resp.json().await?;
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

// ── Subject ID cache ────────────────────────────────────────────────────────────

/// On-disk cache mapping `series_id:season` to a resolved Bangumi subject ID.
///
/// Title search is fuzzy and costs several requests, so a resolved mapping is
/// persisted and reused for the remaining episodes of the same season. The
/// cache is advisory: any read/parse failure degrades to an empty cache rather
/// than aborting the sync.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SubjectCache {
    #[serde(flatten)]
    map: std::collections::BTreeMap<String, u64>,
}

impl SubjectCache {
    /// Build the cache key for a series/season pair.
    fn key(series_id: &str, season: i64) -> String {
        format!("{series_id}:{season}")
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

    /// Look up a cached subject ID for a series/season pair.
    #[must_use]
    pub fn get(&self, series_id: &str, season: i64) -> Option<u64> {
        self.map.get(&Self::key(series_id, season)).copied()
    }

    /// Insert a mapping and persist the whole cache to `path`.
    pub fn insert(
        &mut self,
        series_id: &str,
        season: i64,
        subject_id: u64,
        path: &Path,
    ) -> Result<()> {
        let _ = self.map.insert(Self::key(series_id, season), subject_id);
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
        let results = api
            .search_subjects(
                "テスト",
                Some("2024-03-01"),
                Some("2024-05-01"),
                5,
            )
            .await
            .unwrap();
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
        let results =
            api.search_subjects("missing", None, None, 5).await.unwrap();
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
        let id = api.resolve_subject_id(&["Show"], 1, 0.5).await.unwrap();
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
        let id = api.resolve_subject_id(&["Re:Zero"], 2, 0.5).await.unwrap();
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
        let id = api.resolve_subject_id(&["Re:Zero"], 1, 0.9).await.unwrap();
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
        assert_eq!(cache.get("series-1", 4), None);

        cache.insert("series-1", 4, 12345, path).unwrap();
        cache.insert("series-1", 1, 678, path).unwrap();

        // A fresh load sees the persisted entries.
        let reloaded = SubjectCache::load(path);
        assert_eq!(reloaded.get("series-1", 4), Some(12345));
        assert_eq!(reloaded.get("series-1", 1), Some(678));
        assert_eq!(reloaded.get("series-1", 2), None);
    }

    #[test]
    fn subject_cache_load_missing_file_is_empty() {
        let path = std::path::Path::new("/nonexistent/etlp/bgm_cache.json");
        let cache = SubjectCache::load(path);
        assert_eq!(cache.get("x", 1), None);
    }
}
