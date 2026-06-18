//! Trakt.tv API client and watch-history sync.
//!
//! Authentication uses the **OAuth 2.0 Device Flow**:
//! 1. Call [`TraktApi::request_device_code`] to get a `user_code` and
//!    `verification_url` to display to the user.
//! 2. Call [`TraktApi::poll_device_token`] to poll until the user approves.
//! 3. The resulting [`TraktToken`] is saved to disk and reloaded on future
//!    runs; [`TraktApi::ensure_auth`] handles load + refresh automatically.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::{Result, SyncError};

// ── OAuth token ───────────────────────────────────────────────────────────────

/// A persisted Trakt OAuth token (both access and refresh).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraktToken {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: String,
    /// Unix timestamp (seconds) when the token was issued.
    pub created_at: u64,
}

impl TraktToken {
    /// Returns `true` if the access token is valid for at least 7 more days.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.created_at + self.expires_in > now + 7 * 86_400
    }
}

// ── Device-flow response shapes ───────────────────────────────────────────────

/// Response from `POST /oauth/device/code`.
#[derive(Debug, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_url: String,
    pub expires_in: u64,
    /// Minimum polling interval in seconds.
    pub interval: u64,
}

// ── Sync history payloads ─────────────────────────────────────────────────────

/// The kind of item to add to Trakt history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraktItemKind {
    Movie,
    Episode,
}

/// Provider IDs for a single Trakt item.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TraktIds {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trakt: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imdb: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmdb: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tvdb: Option<u64>,
}

/// One item to add to watch history.
#[derive(Debug, Clone)]
pub struct TraktHistoryItem {
    pub kind: TraktItemKind,
    pub ids: TraktIds,
    /// Optional RFC-3339 timestamp; if `None` the current time is used.
    pub watched_at: Option<String>,
}

// ── API client ────────────────────────────────────────────────────────────────

/// Trakt.tv REST API client.
///
/// Constructed with a `base_url` so unit tests can point it at a mock server
/// without real network access.
pub struct TraktApi {
    client_id: String,
    client_secret: String,
    user_id: String,
    base_url: String,
    token_path: PathBuf,
    http: reqwest::Client,
    token: Option<TraktToken>,
}

