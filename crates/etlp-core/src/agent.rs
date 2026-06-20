//! Shared `User-Agent` identities for outbound HTTP requests.
//!
//! These live in `etlp-core` (the workspace's dependency-free foundation) so
//! every layer — the HTTP client, the downloader, and the third-party sync
//! crate — can reference the same strings without depending on `etlp-net`.
//!
//! Policy: normal requests use [`UA_ETLP`]; only background prefetch and active
//! media downloads use their own dedicated agents so a server can rate-limit or
//! distinguish that traffic.

/// Default `User-Agent` for all normal requests, including third-party sync
/// (Trakt / Bangumi). Follows the standard `Product/Version` form, e.g.
/// `etlp/0.0.1`. Used unless the user overrides it in `dev.user_agent`.
pub const UA_ETLP: &str = concat!("etlp/", env!("CARGO_PKG_VERSION"));

/// `User-Agent` for prefetch background downloads (not user-configurable).
pub const UA_PREFETCH: &str =
    concat!("etlp-prefetch/", env!("CARGO_PKG_VERSION"));

/// `User-Agent` for active media downloads (not user-configurable).
pub const UA_DOWNLOAD: &str =
    concat!("etlp-download/", env!("CARGO_PKG_VERSION"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agents_follow_product_version_form() {
        // `Product/Version`, e.g. "etlp/0.0.1".
        let version = env!("CARGO_PKG_VERSION");
        assert_eq!(UA_ETLP, format!("etlp/{version}"));
        assert_eq!(UA_PREFETCH, format!("etlp-prefetch/{version}"));
        assert_eq!(UA_DOWNLOAD, format!("etlp-download/{version}"));
        assert!(UA_ETLP.starts_with("etlp/"));
    }
}
