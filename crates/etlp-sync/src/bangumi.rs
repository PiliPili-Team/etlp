//! Bangumi (bgm.tv) API client and watch-progress sync.
//!
//! Authentication is via a personal Bearer token (no OAuth flow required).
//! Resolution order for a sync entry:
//!
//! 1. **User mapping** (`subject_map` in config) – explicit `tmdb/tvdb/imdb`
//!    → `bangumi_subject_id` mapping; carries an episode offset for arc splits.
//! 2. **Title search** (gated behind `title_search_fallback` and genre check):
//!    a. Require a TMDB id; warn and bail if absent.
//!    b. Resolve the season-premiere date (S×E1 air date) via TMDB (LRU cached).
//!    c. Search the BGM JSON API with the series name + `series_premiere − 2 days`.
//!    d. Walk the prequel/sequel relation chain to expand the candidate pool (BFS).
//!    e. Pick the subject with an episode within ±2 days of target air date.
//!    f. Match the specific episode by `airdate` via `GET /v0/episodes`.
//!
//! `provider_ids["Bangumi"]` is intentionally ignored: users frequently
//! fill it with incorrect values, making it an unreliable signal.
//!
//! ## Read-cache design
//!
//! A process-global Moka cache (`BGM_READ_CACHE`) protects all read-only GET
//! endpoints against concurrent webhook bursts (e.g. batch-marking 12 episodes
//! triggers 12 simultaneous syncs). User-private `/users/` endpoints are
//! explicitly excluded so stale state never silences a legitimate re-mark.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use moka::future::Cache as MokaCache;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::{Result, SyncError};

/// Log label for this provider's HTTP send/retry/response lines.
const DOMAIN: &str = "bangumi";

// ── BGM read-only API path constants ─────────────────────────────────────────

const PATH_SUBJECTS: &str = "subjects";
const PATH_EPISODES: &str = "episodes";
const PATH_SEARCH_SUBJECTS: &str = "search/subjects";
const PATH_ME: &str = "me";

// ── Shared read cache ─────────────────────────────────────────────────────────

/// Capacity of the in-process BGM read cache (number of distinct keys).
const BGM_CACHE_CAPACITY: u64 = 2048;

/// Minimum title score for an aired candidate without rank/rating to survive
/// API pre-screening.
const UNRANKED_AIRED_TITLE_SIMILARITY: f64 = 0.9;

/// Minimum title score required after the wider fuzzy premiere-date retry.
const FUZZY_RETRY_TITLE_MIN_SCORE: f64 = 0.9;

/// Raw JSON cache for BGM read-only API responses.
///
/// Keyed by an opaque string (`"search:{kw}:{from}"`, `"subject:{id}"`, …).
/// Values are the raw JSON bytes already proven decodable. User-private
/// endpoints (anything containing `/users/`) must never be cached here.
pub type BgmReadCache = Arc<MokaCache<Arc<str>, serde_json::Value>>;

/// Create a new BGM read cache with the default capacity and TTL.
///
/// In production, construct once (e.g. at server startup) and clone the
/// `Arc` into each `BangumiApi`. In tests, create a fresh cache per test so
/// test runs do not interfere with each other.
pub fn new_bgm_read_cache() -> BgmReadCache {
    Arc::new(
        MokaCache::builder()
            .max_capacity(BGM_CACHE_CAPACITY)
            // 10-minute TTL: long enough to absorb bulk-mark bursts, short
            // enough to pick up BGM wiki edits within a reasonable window.
            .time_to_live(std::time::Duration::from_secs(600))
            .build(),
    )
}

// ── Domain types ──────────────────────────────────────────────────────────────

/// User's relationship to a subject (collection state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CollectionState {
    /// Plan to watch.
    Wish = 1,
    /// Finished watching.
    Watched = 2,
    /// Currently watching.
    Watching = 3,
    /// Paused / on hold.
    OnHold = 4,
    /// Abandoned.
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
    #[serde(default)]
    pub id: u64,
    /// Sort index (float because some specials use 0.5, etc.).
    pub sort: f64,
    /// Episode number within the season (0 = SP).
    #[serde(default)]
    pub ep: f64,
    /// Air date, may be absent for unreleased episodes.
    #[serde(alias = "airdate")]
    pub date: Option<String>,
    /// Japanese/original episode name.
    #[serde(default)]
    pub name: String,
    /// Localised (Chinese) episode name.
    #[serde(default)]
    pub name_cn: String,
}

/// Container returned by `GET /episodes`.
#[derive(Debug, Clone, Deserialize)]
pub struct BangumiEpisodeList {
    pub total: u64,
    pub data: Vec<BangumiEpisode>,
}

/// A related-subject entry (sequel, prequel, etc.).
#[derive(Debug, Clone, Deserialize)]
pub struct BangumiRelated {
    pub id: u64,
    pub relation: String,
    pub name: String,
    pub name_cn: String,
}

/// Episode collection state for a single episode.
#[derive(Debug, Clone)]
pub struct EpCollectionState {
    pub ep_id: u64,
    /// `true` if the episode is marked as watched (type == 2).
    pub watched: bool,
    pub airdate: Option<String>,
}

// ── Title & date helpers ──────────────────────────────────────────────────────

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

/// Convert a day index back to `"YYYY-MM-DD"`.
///
/// Inverse of `date_to_days` (civil-from-days, Howard Hinnant).
fn days_to_date(z: i64) -> Option<String> {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y0 = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y0 + 1 } else { y0 };
    if !(1..=9999).contains(&y)
        || !(1..=12).contains(&m)
        || !(1..=31).contains(&d)
    {
        return None;
    }
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// Subtract `days` from a `"YYYY-MM-DD"` string and return the new date.
pub(crate) fn date_subtract_days(date_str: &str, days: i64) -> Option<String> {
    days_to_date(date_to_days(date_str)? - days)
}

/// Add `days` to a `"YYYY-MM-DD"` string and return the new date.
pub(crate) fn date_add_days(date_str: &str, days: i64) -> Option<String> {
    days_to_date(date_to_days(date_str)? + days)
}

/// Absolute day difference between two date strings, if both parse.
fn date_diff_days(a: &str, b: &str) -> Option<i64> {
    Some((date_to_days(a)? - date_to_days(b)?).abs())
}

/// Check if an episode's float `sort` exactly matches `target_sort` as an
/// integer. Avoids `float as u32` truncation (which would cause `6.5` to match
/// `6`).
#[inline]
pub(crate) fn is_exact_sort(sort: f64, target_sort: u32) -> bool {
    (sort - target_sort as f64).abs() < 1e-4
}

/// Identify if an episode is a recap (总集篇) or special (SP/OVA/bonus),
/// rather than a regular season main episode.
pub(crate) fn is_special_or_recap(ep: &BangumiEpisode) -> bool {
    if ep.sort.fract() != 0.0 || ep.sort <= 0.0 || ep.ep <= 0.0 {
        return true;
    }
    let is_keyword = |s: &str| {
        s.contains("总集")
            || s.contains("総集")
            || s.contains("特别篇")
            || s.contains("特別編")
            || s.contains("SP")
            || s.contains("Recap")
            || s.contains("recap")
    };
    is_keyword(&ep.name) || is_keyword(&ep.name_cn)
}

/// Resolve a target episode to a Bangumi episode ID by air-date proximity.
///
/// Steps:
/// 1. Collect every episode whose `airdate` diff from `date` is ≤ `fuzzy_days`.
/// 2. Find the minimum diff among those candidates.
/// 3. Within the minimum-diff group, prefer the episode whose `sort` equals
///    `target_sort` — this handles same-day batch releases (e.g. Netflix drops
///    or premiere weeks where episodes 1-4 all carry the same airdate).
/// 4. If no sort-match exists in the group, take the first minimum-diff entry.
/// 5. Single-Episode Drift Guard: if the candidate drifts by 1 (±1.0) and the
///    exact-sort episode exists as a regular episode within 7-day weekly
///    tolerance, prevent the drift (fixes TMDB 1-week metadata shifts).
/// 6. When no air-date candidates pass, fall back to exact `sort` lookup.
#[allow(dead_code)]
fn pick_episode_id(
    episodes: &[BangumiEpisode],
    target_sort: u32,
    air_date: Option<&str>,
    fuzzy_days: i64,
) -> Option<u64> {
    pick_episode(episodes, target_sort, air_date, fuzzy_days).map(|ep| ep.id)
}

/// Same matching logic as [`pick_episode_id`] but returns the whole episode so
/// callers that need the resolved `sort` (e.g. to check the user's watched
/// state) do not have to look it up a second time.
fn pick_episode<'a>(
    episodes: &'a [BangumiEpisode],
    target_sort: u32,
    air_date: Option<&str>,
    fuzzy_days: i64,
) -> Option<&'a BangumiEpisode> {
    if let Some(date) = air_date {
        let candidates: Vec<(i64, &BangumiEpisode)> = episodes
            .iter()
            .filter_map(|ep| {
                let diff = date_diff_days(ep.date.as_deref()?, date)?;
                (diff <= fuzzy_days).then_some((diff, ep))
            })
            .collect();

        if !candidates.is_empty() {
            let min_diff =
                candidates.iter().map(|(d, _)| *d).min().unwrap_or(i64::MAX);

            let group = candidates
                .iter()
                .filter(|(d, _)| *d == min_diff)
                .map(|(_, ep)| *ep);

            // Prefer the episode in the tie group whose sort matches the
            // target sort; fall back to the first element in the group.
            let chosen = group
                .clone()
                .find(|ep| is_exact_sort(ep.sort, target_sort))
                .or_else(|| group.clone().next());

            if let Some(ep) = chosen {
                // Tier 2: Single-Episode Drift Guard (单集跨周防漂移守护)
                // When single-episode drift step == 1.0 (i.e.
                // |ep.sort - target_sort| == 1.0), check if target_sort exists
                // in subject with date diff <= 7 days. If so, TMDB/server
                // shifted air date by 1 week (+1 or -1 week). Intercept!
                let is_step_one =
                    ((ep.sort - target_sort as f64).abs() - 1.0).abs() < 1e-4;
                if is_step_one {
                    let same_sort_ep = episodes.iter().find(|e| {
                        is_exact_sort(e.sort, target_sort)
                            && !is_special_or_recap(e)
                            && e.date
                                .as_deref()
                                .and_then(|d| date_diff_days(d, date))
                                .is_some_and(|diff| diff <= 7)
                    });
                    if let Some(same_sort_ep) = same_sort_ep {
                        let same_diff = same_sort_ep
                            .date
                            .as_deref()
                            .and_then(|d| date_diff_days(d, date))
                            .unwrap_or(0);
                        let drift_step = ep.sort - target_sort as f64;
                        debug!(
                            target_sort,
                            target_air_date = date,
                            drift_step,
                            skipped_ep_id = ep.id,
                            skipped_ep_sort = ep.sort,
                            same_sort_id = same_sort_ep.id,
                            same_sort_date =
                                same_sort_ep.date.as_deref().unwrap_or(""),
                            same_sort_diff = same_diff,
                            "bangumi: episode drift prevented — \
                             retained exact-sort episode within \
                             7-day weekly tolerance"
                        );
                        return Some(same_sort_ep);
                    }
                }

                if !is_exact_sort(ep.sort, target_sort) {
                    debug!(
                        target_sort,
                        target_air_date = date,
                        ep_id = ep.id,
                        ep_sort = ep.sort,
                        ep_air_date = ep.date.as_deref().unwrap_or(""),
                        date_diff_days = min_diff,
                        "bangumi: episode resolved by air_date \
                         (sort mismatch — nearest date used)"
                    );
                }
                return Some(ep);
            }
        } else {
            debug!(
                target_sort,
                target_air_date = date,
                fuzzy_days,
                "bangumi: no episodes within air_date window,                  fallback to sort"
            );
        }
    }
    // No air-date match: fall back to exact sort lookup.
    let ep = episodes
        .iter()
        .find(|ep| is_exact_sort(ep.sort, target_sort));
    match ep {
        Some(_) => {}
        None => {
            debug!(target_sort, "bangumi: episode not found");
        }
    }
    ep
}

/// Find the episode ID in a scraped page list whose air date is closest to
/// Normalise a title for exact comparison: trim, remove all whitespace,
/// fold fullwidth Latin punctuation (U+FF01–U+FF5E) to halfwidth ASCII,
/// and fold to lowercase. Handles both ASCII and CJK text.
///
/// The fullwidth→halfwidth step folds fullwidth punctuation (e.g. U+FF01 "!",
/// U+FF1F "?") to ASCII, so a title scraped from TMDB with fullwidth punctuation
/// compares equal to the same title scraped from bgm.tv with ASCII punctuation.
pub fn normalize_title(s: &str) -> String {
    s.trim()
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| {
            if ('\u{FF01}'..='\u{FF5E}').contains(&c) {
                char::from_u32(c as u32 - 0xFF01 + 0x0021).unwrap_or(c)
            } else {
                c
            }
        })
        .flat_map(char::to_lowercase)
        .collect()
}

// ── API client ────────────────────────────────────────────────────────────────

/// Bangumi (bgm.tv) REST API v0 client.
///
/// Constructed with a `base_url` so unit tests can point it at a mock server
/// without real network access. A `BgmReadCache` can be shared across multiple
/// `BangumiApi` instances (e.g. concurrent webhook handlers) to prevent
/// duplicate read requests. Pass `new_bgm_read_cache()` for a fresh cache.
pub struct BangumiApi {
    username: String,
    access_token: String,
    private: bool,
    base_url: String,
    http: reqwest::Client,
    /// Shared read-only JSON cache. User-private `/users/` endpoints are
    /// never stored here.
    cache: BgmReadCache,
}

impl BangumiApi {
    /// The official bgm.tv API v0 base URL.
    pub const DEFAULT_BASE_URL: &'static str = "https://api.bgm.tv/v0";

    /// Page shown to regenerate a personal access token.
    pub const TOKEN_PAGE_URL: &'static str =
        "https://next.bgm.tv/demo/access-token";

    /// Filename for the persisted `series:season:episode → subject_id` cache.
    pub const SUBJECT_CACHE_FILE: &'static str = "bangumi_subjects.json";

    /// Filename for the persisted per-season history-backfill high-water cache.
    pub const BACKFILL_CACHE_FILE: &'static str = "bangumi_backfill.json";

    /// Create a new client with a shared `BgmReadCache`.
    ///
    /// `base_url` is normally [`Self::DEFAULT_BASE_URL`]. Pass the address of a
    /// local mock server in tests. `private` controls whether new collection
    /// entries are hidden from the user's public profile. Clone the same
    /// `BgmReadCache` across concurrent API instances to share the read cache.
    pub fn new(
        username: impl Into<String>,
        access_token: impl Into<String>,
        private: bool,
        base_url: impl Into<String>,
        cache: BgmReadCache,
        proxy: crate::SyncProxy,
    ) -> Result<Self> {
        let http = crate::build_http_client(
            std::time::Duration::from_secs(30),
            &proxy,
        )
        .map_err(SyncError::Http)?;
        Ok(Self {
            username: username.into(),
            access_token: access_token.into(),
            private,
            base_url: base_url.into(),
            http,
            cache,
        })
    }

