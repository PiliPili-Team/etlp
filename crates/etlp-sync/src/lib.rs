//! Third-party watch-history sync for etlp: Trakt.tv and Bangumi (bgm.tv).
//!
//! # Trakt
//! [`trakt::TraktApi`] implements the OAuth 2.0 Device Flow and exposes
//! methods to add items to a user's watch history. [`trakt::sync_history`]
//! is the high-level orchestrator that skips already-watched items.
//!
//! # Bangumi
//! [`bangumi::BangumiApi`] authenticates with a personal Bearer token and
//! provides methods to search subjects, query episodes, and mark progress.
//! [`bangumi::sync_episode_by_bangumi_id`] is the entry point for the Emby
//! integration: it accepts a Bangumi subject ID from `ProviderIds.Bangumi`
//! and marks the given episode sort numbers as watched.

pub mod bangumi;
pub mod bangumi_map;
pub mod bangumi_web;
mod curl;
pub mod error;
pub mod tmdb;
pub mod trakt;

pub use bangumi::{
    BackfillCache, BackfillMarkOutcome, BangumiApi, BgmReadCache,
    CollectionState, SubjectCache, WebScrapeReq, mark_backfill_in_subject,
    new_bgm_read_cache, normalize_title, resolve_by_web_scrape_with_chain,
    sync_episode_by_bangumi_id, sync_episodes, sync_movie_subject,
};
pub use bangumi_map::{
    EpRange, MapError, MapProvider, SubjectMapping, match_mapping,
    parse_mapping, parse_mappings, strip_group_prefix,
};
pub use error::{Result, SyncError};
pub use tmdb::TmdbClient;
pub use trakt::{
    DeviceCodeResponse, ScrobbleAction, TraktApi, TraktEpisode,
    TraktHistoryItem, TraktIds, TraktItemKind, TraktToken, sync_history,
    trakt_authorize_url,
};

/// Proxy configuration forwarded from the app config to sync HTTP clients.
///
/// Construct via [`SyncProxy::new`] or [`SyncProxy::default`] (no proxy).
#[derive(Debug, Clone, Default)]
pub struct SyncProxy {
    /// HTTP proxy URL (e.g. `"http://127.0.0.1:6152"`).
    pub http: Option<String>,
    /// HTTPS proxy URL (e.g. `"http://127.0.0.1:6152"`).
    pub https: Option<String>,
    /// When `false`, all proxy settings are ignored and connections are direct.
    pub enabled: bool,
}

impl SyncProxy {
    /// Create a proxy config from the three config fields.
    #[must_use]
    pub fn new(
        http: Option<String>,
        https: Option<String>,
        enabled: bool,
    ) -> Self {
        Self {
            http,
            https,
            enabled,
        }
    }
}

/// Build a `reqwest::Client` with the given `timeout` and optional proxy.
///
/// When `proxy.enabled` is `false` or no proxy URLs are configured, a direct
/// client is returned. HTTPS traffic is routed via `proxy.https`; HTTP via
/// `proxy.http`; if only one is set, it covers both schemes.
pub(crate) fn build_http_client(
    timeout: std::time::Duration,
    proxy: &SyncProxy,
) -> reqwest::Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .user_agent(etlp_core::UA_ETLP)
        .timeout(timeout);

    if proxy.enabled {
        let ph = proxy.http.clone();
        let ps = proxy.https.clone();
        if ph.is_some() || ps.is_some() {
            tracing::debug!(
                http = ?ph.as_deref(),
                https = ?ps.as_deref(),
                "sync: proxy configured"
            );
            let custom = reqwest::Proxy::custom(move |url| {
                let candidate: Option<&str> = match url.scheme() {
                    "http" => ph.as_deref(),
                    _ => ps.as_deref().or(ph.as_deref()),
                };
                candidate.and_then(|u| url::Url::parse(u).ok())
            });
            builder = builder.proxy(custom);
        }
    }

    builder.build()
}
