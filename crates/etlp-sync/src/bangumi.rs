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
//!    c. Search the BGM JSON API with the series name + `season_premiere_date − 2 days`.
//!    d. Walk the 前传/续集 relation chain to expand the candidate pool.
//!    e. Select the subject whose `start_date` is closest to the season-premiere date;
//!    break ties with TMDB alternative titles + name similarity.
//!    f. Call `GET /v0/episodes?subject_id={id}` and match the episode whose
//!    `airdate` is closest to the watched episode's air date (names are ignored).
//!
//! `provider_ids["Bangumi"]` is intentionally ignored: users frequently
//! fill it with incorrect values, making it an unreliable signal.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::{Result, SyncError};

/// Log label for this provider's HTTP send/retry/response lines.
const DOMAIN: &str = "bangumi";

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

/// Absolute day difference between two date strings, if both parse.
fn date_diff_days(a: &str, b: &str) -> Option<i64> {
    Some((date_to_days(a)? - date_to_days(b)?).abs())
}

/// Resolve a target episode to a Bangumi episode ID by air-date proximity.
///
/// Prefers the closest air-date match within `fuzzy_days` — robust against
/// per-subject numbering quirks — then falls back to exact `sort` match.
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

/// Find the episode ID in a scraped page list whose air date is closest to
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
    pub const DEFAULT_BASE_URL: &'static str = "https://api.bgm.tv/v0";

    /// Page shown to regenerate a personal access token.
    pub const TOKEN_PAGE_URL: &'static str =
        "https://next.bgm.tv/demo/access-token";

    /// Filename for the persisted `series:season:episode → subject_id` cache.
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

    /// Verify the access token by calling `GET /me`.
    pub async fn verify_token(&self) -> Result<()> {
        debug!(user = %self.username, "bangumi: GET /me (verify token)");
        let resp = crate::curl::send_logged(
            DOMAIN,
            self.http.get(self.url("me")).headers(self.auth_headers()),
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

    /// Fetch subjects related to `subject_id` (e.g. sequels `续集`).
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

    /// Fetch [`bangumi_web::SubjectDetail`] for one subject using the JSON API.
    pub(crate) async fn fetch_subject_detail_api(
        &self,
        subject_id: u64,
    ) -> Option<crate::bangumi_web::SubjectDetail> {
        use crate::bangumi_web::{EpEntry, SubjectDetail};

        #[derive(serde::Deserialize)]
        struct EpApiEntry {
            sort: f64,
            #[serde(default)]
            name: String,
            #[serde(default)]
            name_cn: String,
            // BGM API returns "airdate"; keep "date" as alias for robustness.
            #[serde(alias = "date")]
            airdate: Option<String>,
        }
        #[derive(serde::Deserialize)]
        struct EpApiPage {
            total: u64,
            #[serde(default)]
            data: Vec<EpApiEntry>,
        }

        let subject = self.get_subject(subject_id).await.ok()?;
        let start_date = subject.date.clone();
        let (name, name_jp) = if subject.name_cn.is_empty() {
            (subject.name.clone(), None)
        } else {
            (subject.name_cn.clone(), Some(subject.name.clone()))
        };

        let mut episodes: Vec<EpEntry> = Vec::new();
        const LIMIT: u64 = 100;
        let mut offset = 0u64;

        loop {
            let resp = match self
                .http
                .get(self.url("episodes"))
                .headers(self.auth_headers())
                .query(&[
                    ("subject_id", subject_id),
                    ("type", 0u64),
                    ("limit", LIMIT),
                    ("offset", offset),
                ])
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    debug!(
                        subject_id,
                        "bangumi: api_episodes request failed: {e}"
                    );
                    break;
                }
            };
            let page: EpApiPage = match resp.json().await {
                Ok(p) => p,
                Err(e) => {
                    debug!(
                        subject_id,
                        "bangumi: api_episodes parse failed: {e}"
                    );
                    break;
                }
            };
            if page.data.is_empty() {
                break;
            }
            let fetched = page.data.len() as u64;
            let total = page.total;
            for entry in page.data {
                let title = if entry.name_cn.is_empty() {
                    entry.name
                } else {
                    entry.name_cn
                };
                episodes.push(EpEntry {
                    sort: entry.sort as u32,
                    title,
                    airdate: entry.airdate,
                });
            }
            offset += fetched;
            if offset >= total {
                break;
            }
        }

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

    /// Search subjects via the BGM JSON v0 API.
    ///
    /// Authorization header is intentionally omitted — the v0 search endpoint
    /// can return 400 or empty results when a user token is present.
    /// When `air_date_from` is provided, the search adds an `air_date >=`
    /// filter so that obviously irrelevant older subjects are excluded up front.
    pub(crate) async fn search_subjects_api(
        &self,
        keyword: &str,
        limit: u32,
        air_date_from: Option<&str>,
    ) -> Vec<crate::bangumi_web::SubjectCandidate> {
        use crate::bangumi_web::{
            BANGUMI_CANDIDATE_PRESCREEN_SCORE, SubjectCandidate,
            base_match_score,
        };

        #[derive(serde::Deserialize)]
        struct Page {
            #[serde(default)]
            data: Vec<Entry>,
        }
        #[derive(serde::Deserialize)]
        struct Entry {
            id: u64,
            name: String,
            #[serde(default)]
            name_cn: String,
        }

        // V0 search path — no /v0 prefix since url() already includes it.
        let url = format!(
            "{}/search/subjects?limit={limit}&offset=0",
            self.base_url.trim_end_matches('/')
        );
        // Deliberately omit `sort` — BGM defaults to `match` (综合匹配度),
        // which ranks exact-name matches first, making it safe to blindly
        // trust `data[0]` for well-formed queries.
        let body = if let Some(from) = air_date_from {
            serde_json::json!({
                "keyword": keyword,
                "filter": {
                    "type": [2],
                    "nsfw": true,
                    "air_date": [format!(">={from}")]
                }
            })
        } else {
            serde_json::json!({
                "keyword": keyword,
                "filter": { "type": [2], "nsfw": true }
            })
        };

        // No auth header: the v0 search endpoint rejects tokens with 400.
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
                debug!(keyword, "bangumi: api_search request failed: {e}");
                return Vec::new();
            }
        };
        let page: Page = match resp.json().await {
            Ok(p) => p,
            Err(e) => {
                debug!(keyword, "bangumi: api_search parse failed: {e}");
                return Vec::new();
            }
        };

        let candidates: Vec<SubjectCandidate> = page
            .data
            .into_iter()
            .filter_map(|e| {
                let (name, name_jp) = if e.name_cn.is_empty() {
                    (e.name.clone(), None)
                } else {
                    (e.name_cn.clone(), Some(e.name.clone()))
                };
                let score = base_match_score(
                    keyword,
                    &name,
                    name_jp.as_deref().unwrap_or(""),
                );
                let pass = score >= BANGUMI_CANDIDATE_PRESCREEN_SCORE;
                debug!(
                    subject_id = e.id,
                    name = %name,
                    score,
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
    /// Air date of the first episode of this season (`"YYYY-MM-DD"`).
    ///
    /// Used as the search lower-bound anchor (`−2 days`) and as the reference
    /// point for ranking BGM subjects by start_date proximity. When `None` the
    /// search is issued without a date filter.
    pub season_premiere_date: Option<&'a str>,
    /// Air date of the specific episode being marked (`"YYYY-MM-DD"` or ISO
    /// timestamp). When present, subject selection validates that the chosen
    /// subject's episode list contains an episode within ±2 days of this date;
    /// candidates that don't are skipped. When `None` selection falls back to
    /// start_date / title scoring only.
    pub episode_air_date: Option<&'a str>,
}

/// Collect [`SubjectDetail`]s for all candidates found by `keywords`.
///
/// `air_date_from` is a pre-computed `"YYYY-MM-DD"` lower bound passed
/// directly to the BGM search API filter. Pass `None` for an unbounded search.
async fn collect_details(
    keywords: &[&str],
    scrape_cache: &mut crate::bangumi_web::ScrapeCache,
    api: &BangumiApi,
    air_date_from: Option<&str>,
) -> Vec<crate::bangumi_web::SubjectDetail> {
    use std::collections::hash_map::Entry;

    let mut seen: std::collections::HashSet<u64> =
        std::collections::HashSet::new();
    let mut details = Vec::new();

    for keyword in keywords.iter().filter(|k| !k.trim().is_empty()) {
        let key = (*keyword).to_owned();
        if let Entry::Vacant(e) = scrape_cache.search_results.entry(key.clone())
        {
            let results =
                api.search_subjects_api(keyword, 50, air_date_from).await;
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
    details
}

/// Walk the 前传/续集 relation chain from every subject currently in `details`.
async fn enrich_with_chain(
    api: &BangumiApi,
    details: &mut Vec<crate::bangumi_web::SubjectDetail>,
    scrape_cache: &mut crate::bangumi_web::ScrapeCache,
) {
    use std::collections::{HashSet, VecDeque};

    const MAX_CHAIN_SUBJECTS: usize = 30;

    let mut in_result: HashSet<u64> =
        details.iter().map(|d| d.subject_id).collect();
    let mut queue: VecDeque<u64> = in_result.iter().copied().collect();

    while let Some(id) = queue.pop_front() {
        if scrape_cache.chain_walked.contains(&id) {
            continue;
        }
        if in_result.len() >= MAX_CHAIN_SUBJECTS {
            break;
        }
        scrape_cache.chain_walked.insert(id);

        let related = match api.get_related_subjects(id).await {
            Ok(r) => r,
            Err(e) => {
                debug!(subject_id = id, "bangumi: chain walk error: {e}");
                continue;
            }
        };

        for rel in related {
            if !matches!(rel.relation.as_str(), "前传" | "续集") {
                continue;
            }
            let rid = rel.id;
            if !in_result.insert(rid) {
                if !scrape_cache.chain_walked.contains(&rid) {
                    queue.push_back(rid);
                }
                continue;
            }

            let detail = if let Some(d) =
                scrape_cache.subject_details.get(&rid).cloned()
            {
                d
            } else {
                let Some(d) = api.fetch_subject_detail_api(rid).await else {
                    continue;
                };
                scrape_cache.subject_details.insert(rid, d.clone());
                d
            };

            details.push(detail);
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

/// Resolve a Bangumi subject ID for the given [`WebScrapeReq`].
///
/// Steps:
/// 1. Compute search lower bound: `season_premiere_date − 2 days`.
/// 2. Search the BGM JSON API with that filter.
/// 3. Enrich results via 前传/续集 relation chain.
/// 4. Select the subject with `start_date` closest to `season_premiere_date`;
///    break ties by TMDB alternative-title similarity.
///
/// Returns the Bangumi subject ID, or `None` when resolution fails.
pub async fn resolve_by_web_scrape_with_chain(
    req: &WebScrapeReq<'_>,
    scrape_cache: &mut crate::bangumi_web::ScrapeCache,
    api: &BangumiApi,
) -> Option<u64> {
    // Lower bound: premiere date minus 2 days so subjects that started just
    // before the queried premiere are still included.
    let air_date_from: Option<String> = req
        .season_premiere_date
        .and_then(|d| date_subtract_days(d, 2));

    info!(
        series = req.series,
        keywords = ?req.keywords,
        search_from = air_date_from.as_deref().unwrap_or("none"),
        episode_air_date = req.episode_air_date.unwrap_or("none"),
        "bangumi: searching subjects"
    );

    let mut details = collect_details(
        req.keywords,
        scrape_cache,
        api,
        air_date_from.as_deref(),
    )
    .await;

    enrich_with_chain(api, &mut details, scrape_cache).await;

    info!(
        series = req.series,
        count = details.len(),
        "bangumi: candidate pool after chain enrichment"
    );

    if details.is_empty() {
        warn!(
            series = req.series,
            "bangumi: no subjects found — \
             add an ID mapping in Settings → Bangumi"
        );
        return None;
    }

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
                let bgm_dates: Vec<&str> = d
                    .episodes
                    .iter()
                    .filter_map(|ep| ep.airdate.as_deref())
                    .collect();
                debug!(
                    subject_id = sid,
                    name = %d.name,
                    target_air_date = ep_date,
                    bgm_episode_dates = ?bgm_dates,
                    "bangumi: subject skipped — no episode matches air_date"
                );
            }
        }

        return match hits.as_slice() {
            [] => {
                warn!(
                    series = req.series,
                    episode_air_date = ep_date,
                    "bangumi: no subject contains an episode matching \
                     target air_date — \
                     add an ID mapping in Settings → Bangumi"
                );
                None
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
                Some(*single)
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
                        Some(*id)
                    }
                    _ => {
                        warn!(
                            series = req.series,
                            tied = ?top,
                            "bangumi: subjects still tied on episode-date \
                             and title — \
                             add an ID mapping in Settings → Bangumi"
                        );
                        None
                    }
                }
            }
        };
    }

    // Fallback when no episode air_date is available: rank by start_date
    // proximity, then by title similarity.
    if let Some(anchor) = req.season_premiere_date {
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
        Some(ref c) if c.get("type").and_then(|t| t.as_u64()) == Some(2) => {
            info!("bangumi: subject {subject_id} already marked Watched, skip");
            return Ok(Vec::new());
        }
        Some(_) => {}
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

    let ep_ids: Vec<u64> = eps
        .iter()
        .filter_map(|(sort, air_date)| {
            pick_episode_id(&ep_list.data, *sort, air_date.as_deref(), 2)
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
        let results = api.search_subjects_api("AnimeA", 50, None).await;
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
            .search_subjects_api("AnimeA", 50, Some("2025-01-01"))
            .await;
        assert_eq!(results.len(), 1);
        assert_eq!(results.first().map(|c| c.subject_id), Some(516416));
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

    // ── resolve_by_web_scrape_with_chain (new algorithm) ─────────────────────

    /// Mount POST /search/subjects returning `candidates` as search hits.
    async fn mount_api_search(
        server: &MockServer,
        candidates: Vec<(u64, &str)>,
    ) {
        let data: Vec<serde_json::Value> = candidates
            .iter()
            .map(|(id, name)| {
                serde_json::json!({ "id": id, "name": name, "name_cn": "" })
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
            season_premiere_date: Some("2024-04-07"),
            episode_air_date: None,
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, Some(200));
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
            season_premiere_date: Some("2024-10-01"),
            episode_air_date: Some("2024-12-25"),
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, None);
    }
}