    /// Verify the access token by calling `GET /me`.
    pub async fn verify_token(&self) -> Result<()> {
        debug!(user = %self.username, "bangumi: GET /me (verify token)");
        let resp = crate::curl::send_logged(
            DOMAIN,
            self.http
                .get(self.url(PATH_ME))
                .headers(self.auth_headers()),
        )
        .await?;
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

    // ── Subject & episode info ────────────────────────────────────────────────

    /// Fetch subject metadata by ID (Moka-cached).
    pub async fn get_subject(&self, subject_id: u64) -> Result<BangumiSubject> {
        let cache_key: Arc<str> = format!("subject:{subject_id}").into();
        if let Some(cached) = self.cache.get(&cache_key).await {
            return serde_json::from_value(cached).map_err(SyncError::Json);
        }
        let resp = crate::curl::send_logged(
            DOMAIN,
            self.http
                .get(self.url(&format!("{PATH_SUBJECTS}/{subject_id}")))
                .headers(self.auth_headers()),
        )
        .await?;
        let val: serde_json::Value =
            crate::curl::json_logged(DOMAIN, resp).await?;
        self.cache.insert(cache_key, val.clone()).await;
        serde_json::from_value(val).map_err(SyncError::Json)
    }

    /// Fetch all main episodes (type=0) for a subject (Moka-cached).
    ///
    /// Paginates automatically when `total > 100` so long-running series like
    /// One Piece / Detective Conan are fetched in full.
    pub async fn get_episodes(
        &self,
        subject_id: u64,
    ) -> Result<BangumiEpisodeList> {
        let cache_key: Arc<str> = format!("episodes:{subject_id}").into();
        if let Some(cached) = self.cache.get(&cache_key).await {
            return serde_json::from_value(cached).map_err(SyncError::Json);
        }

        debug!(subject_id, "bangumi: GET /episodes");
        const LIMIT: u64 = 100;
        let mut all_data: Vec<serde_json::Value> = Vec::new();
        let mut offset = 0u64;
        let mut total = 0u64;

        loop {
            let resp = crate::curl::send_logged(
                DOMAIN,
                self.http
                    .get(self.url(PATH_EPISODES))
                    .headers(self.auth_headers())
                    .query(&[
                        ("subject_id", subject_id),
                        ("type", 0),
                        ("limit", LIMIT),
                        ("offset", offset),
                    ]),
            )
            .await?;
            let page: serde_json::Value =
                crate::curl::json_logged(DOMAIN, resp).await?;
            if total == 0 {
                total = page.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
            }
            let items = page
                .get("data")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
            let fetched = items.len() as u64;
            all_data.extend(items);
            offset += fetched;
            if fetched == 0 || offset >= total {
                break;
            }
        }

        let list_val = serde_json::json!({ "total": total, "data": all_data });
        self.cache.insert(cache_key, list_val.clone()).await;
        serde_json::from_value(list_val).map_err(SyncError::Json)
    }

    /// Fetch subjects related to `subject_id` (sequel/prequel/alternate, Moka-cached).
    pub async fn get_related_subjects(
        &self,
        subject_id: u64,
    ) -> Result<Vec<BangumiRelated>> {
        let cache_key: Arc<str> = format!("related:{subject_id}").into();
        if let Some(cached) = self.cache.get(&cache_key).await {
            return serde_json::from_value(cached).map_err(SyncError::Json);
        }
        let resp = crate::curl::send_logged(
            DOMAIN,
            self.http
                .get(
                    self.url(&format!("{PATH_SUBJECTS}/{subject_id}/subjects")),
                )
                .headers(self.auth_headers()),
        )
        .await?;
        let val: serde_json::Value =
            crate::curl::json_logged(DOMAIN, resp).await?;
        self.cache.insert(cache_key, val.clone()).await;
        serde_json::from_value(val).map_err(SyncError::Json)
    }

    /// Fetch [`bangumi_web::SubjectDetail`] for one subject using the JSON API.
    ///
    /// Delegates to the cached `get_subject` and `get_episodes` calls so
    /// concurrent requests for the same subject are served from the Moka cache.
    pub(crate) async fn fetch_subject_detail_api(
        &self,
        subject_id: u64,
    ) -> Option<crate::bangumi_web::SubjectDetail> {
        use crate::bangumi_web::{EpEntry, SubjectDetail};

        let subject = self.get_subject(subject_id).await.ok()?;
        let start_date = subject.date.clone();
        let (name, name_jp) = if subject.name_cn.is_empty() {
            (subject.name.clone(), None)
        } else {
            (subject.name_cn.clone(), Some(subject.name.clone()))
        };

        let ep_list = match self.get_episodes(subject_id).await {
            Ok(l) => l,
            Err(e) => {
                debug!(
                    subject_id,
                    "bangumi: detail fetch episodes failed: {e}"
                );
                return None;
            }
        };

        let mut episodes: Vec<EpEntry> = ep_list
            .data
            .iter()
            .map(|e| {
                let title = if !e.name_cn.is_empty() {
                    e.name_cn.clone()
                } else {
                    e.name.clone()
                };
                EpEntry {
                    sort: e.sort as u32,
                    title,
                    airdate: e.date.clone(),
                }
            })
            .collect();

        episodes.sort_by_key(|e| e.sort);
        let ep_range = crate::bangumi_web::ep_range(&episodes);

        debug!(
            subject_id,
            start_date = start_date.as_deref().unwrap_or(""),
            ep_count = episodes.len(),
            ep_range = ?ep_range,
            "bangumi: detail_fetch_api"
        );

        Some(SubjectDetail {
            subject_id,
            name,
            name_jp,
            start_date,
            episodes,
            ep_range,
        })
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

        // BGM's PATCH endpoint rejects payloads with more than 100 episode IDs.
        // Split into chunks and send sequentially; first error aborts the loop.
        const PATCH_CHUNK: usize = 100;
        for chunk in ep_ids.chunks(PATCH_CHUNK) {
            let body = serde_json::json!({
                "episode_id": chunk,
                "type":        CollectionState::Watched as u8,
            });
            let resp = crate::curl::send_logged(
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
            if !status.is_success() {
                return Err(SyncError::Api {
                    status: status.as_u16(),
                    body: text,
                });
            }
        }
        Ok(())
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
                    continue;
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

    /// Stage 7: if `auto_mark_subject_watched` is enabled, check whether every
    /// main episode (type=0) for `subject_id` is now marked Watched, and if so
    /// upgrade the subject's collection state from Watching (3) to Watched (2).
    ///
    /// Silently returns `Ok(())` when not all episodes are watched yet; only
    /// errors on actual API failures so transient mismatches never abort the
    /// surrounding sync.  Must be called **after** all PATCH requests for the
    /// current sync cycle have completed.
    pub async fn maybe_mark_subject_watched(
        &self,
        subject_id: u64,
    ) -> Result<()> {
        let ep_list = self.get_episodes(subject_id).await?;
        let total_main = ep_list.data.len();
        if total_main == 0 {
            debug!(subject_id, "bangumi: Stage7 — no main episodes, skip");
            return Ok(());
        }

        let user_eps = self.get_user_eps_collection(subject_id).await?;
        let watched_count = user_eps.values().filter(|e| e.watched).count();

        debug!(
            subject_id,
            total_main, watched_count, "bangumi: Stage7 — checking completion"
        );

        if watched_count < total_main {
            return Ok(());
        }

        info!(
            subject_id,
            total_main,
            "bangumi: all main episodes watched — \
             upgrading subject to Watched (2)"
        );
        self.add_collection_subject(subject_id, CollectionState::Watched)
            .await
    }

    /// Build headers without the Authorization token.
    ///
    /// Required for the v0 search endpoint which rejects authenticated requests
    /// with 400 / empty results on some server-side paths.
    fn anon_headers(&self) -> reqwest::header::HeaderMap {
        use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue};
        let mut map = HeaderMap::new();
        let _ =
            map.insert(ACCEPT, HeaderValue::from_static("application/json"));
        let _ = map
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        map
    }

    /// Search subjects via the BGM JSON v0 API (Moka-cached).
    ///
    /// Authorization header is intentionally omitted — the v0 search endpoint
    /// can return 400 or empty results when a user token is present.
    /// `air_date_from` / `air_date_to` bound the subject air date to a closed
    /// window (`air_date >= from` and `air_date <= to`) so subjects outside the
    /// premiere window are excluded up front. Either bound may be `None`.
    pub(crate) async fn search_subjects_api(
        &self,
        keyword: &str,
        all_keywords: &[&str],
        limit: u32,
        air_date_from: Option<&str>,
        air_date_to: Option<&str>,
    ) -> Vec<crate::bangumi_web::SubjectCandidate> {
        use crate::bangumi_web::{
            BANGUMI_CANDIDATE_PRESCREEN_SCORE, SubjectCandidate,
            base_match_score,
        };

        let cache_key: Arc<str> = format!(
            "search:{}:{}:{}",
            keyword,
            air_date_from.unwrap_or("none"),
            air_date_to.unwrap_or("none")
        )
        .into();

        #[derive(serde::Deserialize)]
        struct Rating {
            #[serde(default)]
            score: f64,
        }

        #[derive(serde::Deserialize)]
        struct Entry {
            id: u64,
            name: String,
            #[serde(default)]
            name_cn: String,
            /// Subject rank — absent (`null`) on unreleased / unrated entries.
            rank: Option<u32>,
            /// BGM rating object — absent on unreleased entries.
            rating: Option<Rating>,
            /// Air date string (YYYY-MM-DD) used for the "rare new anime"
            /// exception when rank/score are both missing.
            #[serde(rename = "date")]
            air_date: Option<String>,
        }

        let page_val: serde_json::Value = if let Some(cached) =
            self.cache.get(&cache_key).await
        {
            cached
        } else {
            // V0 search path — url() already includes the /v0 prefix.
            let url = format!(
                "{}/{PATH_SEARCH_SUBJECTS}?limit={limit}&offset=0",
                self.base_url.trim_end_matches('/')
            );
            // Deliberately omit `sort` — BGM defaults to match ranking.
            let mut air_date: Vec<String> = Vec::new();
            if let Some(from) = air_date_from {
                air_date.push(format!(">={from}"));
            }
            if let Some(to) = air_date_to {
                air_date.push(format!("<={to}"));
            }
            let body = if air_date.is_empty() {
                serde_json::json!({
                    "keyword": keyword.trim(),
                    "filter": { "type": [2], "nsfw": true }
                })
            } else {
                serde_json::json!({
                    "keyword": keyword.trim(),
                    "filter": {
                        "type": [2],
                        "nsfw": true,
                        "air_date": air_date
                    }
                })
            };

            // No auth header: v0 search rejects tokens with 400.
            let resp = match self
                .http
                .post(&url)
                .headers(self.anon_headers())
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    debug!(keyword, "bangumi: api_search failed: {e}");
                    return Vec::new();
                }
            };
            let val: serde_json::Value = match resp.json().await {
                Ok(v) => v,
                Err(e) => {
                    debug!(keyword, "bangumi: api_search parse failed: {e}");
                    return Vec::new();
                }
            };
            self.cache.insert(cache_key, val.clone()).await;
            val
        };

        // Today as UNIX-days converted to a YYYY-MM-DD string; used to pass
        // the "rare new anime" exception for entries without rank or score.
        let today: Option<String> = (|| {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_secs() as i64;
            days_to_date(secs / 86_400)
        })();

        let entries: Vec<Entry> = page_val
            .get("data")
            .and_then(|d| serde_json::from_value(d.clone()).ok())
            .unwrap_or_default();

        let candidates: Vec<SubjectCandidate> = entries
            .into_iter()
            .filter_map(|e| {
                let (name, name_jp) = if e.name_cn.is_empty() {
                    (e.name.clone(), None)
                } else {
                    (e.name_cn.clone(), Some(e.name.clone()))
                };
                let sim = all_keywords
                    .iter()
                    .map(|k| {
                        base_match_score(
                            k,
                            &name,
                            name_jp.as_deref().unwrap_or(""),
                        )
                    })
                    .fold(0.0_f64, f64::max);
                // Stage 2.2: filter out unreleased "satellite projects" —
                // entries with neither a rank nor a non-zero score indicate
                // content that has not yet aired and should not intercept a
                // genuine search result.
                // Exception: when the air_date is today-or-earlier AND the
                // title similarity is very high, the entry is a rare or
                // newly-aired title that simply hasn't accumulated ratings yet
                // — allow it through.
                let has_rank = e.rank.is_some();
                let has_score =
                    e.rating.as_ref().is_some_and(|r| r.score > 0.0);
                if !has_rank && !has_score {
                    let is_aired = e
                        .air_date
                        .as_deref()
                        .zip(today.as_deref())
                        .is_some_and(|(ad, td)| ad <= td);
                    if !(is_aired && sim >= UNRANKED_AIRED_TITLE_SIMILARITY) {
                        debug!(
                            subject_id = e.id,
                            name = %name,
                            air_date = ?e.air_date,
                            sim,
                            "bangumi: api_candidate_dropped (no rank/score)"
                        );
                        return None;
                    }
                }
                let pass = sim >= BANGUMI_CANDIDATE_PRESCREEN_SCORE;
                debug!(
                    subject_id = e.id,
                    name = %name,
                    score = sim,
                    result = if pass { "pass" } else { "skip" },
                    "bangumi: api_candidate_prescreen"
                );
                pass.then_some(SubjectCandidate {
                    subject_id: e.id,
                    name,
                    name_jp,
                })
            })
            .collect();

        debug!(
            keyword,
            hits = candidates.len(),
            air_date_from = air_date_from.unwrap_or("none"),
            "bangumi: api_search"
        );
        candidates
    }
}

// ── Web-scrape resolution algorithm ────────────────────────────────────────────

/// Bundles all parameters for [`resolve_by_web_scrape_with_chain`].
pub struct WebScrapeReq<'a> {
    pub series: &'a str,
    /// Primary search keywords (deduplicated original_title + series_name).
    pub keywords: &'a [&'a str],
    /// TMDB alternative titles — used for tie-breaking when multiple subjects
    /// share the same `start_date` distance.
    pub alt_titles: &'a [String],
    /// The series' first air date (`"YYYY-MM-DD"` or ISO timestamp).
    ///
    /// Used **only** as the centre of the search window (`± 2 days`, widened to
    /// `± 15 days` on the fuzzy retry). Anchoring the filter on the franchise's
    /// premiere — rather than the current season's premiere — keeps the
    /// franchise's earliest subject in the candidate pool, from which the
    /// relation chain expands to the other seasons. Falls back to
    /// `season_premiere_date` when `None`.
    pub search_floor_date: Option<&'a str>,
    /// Air date of the first episode of this season (`"YYYY-MM-DD"`).
    ///
    /// Used as the reference point for ranking BGM subjects by start_date
    /// proximity, and as the search floor when `search_floor_date` is `None`.
    pub season_premiere_date: Option<&'a str>,
    /// Air date of the specific episode being marked (`"YYYY-MM-DD"` or ISO
    /// timestamp). When present, subject selection validates that the chosen
    /// subject's episode list contains an episode within ±2 days of this date;
    /// candidates that don't are skipped. When `None` selection falls back to
    /// start_date / title scoring only.
    pub episode_air_date: Option<&'a str>,
}