impl TraktApi {
    /// Create a new client.
    ///
    /// `base_url` is normally `"https://api.trakt.tv"`.  Pass the address of a
    /// local mock server in tests.
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        user_id: impl Into<String>,
        token_path: impl AsRef<Path>,
        base_url: impl Into<String>,
    ) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent("embyToLocalPlayer/1.1")
            .build()
            .map_err(SyncError::Http)?;
        Ok(Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            user_id: user_id.into(),
            base_url: base_url.into(),
            token_path: token_path.as_ref().to_path_buf(),
            http,
            token: None,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url.trim_end_matches('/'), path)
    }

    /// Build the standard Trakt request headers (no auth).
    fn base_headers(&self) -> reqwest::header::HeaderMap {
        use reqwest::header::{ACCEPT, HeaderMap, HeaderValue};
        let mut map = HeaderMap::new();
        let _ =
            map.insert(ACCEPT, HeaderValue::from_static("application/json"));
        if let Ok(v) = HeaderValue::from_str(&self.client_id) {
            let _ = map.insert("trakt-api-key", v);
        }
        let _ = map.insert("trakt-api-version", HeaderValue::from_static("2"));
        map
    }

    /// Add `Authorization: Bearer …` on top of the base headers.
    fn auth_headers(&self) -> reqwest::header::HeaderMap {
        use reqwest::header::{AUTHORIZATION, HeaderValue};
        let mut map = self.base_headers();
        if let Some(tok) = &self.token {
            let val = format!("Bearer {}", tok.access_token);
            if let Ok(v) = HeaderValue::from_str(&val) {
                let _ = map.insert(AUTHORIZATION, v);
            }
        }
        map
    }

    // ── Token persistence ─────────────────────────────────────────────────────

    /// Persist `token` to the configured JSON file.
    pub fn save_token(&self, token: &TraktToken) -> Result<()> {
        let json = serde_json::to_string_pretty(token)?;
        std::fs::write(&self.token_path, json)?;
        Ok(())
    }

    /// Load a previously saved token from disk.
    ///
    /// Returns `Ok(true)` when a token was found and loaded, `Ok(false)` when
    /// no file exists yet.
    pub fn load_token(&mut self) -> Result<bool> {
        if !self.token_path.exists() {
            return Ok(false);
        }
        let data = std::fs::read_to_string(&self.token_path)?;
        let tok: TraktToken = serde_json::from_str(&data)?;
        self.token = Some(tok);
        Ok(true)
    }

    // ── OAuth Device Flow ─────────────────────────────────────────────────────

    /// Step 1 of the Device Flow: request a device code.
    ///
    /// Display `response.user_code` and `response.verification_url` to the
    /// user so they can authorize the app in a browser.
    pub async fn request_device_code(&self) -> Result<DeviceCodeResponse> {
        let body = serde_json::json!({ "client_id": self.client_id });
        let resp = self
            .http
            .post(self.url("oauth/device/code"))
            .headers(self.base_headers())
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(SyncError::Api { status, body });
        }
        Ok(resp.json::<DeviceCodeResponse>().await?)
    }

    /// Step 2 of the Device Flow: poll until the user approves or the code
    /// expires.
    ///
    /// `interval_secs` is the minimum polling interval supplied by
    /// [`DeviceCodeResponse::interval`].  Pass `0` in tests to avoid sleeping.
    pub async fn poll_device_token(
        &mut self,
        device_code: &str,
        interval_secs: u64,
        expires_in: u64,
    ) -> Result<TraktToken> {
        let deadline = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + expires_in;

        let body = serde_json::json!({
            "code":          device_code,
            "client_id":     self.client_id,
            "client_secret": self.client_secret,
        });

        loop {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if now >= deadline {
                return Err(SyncError::DeviceFlowTimeout);
            }

            let resp = self
                .http
                .post(self.url("oauth/device/token"))
                .headers(self.base_headers())
                .json(&body)
                .send()
                .await?;

            match resp.status().as_u16() {
                200 => {
                    let tok: TraktToken = resp.json().await?;
                    self.save_token(&tok)?;
                    self.token = Some(tok.clone());
                    return Ok(tok);
                }
                400 => {
                    // "authorization_pending" or "slow_down"
                    let text = resp.text().await.unwrap_or_default();
                    info!("trakt: device flow pending: {}", text);
                    if interval_secs > 0 {
                        tokio::time::sleep(Duration::from_secs(interval_secs))
                            .await;
                    }
                }
                429 => {
                    // slow_down: back off an extra second
                    if interval_secs > 0 {
                        tokio::time::sleep(Duration::from_secs(
                            interval_secs + 1,
                        ))
                        .await;
                    }
                }
                status => {
                    let body = resp.text().await.unwrap_or_default();
                    return Err(SyncError::Api { status, body });
                }
            }
        }
    }

    // ── OAuth Authorization Code Flow ─────────────────────────────────────────

    /// Exchange an Authorization Code for an access token and persist it.
    ///
    /// `redirect_uri` must match the URI registered with Trakt and used when
    /// initiating the Authorization Code flow (e.g. the `/trakt_auth` callback
    /// URL). Saves the resulting token to disk and stores it in `self.token`.
    pub async fn exchange_code(
        &mut self,
        code: &str,
        redirect_uri: &str,
    ) -> Result<TraktToken> {
        let body = serde_json::json!({
            "code":          code,
            "client_id":     self.client_id,
            "client_secret": self.client_secret,
            "redirect_uri":  redirect_uri,
            "grant_type":    "authorization_code",
        });
        let resp = self
            .http
            .post(self.url("oauth/token"))
            .headers(self.base_headers())
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(SyncError::Api { status, body });
        }
        let token: TraktToken = resp.json().await?;
        self.save_token(&token)?;
        self.token = Some(token.clone());
        info!("trakt: authorization code exchanged for token");
        Ok(token)
    }

    /// Refresh the access token using the saved refresh token.
    pub async fn refresh_token(&mut self) -> Result<()> {
        let refresh = self
            .token
            .as_ref()
            .and_then(|t| {
                if t.refresh_token.is_empty() {
                    None
                } else {
                    Some(t.refresh_token.clone())
                }
            })
            .ok_or(SyncError::InvalidRefreshToken)?;

        let body = serde_json::json!({
            "refresh_token": refresh,
            "client_id":     self.client_id,
            "client_secret": self.client_secret,
            "redirect_uri":  "urn:ietf:wg:oauth:2.0:oob",
            "grant_type":    "refresh_token",
        });

        let resp = self
            .http
            .post(self.url("oauth/token"))
            .headers(self.base_headers())
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(SyncError::Api { status, body });
        }

        let tok: TraktToken = resp.json().await?;
        self.save_token(&tok)?;
        self.token = Some(tok);
        Ok(())
    }

    /// Ensure a valid access token is loaded, refreshing if necessary.
    ///
    /// Returns `Ok(true)` if auth is ready.  Returns `Ok(false)` if the user
    /// must complete the Device Flow interactively.
    pub async fn ensure_auth(&mut self) -> Result<bool> {
        if self.token.is_none() {
            self.load_token()?;
        }
        match &self.token {
            Some(tok) if tok.is_valid() => Ok(true),
            Some(_) => {
                // Expired; try to refresh.
                match self.refresh_token().await {
                    Ok(()) => Ok(true),
                    Err(e) => {
                        warn!("trakt: refresh failed: {e}");
                        Ok(false)
                    }
                }
            }
            None => Ok(false),
        }
    }

    // ── History & lookup ──────────────────────────────────────────────────────

    /// Add one or more movies/episodes to the user's watch history.
    ///
    /// Returns the raw JSON response from `POST /sync/history`.
    pub async fn add_to_history(
        &self,
        items: &[TraktHistoryItem],
    ) -> Result<serde_json::Value> {
        let mut movies: Vec<serde_json::Value> = Vec::new();
        let mut episodes: Vec<serde_json::Value> = Vec::new();

        for item in items {
            let mut obj = serde_json::json!({ "ids": item.ids });
            if let (Some(wa), Some(map)) =
                (&item.watched_at, obj.as_object_mut())
            {
                let _ = map.insert(
                    "watched_at".to_owned(),
                    serde_json::Value::String(wa.clone()),
                );
            }
            match item.kind {
                TraktItemKind::Movie => movies.push(obj),
                TraktItemKind::Episode => episodes.push(obj),
            }
        }

        let payload = serde_json::json!({
            "movies":   movies,
            "episodes": episodes,
        });

        let resp = self
            .http
            .post(self.url("sync/history"))
            .headers(self.auth_headers())
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(SyncError::Api { status, body });
        }
        Ok(resp.json().await?)
    }

    /// Query a user's watch history for a specific item.
    ///
    /// `item_type` is `"movie"`, `"episode"`, `"show"`, or `"season"`.
    /// `trakt_id` is the numeric Trakt ID.
    pub async fn get_watch_history(
        &self,
        item_type: &str,
        trakt_id: u64,
    ) -> Result<Vec<serde_json::Value>> {
        let path = format!(
            "users/{}/history/{}s/{}",
            self.user_id, item_type, trakt_id
        );
        let resp = self
            .http
            .get(self.url(&path))
            .headers(self.auth_headers())
            .send()
            .await?;

        if resp.status().as_u16() == 404 {
            return Ok(Vec::new());
        }
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(SyncError::Api { status, body });
        }
        Ok(resp.json().await?)
    }

    /// Look up an item by an external provider ID.
    ///
    /// `provider` is one of `"imdb"`, `"tmdb"`, `"tvdb"`, `"trakt"`.
    /// `item_type` narrows to `"movie"`, `"show"`, or `"episode"` when `Some`.
    pub async fn id_lookup(
        &self,
        provider: &str,
        id: &str,
        item_type: Option<&str>,
    ) -> Result<Vec<serde_json::Value>> {
        let mut url = self.url(&format!("search/{}/{}", provider, id));
        if let Some(t) = item_type {
            // imdb does not support type filtering
            if provider != "imdb" {
                url = format!("{}?type={}", url, t);
            }
        }
        let resp = self
            .http
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        if resp.status().as_u16() == 404 {
            return Ok(Vec::new());
        }
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(SyncError::Api { status, body });
        }
        Ok(resp.json().await?)
    }
}

