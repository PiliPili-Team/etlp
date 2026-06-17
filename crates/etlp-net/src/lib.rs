//! HTTP client, redirect cache and progress write-back for etlp.
//!
//! Provides the IO-free building blocks ported from `net_tools.py` (proxy
//! parsing, User-Agent selection, [`url_tools`]) and the async
//! [`HttpClient`] built on `reqwest` + rustls. Redirect caching and progress
//! write-back land on top of these in later steps (see `docs/TODO.md`).

mod client;
pub mod url_tools;

pub use client::{HttpClient, HttpClientBuilder, NetError};

use thiserror::Error;

/// User-Agent used for the upstream media servers (pili/bili hosts), matching
/// the Python `embyToLocalPlayer/1.1` branch.
pub const UA_ETLP: &str = "embyToLocalPlayer/1.1";

/// User-Agent used for everything else (a generic desktop Chrome string),
/// matching the Python fallback branch.
pub const UA_BROWSER: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36";

/// Errors from proxy configuration parsing.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProxyError {
    /// SOCKS proxies are not supported (only HTTP), matching the Python
    /// `raise ValueError('only support http proxy')`.
    #[error("only http proxy is supported, got: {0}")]
    SocksUnsupported(String),
}

/// Select the User-Agent for a request URL.
///
/// Hosts containing `pili` or `bili` use the etlp UA so the upstream can
/// recognize the client; all others masquerade as a browser.
#[must_use]
pub fn user_agent_for(url: &str) -> &'static str {
    if url.contains("pili") || url.contains("bili") {
        UA_ETLP
    } else {
        UA_BROWSER
    }
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
    fn ua_selects_etlp_for_pili_and_bili() {
        assert_eq!(user_agent_for("https://v.pili.app/x"), UA_ETLP);
        assert_eq!(user_agent_for("https://push.bili.io/y"), UA_ETLP);
        assert_eq!(user_agent_for("https://media.example.com/z"), UA_BROWSER);
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