async fn search_and_collect_keyword_v0(
    keyword: &str,
    all_keywords: &[&str],
    scrape_cache: &mut crate::bangumi_web::ScrapeCache,
    api: &BangumiApi,
    air_dates: (Option<&str>, Option<&str>),
    seen: &mut std::collections::HashSet<u64>,
    details: &mut Vec<crate::bangumi_web::SubjectDetail>,
) {
    use std::collections::hash_map::Entry;

    let (air_date_from, air_date_to) = air_dates;
    let key = format!(
        "v0:{}:{}:{}",
        keyword,
        air_date_from.unwrap_or("none"),
        air_date_to.unwrap_or("none")
    );
    if let Entry::Vacant(e) = scrape_cache.search_results.entry(key.clone()) {
        let results = api
            .search_subjects_api(
                keyword,
                all_keywords,
                50,
                air_date_from,
                air_date_to,
            )
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
            && let Some(d) = api.fetch_subject_detail_api(id).await
        {
            e.insert(d);
        }
        if let Some(d) = scrape_cache.subject_details.get(&id).cloned() {
            details.push(d);
        }
    }
}

/// Collect [`SubjectDetail`]s for all candidates found by `keywords` (and fallback `alt_titles`).
///
/// `air_date_from` / `air_date_to` are pre-computed `"YYYY-MM-DD"` bounds passed
/// directly to the BGM search API filter. Pass `None` for an unbounded edge.
async fn collect_details(
    keywords: &[&str],
    alt_titles: &[String],
    scrape_cache: &mut crate::bangumi_web::ScrapeCache,
    api: &BangumiApi,
    air_date_from: Option<&str>,
    air_date_to: Option<&str>,
) -> Vec<crate::bangumi_web::SubjectDetail> {
    let all_keywords: Vec<&str> = keywords
        .iter()
        .copied()
        .chain(alt_titles.iter().map(|s| s.as_str()))
        .collect();

    let mut seen: std::collections::HashSet<u64> =
        std::collections::HashSet::new();
    let mut details = Vec::new();
    let air_dates = (air_date_from, air_date_to);

    // ── Phase 1: v0 search with primary keywords (within premiere air_date window) ──
    for keyword in keywords.iter().filter(|k| !k.trim().is_empty()) {
        search_and_collect_keyword_v0(
            keyword.trim(),
            &all_keywords,
            scrape_cache,
            api,
            air_dates,
            &mut seen,
            &mut details,
        )
        .await;
    }

    // ── Phase 2: v0 search with TMDB alt_titles fallback (within same air_date window) ──
    // Triggered only when primary keywords returned 0 candidates (e.g. rare localized titles).
    if details.is_empty() {
        for alt in alt_titles.iter().filter(|a| !a.trim().is_empty()) {
            let alt_trimmed = alt.trim();
            if keywords.iter().any(|k| k.eq_ignore_ascii_case(alt_trimmed)) {
                continue;
            }
            search_and_collect_keyword_v0(
                alt_trimmed,
                &all_keywords,
                scrape_cache,
                api,
                air_dates,
                &mut seen,
                &mut details,
            )
            .await;
            if !details.is_empty() {
                break;
            }
        }
    }

    details
}

/// Walk the prequel/sequel relation chain from every subject currently in `details`.
async fn enrich_with_chain(
    api: &BangumiApi,
    details: &mut Vec<crate::bangumi_web::SubjectDetail>,
    scrape_cache: &mut crate::bangumi_web::ScrapeCache,
) {
    use std::collections::{HashSet, VecDeque};

    const MAX_CHAIN_SUBJECTS: usize = 30;

    let mut in_result: HashSet<u64> =
        details.iter().map(|d| d.subject_id).collect();
    // `visited` is per call, not cached: it only guards against re-expanding a
    // node within this single BFS. The relation edges themselves are cached in
    // `scrape_cache.chain_relations`, so a later episode re-walks the graph
    // (rebuilding its own full candidate pool) without re-hitting the API —
    // the bug where a shared "walked" set collapsed every later pool to the raw
    // search hits.
    let mut visited: HashSet<u64> = HashSet::new();
    let mut queue: VecDeque<u64> = in_result.iter().copied().collect();

    while let Some(id) = queue.pop_front() {
        if !visited.insert(id) {
            continue;
        }
        if in_result.len() >= MAX_CHAIN_SUBJECTS {
            break;
        }

        // Followed neighbours: prequels (前传) and sequels (续集) are always
        // followed so the whole linear franchise is reachable from any seed
        // node. Alternate adaptations (不同演绎) are only added when this node
        // has no sequel, covering Fate/TYPE-MOON style multi-branch IPs without
        // blindly flooding the queue with every related subject. The resolved
        // edge list is cached so each subject's relations are fetched once.
        let neighbours: Vec<u64> = match scrape_cache.chain_relations.get(&id) {
            Some(cached) => cached.clone(),
            None => {
                let related = match api.get_related_subjects(id).await {
                    Ok(r) => r,
                    Err(e) => {
                        debug!(
                            subject_id = id,
                            "bangumi: chain walk error: {e}"
                        );
                        scrape_cache.chain_relations.insert(id, Vec::new());
                        continue;
                    }
                };

                let has_sequel = related.iter().any(|r| r.relation == "续集");
                let mut edges: Vec<u64> = related
                    .iter()
                    .filter(|r| r.relation == "前传" || r.relation == "续集")
                    .map(|r| r.id)
                    .collect();

                if !has_sequel {
                    let alt: Vec<u64> = related
                        .iter()
                        .filter(|r| r.relation == "不同演绎")
                        .map(|r| r.id)
                        .collect();
                    if !alt.is_empty() {
                        debug!(
                            subject_id = id,
                            count = alt.len(),
                            "bangumi: no sequel, queuing alternate adaptation \
                             as secondary"
                        );
                    }
                    edges.extend(alt);
                }

                scrape_cache.chain_relations.insert(id, edges.clone());
                edges
            }
        };

        for rid in neighbours {
            if in_result.insert(rid) {
                let detail = if let Some(d) =
                    scrape_cache.subject_details.get(&rid).cloned()
                {
                    d
                } else {
                    let Some(d) = api.fetch_subject_detail_api(rid).await
                    else {
                        continue;
                    };
                    scrape_cache.subject_details.insert(rid, d.clone());
                    d
                };
                details.push(detail);
            }
            // Always traverse through `rid` (even when already in the pool) so
            // its neighbours are reachable; `visited` prevents re-expansion.
            queue.push_back(rid);
        }
    }
}

/// Select the subject whose `start_date` is closest to `anchor_date`.
///
/// Tie-breaking: when multiple subjects share the same minimum distance,
/// score them by title similarity against `keywords` + `alt_titles`.  A
/// single winner above the minimum score is returned; otherwise `None` with a
/// warning so the user knows to add an explicit mapping.
fn select_subject_by_start_date(
    series: &str,
    keywords: &[&str],
    alt_titles: &[String],
    anchor_date: &str,
    details: &[crate::bangumi_web::SubjectDetail],
) -> Option<u64> {
    // Subjects that carry a parseable start_date.
    let with_dates: Vec<(&crate::bangumi_web::SubjectDetail, i64)> = details
        .iter()
        .filter_map(|d| {
            let diff = date_diff_days(d.start_date.as_deref()?, anchor_date)?;
            Some((d, diff))
        })
        .collect();

    if with_dates.is_empty() {
        // Fall back to title-only when no start dates are available.
        return select_by_title(series, keywords, alt_titles, details);
    }

    let min_diff = with_dates.iter().map(|(_, d)| *d).min()?;
    let closest: Vec<&crate::bangumi_web::SubjectDetail> = with_dates
        .iter()
        .filter(|(_, d)| *d == min_diff)
        .map(|(s, _)| *s)
        .collect();

    match closest.as_slice() {
        [single] => {
            info!(
                subject_id = single.subject_id,
                name = %single.name,
                start_date = single.start_date.as_deref().unwrap_or(""),
                date_diff_days = min_diff,
                "bangumi: subject selected by date proximity"
            );
            Some(single.subject_id)
        }
        tied => {
            // Tie-break with TMDB alt titles + series name similarity.
            let all_kws: Vec<&str> = keywords
                .iter()
                .copied()
                .chain(alt_titles.iter().map(|s| s.as_str()))
                .collect();

            let mut scored: Vec<(f64, u64, &str)> = tied
                .iter()
                .map(|d| {
                    let score = all_kws
                        .iter()
                        .map(|k| {
                            crate::bangumi_web::base_match_score(
                                k,
                                &d.name,
                                d.name_jp.as_deref().unwrap_or(""),
                            )
                        })
                        .fold(0.0_f64, f64::max);
                    debug!(
                        subject_id = d.subject_id,
                        name = %d.name,
                        score,
                        "bangumi: tie_break_score"
                    );
                    (score, d.subject_id, d.name.as_str())
                })
                .collect();

            scored.sort_by(|(a, _, _), (b, _, _)| {
                b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal)
            });

            let best = scored.first().map(|(s, _, _)| *s).unwrap_or(0.0);
            let top: Vec<(u64, &str)> = scored
                .iter()
                .filter(|(s, _, _)| (s - best).abs() < 1e-9)
                .map(|(_, id, name)| (*id, *name))
                .collect();

            match top.as_slice() {
                [(id, name)] => {
                    info!(
                        subject_id = id,
                        name = name,
                        score = best,
                        date_diff_days = min_diff,
                        "bangumi: subject selected by title similarity (date tie)"
                    );
                    Some(*id)
                }
                _ => {
                    warn!(
                        series,
                        tied = ?top.iter().map(|(id, _)| id).collect::<Vec<_>>(),
                        "bangumi: subjects tied on date and title — \
                         add an ID mapping in Settings → Bangumi"
                    );
                    None
                }
            }
        }
    }
}

