//! Bangumi (bgm.tv) API client and watch-progress sync.
//!
//! Authentication is via a personal Bearer token (no OAuth flow required).
//! The primary sync entry-point is [`sync_episode_by_bangumi_id`], which
//! accepts a Bangumi subject ID taken directly from an Emby item's
//! `ProviderIds.Bangumi` field and marks the specified episodes as watched.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::info;

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

// ── API client ────────────────────────────────────────────────────────────────

/// Bangumi (bgm.tv) REST API v0 client.
///
/// Constructed with a `base_url` so unit tests can point it at a mock server
/// without real network access.
pub struct BangumiApi {
    username: String,
    access_token: String,
    base_url: String,
    http: reqwest::Client,
}

impl BangumiApi {
    /// Create a new client.
    ///
    /// `base_url` is normally `"https://api.bgm.tv/v0"`.  Pass the address of
    /// a local mock server in tests.
    pub fn new(
        username: impl Into<String>,
        access_token: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent("kjtsune/embyBangumi")
            .build()
            .map_err(SyncError::Http)?;
        Ok(Self {
            username: username.into(),
            access_token: access_token.into(),
            base_url: base_url.into(),
            http,
        })
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

    /// Search for anime subjects by keyword and air-date range.
    ///
    /// `start_date` and `end_date` are `YYYY-MM-DD` strings.
    pub async fn search_subjects(
        &self,
        keyword: &str,
        start_date: &str,
        end_date: &str,
        limit: u32,
    ) -> Result<Vec<BangumiSearchSubject>> {
        let body = serde_json::json!({
            "keyword": keyword,
            "filter": {
                "type": [2],
                "air_date": [
                    format!(">={start_date}"),
                    format!("<{end_date}"),
                ],
                "nsfw": true,
            },
        });
        let resp = self
            .http
            .post(self.url("search/subjects"))
            .headers(self.auth_headers())
            .query(&[("limit", limit)])
            .json(&body)
            .send()
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
        let resp = self
            .http
            .get(self.url(&format!("subjects/{subject_id}")))
            .headers(self.auth_headers())
            .send()
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
        let resp = self
            .http
            .get(self.url("episodes"))
            .headers(self.auth_headers())
            .query(&[("subject_id", subject_id), ("type", 0)])
            .send()
            .await?;
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
        let resp = self
            .http
            .get(self.url(&format!("subjects/{subject_id}/subjects")))
            .headers(self.auth_headers())
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(SyncError::Api { status, body });
        }
        Ok(resp.json().await?)
    }

    // ── User collection ───────────────────────────────────────────────────────

    /// Get the user's collection entry for a subject, or `None` if uncollected.
    pub async fn get_subject_collection(
        &self,
        subject_id: u64,
    ) -> Result<Option<serde_json::Value>> {
        let resp = self
            .http
            .get(self.url(&format!(
                "users/{}/collections/{}",
                self.username, subject_id
            )))
            .headers(self.auth_headers())
            .send()
            .await?;
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
        let body = serde_json::json!({
            "type":    state as u8,
            "private": false,
        });
        let resp = self
            .http
            .post(self.url(&format!("users/-/collections/{subject_id}")))
            .headers(self.auth_headers())
            .json(&body)
            .send()
            .await?;
        // 200/202/204 all count as success.
        if resp.status().is_success() || resp.status().as_u16() == 204 {
            return Ok(());
        }
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        Err(SyncError::Api { status, body })
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

        if ep_ids.len() == 1 {
            let ep_id = ep_ids
                .first()
                .copied()
                .ok_or(SyncError::MissingField { field: "ep_id" })?;
            let body =
                serde_json::json!({ "type": CollectionState::Watched as u8 });
            let resp =
                self.http
                    .put(self.url(&format!(
                        "users/-/collections/-/episodes/{ep_id}"
                    )))
                    .headers(self.auth_headers())
                    .json(&body)
                    .send()
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
        let resp = self
            .http
            .patch(
                self.url(&format!("users/-/collections/{subject_id}/episodes")),
            )
            .headers(self.auth_headers())
            .json(&body)
            .send()
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
        let resp = self
            .http
            .get(
                self.url(&format!("users/-/collections/{subject_id}/episodes")),
            )
            .headers(self.auth_headers())
            .send()
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

// ── Sync orchestration ────────────────────────────────────────────────────────

/// Sync episode watch status to Bangumi using a subject ID obtained directly
/// from the Emby item's `ProviderIds.Bangumi` field.
///
/// 1. Ensures the subject is in the user's collection (adds as Watching if not).
/// 2. Resolves `ep_sorts` (episode sort numbers) to Bangumi episode IDs.
/// 3. Marks the resolved episodes as watched.
///
/// Returns the list of Bangumi episode IDs that were marked.
pub async fn sync_episode_by_bangumi_id(
    api: &BangumiApi,
    subject_id: u64,
    ep_sorts: &[u32],
) -> Result<Vec<u64>> {
    if ep_sorts.is_empty() {
        return Ok(Vec::new());
    }

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

    // Resolve sort numbers to episode IDs.
    let ep_list = api.get_episodes(subject_id).await?;
    let ep_ids: Vec<u64> = ep_sorts
        .iter()
        .filter_map(|&sort| {
            ep_list
                .data
                .iter()
                .find(|ep| ep.sort as u32 == sort)
                .map(|ep| ep.id)
        })
        .collect();

    if ep_ids.is_empty() {
        info!(
            "bangumi: no matching episodes found in subject {subject_id} \
             for sorts {ep_sorts:?}"
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
        BangumiApi::new("testuser", "tok123", server.uri()).unwrap()
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
            .search_subjects("テスト", "2024-03-01", "2024-05-01", 5)
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
        let results = api
            .search_subjects("missing", "2024-01-01", "2024-02-01", 5)
            .await
            .unwrap();
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
}
