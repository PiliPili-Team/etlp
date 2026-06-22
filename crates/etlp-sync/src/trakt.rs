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
use tracing::{debug, info, warn};

use crate::error::{Result, SyncError};

/// Log label for this provider's HTTP send/retry/response lines.
const DOMAIN: &str = "trakt";

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
    /// Refresh this many seconds before the real expiry, so a sync started just
    /// before the deadline still completes on the current token.
    ///
    /// Must stay well below a token's lifetime. Trakt cut access tokens from 90
    /// days to 24 hours in 2025; the old 7-day margin then exceeded the whole
    /// 24h lifetime, so `is_valid` was always false and every playback forced a
    /// refresh ("token expired, attempting refresh" on a token that had just
    /// been refreshed and saved).
    const EXPIRY_MARGIN_SECS: u64 = 300;

    /// Returns `true` while the access token is still usable (i.e. not yet
    /// within the small pre-expiry refresh buffer).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.created_at + self.expires_in > now + Self::EXPIRY_MARGIN_SECS
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

/// Locates an episode within a show by season and episode number.
///
/// Paired with the show's [`TraktIds`] this is the match Trakt resolves most
/// reliably — far more so than an episode's own external ids, which media
/// servers often omit or populate with the series-level id (yielding a 404).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct TraktEpisode {
    pub season: u32,
    pub number: u32,
}

/// One item to add to watch history or scrobble.
#[derive(Debug, Clone)]
pub struct TraktHistoryItem {
    pub kind: TraktItemKind,
    /// Provider ids. When `episode` is `Some`, these address the **show**;
    /// otherwise they address the item itself (movie or episode).
    pub ids: TraktIds,
    /// When `Some` (episodes only), `ids` are the show's and the episode is
    /// addressed by season/number. When `None`, `ids` address the item directly.
    pub episode: Option<TraktEpisode>,
    /// Optional RFC-3339 timestamp; if `None` the current time is used.
    pub watched_at: Option<String>,
}

/// Playback action reported to Trakt's scrobble endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrobbleAction {
    /// `POST /scrobble/pause` — save the playback position without ever marking
    /// the item watched, regardless of progress.
    Pause,
    /// `POST /scrobble/stop` — Trakt decides by progress: at ≥ 80 % it marks the
    /// item watched and writes history; between 1 % and 79 % it saves the
    /// position to `/sync/playback` for resume; below 1 % it returns 422.
    Stop,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build the Trakt OAuth authorization URL the user opens to grant access.
///
/// Free function (no client instance needed) so the GUI can construct the URL
/// straight from the configured `client_id` and `redirect_uri`. After approval
/// Trakt redirects to `redirect_uri` with a `?code` that `/trakt_auth` exchanges
/// for a token.
#[must_use]
pub fn trakt_authorize_url(client_id: &str, redirect_uri: &str) -> String {
    format!(
        "{}?response_type=code&client_id={}&redirect_uri={}",
        TraktApi::AUTHORIZE_URL,
        percent_encode(client_id),
        percent_encode(redirect_uri),
    )
}