/// Return subject IDs ordered by `(date_diff_days ASC, title_score DESC)`.
///
/// Subjects without a parseable `start_date` are sorted last. Used as the
/// traversal order for episode-air-date–based subject validation.
fn rank_candidates(
    keywords: &[&str],
    alt_titles: &[String],
    anchor_date: Option<&str>,
    details: &[crate::bangumi_web::SubjectDetail],
) -> Vec<u64> {
    let all_kws: Vec<&str> = keywords
        .iter()
        .copied()
        .chain(alt_titles.iter().map(|s| s.as_str()))
        .collect();

    let mut scored: Vec<(i64, f64, u64)> = details
        .iter()
        .map(|d| {
            let date_diff = anchor_date
                .and_then(|anchor| {
                    d.start_date
                        .as_deref()
                        .and_then(|sd| date_diff_days(sd, anchor))
                })
                .unwrap_or(i64::MAX);

            let title_score = all_kws
                .iter()
                .map(|k| {
                    crate::bangumi_web::base_match_score(
                        k,
                        &d.name,
                        d.name_jp.as_deref().unwrap_or(""),
                    )
                })
                .fold(0.0_f64, f64::max);

            (date_diff, title_score, d.subject_id)
        })
        .collect();

    scored.sort_by(|(da, sa, _), (db, sb, _)| {
        da.cmp(db).then_with(|| {
            sb.partial_cmp(sa).unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    scored.into_iter().map(|(_, _, id)| id).collect()
}

/// Title-similarity-only fallback when no subject carries a usable start date.
fn select_by_title(
    series: &str,
    keywords: &[&str],
    alt_titles: &[String],
    details: &[crate::bangumi_web::SubjectDetail],
) -> Option<u64> {
    let all_kws: Vec<&str> = keywords
        .iter()
        .copied()
        .chain(alt_titles.iter().map(|s| s.as_str()))
        .collect();

    let mut scored: Vec<(f64, u64)> = details
        .iter()
        .map(|d| {
            let score = all_kws
                .iter()
                .map(|k| {
                    crate::bangumi_web::base_match_score(
                        k,
                        &d.name,
                        d.name_jp.as_deref().unwrap_or(""),
                    )
                })
                .fold(0.0_f64, f64::max);
            (score, d.subject_id)
        })
        .collect();

    scored.sort_by(|(a, _), (b, _)| {
        b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal)
    });

    let &(best, _) = scored.first()?;
    if best < crate::bangumi_web::BANGUMI_TITLE_MIN_SCORE {
        warn!(
            series,
            score = best,
            "bangumi: no subject matched title threshold — \
             add an ID mapping in Settings → Bangumi"
        );
        return None;
    }

    let top: Vec<u64> = scored
        .iter()
        .filter(|(s, _)| (s - best).abs() < 1e-9)
        .map(|(_, id)| *id)
        .collect();

    // Find the name of the winning subject for logging.
    let winner_name = top
        .first()
        .and_then(|id| details.iter().find(|d| d.subject_id == *id))
        .map(|d| d.name.as_str())
        .unwrap_or("");

    match top.as_slice() {
        [id] => {
            info!(
                subject_id = id,
                name = winner_name,
                score = best,
                "bangumi: subject selected by title similarity"
            );
            Some(*id)
        }
        _ => {
            warn!(
                series,
                tied = ?top,
                "bangumi: subjects tied on title — \
                 add an ID mapping in Settings → Bangumi"
            );
            None
        }
    }
}

/// Fuzzy variant of [`select_subject_by_start_date`] used in Stage 2.4.
///
/// Identical logic, but enforces a strict title-similarity minimum on the final
/// winner to compensate for the wider ±15-day premiere-date window that
/// produces the candidate pool.  A low-similarity winner from a tight date
/// window is usually correct; from a 15-day window it is more likely a false
/// positive, so the stricter gate is justified.
fn select_subject_by_start_date_fuzzy(
    series: &str,
    keywords: &[&str],
    alt_titles: &[String],
    anchor_date: &str,
    details: &[crate::bangumi_web::SubjectDetail],
) -> Option<u64> {
    let with_dates: Vec<(&crate::bangumi_web::SubjectDetail, i64)> = details
        .iter()
        .filter_map(|d| {
            let diff = date_diff_days(d.start_date.as_deref()?, anchor_date)?;
            Some((d, diff))
        })
        .collect();

    if with_dates.is_empty() {
        warn!(
            series,
            "bangumi: fuzzy retry — candidates have no start_date, \
             add an ID mapping in Settings → Bangumi"
        );
        return None;
    }

    let min_diff = with_dates.iter().map(|(_, d)| *d).min()?;
    let closest: Vec<&crate::bangumi_web::SubjectDetail> = with_dates
        .iter()
        .filter(|(_, d)| *d == min_diff)
        .map(|(s, _)| *s)
        .collect();

    let all_kws: Vec<&str> = keywords
        .iter()
        .copied()
        .chain(alt_titles.iter().map(|s| s.as_str()))
        .collect();

    let mut scored: Vec<(f64, u64)> = closest
        .iter()
        .map(|d| {
            let score = all_kws
                .iter()
                .map(|k| {
                    crate::bangumi_web::base_match_score(
                        k,
                        &d.name,
                        d.name_jp.as_deref().unwrap_or(""),
                    )
                })
                .fold(0.0_f64, f64::max);
            debug!(
                subject_id = d.subject_id,
                name = %d.name,
                score,
                date_diff_days = min_diff,
                "bangumi: fuzzy_retry_score"
            );
            (score, d.subject_id)
        })
        .collect();

    scored.sort_by(|(a, _), (b, _)| {
        b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal)
    });

    let &(best, best_id) = scored.first()?;
    if best < FUZZY_RETRY_TITLE_MIN_SCORE {
        warn!(
            series,
            best_score = best,
            min_required = FUZZY_RETRY_TITLE_MIN_SCORE,
            "bangumi: fuzzy retry — title similarity below threshold, \
             add an ID mapping in Settings → Bangumi"
        );
        return None;
    }

    let top: Vec<u64> = scored
        .iter()
        .filter(|(s, _)| (s - best).abs() < 1e-9)
        .map(|(_, id)| *id)
        .collect();

    match top.as_slice() {
        [id] => {
            let name = details
                .iter()
                .find(|d| d.subject_id == *id)
                .map(|d| d.name.as_str())
                .unwrap_or("");
            info!(
                subject_id = best_id,
                name,
                score = best,
                date_diff_days = min_diff,
                "bangumi: subject selected via fuzzy retry"
            );
            Some(*id)
        }
        _ => {
            warn!(
                series,
                tied = ?top,
                "bangumi: fuzzy retry — subjects still tied after title \
                 scoring — add an ID mapping in Settings → Bangumi"
            );
            None
        }
    }
}

/// Resolve a Bangumi subject ID for the given [`WebScrapeReq`].
///
/// Steps:
/// 1. Compute the search window: `search_floor_date ± 2 days` (the series'
///    first air date; falls back to `season_premiere_date`).
/// 2. Search the BGM v0 JSON API with that filter (Stage 1); retry with ±15 day
///    fuzzy window when normal window returns nothing (Stage 2); fallback to
///    unbounded pure-text v0 search when both return nothing (Stage 3).
/// 3. Enrich results via prequel/sequel relation chain.
/// 4. Select the subject with `start_date` closest to `season_premiere_date`;
///    break ties by TMDB alternative-title similarity.
///
/// Returns the Bangumi subject ID, or `None` when resolution fails.
pub async fn resolve_by_web_scrape_with_chain(
    req: &WebScrapeReq<'_>,
    scrape_cache: &mut crate::bangumi_web::ScrapeCache,
    api: &BangumiApi,
) -> Option<u64> {
    // Search window: the series' first air date when available, else the
    // current season's premiere, widened to `floor ± 2 days`. Anchoring on the
    // franchise premiere keeps the franchise's earliest subject in the pool;
    // the relation chain then expands to later/earlier seasons.
    let floor = req.search_floor_date.or(req.season_premiere_date);
    let air_date_from: Option<String> =
        floor.and_then(|d| date_subtract_days(d, 2));
    let air_date_to: Option<String> = floor.and_then(|d| date_add_days(d, 2));

    info!(
        series = req.series,
        keywords = ?req.keywords,
        search_from = air_date_from.as_deref().unwrap_or("none"),
        search_to = air_date_to.as_deref().unwrap_or("none"),
        episode_air_date = req.episode_air_date.unwrap_or("none"),
        "bangumi: searching subjects"
    );

    // Stage 1: Windowed v0 search (floor ± 2 days)
    let mut details = collect_details(
        req.keywords,
        req.alt_titles,
        scrape_cache,
        api,
        air_date_from.as_deref(),
        air_date_to.as_deref(),
    )
    .await;

    // Stage 2: Fuzzy date retry (floor ± 15 days) if normal window returned nothing
    let mut is_fuzzy = false;
    if details.is_empty() {
        let fuzzy_window = floor.and_then(|d| {
            Some((date_subtract_days(d, 15)?, date_add_days(d, 15)?))
        });
        if let Some((fuzzy_from, fuzzy_to)) = fuzzy_window {
            warn!(
                series = req.series,
                fuzzy_from = %fuzzy_from,
                fuzzy_to = %fuzzy_to,
                "bangumi: no subjects found with normal window; \
                 retrying with ±15 day fuzzy window"
            );
            details = collect_details(
                req.keywords,
                req.alt_titles,
                scrape_cache,
                api,
                Some(&fuzzy_from),
                Some(&fuzzy_to),
            )
            .await;
            if !details.is_empty() {
                is_fuzzy = true;
            }
        }
    }

    // Stage 3: Unbounded pure-text v0 search fallback if date-windowed searches returned nothing
    if details.is_empty() && floor.is_some() {
        warn!(
            series = req.series,
            "bangumi: no subjects found within date windows; \
             falling back to unbounded pure-text v0 search"
        );
        details = collect_details(
            req.keywords,
            req.alt_titles,
            scrape_cache,
            api,
            None,
            None,
        )
        .await;
        if !details.is_empty() {
            is_fuzzy = true;
        }
    }

    if details.is_empty() {
        warn!(
            series = req.series,
            "bangumi: no subjects found — \
             add an ID mapping in Settings → Bangumi"
        );
        return None;
    }

    enrich_with_chain(api, &mut details, scrape_cache).await;

    info!(
        series = req.series,
        count = details.len(),
        "bangumi: candidate pool after chain enrichment"
    );

    // Primary path: match the target episode's air_date against each subject's
    // episode list. Candidates are pre-ranked by (date_diff ASC, title_score
    // DESC) so a unique hit on the top-ranked subject is the fast path.
    if let Some(ep_date) = req.episode_air_date {
        let ranked = rank_candidates(
            req.keywords,
            req.alt_titles,
            req.season_premiere_date,
            &details,
        );

        // Collect every subject that contains an episode within ±2 days.
        let mut hits: Vec<u64> = Vec::new();
        for &sid in &ranked {
            let Some(d) = scrape_cache.subject_details.get(&sid) else {
                continue;
            };
            let matched = d.episodes.iter().any(|ep| {
                ep.airdate
                    .as_deref()
                    .and_then(|a| date_diff_days(a, ep_date))
                    .is_some_and(|diff| diff <= 2)
            });
            if matched {
                hits.push(sid);
            } else {
                debug!(
                    subject_id = sid,
                    name = %d.name,
                    target_air_date = ep_date,
                    ep_count = d.episodes.len(),
                    "bangumi: subject skipped — no episode matches air_date"
                );
            }
        }

        match hits.as_slice() {
            [] => {
                let all_candidates_have_dates = !details.is_empty()
                    && details.iter().all(|d| {
                        !d.episodes.is_empty()
                            && d.episodes.iter().any(|ep| {
                                ep.airdate
                                    .as_deref()
                                    .is_some_and(|a| !a.trim().is_empty())
                            })
                    });

                if all_candidates_have_dates {
                    warn!(
                        series = req.series,
                        episode_air_date = ep_date,
                        "bangumi: no subject contains an episode matching \
                         target air_date — \
                         add an ID mapping in Settings → Bangumi"
                    );
                    return None;
                }

                debug!(
                    series = req.series,
                    episode_air_date = ep_date,
                    "bangumi: candidate episodes lack air dates; \
                     falling back to season premiere date / title"
                );
            }
            [single] => {
                let name = scrape_cache
                    .subject_details
                    .get(single)
                    .map(|d| d.name.as_str())
                    .unwrap_or("");
                info!(
                    subject_id = single,
                    name,
                    episode_air_date = ep_date,
                    "bangumi: subject selected — episode air_date matched"
                );
                return Some(*single);
            }
            tied => {
                // Multiple subjects share an episode on this date → break tie
                // by subject-title similarity against series name + alt titles.
                let all_kws: Vec<&str> = req
                    .keywords
                    .iter()
                    .copied()
                    .chain(req.alt_titles.iter().map(|s| s.as_str()))
                    .collect();

                let mut scored: Vec<(f64, u64)> = tied
                    .iter()
                    .filter_map(|&sid| {
                        let d = scrape_cache.subject_details.get(&sid)?;
                        let score = all_kws
                            .iter()
                            .map(|k| {
                                crate::bangumi_web::base_match_score(
                                    k,
                                    &d.name,
                                    d.name_jp.as_deref().unwrap_or(""),
                                )
                            })
                            .fold(0.0_f64, f64::max);
                        debug!(
                            subject_id = sid,
                            name = %d.name,
                            score,
                            "bangumi: ep_date_tie_score"
                        );
                        Some((score, sid))
                    })
                    .collect();

                scored.sort_by(|(a, _), (b, _)| {
                    b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal)
                });

                let &(best_score, best_id) = scored.first()?;

                // Reject if similarity is below threshold.
                if best_score < crate::bangumi_web::BANGUMI_TITLE_MIN_SCORE {
                    warn!(
                        series = req.series,
                        best_score,
                        "bangumi: episode-date tie, title similarity too low — \
                         add an ID mapping in Settings → Bangumi"
                    );
                    return None;
                }

                // Reject if levenshtein distance > 5 for every keyword.
                let best_detail = scrape_cache.subject_details.get(&best_id)?;
                let nm_chars: Vec<char> =
                    best_detail.name.to_lowercase().chars().collect();
                let min_lev = all_kws
                    .iter()
                    .map(|k| {
                        let kw_chars: Vec<char> =
                            k.to_lowercase().chars().collect();
                        crate::bangumi_web::levenshtein(&kw_chars, &nm_chars)
                    })
                    .min()
                    .unwrap_or(usize::MAX);

                if min_lev > 5 {
                    warn!(
                        series = req.series,
                        subject_id = best_id,
                        name = %best_detail.name,
                        min_lev,
                        "bangumi: episode-date tie, title deviation too large \
                         (>{} chars) — \
                         add an ID mapping in Settings → Bangumi",
                        5
                    );
                    return None;
                }

                // Ensure a unique winner (not tied on score too).
                let top: Vec<u64> = scored
                    .iter()
                    .filter(|(s, _)| (s - best_score).abs() < 1e-9)
                    .map(|(_, id)| *id)
                    .collect();

                match top.as_slice() {
                    [id] => {
                        info!(
                            subject_id = id,
                            name = %best_detail.name,
                            score = best_score,
                            target_air_date = ep_date,
                            "bangumi: subject selected — \
                             episode-date tie broken by title"
                        );
                        return Some(*id);
                    }
                    _ => {
                        warn!(
                            series = req.series,
                            tied = ?top,
                            "bangumi: subjects still tied on episode-date \
                             and title — \
                             add an ID mapping in Settings → Bangumi"
                        );
                        return None;
                    }
                }
            }
        };
    }

    // Fallback when no episode air_date is available: rank by start_date
    // proximity, then by title similarity.
    if let Some(anchor) = req.season_premiere_date {
        if is_fuzzy {
            return select_subject_by_start_date_fuzzy(
                req.series,
                req.keywords,
                req.alt_titles,
                anchor,
                &details,
            );
        }
        let id = select_subject_by_start_date(
            req.series,
            req.keywords,
            req.alt_titles,
            anchor,
            &details,
        );
        if id.is_some() {
            return id;
        }
    }

    select_by_title(req.series, req.keywords, req.alt_titles, &details)
}

// ── Subject ID cache ────────────────────────────────────────────────────────────

/// On-disk cache mapping `series_id:season:episode` to a resolved Bangumi
/// subject ID.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SubjectCache {
    #[serde(flatten)]
    map: std::collections::BTreeMap<String, u64>,
}

impl SubjectCache {
    fn key(series_id: &str, season: i64, episode: i64) -> String {
        format!("{series_id}:{season}:{episode}")
    }

    /// Load the cache from `path`, returning an empty cache on any error.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Look up a cached subject ID.
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

    /// Insert a mapping and persist the cache to `path`.
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

// ── History-backfill high-water cache ───────────────────────────────────────────

/// One persisted high-water entry for a `(series_id, season)` pair.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct BackfillMark {
    /// Highest media-server episode index already processed by the backfill.
    high_water: i64,
    /// The `follow_media_server` scope in effect when this mark was written.
    /// A later pass with a different scope must ignore the mark and re-scan,
    /// because the two scopes mark a different set of collections.
    follow: bool,
}

/// On-disk cache recording, per `(series_id, season)`, the highest episode
/// index the history backfill has already processed.
///
/// A long-watched season would otherwise have its entire watched history
/// re-matched against Bangumi on every single playback. Gating the backfill on
/// the recorded high-water mark makes each subsequent pass touch only the
/// genuinely new episodes. The stored `follow` scope guards correctness: when
/// the user flips `history_follow_media_server`, the set of collections that
/// should be marked changes, so a mismatched mark is treated as absent and the
/// season is fully re-scanned once under the new scope.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BackfillCache {
    #[serde(flatten)]
    map: std::collections::BTreeMap<String, BackfillMark>,
}

impl BackfillCache {
    fn key(series_id: &str, season: i64) -> String {
        format!("{series_id}:{season}")
    }

    /// Load the cache from `path`, returning an empty cache on any error.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Highest already-processed episode index for `(series_id, season)`.
    ///
    /// Returns `0` (process everything) when no mark exists or when the stored
    /// scope differs from the requested `follow`, forcing a full re-scan after
    /// a scope change.
    #[must_use]
    pub fn high_water(
        &self,
        series_id: &str,
        season: i64,
        follow: bool,
    ) -> i64 {
        match self.map.get(&Self::key(series_id, season)) {
            Some(m) if m.follow == follow => m.high_water,
            _ => 0,
        }
    }

    /// Record a high-water mark and persist the cache to `path`.
    ///
    /// The mark is only advanced; a lower `high_water` for the same scope never
    /// overwrites a higher one, so an out-of-order replay cannot shrink it.
    pub fn record(
        &mut self,
        series_id: &str,
        season: i64,
        high_water: i64,
        follow: bool,
        path: &Path,
    ) -> Result<()> {
        let key = Self::key(series_id, season);
        let next = match self.map.get(&key) {
            Some(m) if m.follow == follow && m.high_water >= high_water => {
                m.high_water
            }
            _ => high_water,
        };
        let _ = self.map.insert(
            key,
            BackfillMark {
                high_water: next,
                follow,
            },
        );
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

// ── Sync orchestration ────────────────────────────────────────────────────────

/// Sync episode watch status to Bangumi using a subject ID obtained directly
/// from a user mapping (Gate 1 path).
///
/// Thin wrapper over [`sync_episodes`] that matches purely by `sort` number
/// (no air-date hints), preserving the behaviour of the direct-mapping path.
pub async fn sync_episode_by_bangumi_id(
    api: &BangumiApi,
    subject_id: u64,
    ep_sorts: &[u32],
) -> Result<Vec<u64>> {
    let eps: Vec<(u32, Option<String>)> =
        ep_sorts.iter().map(|&sort| (sort, None)).collect();
    sync_episodes(api, subject_id, &eps).await
}

/// Sync a movie subject to Bangumi by marking all its main episodes as watched.
///
/// Implements the Stage 2.5 movie short-circuit path:
/// 1. If the subject is already `Watched (2)`, return early to avoid redundant
///    API calls (BGM silently ignores re-marks on completed subjects anyway).
/// 2. Mark the subject as `Watching (3)` so it surfaces on the user's BGM
///    timeline — the spec calls this the "first-screen activity wake-up".
/// 3. Fetch all main episodes (`type=0`, paginated for long films) and mark
///    them all as `Watched` in one batched call.
///
/// Stage 7 (`auto_mark_subject_watched`) must be triggered by the **caller**
/// after this function returns, not here, to keep the movie and TV paths
/// symmetrical at the call site.
pub async fn sync_movie_subject(
    api: &BangumiApi,
    subject_id: u64,
) -> Result<Vec<u64>> {
    debug!(subject_id, "bangumi: sync_movie_subject start");

    // Stage 5.1: if already Watched (2), skip to avoid redundant marking.
    match api.get_subject_collection(subject_id).await? {
        Some(ref c) if c.get("type").and_then(|t| t.as_u64()) == Some(2) => {
            info!(subject_id, "bangumi: movie subject already Watched, skip");
            return Ok(Vec::new());
        }
        _ => {}
    }

    // Stage 2.5 "first-screen activity wake-up": mark Watching so the entry
    // shows on the user's BGM timeline before the episodes are marked.
    api.add_collection_subject(subject_id, CollectionState::Watching)
        .await?;

    let ep_list = api.get_episodes(subject_id).await?;
    debug!(
        subject_id,
        ep_count = ep_list.data.len(),
        "bangumi: movie fetched episode list"
    );

    let ep_ids: Vec<u64> = ep_list.data.iter().map(|e| e.id).collect();
    if ep_ids.is_empty() {
        info!(subject_id, "bangumi: movie subject has no main episodes");
        return Ok(Vec::new());
    }

    api.mark_episodes_watched(subject_id, &ep_ids).await?;
    info!(
        subject_id,
        count = ep_ids.len(),
        "bangumi: movie marked all episodes watched"
    );
    Ok(ep_ids)
}

/// Mark episodes of a known subject as watched.
///
/// 1. Ensures the subject is in the user's collection.
/// 2. Resolves each `(sort, air_date)` pair to a Bangumi episode ID.
///    - **Primary**: scrapes `bangumi.tv/subject/{id}` and matches by air date.
///      Episode *names* are intentionally ignored because media-server episode
///      titles rarely match bgm.tv's naming.
///    - **Fallback**: queries `GET /v0/episodes` and uses the closest air-date
///      or, when no date is available, the `sort` number directly.
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
        Some(ref c) => match c.get("type").and_then(|t| t.as_u64()) {
            Some(2) => {
                info!(
                    "bangumi: subject {subject_id} already marked Watched, skip"
                );
                return Ok(Vec::new());
            }
            Some(1) | Some(4) => {
                info!(
                    "bangumi: subject {subject_id} marked as Wish or OnHold, \
                     upgrading to Watching"
                );
                api.add_collection_subject(
                    subject_id,
                    CollectionState::Watching,
                )
                .await?;
            }
            _ => {}
        },
    }

