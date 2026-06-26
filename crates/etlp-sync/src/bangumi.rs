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
//!    d. Walk the 前传/续集 relation chain to expand the candidate pool (BFS).
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
/// Legacy (non-v0) search path: `GET /search/subject/{keyword}`.
/// Not under `/v0/` — must be joined against the root host.
const PATH_LEGACY_SEARCH: &str = "search/subject";
const PATH_ME: &str = "me";

// ── Shared read cache ─────────────────────────────────────────────────────────

/// Capacity of the in-process BGM read cache (number of distinct keys).
const BGM_CACHE_CAPACITY: u64 = 2048;

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
/// Steps:
/// 1. Collect every episode whose `airdate` diff from `date` is ≤ `fuzzy_days`.
/// 2. Find the minimum diff among those candidates.
/// 3. Within the minimum-diff group, prefer the episode whose `sort` equals
///    `target_sort` — this handles same-day batch releases (e.g. Netflix drops
///    or premiere weeks where episodes 1-4 all carry the same airdate).
/// 4. If no sort-match exists in the group, take the first minimum-diff entry.
/// 5. When no air-date candidates pass, fall back to exact `sort` lookup.
fn pick_episode_id(
    episodes: &[BangumiEpisode],
    target_sort: u32,
    air_date: Option<&str>,
    fuzzy_days: i64,
) -> Option<u64> {
    if let Some(date) = air_date {
        let candidates: Vec<(i64, &BangumiEpisode)> = episodes
            .iter()
            .filter_map(|ep| {
                let diff = date_diff_days(ep.date.as_deref()?, date)?;
                (diff <= fuzzy_days).then_some((diff, ep))
            })
            .collect();

        if !candidates.is_empty() {
            let min_diff = candidates
                .iter()
                .map(|(d, _)| *d)
                .min()
                .unwrap_or(i64::MAX);

            let group = candidates
                .iter()
                .filter(|(d, _)| *d == min_diff)
                .map(|(_, ep)| *ep);

            // Prefer the episode in the tie group whose sort matches the
            // Emby index number; fall back to the first element in the group.
            let chosen = group
                .clone()
                .find(|ep| ep.sort as u32 == target_sort)
                .or_else(|| group.clone().next());

            if let Some(ep) = chosen {
                return Some(ep.id);
            }
        }
    }
    // No air-date match: fall back to exact sort lookup.
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
            debug!(subject_id, "bangumi: subject cache hit");
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
            debug!(subject_id, "bangumi: episodes cache hit");
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

    /// Fetch subjects related to `subject_id` (续集/前传/不同演绎, Moka-cached).
    pub async fn get_related_subjects(
        &self,
        subject_id: u64,
    ) -> Result<Vec<BangumiRelated>> {
        let cache_key: Arc<str> = format!("related:{subject_id}").into();
        if let Some(cached) = self.cache.get(&cache_key).await {
            debug!(subject_id, "bangumi: related cache hit");
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

        let cache_key: Arc<str> =
            format!("search:{}:{}", keyword, air_date_from.unwrap_or("none"))
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
            debug!(keyword, "bangumi: search cache hit");
            cached
        } else {
            // V0 search path — url() already includes the /v0 prefix.
            let url = format!(
                "{}/{PATH_SEARCH_SUBJECTS}?limit={limit}&offset=0",
                self.base_url.trim_end_matches('/')
            );
            // Deliberately omit `sort` — BGM defaults to match ranking.
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
                let sim = base_match_score(
                    keyword,
                    &name,
                    name_jp.as_deref().unwrap_or(""),
                );
                // Stage 2.2: filter out unreleased "satellite projects" —
                // entries with neither a rank nor a non-zero score indicate
                // content that has not yet aired and should not intercept a
                // genuine search result.
                // Exception: when the air_date is today-or-earlier AND the
                // title similarity is very high (≥ 0.9), the entry is a rare
                // or newly-aired title that simply hasn't accumulated ratings
                // yet — allow it through.
                let has_rank = e.rank.is_some();
                let has_score =
                    e.rating.as_ref().is_some_and(|r| r.score > 0.0);
                if !has_rank && !has_score {
                    let is_aired = e
                        .air_date
                        .as_deref()
                        .zip(today.as_deref())
                        .is_some_and(|(ad, td)| ad <= td);
                    const HIGH_SIM: f64 = 0.9;
                    if !(is_aired && sim >= HIGH_SIM) {
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

    /// Fallback keyword search using the legacy (non-v0) BGM API.
    ///
    /// Called when the v0 `POST /v0/search/subjects` returns zero candidates.
    /// The legacy endpoint lives at `GET /search/subject/{keyword}` — **outside**
    /// the `/v0/` namespace — so the base URL must be stripped of its `/v0` suffix
    /// before constructing the request.  `responseGroup=large` ensures the
    /// response includes `air_date` and rank/score fields needed for filtering.
    ///
    /// Results are cached under `"legacy_search:{keyword}"` in the shared Moka
    /// cache to avoid hammering the API on repeated calls for the same keyword.
    pub(crate) async fn search_subjects_legacy_api(
        &self,
        keyword: &str,
    ) -> Vec<crate::bangumi_web::SubjectCandidate> {
        let cache_key: Arc<str> = format!("legacy_search:{keyword}").into();
        if let Some(cached) = self.cache.get(&cache_key).await {
            debug!(keyword, "bangumi: legacy_search cache hit");
            return Self::parse_legacy_candidates(cached, keyword);
        }

        // Strip the `/v0` suffix: legacy API is at the host root.
        let root = self
            .base_url
            .trim_end_matches('/')
            .strip_suffix("/v0")
            .unwrap_or_else(|| self.base_url.trim_end_matches('/'));

        // Percent-encode the keyword for safe embedding in a URL path segment.
        let encoded = Self::percent_encode_path(keyword);
        let url = format!("{root}/{PATH_LEGACY_SEARCH}/{encoded}");

        debug!(keyword, url, "bangumi: legacy_search GET");

        let resp = match self
            .http
            .get(&url)
            .headers(self.auth_headers())
            .query(&[("type", "2"), ("responseGroup", "large")])
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                debug!(keyword, "bangumi: legacy_search failed: {e}");
                return Vec::new();
            }
        };

        if !resp.status().is_success() {
            debug!(
                keyword,
                status = resp.status().as_u16(),
                "bangumi: legacy_search non-200"
            );
            return Vec::new();
        }

        let val: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                debug!(keyword, "bangumi: legacy_search parse failed: {e}");
                return Vec::new();
            }
        };

        self.cache.insert(cache_key, val.clone()).await;

        let candidates = Self::parse_legacy_candidates(val, keyword);
        debug!(keyword, hits = candidates.len(), "bangumi: legacy_search");
        candidates
    }

    /// Parse the legacy `{ "list": [...] }` response into `SubjectCandidate`s.
    fn parse_legacy_candidates(
        val: serde_json::Value,
        keyword: &str,
    ) -> Vec<crate::bangumi_web::SubjectCandidate> {
        use crate::bangumi_web::{
            BANGUMI_CANDIDATE_PRESCREEN_SCORE, SubjectCandidate,
            base_match_score,
        };

        #[derive(serde::Deserialize)]
        struct LegacyEntry {
            id: u64,
            name: String,
            #[serde(default)]
            name_cn: String,
        }

        let items: Vec<LegacyEntry> = val
            .get("list")
            .and_then(|l| serde_json::from_value(l.clone()).ok())
            .unwrap_or_default();

        items
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
                    "bangumi: legacy_candidate_prescreen"
                );
                pass.then_some(SubjectCandidate {
                    subject_id: e.id,
                    name,
                    name_jp,
                })
            })
            .collect()
    }

    /// Percent-encode a string for safe use in a URL path segment.
    ///
    /// Unreserved characters (`A-Z a-z 0-9 - _ . ~`) pass through unchanged;
    /// everything else (including spaces and non-ASCII) is encoded as `%XX`.
    fn percent_encode_path(s: &str) -> String {
        let mut out = String::with_capacity(s.len() * 2);
        for byte in s.bytes() {
            match byte {
                b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b'-'
                | b'_'
                | b'.'
                | b'~' => {
                    out.push(byte as char);
                }
                b => {
                    out.push('%');
                    out.push(
                        char::from_digit(u32::from(b >> 4), 16)
                            .unwrap_or('0')
                            .to_ascii_uppercase(),
                    );
                    out.push(
                        char::from_digit(u32::from(b & 0x0f), 16)
                            .unwrap_or('0')
                            .to_ascii_uppercase(),
                    );
                }
            }
        }
        out
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
            let mut results =
                api.search_subjects_api(keyword, 50, air_date_from).await;
            // Stage 2.3: if v0 search returns nothing, fall back to the
            // legacy (non-v0) keyword search endpoint as it handles titles
            // that contain special characters or are not yet indexed by v0.
            if results.is_empty() {
                debug!(
                    keyword,
                    "bangumi: v0 search empty, trying legacy API fallback"
                );
                results = api.search_subjects_legacy_api(keyword).await;
            }
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

        // Primary BFS candidates: direct sequels (续集).
        // If none exist for this node, demote to alternate adaptations
        // (不同演绎) so Fate/TYPE-MOON style multi-branch IPs are covered
        // without blindly flooding the queue with every related subject.
        let sequels: Vec<u64> = related
            .iter()
            .filter(|r| r.relation == "续集")
            .map(|r| r.id)
            .collect();

        let candidates: Vec<u64> = if sequels.is_empty() {
            let alt: Vec<u64> = related
                .iter()
                .filter(|r| r.relation == "不同演绎")
                .map(|r| r.id)
                .collect();
            if !alt.is_empty() {
                debug!(
                    subject_id = id,
                    count = alt.len(),
                    "bangumi: no 续集, queuing 不同演绎 as secondary"
                );
            }
            alt
        } else {
            sequels
        };

        for rid in candidates {
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

/// Fuzzy variant of [`select_subject_by_start_date`] used in Stage 2.4.
///
/// Identical logic, but enforces a **0.9 title-similarity minimum** on the
/// final winner to compensate for the wider ±15-day premiere-date window that
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
    const FUZZY_TITLE_MIN_SCORE: f64 = 0.9;

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
    if best < FUZZY_TITLE_MIN_SCORE {
        warn!(
            series,
            best_score = best,
            min_required = FUZZY_TITLE_MIN_SCORE,
            "bangumi: fuzzy retry — title similarity below 0.9 threshold, \
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
/// 1. Compute search lower bound: `season_premiere_date − 2 days`.
/// 2. Search the BGM JSON API with that filter; fallback to legacy API
///    when v0 returns nothing (Stage 2.3); retry with ±15 day window when
///    both return nothing (Stage 2.4).
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

    // Stage 2.4: fuzzy date retry — if v0 + legacy search both returned
    // nothing, widen the premiere lower-bound from −2 days to −15 days and
    // repeat with a fresh per-call cache (the existing cache would serve the
    // previous empty v0 result and bypass the retry).  The title similarity
    // threshold is raised to 0.9 to compensate for the wider date window.
    if details.is_empty() {
        if let Some(premiere) = req.season_premiere_date
            && let Some(fuzzy_from) = date_subtract_days(premiere, 15)
        {
            warn!(
                series = req.series,
                fuzzy_from,
                "bangumi: no subjects found with normal window; \
                 retrying with ±15 day fuzzy window"
            );
            let mut fuzzy_cache = crate::bangumi_web::ScrapeCache::default();
            let fuzzy_details = collect_details(
                req.keywords,
                &mut fuzzy_cache,
                api,
                Some(&fuzzy_from),
            )
            .await;
            if !fuzzy_details.is_empty() {
                return select_subject_by_start_date_fuzzy(
                    req.series,
                    req.keywords,
                    req.alt_titles,
                    premiere,
                    &fuzzy_details,
                );
            }
        }
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
        BangumiApi::new(
            "testuser",
            "tok123",
            true,
            server.uri(),
            new_bgm_read_cache(),
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

    // ── pick_episode_id: same-day batch tie-breaking ──────────────────────────

    /// Episodes 1-4 all share the same airdate (premiere week batch release,
    /// e.g. 葬送のフリーレン E01-E04 all airing on 2023-09-29).
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
        Mock::given(method("PUT"))
            .and(path("/users/-/collections/-/episodes/1227088"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        // Emby reports S1E2; UTC premiere "2023-09-28T16:00:00Z" = 2023-09-29 JST.
        // target_sort=2, diff to "2023-09-29" = 1 day (within fuzzy_days=2).
        let eps =
            vec![(2u32, Some("2023-09-28T16:00:00.0000000Z".to_owned()))];
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
        let results = api.search_subjects_api("AnimeA", 10, None).await;
        assert!(results.is_empty());
    }

    // ── legacy search fallback (Stage 2.3) ───────────────────────────────────

    #[tokio::test]
    async fn legacy_search_fallback_fires_when_v0_empty() {
        let server = MockServer::start().await;

        // v0 returns nothing.
        Mock::given(method("POST"))
            .and(path("/search/subjects"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({ "total": 0, "data": [] }),
                ),
            )
            .mount(&server)
            .await;

        // Legacy API returns one hit.
        Mock::given(method("GET"))
            .and(path("/search/subject/AnimeA"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "results": 1,
                    "list": [{ "id": 999, "name": "AnimeA", "name_cn": "" }]
                }),
            ))
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
            season_premiere_date: Some("2024-04-07"),
            episode_air_date: None,
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        // Legacy fallback should deliver subject 999.
        assert_eq!(id, Some(999));
    }

    // ── fuzzy date retry (Stage 2.4) ─────────────────────────────────────────

    #[tokio::test]
    async fn fuzzy_retry_fires_when_both_v0_and_legacy_empty() {
        let server = MockServer::start().await;

        // First v0 call (normal window): returns nothing.
        // Second v0 call (fuzzy window, 15 days): returns the subject.
        // Wiremock can't distinguish by request body easily in sequence, so
        // we accept any POST to /search/subjects — the first returns empty,
        // the rest return the subject. Using `up_to` to allow both calls.
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
        // The first call (premiere − 2 days = 2024-04-05) returns nothing.
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
        // Legacy search also returns nothing.
        Mock::given(method("GET"))
            .and(path("/search/subject/RareFuzzyAnime"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({ "results": 0, "list": [] }),
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
            season_premiere_date: Some("2024-04-07"),
            episode_air_date: None,
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        assert_eq!(id, Some(777));
    }

    // ── BFS: 不同演绎 queued when no 续集 ────────────────────────────────────

    #[tokio::test]
    async fn bfs_queues_alt_adaptations_when_no_sequels() {
        let server = MockServer::start().await;

        // Search returns root subject 500.
        mount_api_search(&server, vec![(500, "FateAnime")]).await;
        // Root subject starts in 2006, episode aired 2024-10-07.
        mount_api_subject(&server, 500, "FateAnime", Some("2006-01-01")).await;
        mount_api_episodes_with_dates(&server, 500, vec![(1, "2006-01-07")])
            .await;

        // Root has no 続集 but one 不同演绎: subject 501.
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
            season_premiere_date: Some("2024-10-01"),
            episode_air_date: Some("2024-10-07"),
        };
        let id = resolve_by_web_scrape_with_chain(&req, &mut cache, &api).await;
        // The 不同演绎 subject 501 should be found via BFS.
        assert_eq!(id, Some(501));
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

    // ── percent_encode_path ───────────────────────────────────────────────────

    #[test]
    fn percent_encode_path_encodes_spaces_and_special_chars() {
        assert_eq!(
            BangumiApi::percent_encode_path("進撃の巨人"),
            "%E9%80%B2%E6%92%83%E3%81%AE%E5%B7%A8%E4%BA%BA"
        );
        assert_eq!(BangumiApi::percent_encode_path("Re:Zero"), "Re%3AZero");
        assert_eq!(
            BangumiApi::percent_encode_path("unreserved-_.~"),
            "unreserved-_.~"
        );
        assert_eq!(
            BangumiApi::percent_encode_path("hello world"),
            "hello%20world"
        );
    }
}