/// Percent-encode `s` for use in a URL query value (RFC 3986 unreserved set).
///
/// Dependency-free and panic-free; used to assemble the OAuth authorize URL.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
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
    /// The official Trakt.tv REST API base URL.
    pub const DEFAULT_BASE_URL: &'static str = "https://api.trakt.tv";

    /// Base URL of the OAuth authorization page the user opens in a browser.
    pub const AUTHORIZE_URL: &'static str = "https://trakt.tv/oauth/authorize";

    /// Canonical filename for the persisted OAuth token.
    pub const TOKEN_FILE_NAME: &'static str = "trakt_token.json";

    /// `redirect_uri` used for the refresh-token grant (out-of-band).
    const REFRESH_REDIRECT_URI: &'static str = "urn:ietf:wg:oauth:2.0:oob";

    /// Create a new client.
    ///
    /// `base_url` is normally [`Self::DEFAULT_BASE_URL`]. Pass the address of a
    /// local mock server in tests.
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        user_id: impl Into<String>,
        token_path: impl AsRef<Path>,
        base_url: impl Into<String>,
    ) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(etlp_core::UA_ETLP)
            .timeout(Duration::from_secs(30))
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

    /// Build the Trakt OAuth authorization URL the user must open to grant
    /// access. After approval Trakt redirects to `redirect_uri` with a `?code`
    /// that the `/trakt_auth` callback exchanges for a token.
    ///
    /// Always points at the public `trakt.tv` host (not `base_url`, which may
    /// be a test mock) because the user opens it in a real browser.
    #[must_use]
    pub fn authorize_url(&self, redirect_uri: &str) -> String {
        trakt_authorize_url(&self.client_id, redirect_uri)
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
            "redirect_uri":  Self::REFRESH_REDIRECT_URI,
            "grant_type":    "refresh_token",
        });

        // Routed through the shared retry helper (not `send_logged`: this body
        // carries the client secret and must never be logged), so a transient
        // 504 from Trakt's edge does not abandon the refresh.
        let resp = crate::curl::send_retrying(
            DOMAIN,
            self.http
                .post(self.url("oauth/token"))
                .headers(self.base_headers())
                .json(&body),
        )
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
            let loaded = self.load_token()?;
            debug!(
                loaded,
                path = %self.token_path.display(),
                "trakt: load token from disk"
            );
        }
        match &self.token {
            Some(tok) if tok.is_valid() => {
                debug!("trakt: token valid");
                Ok(true)
            }
            Some(_) => {
                // Expired; try to refresh.
                debug!("trakt: token expired, attempting refresh");
                match self.refresh_token().await {
                    Ok(()) => {
                        debug!("trakt: token refreshed");
                        Ok(true)
                    }
                    Err(e) => {
                        warn!("trakt: refresh failed: {e}");
                        Ok(false)
                    }
                }
            }
            None => {
                debug!("trakt: no token available, interactive auth required");
                Ok(false)
            }
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
        let mut shows: Vec<serde_json::Value> = Vec::new();

        for item in items {
            // Show + season/number locator: the reliable episode match. Sent
            // under "shows" with the season/episode hierarchy.
            if let (TraktItemKind::Episode, Some(ep)) =
                (item.kind, item.episode)
            {
                let mut ep_obj = serde_json::json!({ "number": ep.number });
                if let (Some(wa), Some(map)) =
                    (&item.watched_at, ep_obj.as_object_mut())
                {
                    let _ = map.insert(
                        "watched_at".to_owned(),
                        serde_json::Value::String(wa.clone()),
                    );
                }
                shows.push(serde_json::json!({
                    "ids": item.ids,
                    "seasons": [{
                        "number": ep.season,
                        "episodes": [ep_obj],
                    }],
                }));
                continue;
            }

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

        debug!(
            movies = movies.len(),
            episodes = episodes.len(),
            shows = shows.len(),
            "trakt: POST /sync/history"
        );
        let payload = serde_json::json!({
            "movies":   movies,
            "episodes": episodes,
            "shows":    shows,
        });

        let resp = crate::curl::send_logged(
            DOMAIN,
            self.http
                .post(self.url("sync/history"))
                .headers(self.auth_headers())
                .json(&payload),
        )
        .await?;
        crate::curl::json_logged(DOMAIN, resp).await
    }

    /// Report playback for one movie/episode via the scrobble API.
    ///
    /// `progress` is the watched percentage (`0.0..=100.0`).
    /// [`ScrobbleAction::Stop`] at ≥ 80 % makes Trakt mark the item watched and
    /// add it to history; [`ScrobbleAction::Pause`] always leaves it in the
    /// in-progress / "currently watching" state so it surfaces under Up Next.
    ///
    /// A `409 Conflict` (an identical scrobble still in flight) is treated as a
    /// successful no-op rather than an error.
    pub async fn scrobble(
        &self,
        action: ScrobbleAction,
        item: &TraktHistoryItem,
        progress: f64,
    ) -> Result<serde_json::Value> {
        let path = match action {
            ScrobbleAction::Pause => "scrobble/pause",
            ScrobbleAction::Stop => "scrobble/stop",
        };
        let progress = progress.clamp(0.0, 100.0);
        // Show + season/number is Trakt's most reliable episode match; fall back
        // to the item's own ids for movies and id-only episodes.
        let payload = match (item.kind, item.episode) {
            (TraktItemKind::Episode, Some(ep)) => serde_json::json!({
                "show": { "ids": item.ids },
                "episode": { "season": ep.season, "number": ep.number },
                "progress": progress,
            }),

            (TraktItemKind::Movie, _) => serde_json::json!({
                "movie": { "ids": item.ids },
                "progress": progress,
            }),

            (TraktItemKind::Episode, None) => serde_json::json!({
                "episode": { "ids": item.ids },
                "progress": progress,
            }),
        };

        debug!(path, progress, "trakt: POST /{path}");
        let resp = crate::curl::send_logged(
            DOMAIN,
            self.http
                .post(self.url(path))
                .headers(self.auth_headers())
                .json(&payload),
        )
        .await?;
        let (status, body) = crate::curl::read_logged(DOMAIN, resp).await?;
        if status.as_u16() == 409 {
            debug!("trakt: scrobble conflict (already in flight), ignoring");
            return Ok(serde_json::json!({ "ignored": "conflict" }));
        }
        if !status.is_success() {
            return Err(SyncError::Api {
                status: status.as_u16(),
                body,
            });
        }
        serde_json::from_str(&body).map_err(SyncError::Json)
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
        let resp = crate::curl::send_logged(
            DOMAIN,
            self.http.get(self.url(&path)).headers(self.auth_headers()),
        )
        .await?;

        let (status, body) = crate::curl::read_logged(DOMAIN, resp).await?;
        if status.as_u16() == 404 {
            return Ok(Vec::new());
        }
        if !status.is_success() {
            return Err(SyncError::Api {
                status: status.as_u16(),
                body,
            });
        }
        serde_json::from_str(&body).map_err(SyncError::Json)
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
        let resp = crate::curl::send_logged(
            DOMAIN,
            self.http.get(&url).headers(self.auth_headers()),
        )
        .await?;

        let (status, body) = crate::curl::read_logged(DOMAIN, resp).await?;
        if status.as_u16() == 404 {
            return Ok(Vec::new());
        }
        if !status.is_success() {
            return Err(SyncError::Api {
                status: status.as_u16(),
                body,
            });
        }
        serde_json::from_str(&body).map_err(SyncError::Json)
    }

    /// Resolve the numeric Trakt id for a movie/episode from its external
    /// provider ids, returning `None` when none of them resolve.
    ///
    /// Tries `tvdb` → `tmdb` → `imdb` in turn (tvdb matches episodes most
    /// reliably). The resolved id lets [`sync_history`] de-duplicate the item
    /// against the user's existing watch history before re-adding it.
    pub async fn resolve_trakt_id(
        &self,
        kind: TraktItemKind,
        ids: &TraktIds,
    ) -> Option<u64> {
        let type_str = match kind {
            TraktItemKind::Movie => "movie",
            TraktItemKind::Episode => "episode",
        };
        let lookups = [
            ("tvdb", ids.tvdb.map(|v| v.to_string())),
            ("tmdb", ids.tmdb.map(|v| v.to_string())),
            ("imdb", ids.imdb.clone()),
        ];
        for (provider, id) in lookups {
            let Some(id) = id else {
                continue;
            };
            let Ok(results) =
                self.id_lookup(provider, &id, Some(type_str)).await
            else {
                continue;
            };
            for r in &results {
                if let Some(t) = r
                    .get(type_str)
                    .and_then(|o| o.get("ids"))
                    .and_then(|i| i.get("trakt"))
                    .and_then(serde_json::Value::as_u64)
                {
                    return Some(t);
                }
            }
        }
        None
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
    debug!(count = items.len(), "trakt: sync_history start");

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

    #[test]
    fn fresh_24h_token_is_valid() {
        // Trakt's 2025 switch to 24-hour access tokens: a just-issued one must
        // count as valid. The old 7-day refresh margin exceeded the 24h
        // lifetime and reported it expired, forcing a refresh on every check.
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let tok = TraktToken {
            access_token: "x".into(),
            token_type: "Bearer".into(),
            expires_in: 86_400,
            refresh_token: "y".into(),
            created_at: now,
        };
        assert!(tok.is_valid());
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

    // ── authorize_url ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn authorize_url_encodes_redirect() {
        let server = MockServer::start().await;
        let api = make_api(&server).await;
        let url = api.authorize_url("http://localhost:58000/trakt_auth");
        assert!(url.starts_with("https://trakt.tv/oauth/authorize?"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=cid"));
        // The redirect URI's reserved characters must be percent-encoded.
        assert!(url.contains(
            "redirect_uri=http%3A%2F%2Flocalhost%3A58000%2Ftrakt_auth"
        ));
    }

    #[test]
    fn percent_encode_preserves_unreserved() {
        assert_eq!(percent_encode("aZ09-_.~"), "aZ09-_.~");
        assert_eq!(percent_encode("a b/c"), "a%20b%2Fc");
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
    async fn requests_use_unified_user_agent() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/device/code"))
            .and(wiremock::matchers::header("user-agent", etlp_core::UA_ETLP))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "device_code":      "d",
                    "user_code":        "U",
                    "verification_url": "https://trakt.tv/activate",
                    "expires_in":       600,
                    "interval":         5,
                }),
            ))
            .mount(&server)
            .await;

        let api = make_api(&server).await;
        // The mock only matches when the request carried `User-Agent: etlp`,
        // so a non-error result proves the unified agent was sent.
        assert!(api.request_device_code().await.is_ok());
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
            episode: None,
            watched_at: None,
        }];
        let resp = api.add_to_history(&items).await.unwrap();
        let added_movies = resp
            .get("added")
            .and_then(|a| a.get("movies"))
            .and_then(|m| m.as_u64());
        assert_eq!(added_movies, Some(1));
    }

    // ── scrobble ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn scrobble_stop_posts_progress_to_stop_endpoint() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/scrobble/stop"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "progress": 100.0,
                "episode": { "ids": { "tvdb": 42 } },
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(
                serde_json::json!({ "action": "scrobble", "progress": 100.0 }),
            ))
            .mount(&server)
            .await;

        let mut api = make_api(&server).await;
        api.token = Some(test_token());
        let item = TraktHistoryItem {
            kind: TraktItemKind::Episode,
            ids: TraktIds {
                tvdb: Some(42),
                ..Default::default()
            },
            episode: None,
            watched_at: None,
        };
        let resp = api
            .scrobble(ScrobbleAction::Stop, &item, 100.0)
            .await
            .unwrap();
        assert_eq!(
            resp.get("action").and_then(|a| a.as_str()),
            Some("scrobble")
        );
    }

    #[tokio::test]
    async fn scrobble_pause_uses_pause_endpoint() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/scrobble/pause"))
            .respond_with(ResponseTemplate::new(201).set_body_json(
                serde_json::json!({ "action": "pause", "progress": 45.0 }),
            ))
            .mount(&server)
            .await;

        let mut api = make_api(&server).await;
        api.token = Some(test_token());
        let item = TraktHistoryItem {
            kind: TraktItemKind::Episode,
            ids: TraktIds {
                tvdb: Some(7),
                ..Default::default()
            },
            episode: None,
            watched_at: None,
        };
        let resp = api
            .scrobble(ScrobbleAction::Pause, &item, 45.0)
            .await
            .unwrap();
        assert_eq!(resp.get("action").and_then(|a| a.as_str()), Some("pause"));
    }

    #[tokio::test]
    async fn scrobble_conflict_is_treated_as_noop() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/scrobble/stop"))
            .respond_with(ResponseTemplate::new(409))
            .mount(&server)
            .await;

        let mut api = make_api(&server).await;
        api.token = Some(test_token());
        let item = TraktHistoryItem {
            kind: TraktItemKind::Movie,
            ids: TraktIds {
                imdb: Some("tt1".into()),
                ..Default::default()
            },
            episode: None,
            watched_at: None,
        };
        // A 409 (scrobble already in flight) must resolve to Ok, not an error.
        let resp = api
            .scrobble(ScrobbleAction::Stop, &item, 100.0)
            .await
            .unwrap();
        assert_eq!(
            resp.get("ignored").and_then(|v| v.as_str()),
            Some("conflict")
        );
    }

    #[tokio::test]
    async fn scrobble_episode_with_season_uses_show_format() {
        let server = MockServer::start().await;
        // An episode located by season/number must be sent as show + episode,
        // which is the match Trakt resolves reliably (no 404).
        Mock::given(method("POST"))
            .and(path("/scrobble/pause"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "progress": 35.0,
                "show": { "ids": { "tvdb": 121 } },
                "episode": { "season": 2, "number": 5 },
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(
                serde_json::json!({ "action": "pause", "progress": 35.0 }),
            ))
            .mount(&server)
            .await;

        let mut api = make_api(&server).await;
        api.token = Some(test_token());
        let item = TraktHistoryItem {
            kind: TraktItemKind::Episode,
            ids: TraktIds {
                tvdb: Some(121),
                ..Default::default()
            },
            episode: Some(TraktEpisode {
                season: 2,
                number: 5,
            }),
            watched_at: None,
        };
        let resp = api
            .scrobble(ScrobbleAction::Pause, &item, 35.0)
            .await
            .unwrap();
        assert_eq!(resp.get("action").and_then(|a| a.as_str()), Some("pause"));
    }

    #[tokio::test]
    async fn add_to_history_episode_with_season_uses_shows() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sync/history"))
            .and(wiremock::matchers::body_partial_json(serde_json::json!({
                "shows": [{
                    "ids": { "tvdb": 121 },
                    "seasons": [{
                        "number": 2,
                        "episodes": [{ "number": 5 }],
                    }],
                }],
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(
                serde_json::json!({
                    "added": { "movies": 0, "episodes": 1 },
                }),
            ))
            .mount(&server)
            .await;

        let mut api = make_api(&server).await;
        api.token = Some(test_token());
        let items = vec![TraktHistoryItem {
            kind: TraktItemKind::Episode,
            ids: TraktIds {
                tvdb: Some(121),
                ..Default::default()
            },
            episode: Some(TraktEpisode {
                season: 2,
                number: 5,
            }),
            watched_at: None,
        }];
        let resp = api.add_to_history(&items).await.unwrap();
        let added = resp
            .get("added")
            .and_then(|a| a.get("episodes"))
            .and_then(serde_json::Value::as_u64);
        assert_eq!(added, Some(1));
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

    // ── resolve_trakt_id ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn resolve_trakt_id_extracts_episode_id_from_tvdb() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search/tvdb/555"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!([
                    { "type": "episode",
                      "episode": { "season": 1, "number": 3,
                                   "ids": { "trakt": 42, "tvdb": 555 } },
                      "show": { "ids": { "trakt": 7 } } }
                ]),
            ))
            .mount(&server)
            .await;

        let mut api = make_api(&server).await;
        api.token = Some(test_token());
        let ids = TraktIds {
            tvdb: Some(555),
            ..Default::default()
        };
        let resolved = api.resolve_trakt_id(TraktItemKind::Episode, &ids).await;
        // The episode's own Trakt id is returned, not the show's.
        assert_eq!(resolved, Some(42));
    }

    #[tokio::test]
    async fn resolve_trakt_id_returns_none_without_ids() {
        let server = MockServer::start().await;
        let mut api = make_api(&server).await;
        api.token = Some(test_token());
        let resolved = api
            .resolve_trakt_id(TraktItemKind::Episode, &TraktIds::default())
            .await;
        assert_eq!(resolved, None);
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
            episode: None,
            watched_at: None,
        }];
        let count = sync_history(&api, items).await.unwrap();
        assert_eq!(count, 0);
    }
}