    // Log which episode sort numbers the user has already watched on BGM.
    if let Ok(user_eps) = api.get_user_eps_collection(subject_id).await {
        let mut watched: Vec<u64> = user_eps
            .iter()
            .filter(|(_, s)| s.watched)
            .map(|(sort, _)| *sort)
            .collect();
        watched.sort_unstable();
        if !watched.is_empty() {
            let sorts: Vec<u32> = eps.iter().map(|(s, _)| *s).collect();
            debug!(
                subject_id,
                already_watched = ?watched,
                syncing = ?sorts,
                "bangumi: already watched on BGM"
            );
        }
    }

    // Fetch the episode list from the BGM JSON API.
    // `GET /v0/episodes?subject_id={id}` returns each episode's `id` and
    // `airdate`, so we match by air-date proximity (≤ 2 days) and fall back to
    // the `sort` number when no air date is available.
    let ep_list = api.get_episodes(subject_id).await?;
    debug!(
        subject_id,
        ep_count = ep_list.data.len(),
        "bangumi: fetched episode list"
    );

    let batch_targets: std::collections::HashSet<u32> =
        eps.iter().map(|(sort, _)| *sort).collect();
    let mut claimed_ids: std::collections::HashSet<u64> =
        std::collections::HashSet::new();

    let ep_ids: Vec<u64> = eps
        .iter()
        .filter_map(|(sort, air_date)| {
            let mut ep =
                pick_episode(&ep_list.data, *sort, air_date.as_deref(), 2)?;
            // Tier 1 Preemptive Target Claim Prohibition (前车不准抢后车的坑):
            if !is_exact_sort(ep.sort, *sort) && ep.sort > *sort as f64 {
                let cand_int = ep.sort.round() as u32;
                let is_preempted = is_exact_sort(ep.sort, cand_int)
                    && batch_targets.contains(&cand_int);
                let fallback = if is_preempted {
                    ep_list.data.iter().find(|e| is_exact_sort(e.sort, *sort))
                } else {
                    None
                };
                if let Some(same) = fallback {
                    debug!(
                        subject_id,
                        target_sort = sort,
                        stolen_sort = ep.sort,
                        fallback_id = same.id,
                        "bangumi: preempted forward theft of \
                         subsequent batch target — reverted to exact sort"
                    );
                    ep = same;
                }
            }
            if claimed_ids.contains(&ep.id) {
                let fallback = ep_list.data.iter().find(|e| {
                    is_exact_sort(e.sort, *sort) && !claimed_ids.contains(&e.id)
                });
                if let Some(same) = fallback {
                    debug!(
                        subject_id,
                        target_sort = sort,
                        stolen_id = ep.id,
                        fallback_id = same.id,
                        "bangumi: candidate already claimed in batch — \
                         reverted to exact sort"
                    );
                    ep = same;
                } else {
                    debug!(
                        subject_id,
                        target_sort = sort,
                        ep_id = ep.id,
                        "bangumi: episode already claimed in this batch, \
                         skipping duplicate"
                    );
                    return None;
                }
            }
            claimed_ids.insert(ep.id);
            Some(ep.id)
        })
        .collect();

    debug!(
        subject_id,
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
    let ep_ids_display = if let [single] = ep_ids.as_slice() {
        single.to_string()
    } else {
        format!(
            "[{}]",
            ep_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    info!(
        subject_id,
        ep_ids = ep_ids_display,
        count = ep_ids.len(),
        "bangumi: marked episodes watched"
    );
    Ok(ep_ids)
}

/// Outcome of [`mark_backfill_in_subject`].
#[derive(Debug, Default, Clone)]
pub struct BackfillMarkOutcome {
    /// Media-server episode indices this subject accounted for — freshly
    /// marked *or* already watched on Bangumi. The caller removes these from
    /// its pending set so the per-collection loop is guaranteed to terminate.
    pub consumed: Vec<i64>,
    /// Bangumi episode IDs freshly marked watched in this call (excludes
    /// episodes that were already watched).
    pub marked: Vec<u64>,
}

/// Mark a batch of history episodes that belong to one already-resolved subject.
///
/// Unlike [`sync_episodes`], the input is keyed by the media server's episode
/// *index* (not the Bangumi `sort`) and every episode is matched against this
/// single subject's episode list in one shot:
///
/// 1. Ensure the subject is collected (added as *Watching* when absent).
/// 2. Fetch the subject's episode list and the user's per-episode watched
///    state — one request each, regardless of how many episodes are pending.
/// 3. For each pending `(index, air_date)`, resolve it to a Bangumi episode by
///    air-date proximity (≤ 2 days) with a `sort = index + ep_offset` fallback.
///    An episode that resolves is *consumed*; one already watched on Bangumi is
///    skipped (not re-marked) but still consumed.
/// 4. PATCH the remaining freshly-resolved episodes in a single batch.
///
/// Pending entries that resolve to no episode in this subject are left
/// untouched, so the caller can route them to the next collection.
pub async fn mark_backfill_in_subject(
    api: &BangumiApi,
    subject_id: u64,
    pending: &[(i64, Option<String>)],
    ep_offset: i64,
) -> Result<BackfillMarkOutcome> {
    let mut outcome = BackfillMarkOutcome::default();
    if pending.is_empty() {
        return Ok(outcome);
    }

    // Ensure the subject is collected. A subject already marked Watched (2) is
    // left as-is: its episodes are all watched, so the matching below simply
    // consumes the relevant pending entries without re-marking anything.
    match api.get_subject_collection(subject_id).await? {
        None => {
            info!(
                "bangumi: subject {subject_id} not collected, adding as Watching"
            );
            api.add_collection_subject(subject_id, CollectionState::Watching)
                .await?;
        }
        Some(ref c) => match c.get("type").and_then(|t| t.as_u64()) {
            Some(1) | Some(4) => {
                info!(
                    "bangumi: subject {subject_id} marked as Wish or OnHold, \
                     upgrading to Watching"
                );
                api.add_collection_subject(
                    subject_id,
                    CollectionState::Watching,
                )
                .await?;
            }
            _ => {}
        },
    }

    let ep_list = api.get_episodes(subject_id).await?;
    let user_eps = api
        .get_user_eps_collection(subject_id)
        .await
        .unwrap_or_default();

    let batch_targets: std::collections::HashSet<u32> = pending
        .iter()
        .filter_map(|(idx, _)| {
            idx.checked_add(ep_offset)
                .and_then(|s| u32::try_from(s).ok())
                .filter(|&s| s > 0)
        })
        .collect();

    let mut to_mark: Vec<u64> = Vec::new();
    let mut claimed_ids: std::collections::HashSet<u64> =
        std::collections::HashSet::new();

    for (index, air_date) in pending {
        // The sort fallback only applies when index + offset is a valid sort.
        let Some(target_sort) = index
            .checked_add(ep_offset)
            .and_then(|s| u32::try_from(s).ok())
            .filter(|&s| s > 0)
        else {
            continue;
        };
        let Some(mut ep) =
            pick_episode(&ep_list.data, target_sort, air_date.as_deref(), 2)
        else {
            continue;
        };

        // Tier 1 Preemptive Target Claim Prohibition (前车不准抢后车的坑):
        // If pick_episode chose an episode whose sort is strictly greater than
        // target_sort, and that sort is also one of the target sorts in this
        // batch, the earlier episode must not steal the slot from the
        // subsequent episode. Fall back to exact sort!
        if !is_exact_sort(ep.sort, target_sort) && ep.sort > target_sort as f64
        {
            let cand_int_sort = ep.sort.round() as u32;
            let is_preempted = is_exact_sort(ep.sort, cand_int_sort)
                && batch_targets.contains(&cand_int_sort);
            let fallback = if is_preempted {
                ep_list
                    .data
                    .iter()
                    .find(|e| is_exact_sort(e.sort, target_sort))
            } else {
                None
            };
            if let Some(same_sort_ep) = fallback {
                debug!(
                    subject_id,
                    index,
                    target_sort,
                    stolen_sort = ep.sort,
                    fallback_id = same_sort_ep.id,
                    "bangumi: preempted forward theft of \
                     subsequent batch target — reverted to exact sort"
                );
                ep = same_sort_ep;
            }
        }

        // Tier 1 Retrospective Claim Tracking (后向独占检查):
        if claimed_ids.contains(&ep.id) {
            let fallback = ep_list.data.iter().find(|e| {
                is_exact_sort(e.sort, target_sort)
                    && !claimed_ids.contains(&e.id)
            });
            if let Some(same_sort_ep) = fallback {
                debug!(
                    subject_id,
                    index,
                    target_sort,
                    stolen_id = ep.id,
                    fallback_id = same_sort_ep.id,
                    "bangumi: candidate already claimed in backfill batch — \
                     reverted to exact sort"
                );
                ep = same_sort_ep;
            } else {
                debug!(
                    subject_id,
                    index,
                    ep_id = ep.id,
                    "bangumi: episode already claimed in this batch, \
                     skipping duplicate"
                );
                continue;
            }
        }
        claimed_ids.insert(ep.id);

        outcome.consumed.push(*index);
        let already = user_eps
            .get(&(ep.sort as u64))
            .map(|s| s.watched)
            .unwrap_or(false);
        if already {
            debug!(
                subject_id,
                index,
                ep_id = ep.id,
                "bangumi: backfill already watched, skip"
            );
        } else {
            to_mark.push(ep.id);
        }
    }

    to_mark.sort_unstable();
    to_mark.dedup();
    if !to_mark.is_empty() {
        api.mark_episodes_watched(subject_id, &to_mark).await?;
        info!(
            subject_id,
            count = to_mark.len(),
            "bangumi: backfill marked episodes watched"
        );
        outcome.marked = to_mark;
    }
    Ok(outcome)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn make_api(server: &MockServer) -> BangumiApi {
        BangumiApi::new(
            "testuser",
            "tok123",
            true,
            server.uri(),
            new_bgm_read_cache(),
            crate::SyncProxy::default(),
        )
        .unwrap()
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
                        { "id": 1001, "sort": 1.0, "ep": 1.0,
                          "date": "2024-01-07" },
                        { "id": 1002, "sort": 2.0, "ep": 2.0,
                          "date": "2024-01-14" },
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

    // ── search_subjects_api ───────────────────────────────────────────────────

    #[tokio::test]
    async fn search_subjects_api_uses_correct_path() {
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
        let results = api
            .search_subjects_api("AnimeA", &["AnimeA"], 50, None, None)
            .await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn search_subjects_api_returns_candidates() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 1,
                    "data": [{
                        "id": 516416,
                        "name": "AnimeA 年番2",
                        "name_cn": "",
                        "date": "2025-06-05"
                    }]
                }),
            ))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let results = api
            .search_subjects_api(
                "AnimeA",
                &["AnimeA"],
                50,
                Some("2025-01-01"),
                None,
            )
            .await;
        assert_eq!(results.len(), 1);
        assert_eq!(results.first().map(|c| c.subject_id), Some(516416));
    }

    #[tokio::test]
    async fn search_subjects_api_sends_closed_air_date_window() {
        use wiremock::matchers::body_string_contains;

        // A bounded ± window must serialise to BOTH a `>=` lower bound and a
        // `<=` upper bound in the `air_date` filter array.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .and(body_string_contains(">=2020-07-01"))
            .and(body_string_contains("<=2020-07-05"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({ "total": 0, "data": [] }),
                ),
            )
            .expect(1)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let results = api
            .search_subjects_api(
                "AnimeA",
                &["AnimeA"],
                50,
                Some("2020-07-01"),
                Some("2020-07-05"),
            )
            .await;
        assert!(results.is_empty());
    }

    // ── fetch_subject_detail_api ──────────────────────────────────────────────

