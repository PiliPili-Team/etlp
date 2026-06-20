//! HTTP client, redirect cache and progress write-back for etlp.
//!
//! Provides proxy parsing, User-Agent constants, [`url_tools`], and the async
//! [`HttpClient`] built on `reqwest` + rustls.

mod client;
pub mod progress;
mod redirect;
pub mod url_tools;

pub use client::{HttpClient, HttpClientBuilder, NetError};
pub use progress::{
    PlaybackEvent, mark_as_played, realtime_progress, update_progress,
};
pub use redirect::{RedirectCache, cache_key};

use thiserror::Error;

// User-Agent constants live in `etlp-core` so the sync layer (and any other
// crate that does not depend on `etlp-net`) can share them; re-exported here so
// existing `etlp_net::UA_*` references keep working.
pub use etlp_core::{UA_DOWNLOAD, UA_ETLP, UA_PREFETCH};

/// `X-Emby-Client` / `X-Emby-Device-Name` header value sent with every
/// Emby / Jellyfin progress report.
pub const DEVICE_NAME: &str = "Genshin";

/// Errors from proxy configuration parsing.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProxyError {
    /// SOCKS proxies are not supported (only HTTP).
    #[error("only http proxy is supported, got: {0}")]
    SocksUnsupported(String),
}

/// Normalize an HTTP proxy string the way `Configs._get_proxy` does:
///
/// * empty input yields `None` (no proxy);
/// * a `socks*` proxy is rejected;
/// * any `scheme://` prefix is stripped, leaving `host:port`.
pub fn parse_http_proxy(raw: &str) -> Result<Option<String>, ProxyError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    if raw.to_lowercase().contains("socks") {
        return Err(ProxyError::SocksUnsupported(raw.to_owned()));
    }
    let host_port = match raw.split_once("://") {
        Some((_scheme, rest)) => rest,
        None => raw,
    };
    Ok(Some(host_port.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ua_constants_are_correct() {
        assert_eq!(UA_ETLP, "etlp");
        assert_eq!(UA_PREFETCH, "etlp-prefetch");
        assert_eq!(UA_DOWNLOAD, "etlp-download");
    }

    #[test]
    fn proxy_empty_is_none() {
        assert_eq!(parse_http_proxy("   "), Ok(None));
        assert_eq!(parse_http_proxy(""), Ok(None));
    }

    #[test]
    fn proxy_strips_scheme() {
        assert_eq!(
            parse_http_proxy("http://127.0.0.1:7890"),
            Ok(Some("127.0.0.1:7890".to_owned()))
        );
        assert_eq!(
            parse_http_proxy("127.0.0.1:7890"),
            Ok(Some("127.0.0.1:7890".to_owned()))
        );
    }

    #[test]
    fn proxy_rejects_socks() {
        assert!(matches!(
            parse_http_proxy("socks5://127.0.0.1:1080"),
            Err(ProxyError::SocksUnsupported(_))
        ));
    }
}
