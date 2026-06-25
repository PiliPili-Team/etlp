//! Lightweight TMDB API client for air-date lookups.
//!
//! Used by the Bangumi sync path as a fallback when the media server carries
//! no `PremiereDate` for a given episode or movie. Responses are cached in an
//! in-process LRU cache to avoid redundant network requests.

use std::num::NonZeroUsize;

use lru::LruCache;
use serde::Deserialize;
use tokio::sync::Mutex;
use tracing::{debug, warn};

use crate::curl;
use crate::error::{Result, SyncError};

const DOMAIN: &str = "tmdb";

// ── Cache key ────────────────────────────────────────────────────────────────

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub enum TmdbCacheKey {
    Movie {
        tmdb_id: u64,
    },
    Episode {
        tmdb_id: u64,
        season: u32,
        episode: u32,
    },
}

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TmdbMovieDetail {
    release_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TmdbEpisodeDetail {
    air_date: Option<String>,
}

// ── Client ────────────────────────────────────────────────────────────────────

/// TMDB REST API client for air-date lookups, backed by an LRU cache.
///
/// `None` results are also cached so a failing lookup is not retried on every
/// playback event. Construct with [`TmdbClient::new`] and share across the
/// lifetime of a sync pass.
pub struct TmdbClient {
    api_key: String,
    base_url: String,
    http: reqwest::Client,
    cache: Mutex<LruCache<TmdbCacheKey, Option<String>>>,
}

impl TmdbClient {
    /// The official TMDB API v3 base URL.
    pub const DEFAULT_BASE_URL: &'static str = "https://api.themoviedb.org/3";

    /// In-process LRU capacity (entries). Keeps memory cost negligible.
    const CACHE_CAPACITY: usize = 512;

    /// Build a new client.
    ///
    /// `base_url` is normally [`Self::DEFAULT_BASE_URL`]. Pass a local mock
    /// server address in tests.
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(etlp_core::UA_ETLP)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(SyncError::Http)?;
        let capacity = NonZeroUsize::new(Self::CACHE_CAPACITY)
            .unwrap_or(NonZeroUsize::MIN);
        Ok(Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            http,
            cache: Mutex::new(LruCache::new(capacity)),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url.trim_end_matches('/'), path)
    }

    // ── Public lookup methods ─────────────────────────────────────────────────

    /// Return the release date of a movie as `"YYYY-MM-DD"`, or `None`.
    ///
    /// Results are cached; a `None` result is cached too so repeated misses
    /// do not cause repeated network requests.
    pub async fn movie_release_date(&self, tmdb_id: u64) -> Option<String> {
        let key = TmdbCacheKey::Movie { tmdb_id };
        if let Some(cached) = self.cache.lock().await.get(&key).cloned() {
            debug!(tmdb_id, "tmdb: movie date cache hit");
            return cached;
        }
        let result = self.fetch_movie_date(tmdb_id).await;
        self.cache.lock().await.put(key, result.clone());
        result
    }

    /// Return the air date of a TV episode as `"YYYY-MM-DD"`, or `None`.
    ///
    /// `tmdb_id` must be the **series-level** TMDB id, not an episode id.
    pub async fn episode_air_date(
        &self,
        tmdb_id: u64,
        season: u32,
        episode: u32,
    ) -> Option<String> {
        let key = TmdbCacheKey::Episode {
            tmdb_id,
            season,
            episode,
        };
        if let Some(cached) = self.cache.lock().await.get(&key).cloned() {
            debug!(tmdb_id, season, episode, "tmdb: episode date cache hit");
            return cached;
        }
        let result = self.fetch_episode_date(tmdb_id, season, episode).await;
        self.cache.lock().await.put(key, result.clone());
        result
    }

    // ── Private fetch helpers ─────────────────────────────────────────────────

    fn params(&self) -> [(&'static str, &str); 2] {
        [("api_key", self.api_key.as_str()), ("language", "zh-CN")]
    }

    async fn fetch_movie_date(&self, tmdb_id: u64) -> Option<String> {
        let url = self.url(&format!("movie/{tmdb_id}"));
        let builder = self.http.get(&url).query(&self.params());
        let logged = match curl::send_logged(DOMAIN, builder).await {
            Ok(l) => l,
            Err(e) => {
                warn!("tmdb: movie {tmdb_id} request failed: {e}");
                return None;
            }
        };
        match curl::json_logged::<TmdbMovieDetail>(DOMAIN, logged).await {
            Ok(detail) => {
                let date = detail.release_date.filter(|d| !d.is_empty());
                if date.is_none() {
                    warn!(tmdb_id, "tmdb: movie has no release_date");
                } else {
                    debug!(tmdb_id, date = ?date, "tmdb: movie date resolved");
                }
                date
            }
            Err(SyncError::Api { status, .. }) => {
                warn!("tmdb: movie {tmdb_id} returned HTTP {status}");
                None
            }
            Err(e) => {
                warn!("tmdb: movie {tmdb_id} parse failed: {e}");
                None
            }
        }
    }

    async fn fetch_episode_date(
        &self,
        tmdb_id: u64,
        season: u32,
        episode: u32,
    ) -> Option<String> {
        let url = self
            .url(&format!("tv/{tmdb_id}/season/{season}/episode/{episode}"));
        let builder = self.http.get(&url).query(&self.params());
        let logged = match curl::send_logged(DOMAIN, builder).await {
            Ok(l) => l,
            Err(e) => {
                warn!(
                    "tmdb: episode {tmdb_id} S{season}E{episode} \
                     request failed: {e}"
                );
                return None;
            }
        };
        match curl::json_logged::<TmdbEpisodeDetail>(DOMAIN, logged).await {
            Ok(detail) => {
                let date = detail.air_date.filter(|d| !d.is_empty());
                if date.is_none() {
                    warn!(
                        tmdb_id,
                        season, episode, "tmdb: episode has no air_date"
                    );
                } else {
                    debug!(
                        tmdb_id,
                        season,
                        episode,
                        date = ?date,
                        "tmdb: episode date resolved"
                    );
                }
                date
            }
            Err(SyncError::Api { status, .. }) => {
                warn!(
                    "tmdb: episode {tmdb_id} S{season}E{episode} \
                     returned HTTP {status}"
                );
                None
            }
            Err(e) => {
                warn!(
                    "tmdb: episode {tmdb_id} S{season}E{episode} \
                     parse failed: {e}"
                );
                None
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    async fn make_client(server: &MockServer) -> TmdbClient {
        TmdbClient::new("test-key", server.uri()).unwrap()
    }

    #[tokio::test]
    async fn movie_release_date_returns_date() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/12345"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "release_date": "2024-07-01" }),
            ))
            .mount(&server)
            .await;

        let client = make_client(&server).await;
        let date = client.movie_release_date(12345).await;
        assert_eq!(date.as_deref(), Some("2024-07-01"));
    }

    #[tokio::test]
    async fn movie_release_date_cached_on_second_call() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/99"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "release_date": "2023-03-15" }),
            ))
            .expect(1) // only one real request even after two calls
            .mount(&server)
            .await;

        let client = make_client(&server).await;
        let _ = client.movie_release_date(99).await;
        let second = client.movie_release_date(99).await;
        assert_eq!(second.as_deref(), Some("2023-03-15"));
    }

    #[tokio::test]
    async fn movie_release_date_returns_none_on_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/0"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = make_client(&server).await;
        assert!(client.movie_release_date(0).await.is_none());
    }

    #[tokio::test]
    async fn movie_release_date_returns_none_for_empty_field() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "release_date": "" })),
            )
            .mount(&server)
            .await;

        let client = make_client(&server).await;
        assert!(client.movie_release_date(1).await.is_none());
    }

    #[tokio::test]
    async fn episode_air_date_returns_date() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tv/79481/season/5/episode/50"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({ "air_date": "2023-06-25" }),
                ),
            )
            .mount(&server)
            .await;

        let client = make_client(&server).await;
        let date = client.episode_air_date(79481, 5, 50).await;
        assert_eq!(date.as_deref(), Some("2023-06-25"));
    }

    #[tokio::test]
    async fn episode_air_date_cached_on_second_call() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tv/100/season/1/episode/1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({ "air_date": "2024-01-07" }),
                ),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = make_client(&server).await;
        let _ = client.episode_air_date(100, 1, 1).await;
        let second = client.episode_air_date(100, 1, 1).await;
        assert_eq!(second.as_deref(), Some("2024-01-07"));
    }

    #[tokio::test]
    async fn episode_air_date_returns_none_on_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tv/0/season/1/episode/1"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = make_client(&server).await;
        assert!(client.episode_air_date(0, 1, 1).await.is_none());
    }

    #[tokio::test]
    async fn none_result_is_cached_to_avoid_retry() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/42"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1) // only one real request
            .mount(&server)
            .await;

        let client = make_client(&server).await;
        let first = client.movie_release_date(42).await;
        let second = client.movie_release_date(42).await;
        assert!(first.is_none());
        assert!(second.is_none());
    }

    #[tokio::test]
    async fn api_key_is_sent_as_query_param() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/movie/55"))
            .and(query_param("api_key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "release_date": "2025-01-01" }),
            ))
            .mount(&server)
            .await;

        let client = make_client(&server).await;
        let date = client.movie_release_date(55).await;
        assert!(date.is_some());
    }
}
