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
/// (Trakt / Bangumi). Used unless the user overrides it in `dev.user_agent`.
pub const UA_ETLP: &str = "etlp";

/// `User-Agent` for prefetch background downloads (not user-configurable).
pub const UA_PREFETCH: &str = "etlp-prefetch";

/// `User-Agent` for active media downloads (not user-configurable).
pub const UA_DOWNLOAD: &str = "etlp-download";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_strings_are_stable() {
        assert_eq!(UA_ETLP, "etlp");
        assert_eq!(UA_PREFETCH, "etlp-prefetch");
        assert_eq!(UA_DOWNLOAD, "etlp-download");
    }
}