// ── Sync orchestration ────────────────────────────────────────────────────────

/// Mark a list of episodes or movies as watched on Trakt.
///
/// Items already in the user's history are silently skipped.
/// Returns the number of items actually synced.
pub async fn sync_history(
    api: &TraktApi,
    items: Vec<TraktHistoryItem>,
) -> Result<usize> {
    if items.is_empty() {
        return Ok(0);
    }

    // Filter out items that already have a watch entry.
    let mut to_add: Vec<TraktHistoryItem> = Vec::new();
    for item in &items {
        if let Some(trakt_id) = item.ids.trakt {
            let kind_str = match item.kind {
                TraktItemKind::Movie => "movie",
                TraktItemKind::Episode => "episode",
            };
            let history = api.get_watch_history(kind_str, trakt_id).await?;
            if history.is_empty() {
                to_add.push(item.clone());
            } else {
                info!("trakt: already watched trakt_id={trakt_id}, skip");
            }
        } else {
            // No trakt_id to check; add unconditionally.
            to_add.push(item.clone());
        }
    }

    let count = to_add.len();
    if count > 0 {
        let result = api.add_to_history(&to_add).await?;
        info!("trakt: sync result: {result}");
    }
    Ok(count)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_token() -> TraktToken {
        TraktToken {
            access_token: "acc123".into(),
            token_type: "Bearer".into(),
            expires_in: 7_776_000,
            refresh_token: "ref456".into(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    async fn make_api(server: &MockServer) -> TraktApi {
        let tmp = NamedTempFile::new().unwrap();
        TraktApi::new("cid", "csec", "user1", tmp.path(), server.uri()).unwrap()
    }

    // ── TraktToken::is_valid ──────────────────────────────────────────────────

    #[test]
    fn token_is_valid_when_fresh() {
        let tok = test_token();
        assert!(tok.is_valid());
    }

    #[test]
    fn token_is_invalid_when_expired() {
        let tok = TraktToken {
            access_token: "x".into(),
            token_type: "Bearer".into(),
            expires_in: 0,
            refresh_token: "y".into(),
            created_at: 0,
        };
        assert!(!tok.is_valid());
    }

    // ── save / load token ─────────────────────────────────────────────────────

    #[test]
    fn save_and_load_token_roundtrip() {
        let tmp = NamedTempFile::new().unwrap();
        let mut api =
            TraktApi::new("c", "s", "u", tmp.path(), "http://x").unwrap();
        let tok = test_token();
        api.save_token(&tok).unwrap();
        assert!(api.load_token().unwrap());
        let loaded = api.token.as_ref().unwrap();
        assert_eq!(loaded.access_token, "acc123");
        assert_eq!(loaded.refresh_token, "ref456");
    }

    #[test]
    fn load_token_returns_false_when_missing() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().with_extension("nonexistent");
        let mut api = TraktApi::new("c", "s", "u", &path, "http://x").unwrap();
        assert!(!api.load_token().unwrap());
    }

    // ── request_device_code ───────────────────────────────────────────────────

    #[tokio::test]
    async fn request_device_code_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "device_code":      "dcode",
                    "user_code":        "ABCD-1234",
                    "verification_url": "https://trakt.tv/activate",
                    "expires_in":       600,
                    "interval":         5,
                }),
            ))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let resp = api.request_device_code().await.unwrap();
        assert_eq!(resp.device_code, "dcode");
        assert_eq!(resp.user_code, "ABCD-1234");
        assert_eq!(resp.expires_in, 600);
        assert_eq!(resp.interval, 5);
    }

    #[tokio::test]
    async fn request_device_code_propagates_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/device/code"))
            .respond_with(
                ResponseTemplate::new(403).set_body_string("Forbidden"),
            )
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        let err = api.request_device_code().await.unwrap_err();
        assert!(matches!(err, SyncError::Api { status: 403, .. }));
    }

    // ── poll_device_token ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn poll_device_token_succeeds_on_first_200() {
        let server = MockServer::start().await;
        let tok = test_token();
        Mock::given(method("POST"))
            .and(path("/oauth/device/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "access_token":  tok.access_token,
                    "token_type":    tok.token_type,
                    "expires_in":    tok.expires_in,
                    "refresh_token": tok.refresh_token,
                    "created_at":    tok.created_at,
                }),
            ))
            .mount(&server)
            .await;

        let mut api = make_api(&server).await;
        let result = api.poll_device_token("dcode", 0, 600).await.unwrap();
        assert_eq!(result.access_token, "acc123");
    }

    #[tokio::test]
    async fn poll_device_token_times_out() {
        let server = MockServer::start().await;
        // Never respond with success.
        Mock::given(method("POST"))
            .and(path("/oauth/device/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(
                serde_json::json!({"error": "authorization_pending"}),
            ))
            .mount(&server)
            .await;

        let mut api = make_api(&server).await;
        // expires_in=0 so the deadline is already past on first check.
        let err = api.poll_device_token("dcode", 0, 0).await.unwrap_err();
        assert!(matches!(err, SyncError::DeviceFlowTimeout));
    }

    // ── refresh_token ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn refresh_token_updates_stored_token() {
        let server = MockServer::start().await;
        let new_tok = TraktToken {
            access_token: "new_acc".into(),
            token_type: "Bearer".into(),
            expires_in: 7_776_000,
            refresh_token: "new_ref".into(),
            created_at: 1_000_000,
        };
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::to_value(&new_tok).unwrap()),
            )
            .mount(&server)
            .await;

        let mut api = make_api(&server).await;
        api.token = Some(test_token());
        api.refresh_token().await.unwrap();
        assert_eq!(api.token.as_ref().unwrap().access_token, "new_acc");
    }

    #[tokio::test]
    async fn refresh_token_fails_without_token() {
        let server = MockServer::start().await;
        let mut api = make_api(&server).await;
        let err = api.refresh_token().await.unwrap_err();
        assert!(matches!(err, SyncError::InvalidRefreshToken));
    }

    // ── add_to_history ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn add_to_history_posts_correct_payload() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sync/history"))
            .respond_with(ResponseTemplate::new(201).set_body_json(
                serde_json::json!({
                    "added":     { "movies": 1, "episodes": 0 },
                    "not_found": { "movies": [], "episodes": [] },
                }),
            ))
            .mount(&server)
            .await;

        let mut api = make_api(&server).await;
        api.token = Some(test_token());
        let items = vec![TraktHistoryItem {
            kind: TraktItemKind::Movie,
            ids: TraktIds {
                imdb: Some("tt1234567".into()),
                ..Default::default()
            },
            watched_at: None,
        }];
        let resp = api.add_to_history(&items).await.unwrap();
        let added_movies = resp
            .get("added")
            .and_then(|a| a.get("movies"))
            .and_then(|m| m.as_u64());
        assert_eq!(added_movies, Some(1));
    }

    // ── get_watch_history ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn get_watch_history_returns_list() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/user1/history/movies/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([
                    { "id": 1, "watched_at": "2024-01-01T00:00:00Z",
                      "action": "watch", "type": "movie" }
                ]),
            ))
            .mount(&server)
            .await;

        let mut api = make_api(&server).await;
        api.token = Some(test_token());
        let hist = api.get_watch_history("movie", 42).await.unwrap();
        assert_eq!(hist.len(), 1);
    }

    #[tokio::test]
    async fn get_watch_history_returns_empty_on_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/user1/history/movies/99"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let mut api = make_api(&server).await;
        api.token = Some(test_token());
        let hist = api.get_watch_history("movie", 99).await.unwrap();
        assert!(hist.is_empty());
    }

    // ── id_lookup ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn id_lookup_returns_results() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search/imdb/tt0000001"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([
                    { "type": "movie",
                      "score": 1000,
                      "movie": { "title": "Test", "year": 2000,
                                 "ids": { "trakt": 1, "imdb": "tt0000001" } } }
                ]),
            ))
            .mount(&server)
            .await;

        let mut api = make_api(&server).await;
        api.token = Some(test_token());
        let res = api.id_lookup("imdb", "tt0000001", None).await.unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res.first().and_then(|v| v["type"].as_str()), Some("movie"));
    }

    // ── sync_history ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn sync_history_skips_already_watched() {
        let server = MockServer::start().await;
        // History check returns existing entry.
        Mock::given(method("GET"))
            .and(path("/users/user1/history/episodes/7"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([{ "id": 99, "watched_at": "2024-01-01T00:00:00Z" }]),
            ))
            .mount(&server)
            .await;
        // /sync/history should NOT be called.

        let mut api = make_api(&server).await;
        api.token = Some(test_token());
        let items = vec![TraktHistoryItem {
            kind: TraktItemKind::Episode,
            ids: TraktIds {
                trakt: Some(7),
                ..Default::default()
            },
            watched_at: None,
        }];
        let count = sync_history(&api, items).await.unwrap();
        assert_eq!(count, 0);
    }
}
