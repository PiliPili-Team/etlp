//! HTTP client, redirect cache and progress write-back for etlp.
//!
//! Provides proxy parsing, User-Agent constants, [`url_tools`], and the async
//! [`HttpClient`] built on `reqwest` + rustls.

mod client;
pub mod progress;
mod redirect;
pub mod url_tools;

pub use client::{HttpClient, HttpClientBuilder, NetError};
pub use progress::{PlaybackEvent, realtime_progress, update_progress};
pub use redirect::{RedirectCache, cache_key};

use thiserror::Error;

/// Default User-Agent for all etlp HTTP requests.
pub const UA_ETLP: &str = "etlp";

/// User-Agent for prefetch background downloads.
pub const UA_PREFETCH: &str = "etlp-prefetch";

/// User-Agent for active media downloads.
pub const UA_DOWNLOAD: &str = "etlp-download";

/// Errors from proxy configuration parsing.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProxyError {
    /// SOCKS proxies are not supported (only HTTP).
    #[error("only http proxy is supported, got: {0}")]
    SocksUnsupported(String),
}

/// Returns the default etlp User-Agent regardless of URL.
///
/// All requests use `UA_ETLP` ("etlp") by default. Use [`UA_PREFETCH`] or
/// [`UA_DOWNLOAD`] when constructing specialised clients for those purposes.
#[must_use]
pub fn user_agent_for(_url: &str) -> &'static str {
    UA_ETLP
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
    fn ua_always_returns_etlp() {
        assert_eq!(user_agent_for("https://v.pili.app/x"), UA_ETLP);
        assert_eq!(user_agent_for("https://push.bili.io/y"), UA_ETLP);
        assert_eq!(user_agent_for("https://media.example.com/z"), UA_ETLP);
    }

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