    #[tokio::test]
    async fn fetch_subject_detail_api_builds_detail_from_json() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/subjects/516416"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "id": 516416,
                    "name": "师兔啊师兔 年番2",
                    "name_cn": "",
                    "date": "2025-06-05",
                    "platform": "WEB"
                }),
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .and(wiremock::matchers::query_param("subject_id", "516416"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 2,
                    "data": [
                        { "sort": 92.0, "name": "Ep92", "name_cn": "",
                          "airdate": "2025-06-05" },
                        { "sort": 93.0, "name": "Ep93", "name_cn": "第93集",
                          "airdate": "2025-06-12" }
                    ]
                }),
            ))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let detail = api.fetch_subject_detail_api(516416).await.unwrap();
        assert_eq!(detail.subject_id, 516416);
        assert_eq!(detail.name, "师兔啊师兔 年番2");
        assert!(detail.name_jp.is_none());
        assert_eq!(detail.start_date.as_deref(), Some("2025-06-05"));
        assert_eq!(detail.episodes.len(), 2);
        assert_eq!(detail.ep_range, Some((92, 93)));
        assert_eq!(
            detail.episodes.get(1).map(|e| e.title.as_str()),
            Some("第93集")
        );
        assert_eq!(
            detail.episodes.first().map(|e| e.title.as_str()),
            Some("Ep92")
        );
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
        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/42"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
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
    async fn mark_single_episode_watched_uses_patch() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/users/-/collections/42/episodes"))
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

        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/42"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/users/-/collections/42"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 2,
                    "data": [
                        { "id": 1001, "sort": 1.0, "ep": 1.0,
                          "date": "2024-01-07" },
                        { "id": 1002, "sort": 2.0, "ep": 2.0,
                          "date": "2024-01-14" },
                    ],
                }),
            ))
            .mount(&server)
            .await;
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

    // ── sync_movie_subject ────────────────────────────────────────────────────

    #[tokio::test]
    async fn sync_movie_subject_marks_all_episodes_watched() {
        let server = MockServer::start().await;

        // Not yet collected.
        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/99"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        // Mark as Watching.
        Mock::given(method("POST"))
            .and(path("/users/-/collections/99"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "type": 3,
            })))
            .respond_with(ResponseTemplate::new(202))
            .expect(1)
            .mount(&server)
            .await;
        // Two main episodes.
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 2,
                    "data": [
                        { "id": 5001, "sort": 1.0, "ep": 1.0,
                          "date": "2024-06-01" },
                        { "id": 5002, "sort": 2.0, "ep": 2.0,
                          "date": "2024-06-01" },
                    ],
                }),
            ))
            .mount(&server)
            .await;
        // PATCH batch (>1 ep).
        Mock::given(method("PATCH"))
            .and(path("/users/-/collections/99/episodes"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let ids = sync_movie_subject(&api, 99).await.unwrap();
        assert_eq!(ids, vec![5001, 5002]);
    }

    #[tokio::test]
    async fn sync_movie_subject_skips_already_watched() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/99"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "type": 2, "subject_id": 99 }),
            ))
            .mount(&server)
            .await;
        // Must not POST or PATCH anything.
        Mock::given(method("POST"))
            .and(path("/users/-/collections/99"))
            .respond_with(ResponseTemplate::new(202))
            .expect(0)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let ids = sync_movie_subject(&api, 99).await.unwrap();
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn sync_movie_subject_handles_empty_episode_list() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/99"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/users/-/collections/99"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({ "total": 0, "data": [] }),
                ),
            )
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let ids = sync_movie_subject(&api, 99).await.unwrap();
        assert!(ids.is_empty());
    }

    // ── sync_episodes with API date matching ──────────────────────────────────

    #[tokio::test]
    async fn sync_episodes_matches_episode_by_air_date() {
        let server = MockServer::start().await;

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
                    "total": 2,
                    "data": [
                        { "id": 3001, "sort": 1.0, "ep": 1.0,
                          "date": "2024-10-02" },
                        { "id": 3002, "sort": 2.0, "ep": 2.0,
                          "date": "2024-10-09" },
                    ],
                }),
            ))
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/users/-/collections/300/episodes"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let eps = vec![(99u32, Some("2024-10-09T00:00:00Z".to_owned()))];
        let marked = sync_episodes(&api, 300, &eps).await.unwrap();
        assert_eq!(marked, vec![3002]);
    }

    // ── mark_backfill_in_subject ──────────────────────────────────────────────

    #[tokio::test]
    async fn mark_backfill_matches_air_date_and_skips_already_watched() {
        let server = MockServer::start().await;

        // Already collected (Watching), so no POST add is issued.
        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/400"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "type": 3, "subject_id": 400 }),
            ))
            .mount(&server)
            .await;
        // Subject episode list: two arcs-worth of episodes.
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 2,
                    "data": [
                        { "id": 8701, "sort": 87.0, "ep": 87.0,
                          "date": "2026-05-25" },
                        { "id": 8801, "sort": 88.0, "ep": 88.0,
                          "date": "2026-06-01" },
                    ],
                }),
            ))
            .mount(&server)
            .await;
        // User episode state: ep 87 already watched (type 2), ep 88 not.
        Mock::given(method("GET"))
            .and(path("/users/-/collections/400/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "data": [
                        { "type": 2, "episode": { "id": 8701, "sort": 87.0,
                          "ep": 87.0, "date": "2026-05-25" } },
                        { "type": 0, "episode": { "id": 8801, "sort": 88.0,
                          "ep": 88.0, "date": "2026-06-01" } },
                    ],
                }),
            ))
            .mount(&server)
            .await;
        // Only the single unwatched episode (88) is freshly marked → PATCH.
        Mock::given(method("PATCH"))
            .and(path("/users/-/collections/400/episodes"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let pending = vec![
            (87, Some("2026-05-25".to_owned())),
            (88, Some("2026-06-01".to_owned())),
            // Belongs to another collection; matches no episode here.
            (10, Some("2010-01-01".to_owned())),
        ];
        let out = mark_backfill_in_subject(&api, 400, &pending, 0)
            .await
            .unwrap();

        let mut consumed = out.consumed.clone();
        consumed.sort_unstable();
        // Episodes 87 and 88 are accounted for; 10 is left for another subject.
        assert_eq!(consumed, vec![87, 88]);
        // Only 88 is freshly marked; 87 was already watched.
        assert_eq!(out.marked, vec![8801]);
    }

    #[tokio::test]
    async fn mark_backfill_batch_marks_unwatched_and_adds_when_uncollected() {
        let server = MockServer::start().await;

        // Not collected yet → triggers a POST add as Watching.
        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/401"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/users/-/collections/401"))
            .respond_with(ResponseTemplate::new(202))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 2,
                    "data": [
                        { "id": 11, "sort": 1.0, "ep": 1.0,
                          "date": "2026-01-01" },
                        { "id": 12, "sort": 2.0, "ep": 2.0,
                          "date": "2026-01-08" },
                    ],
                }),
            ))
            .mount(&server)
            .await;
        // No user episode state yet (nothing watched).
        Mock::given(method("GET"))
            .and(path("/users/-/collections/401/episodes"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "data": [] })),
            )
            .mount(&server)
            .await;
        // Two unwatched episodes → one PATCH batch.
        Mock::given(method("PATCH"))
            .and(path("/users/-/collections/401/episodes"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let pending = vec![
            (1, Some("2026-01-01".to_owned())),
            (2, Some("2026-01-08".to_owned())),
        ];
        let out = mark_backfill_in_subject(&api, 401, &pending, 0)
            .await
            .unwrap();

        let mut consumed = out.consumed.clone();
        consumed.sort_unstable();
        assert_eq!(consumed, vec![1, 2]);
        let mut marked = out.marked.clone();
        marked.sort_unstable();
        assert_eq!(marked, vec![11, 12]);
    }

    #[tokio::test]
    async fn mark_backfill_empty_pending_is_noop() {
        let server = MockServer::start().await;
        let api = make_api(&server).await;
        let out = mark_backfill_in_subject(&api, 402, &[], 0).await.unwrap();
        assert!(out.consumed.is_empty());
        assert!(out.marked.is_empty());
    }

    #[tokio::test]
    async fn mark_backfill_fanren_metadata_shift_preempts_theft_and_marks_all()
    {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/313634"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "type": 3, "subject_id": 313634 }),
            ))
            .mount(&server)
            .await;
        // Bangumi episodes for Fanren: 9 and 10 on 2020-10-11, 11 on 2020-10-18.
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 3,
                    "data": [
                        { "id": 962321, "sort": 9.0, "ep": 1.0,
                          "date": "2020-10-11" },
                        { "id": 962322, "sort": 10.0, "ep": 2.0,
                          "date": "2020-10-11" },
                        { "id": 962323, "sort": 11.0, "ep": 3.0,
                          "date": "2020-10-18" },
                    ],
                }),
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/users/-/collections/313634/episodes"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "data": [] })),
            )
            .mount(&server)
            .await;
        // Must batch mark all 3 episodes: 962321, 962322, 962323.
        Mock::given(method("PATCH"))
            .and(path("/users/-/collections/313634/episodes"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        // Emby/TMDB metadata with Ep 10 mistakenly entered as 2020-10-18.
        let pending = vec![
            (9, Some("2020-10-11".to_owned())),
            (10, Some("2020-10-18".to_owned())),
            (11, Some("2020-10-18".to_owned())),
        ];
        let out = mark_backfill_in_subject(&api, 313634, &pending, 0)
            .await
            .unwrap();

        let mut consumed = out.consumed.clone();
        consumed.sort_unstable();
        assert_eq!(consumed, vec![9, 10, 11]);
        assert_eq!(out.marked, vec![962321, 962322, 962323]);
    }

    #[tokio::test]
    async fn mark_backfill_retrospective_claim_collision_reverts_to_exact_sort()
    {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/500"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "type": 3, "subject_id": 500 }),
            ))
            .mount(&server)
            .await;

        // Two episodes: sort 1 (id 101), sort 2 (id 102).
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 2,
                    "data": [
                        {
                            "id": 101,
                            "sort": 1.0,
                            "ep": 1.0,
                            "date": "2024-10-01"
                        },
                        {
                            "id": 102,
                            "sort": 2.0,
                            "ep": 2.0,
                            "date": "2024-10-08"
                        }
                    ]
                }),
            ))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/users/-/collections/500/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "data": []
                }),
            ))
            .mount(&server)
            .await;

        Mock::given(method("PATCH"))
            .and(path("/users/-/collections/500/episodes"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        // Target 2 erroneously dated as target 1 ("2024-10-01").
        let pending = vec![
            (1, Some("2024-10-01".to_owned())),
            (2, Some("2024-10-01".to_owned())),
        ];
        let out = mark_backfill_in_subject(&api, 500, &pending, 0)
            .await
            .unwrap();

        let mut consumed = out.consumed.clone();
        consumed.sort_unstable();
        assert_eq!(consumed, vec![1, 2]);
        assert_eq!(out.marked, vec![101, 102]);
    }

    // ── pick_episode_id: same-day batch tie-breaking ──────────────────────────

    /// Episodes 1-4 all share the same airdate (premiere week batch release,
    /// e.g. Frieren E01-E04 all airing on 2023-09-29).
    /// Emby reports the target episode as S1E2, whose UTC premiere date is
    /// 2023-09-28T16:00:00Z (= 2023-09-29 in JST).
    /// `pick_episode_id` must resolve to episode 2 (sort=2), NOT episode 1.
    #[tokio::test]
    async fn sync_episodes_picks_correct_sort_when_same_day_batch() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/users/testuser/collections/400602"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/users/-/collections/400602"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&server)
            .await;
        // Episodes 1-4 all share the same airdate.
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 4,
                    "data": [
                        { "id": 1227087, "sort": 1.0, "ep": 1.0,
                          "date": "2023-09-29" },
                        { "id": 1227088, "sort": 2.0, "ep": 2.0,
                          "date": "2023-09-29" },
                        { "id": 1227089, "sort": 3.0, "ep": 3.0,
                          "date": "2023-09-29" },
                        { "id": 1227090, "sort": 4.0, "ep": 4.0,
                          "date": "2023-09-29" },
                    ],
                }),
            ))
            .mount(&server)
            .await;
        // The correct episode is E02 (id 1227088), not E01.
        Mock::given(method("PATCH"))
            .and(path("/users/-/collections/400602/episodes"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        // Emby reports S1E2; UTC premiere "2023-09-28T16:00:00Z" = 2023-09-29 JST.
        // target_sort=2, diff to "2023-09-29" = 1 day (within fuzzy_days=2).
        let eps = vec![(2u32, Some("2023-09-28T16:00:00.0000000Z".to_owned()))];
        let marked = sync_episodes(&api, 400602, &eps).await.unwrap();
        assert_eq!(marked, vec![1227088]);
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
        assert_eq!(normalize_title("魔法　姐妹"), "魔法姐妹");
    }

    #[test]
    fn normalize_title_equalizes_fullwidth_and_halfwidth_punctuation() {
        let bgm = normalize_title("没有辣妹会对阿宅温柔!?");
        let tmdb = normalize_title("没有辣妹会对阿宅温柔！？");
        assert_eq!(bgm, tmdb);
        assert_eq!(normalize_title("ＡＢＣＤ"), "abcd");
        assert_eq!(normalize_title("１２３"), "123");
        assert_eq!(normalize_title("（test）"), "(test)");
    }

    // ── date helpers ──────────────────────────────────────────────────────────

    #[test]
    fn date_diff_handles_iso_prefix_and_gap() {
        assert_eq!(
            date_diff_days("2024-10-09T00:00:00Z", "2024-10-09"),
            Some(0)
        );
        assert_eq!(date_diff_days("2024-10-16", "2024-10-09"), Some(7));
        assert_eq!(date_diff_days("2024-11-01", "2024-10-30"), Some(2));
        assert_eq!(date_diff_days("not-a-date", "2024-10-09"), None);
    }

    #[test]
    fn date_subtract_days_basic() {
        assert_eq!(
            date_subtract_days("2024-04-07", 2),
            Some("2024-04-05".to_owned())
        );
    }

    #[test]
    fn date_subtract_days_crosses_month_boundary() {
        assert_eq!(
            date_subtract_days("2024-03-01", 2),
            Some("2024-02-28".to_owned())
        );
    }

    #[test]
    fn date_subtract_days_crosses_year_boundary() {
        assert_eq!(
            date_subtract_days("2024-01-01", 2),
            Some("2023-12-30".to_owned())
        );
    }

    #[test]
    fn date_add_days_basic_and_boundaries() {
        assert_eq!(
            date_add_days("2020-07-03", 2),
            Some("2020-07-05".to_owned())
        );
        // Crosses month boundary.
        assert_eq!(
            date_add_days("2024-02-28", 2),
            Some("2024-03-01".to_owned())
        );
        // Crosses year boundary.
        assert_eq!(
            date_add_days("2023-12-30", 2),
            Some("2024-01-01".to_owned())
        );
    }

    #[test]
    fn days_to_date_roundtrips() {
        for date in ["2024-04-07", "2023-12-31", "2000-02-29", "1999-01-01"] {
            let d = date_to_days(date).expect("parse");
            let back = days_to_date(d).expect("roundtrip");
            assert_eq!(back, date, "roundtrip failed for {date}");
        }
    }

    // ── pick_episode_id ───────────────────────────────────────────────────────

    fn ep(id: u64, sort: f64, date: Option<&str>) -> BangumiEpisode {
        BangumiEpisode {
            id,
            sort,
            ep: sort,
            date: date.map(str::to_owned),
            name: String::new(),
            name_cn: String::new(),
        }
    }

    #[test]
    fn pick_episode_prefers_air_date_over_sort() {
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
        assert_eq!(pick_episode_id(&eps, 2, None, 2), Some(11));
        assert_eq!(pick_episode_id(&eps, 1, Some("2024-10-09"), 2), Some(10));
    }

    #[test]
    fn pick_episode_returns_none_when_unmatched() {
        let eps = vec![ep(10, 1.0, Some("2020-01-01"))];
        assert_eq!(pick_episode_id(&eps, 5, Some("2024-10-09"), 2), None);
    }

    #[test]
    fn pick_episode_fanren_metadata_error_picks_correct_sort() {
        // Reproduces the exact Fanren (凡人修仙传) bug:
        // Bangumi has Ep 9 and Ep 10 airing on 2020-10-11, and Ep 11 on 2020-10-18.
        // TMDB erroneously recorded Ep 10 as 2020-10-18 (shifted by 1 week).
        // Forward jump guard must prevent Ep 10 from jumping to Ep 11.
        let eps = vec![
            ep(962321, 9.0, Some("2020-10-11")),
            ep(962322, 10.0, Some("2020-10-11")),
            ep(962323, 11.0, Some("2020-10-18")),
        ];
        // For target_sort 10 with air_date 2020-10-18:
        let id_10 = pick_episode_id(&eps, 10, Some("2020-10-18"), 2);
        assert_eq!(
            id_10,
            Some(962322),
            "Target 10 must resolve to Ep 10, not 11"
        );

        // For target_sort 11 with air_date 2020-10-18:
        let id_11 = pick_episode_id(&eps, 11, Some("2020-10-18"), 2);
        assert_eq!(id_11, Some(962323), "Target 11 must resolve to Ep 11");
    }

    #[test]
    fn pick_episode_backward_metadata_error_picks_correct_sort() {
        // Ep 1 is 2024-10-01, Ep 2 is 2024-10-08.
        // Target 2 air_date is erroneously entered as 2024-10-01 (-1 week).
        let eps = vec![
            ep(101, 1.0, Some("2024-10-01")),
            ep(102, 2.0, Some("2024-10-08")),
        ];
        let id_2 = pick_episode_id(&eps, 2, Some("2024-10-01"), 2);
        assert_eq!(id_2, Some(102), "Target 2 must resolve to Ep 2, not 1");
    }

    #[test]
    fn pick_episode_monogatari_special_allows_jump() {
        // Reproduces Monogatari Off & Monster Season:
        // Bangumi Ep 1 is 2024-07-06, Ep 6 is 2024-08-10, Ep 6.5 is 2024-08-17.
        // Emby plays S17E01 (target_sort 1) with air date 2024-08-17.
        // Step is 5.5 (not 1) and diff to Ep 1 is 42 days (not <= 7).
        // Must allow jump and pick Ep 6.5!
        let eps = vec![
            ep(1319027, 1.0, Some("2024-07-06")),
            ep(1364718, 6.0, Some("2024-08-10")),
            ep(1364719, 6.5, Some("2024-08-17")),
            ep(1364720, 7.0, Some("2024-08-24")),
        ];
        let id = pick_episode_id(&eps, 1, Some("2024-08-17"), 2);
        assert_eq!(
            id,
            Some(1364719),
            "Target 1 on 2024-08-17 must resolve to Ep 6.5"
        );
    }

    #[test]
    fn pick_episode_recap_same_sort_allows_forward_jump() {
        // Anime has a recap at Sort 7, while regular Ep 7 is Sort 8.
        // Emby plays S01E07 (target_sort 7) with air date 2024-10-15.
        let mut ep7 = ep(107, 7.0, Some("2024-10-08"));
        ep7.name_cn = "总集篇".to_owned();
        let eps = vec![
            ep(106, 6.0, Some("2024-10-01")),
            ep7,
            ep(108, 8.0, Some("2024-10-15")),
        ];
        let id = pick_episode_id(&eps, 7, Some("2024-10-15"), 2);
        assert_eq!(id, Some(108), "Must jump over recap to regular Ep 8");
    }

    #[test]
    fn pick_episode_exact_float_sort_does_not_match_decimal() {
        let eps = vec![ep(200, 10.5, None)];
        assert_eq!(pick_episode_id(&eps, 10, None, 2), None);
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

        let reloaded = SubjectCache::load(path);
        assert_eq!(reloaded.get("series-1", 4, 1), Some(12345));
        assert_eq!(reloaded.get("series-1", 1, 5), Some(678));
        assert_eq!(reloaded.get("series-1", 4, 2), None);
        assert_eq!(reloaded.get("series-1", 2, 1), None);
    }

    #[test]
    fn subject_cache_load_missing_file_is_empty() {
        let path = std::path::Path::new("/nonexistent/etlp/bgm_cache.json");
        let cache = SubjectCache::load(path);
        assert_eq!(cache.get("x", 1, 1), None);
    }

    // ── BackfillCache ─────────────────────────────────────────────────────────

    #[test]
    fn backfill_cache_roundtrips_and_only_advances() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path();

        let mut cache = BackfillCache::load(path);
        assert_eq!(cache.high_water("s1", 1, false), 0);

        cache.record("s1", 1, 88, false, path).unwrap();
        let reloaded = BackfillCache::load(path);
        assert_eq!(reloaded.high_water("s1", 1, false), 88);

        // A lower mark for the same scope never shrinks the recorded one.
        let mut cache = BackfillCache::load(path);
        cache.record("s1", 1, 40, false, path).unwrap();
        assert_eq!(BackfillCache::load(path).high_water("s1", 1, false), 88);

        // A higher mark advances it.
        cache.record("s1", 1, 120, false, path).unwrap();
        assert_eq!(BackfillCache::load(path).high_water("s1", 1, false), 120);
    }

    #[test]
    fn backfill_cache_scope_change_invalidates_mark() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path();

        let mut cache = BackfillCache::load(path);
        cache.record("s1", 1, 88, false, path).unwrap();

        // Reading with the opposite scope ignores the mark (full re-scan).
        assert_eq!(cache.high_water("s1", 1, true), 0);
        // Reading with the matching scope still sees it.
        assert_eq!(cache.high_water("s1", 1, false), 88);

        // Recording under the new scope overwrites with the new high-water.
        cache.record("s1", 1, 50, true, path).unwrap();
        assert_eq!(BackfillCache::load(path).high_water("s1", 1, true), 50);
        assert_eq!(BackfillCache::load(path).high_water("s1", 1, false), 0);
    }

    // ── resolve_by_web_scrape_with_chain (new algorithm) ─────────────────────

    /// Mount POST /search/subjects returning `candidates` as search hits.
    async fn mount_api_search(
        server: &MockServer,
        candidates: Vec<(u64, &str)>,
    ) {
        let data: Vec<serde_json::Value> = candidates
            .iter()
            .map(|(id, name)| {
                // Include rank and rating so the Stage 2.2 rank/score filter
                // does not drop test candidates.
                serde_json::json!({
                    "id": id, "name": name, "name_cn": "",
                    "rank": 100,
                    "rating": { "score": 7.0 }
                })
            })
            .collect();
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "total": data.len(), "data": data }),
            ))
            .mount(server)
            .await;
    }

    /// Mount GET /subjects/{id} returning subject metadata.
    async fn mount_api_subject(
        server: &MockServer,
        id: u64,
        name: &str,
        start_date: Option<&str>,
    ) {
        Mock::given(method("GET"))
            .and(path(format!("/subjects/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "id": id, "name": name, "name_cn": "",
                    "date": start_date,
                }),
            ))
            .mount(server)
            .await;
    }

    /// Mount GET /episodes?subject_id={id} returning episodes by sort.
    async fn mount_api_episodes(
        server: &MockServer,
        id: u64,
        episodes: Vec<u32>,
    ) {
        let data: Vec<serde_json::Value> = episodes
            .iter()
            .map(|sort| {
                serde_json::json!({ "sort": *sort as f64, "name": "", "name_cn": "" })
            })
            .collect();
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .and(wiremock::matchers::query_param(
                "subject_id",
                id.to_string().as_str(),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "total": data.len(), "data": data }),
            ))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn resolve_selects_subject_with_closest_start_date() {
        let server = MockServer::start().await;
        // Subject 100: start_date far from premiere → loses
        // Subject 200: start_date matches premiere → wins
        mount_api_search(&server, vec![(100, "AnimeA"), (200, "AnimeA")]).await;
        mount_api_subject(&server, 100, "AnimeA", Some("2024-01-07")).await;
        mount_api_episodes(&server, 100, (1..=12).collect()).await;
        mount_api_subject(&server, 200, "AnimeA", Some("2024-04-07")).await;
        mount_api_episodes(&server, 200, (1..=12).collect()).await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeA"];
        let req = WebScrapeReq {
            series: "AnimeA",
            keywords: kws,
            alt_titles: &[],
            search_floor_date: None,
            season_premiere_date: Some("2024-04-07"),
            episode_air_date: None,
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, Some(200));
    }

    #[tokio::test]
    async fn resolve_preserves_candidate_matching_alt_titles_when_primary_keyword_differs()
     {
        let server = MockServer::start().await;
        // Search API returns Fate/Zero when searching for "命运之夜 前传".
        // Subject 10639 has name "Fate/Zero", name_cn "".
        mount_api_search(&server, vec![(10639, "Fate/Zero")]).await;
        mount_api_subject(&server, 10639, "Fate/Zero", Some("2011-10-02"))
            .await;
        mount_api_episodes(&server, 10639, (1..=13).collect()).await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["命运之夜 前传"];
        let alt: Vec<String> =
            vec!["Fate/Zero".to_string(), "命运守护之夜前传".to_string()];
        let req = WebScrapeReq {
            series: "命运之夜 前传",
            keywords: kws,
            alt_titles: &alt,
            search_floor_date: Some("2011-10-02"),
            season_premiere_date: Some("2011-10-02"),
            episode_air_date: None,
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, Some(10639));
    }

    #[tokio::test]
    async fn resolve_falls_back_to_alt_titles_in_v0_when_primary_title_returns_no_candidates()
     {
        let server = MockServer::start().await;
        // Primary keyword returns total: 0
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "keyword": "终物语 第二季"
            })))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({ "total": 0, "data": [] }),
                ),
            )
            .mount(&server)
            .await;

        // Fallback alt_title returns subject 1671
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "keyword": "化物语"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 1,
                    "data": [{
                        "id": 1671,
                        "name": "化物語",
                        "name_cn": "化物语",
                        "rank": 50,
                        "rating": { "score": 8.2 }
                    }]
                }),
            ))
            .mount(&server)
            .await;

        mount_api_subject(&server, 1671, "化物語", Some("2009-07-03")).await;
        mount_api_episodes(&server, 1671, (1..=15).collect()).await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["终物语 第二季"];
        let alt: Vec<String> = vec!["化物语".to_string()];
        let req = WebScrapeReq {
            series: "终物语 第二季",
            keywords: kws,
            alt_titles: &alt,
            search_floor_date: Some("2009-07-03"),
            season_premiere_date: Some("2009-07-03"),
            episode_air_date: None,
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, Some(1671));
    }

    #[tokio::test]
    async fn search_floor_date_overrides_season_premiere_for_filter() {
        let server = MockServer::start().await;
        // The search filter must use search_floor_date − 2 days (2019-12-30),
        // NOT season_premiere_date − 2 days (2024-04-05). The body matcher
        // fails the mock (→ no candidates → None) if the wrong floor is used.
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "filter": { "air_date": [">=2019-12-30"] }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 1,
                    "data": [{
                        "id": 300, "name": "AnimeA", "name_cn": "",
                        "rank": 100, "rating": { "score": 7.0 }
                    }]
                }),
            ))
            .mount(&server)
            .await;
        mount_api_subject(&server, 300, "AnimeA", Some("2020-01-01")).await;
        mount_api_episodes(&server, 300, (1..=12).collect()).await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeA"];
        let req = WebScrapeReq {
            series: "AnimeA",
            keywords: kws,
            alt_titles: &[],
            search_floor_date: Some("2020-01-01"),
            season_premiere_date: Some("2024-04-07"),
            episode_air_date: None,
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, Some(300));
    }

    #[tokio::test]
    async fn resolve_tie_broken_by_title_similarity() {
        let server = MockServer::start().await;
        // Both subjects start on same date; keyword = "AnimeABC" → 100 wins.
        mount_api_search(&server, vec![(100, "AnimeABC"), (200, "AnimeXYZ")])
            .await;
        mount_api_subject(&server, 100, "AnimeABC", Some("2024-04-07")).await;
        mount_api_episodes(&server, 100, (1..=12).collect()).await;
        mount_api_subject(&server, 200, "AnimeXYZ", Some("2024-04-07")).await;
        mount_api_episodes(&server, 200, (1..=12).collect()).await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeABC"];
        let req = WebScrapeReq {
            series: "AnimeABC",
            keywords: kws,
            alt_titles: &[],
            search_floor_date: None,
            season_premiere_date: Some("2024-04-07"),
            episode_air_date: None,
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, Some(100));
    }

    #[tokio::test]
    async fn resolve_returns_none_when_still_tied_after_title() {
        let server = MockServer::start().await;
        // Both subjects have same name and same start date → tied even after
        // title scoring → None.
        mount_api_search(&server, vec![(100, "AnimeA"), (200, "AnimeA")]).await;
        mount_api_subject(&server, 100, "AnimeA", Some("2024-04-07")).await;
        mount_api_episodes(&server, 100, (1..=12).collect()).await;
        mount_api_subject(&server, 200, "AnimeA", Some("2024-04-07")).await;
        mount_api_episodes(&server, 200, (1..=12).collect()).await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeA"];
        let req = WebScrapeReq {
            series: "AnimeA",
            keywords: kws,
            alt_titles: &[],
            search_floor_date: None,
            season_premiere_date: Some("2024-04-07"),
            episode_air_date: None,
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, None);
    }

    #[tokio::test]
    async fn resolve_uses_title_only_when_no_premiere_date() {
        let server = MockServer::start().await;
        mount_api_search(&server, vec![(100, "AnimeABC"), (200, "AnimeXYZ")])
            .await;
        // No start dates provided.
        mount_api_subject(&server, 100, "AnimeABC", None).await;
        mount_api_episodes(&server, 100, (1..=12).collect()).await;
        mount_api_subject(&server, 200, "AnimeXYZ", None).await;
        mount_api_episodes(&server, 200, (1..=12).collect()).await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeABC"];
        let req = WebScrapeReq {
            series: "AnimeABC",
            keywords: kws,
            alt_titles: &[],
            search_floor_date: None,
            season_premiere_date: None,
            episode_air_date: None,
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, Some(100));
    }

    #[tokio::test]
    async fn search_lower_bound_uses_premiere_minus_2_days() {
        // Verifies that the search filter sent to the API is E1 − 2 days,
        // not the first day of the month (the old behaviour).
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "filter": { "air_date": [">=2024-04-05"] }
            })))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({ "total": 0, "data": [] }),
                ),
            )
            .expect(1)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeA"];
        let req = WebScrapeReq {
            series: "AnimeA",
            keywords: kws,
            alt_titles: &[],
            search_floor_date: None,
            season_premiere_date: Some("2024-04-07"),
            episode_air_date: None,
        };
        // No subjects returned → None, but the mock verifies the filter value.
        let _ = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
    }

    // ── episode_air_date subject validation ───────────────────────────────────

    /// Helper: mount episodes with explicit air_dates for subject `id`.
    async fn mount_api_episodes_with_dates(
        server: &MockServer,
        id: u64,
        entries: Vec<(u32, &str)>,
    ) {
        let data: Vec<serde_json::Value> = entries
            .iter()
            .map(|(sort, date)| {
                serde_json::json!({
                    "sort": *sort as f64,
                    "name": "",
                    "name_cn": "",
                    "airdate": date,
                })
            })
            .collect();
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .and(wiremock::matchers::query_param(
                "subject_id",
                id.to_string().as_str(),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "total": data.len(), "data": data }),
            ))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn resolve_validates_subject_by_episode_air_date() {
        // Subject 100 starts closer (2024-10-01) but its only episode aired
        // 2024-10-02, which does NOT match the target date 2024-10-09.
        // Subject 200 starts on 2024-10-07 and HAS an episode on 2024-10-09
        // → 200 should win despite being further from season_premiere_date.
        let server = MockServer::start().await;
        mount_api_search(&server, vec![(100, "AnimeA"), (200, "AnimeA")]).await;
        mount_api_subject(&server, 100, "AnimeA", Some("2024-10-01")).await;
        mount_api_episodes_with_dates(&server, 100, vec![(1, "2024-10-02")])
            .await;
        mount_api_subject(&server, 200, "AnimeA", Some("2024-10-07")).await;
        mount_api_episodes_with_dates(&server, 200, vec![(1, "2024-10-09")])
            .await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeA"];
        let req = WebScrapeReq {
            series: "AnimeA",
            keywords: kws,
            alt_titles: &[],
            search_floor_date: None,
            season_premiere_date: Some("2024-10-01"),
            episode_air_date: Some("2024-10-09"),
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, Some(200));
    }

    #[tokio::test]
    async fn resolve_returns_none_when_no_subject_contains_episode_air_date() {
        let server = MockServer::start().await;
        mount_api_search(&server, vec![(100, "AnimeA")]).await;
        mount_api_subject(&server, 100, "AnimeA", Some("2024-10-01")).await;
        mount_api_episodes_with_dates(&server, 100, vec![(1, "2024-10-02")])
            .await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeA"];
        let req = WebScrapeReq {
            series: "AnimeA",
            keywords: kws,
            alt_titles: &[],
            search_floor_date: None,
            season_premiere_date: Some("2024-10-01"),
            episode_air_date: Some("2024-12-25"),
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, None);
    }

    // ── v0 search: no Authorization header ───────────────────────────────────

    /// Custom wiremock matcher: request must NOT contain an Authorization header.
    struct NoAuthHeader;
    impl wiremock::Match for NoAuthHeader {
        fn matches(&self, req: &wiremock::Request) -> bool {
            !req.headers.contains_key("authorization")
        }
    }

    #[tokio::test]
    async fn search_subjects_api_omits_auth_header() {
        // The v0 search endpoint must not carry an Authorization header.
        // The mock only fires when the custom NoAuthHeader matcher passes.
        // .expect(1) verifies it was called exactly once without that header.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .and(NoAuthHeader)
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({ "total": 0, "data": [] }),
                ),
            )
            .expect(1)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let results = api
            .search_subjects_api("AnimeA", &["AnimeA"], 10, None, None)
            .await;
        assert!(results.is_empty());
    }

    // ── pure-text v0 fallback (Stage 3) ──────────────────────────────────────

    #[tokio::test]
    async fn pure_text_v0_fallback_fires_when_windowed_search_empty() {
        let server = MockServer::start().await;

        // Stage 3 unbounded search (no air_date in filter) returns subject 999.
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "keyword": "AnimeA",
                "filter": { "type": [2] }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 1,
                    "data": [{ "id": 999, "name": "AnimeA", "name_cn": "",
                               "rank": 100, "rating": { "score": 8.0 } }]
                }),
            ))
            .mount(&server)
            .await;

        // Normal window (premiere ± 2 days = 2024-04-05) returns nothing.
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "filter": { "air_date": [">=2024-04-05"] }
            })))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({ "total": 0, "data": [] }),
                ),
            )
            .mount(&server)
            .await;

        // Fuzzy window (premiere ± 15 days = 2024-03-23) also returns nothing.
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "filter": { "air_date": [">=2024-03-23"] }
            })))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({ "total": 0, "data": [] }),
                ),
            )
            .mount(&server)
            .await;

        // Subject / episode details needed by collect_details.
        mount_api_subject(&server, 999, "AnimeA", Some("2024-04-07")).await;
        mount_api_episodes(&server, 999, (1..=12).collect()).await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["AnimeA"];
        let req = WebScrapeReq {
            series: "AnimeA",
            keywords: kws,
            alt_titles: &[],
            search_floor_date: None,
            season_premiere_date: Some("2024-04-07"),
            episode_air_date: None,
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, Some(999));
    }

    // ── fuzzy date retry (Stage 2) ───────────────────────────────────────────

    #[tokio::test]
    async fn fuzzy_retry_fires_when_normal_window_empty() {
        let server = MockServer::start().await;

        // Second v0 call (fuzzy window, 15 days): returns subject 777.
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "filter": { "air_date": [">=2024-03-23"] }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 1,
                    "data": [{ "id": 777, "name": "RareFuzzyAnime",
                               "name_cn": "",
                               "rank": 500,
                               "rating": { "score": 7.5 } }]
                }),
            ))
            .mount(&server)
            .await;

        // First v0 call (normal window, premiere − 2 days = 2024-04-05) returns nothing.
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "filter": { "air_date": [">=2024-04-05"] }
            })))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({ "total": 0, "data": [] }),
                ),
            )
            .mount(&server)
            .await;

        // Subject/episode details for the fuzzy-found subject.
        mock_subject_and_episodes(&server, 777, "RareFuzzyAnime", "2024-04-07")
            .await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["RareFuzzyAnime"];
        let req = WebScrapeReq {
            series: "RareFuzzyAnime",
            keywords: kws,
            alt_titles: &[],
            search_floor_date: None,
            season_premiere_date: Some("2024-04-07"),
            episode_air_date: None,
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, Some(777));
    }

    // ── BFS: alternate adaptations queued when no sequels ─────────────────────

    #[tokio::test]
    async fn bfs_queues_alt_adaptations_when_no_sequels() {
        let server = MockServer::start().await;

        // Search returns root subject 500.
        mount_api_search(&server, vec![(500, "FateAnime")]).await;
        // Root subject starts in 2006, episode aired 2024-10-07.
        mount_api_subject(&server, 500, "FateAnime", Some("2006-01-01")).await;
        mount_api_episodes_with_dates(&server, 500, vec![(1, "2006-01-07")])
            .await;

        // Root has no sequel but one alternate adaptation: subject 501.
        Mock::given(method("GET"))
            .and(path("/subjects/500/subjects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([
                    { "id": 501, "name": "FateAnime UBW",
                      "name_cn": "", "relation": "不同演绎" }
                ]),
            ))
            .mount(&server)
            .await;

        // Subject 501 started 2024-10-01, episode aired 2024-10-07 → match.
        mount_api_subject(&server, 501, "FateAnime UBW", Some("2024-10-01"))
            .await;
        mount_api_episodes_with_dates(&server, 501, vec![(1, "2024-10-07")])
            .await;
        // Subject 501 has no further relations.
        Mock::given(method("GET"))
            .and(path("/subjects/501/subjects"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([])),
            )
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["FateAnime"];
        let req = WebScrapeReq {
            series: "FateAnime",
            keywords: kws,
            alt_titles: &[],
            search_floor_date: None,
            season_premiere_date: Some("2024-10-01"),
            episode_air_date: Some("2024-10-07"),
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        // The alternate-adaptation subject 501 should be found via BFS.
        assert_eq!(id, Some(501));
    }

    // ── BFS: prequel relation is followed ─────────────────────────────────────

    #[tokio::test]
    async fn bfs_follows_prequel_relation() {
        let server = MockServer::start().await;

        // Keyword search only finds the later season (seed 600, premiere 2020).
        mount_api_search(&server, vec![(600, "ShowS2")]).await;
        mount_api_subject(&server, 600, "ShowS2", Some("2020-01-01")).await;
        mount_api_episodes_with_dates(&server, 600, vec![(1, "2020-01-07")])
            .await;

        // Seed 600 has a prequel (前传): the earlier season, subject 601.
        Mock::given(method("GET"))
            .and(path("/subjects/600/subjects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([
                    { "id": 601, "name": "ShowS1",
                      "name_cn": "", "relation": "前传" }
                ]),
            ))
            .mount(&server)
            .await;
        mount_api_subject(&server, 601, "ShowS1", Some("2019-01-01")).await;
        mount_api_episodes_with_dates(&server, 601, vec![(1, "2019-01-07")])
            .await;
        Mock::given(method("GET"))
            .and(path("/subjects/601/subjects"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([])),
            )
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["ShowS2"];
        let req = WebScrapeReq {
            series: "ShowS2",
            keywords: kws,
            alt_titles: &[],
            search_floor_date: None,
            season_premiere_date: Some("2020-01-01"),
            // The episode aired during the prequel season → only subject 601
            // contains a matching episode, so the prequel must be walked.
            episode_air_date: Some("2019-01-07"),
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, Some(601));
    }

    // ── BFS: candidate pool survives multi-episode backfill ───────────────────

    /// A backfill resolves many episodes against one shared `ScrapeCache`.
    /// Regression: a long franchise where the keyword search only ever returns
    /// the earliest season (anchored on the franchise premiere), and a later
    /// season is reachable only through the relation chain. The first episode
    /// walks the full chain; a later-season episode resolved afterwards must
    /// still rebuild the complete candidate pool — it previously collapsed to
    /// the raw search hit because a shared "walked" set blocked re-expansion,
    /// so the later season's subject was never a candidate. The relation API
    /// must also be hit only once per subject across the whole backfill.
    #[tokio::test]
    async fn bfs_pool_survives_across_episodes_sharing_cache() {
        let server = MockServer::start().await;

        // Keyword search always returns only the franchise's first season (700,
        // premiere 2017), because the search window is anchored on the premiere.
        mount_api_search(&server, vec![(700, "Franchise")]).await;
        mount_api_subject(&server, 700, "Franchise", Some("2017-01-01")).await;
        mount_api_episodes_with_dates(&server, 700, vec![(1, "2017-01-07")])
            .await;
        // Season 1 → sequel season 2 (701). The relation endpoint is expected
        // to be queried exactly once even though two episodes are resolved.
        Mock::given(method("GET"))
            .and(path("/subjects/700/subjects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([
                    { "id": 701, "name": "Franchise S2",
                      "name_cn": "", "relation": "续集" }
                ]),
            ))
            .expect(1)
            .mount(&server)
            .await;

        // Season 2 (701) airs years later; its episode aired 2026-06-21 while
        // the media server reports 2026-06-20 — a one-day skew within tolerance.
        mount_api_subject(&server, 701, "Franchise S2", Some("2026-01-01"))
            .await;
        mount_api_episodes_with_dates(&server, 701, vec![(204, "2026-06-21")])
            .await;
        Mock::given(method("GET"))
            .and(path("/subjects/701/subjects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([
                    { "id": 700, "name": "Franchise",
                      "name_cn": "", "relation": "前传" }
                ]),
            ))
            .expect(1)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["Franchise"];

        // First episode belongs to season 1 → walks the full chain, selects 700.
        let s1 = WebScrapeReq {
            series: "Franchise",
            keywords: kws,
            alt_titles: &[],
            search_floor_date: Some("2017-01-01"),
            season_premiere_date: Some("2017-01-01"),
            episode_air_date: Some("2017-01-07"),
        };
        let first =
            resolve_by_web_scrape_with_chain(&s1, &mut cache, &api).await;
        assert_eq!(
            first,
            Some(700),
            "season 1 episode resolves to subject 700"
        );

        // Later season-2 episode, resolved against the SAME cache. The pool must
        // be rebuilt to include 701 even though its relations are now cached.
        let s2 = WebScrapeReq {
            series: "Franchise",
            keywords: kws,
            alt_titles: &[],
            search_floor_date: Some("2017-01-01"),
            season_premiere_date: Some("2017-01-01"),
            episode_air_date: Some("2026-06-20"),
        };
        let second =
            resolve_by_web_scrape_with_chain(&s2, &mut cache, &api).await;
        assert_eq!(
            second,
            Some(701),
            "season 2 episode still resolves after the cache was warmed"
        );
    }

    // ── mark_episodes_watched PATCH chunking ──────────────────────────────────

    #[tokio::test]
    async fn mark_episodes_watched_chunks_to_100() {
        let server = MockServer::start().await;
        // The endpoint is called twice for 150 episode IDs (100 + 50).
        Mock::given(method("PATCH"))
            .and(path("/users/-/collections/42/episodes"))
            .respond_with(ResponseTemplate::new(204))
            .expect(2)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let ep_ids: Vec<u64> = (1..=150).collect();
        api.mark_episodes_watched(42, &ep_ids).await.unwrap();
    }

    // ── maybe_mark_subject_watched (Stage 7) ─────────────────────────────────

    /// Mount subject + episodes + user collection on a single server.
    async fn mock_subject_and_episodes(
        server: &MockServer,
        id: u64,
        name: &str,
        start_date: &str,
    ) {
        mock_subject_only(server, id, name, start_date).await;
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .and(wiremock::matchers::query_param(
                "subject_id",
                id.to_string().as_str(),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 1,
                    "data": [{ "sort": 1.0, "name": "", "name_cn": "" }]
                }),
            ))
            .mount(server)
            .await;
    }

    async fn mock_subject_only(
        server: &MockServer,
        id: u64,
        name: &str,
        start_date: &str,
    ) {
        Mock::given(method("GET"))
            .and(path(format!("/subjects/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "id": id, "name": name, "name_cn": "",
                    "date": start_date,
                }),
            ))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn maybe_mark_subject_watched_upgrades_when_all_done() {
        let server = MockServer::start().await;

        // Episode list: 1 main episode.
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 1,
                    "data": [{ "sort": 1.0, "ep": 1.0, "id": 101 }]
                }),
            ))
            .mount(&server)
            .await;

        // User collection: episode 1 is watched.
        Mock::given(method("GET"))
            .and(path("/users/-/collections/10/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "data": [
                        {
                            "episode": {
                                "id": 101, "sort": 1.0, "ep": 1.0
                            },
                            "type": 2
                        }
                    ]
                }),
            ))
            .mount(&server)
            .await;

        // The function should POST Watched (2) for the subject.
        Mock::given(method("POST"))
            .and(path("/users/-/collections/10"))
            .respond_with(ResponseTemplate::new(202))
            .expect(1)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        api.maybe_mark_subject_watched(10).await.unwrap();
    }

    #[tokio::test]
    async fn maybe_mark_subject_watched_skips_when_incomplete() {
        let server = MockServer::start().await;

        // Episode list: 2 main episodes.
        Mock::given(method("GET"))
            .and(path("/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 2,
                    "data": [
                        { "sort": 1.0, "ep": 1.0, "id": 101 },
                        { "sort": 2.0, "ep": 2.0, "id": 102 }
                    ]
                }),
            ))
            .mount(&server)
            .await;

        // User collection: only episode 1 is watched; episode 2 is not.
        Mock::given(method("GET"))
            .and(path("/users/-/collections/10/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "data": [
                        {
                            "episode": { "id": 101, "sort": 1.0, "ep": 1.0 },
                            "type": 2
                        },
                        {
                            "episode": { "id": 102, "sort": 2.0, "ep": 2.0 },
                            "type": 1
                        }
                    ]
                }),
            ))
            .mount(&server)
            .await;

        // POST must NOT be called.
        Mock::given(method("POST"))
            .and(path("/users/-/collections/10"))
            .respond_with(ResponseTemplate::new(202))
            .expect(0)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        api.maybe_mark_subject_watched(10).await.unwrap();
        // Verification: if the mock's .expect(0) fires a POST, it panics at drop.
    }

    #[tokio::test]
    async fn resolve_falls_back_to_season_premiere_when_episodes_lack_air_dates()
     {
        let server = MockServer::start().await;
        mount_api_search(&server, vec![(400, "Family Guy")]).await;
        mount_api_subject(&server, 400, "Family Guy", Some("1999-01-31")).await;
        mount_api_episodes_with_dates(&server, 400, vec![(1, "")]).await;

        Mock::given(method("GET"))
            .and(path("/subjects/400/subjects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([
                    { "id": 401, "name": "Family Guy (Season 15)",
                      "name_cn": "恶搞之家 第十五季", "relation": "续集" }
                ]),
            ))
            .mount(&server)
            .await;
        mount_api_subject(
            &server,
            401,
            "Family Guy (Season 15)",
            Some("2016-09-25"),
        )
        .await;
        mount_api_episodes_with_dates(&server, 401, vec![(1, ""), (2, "")])
            .await;
        Mock::given(method("GET"))
            .and(path("/subjects/401/subjects"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([])),
            )
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let mut cache = crate::bangumi_web::ScrapeCache::default();
        let kws: &[&str] = &["Family Guy"];
        let req = WebScrapeReq {
            series: "Family Guy",
            keywords: kws,
            alt_titles: &[],
            search_floor_date: Some("1999-01-31"),
            season_premiere_date: Some("2016-09-25"),
            episode_air_date: Some("2016-10-02"),
        };

        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, Some(401));
    }
}
