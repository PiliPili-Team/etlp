//! Redirect URL cache with per-host TTL expiry.
//!
//! Ports `net_tools.get_redirect_url` caching and
//! `check_redirect_cache_expired_loop`. The cache key is the request URL up to
//! (but excluding) `PlaySessionId`, so the same item resolves to one entry
//! regardless of session id. Entries expire based on host-keyword TTL rules.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Derive the cache key: the URL truncated before `PlaySessionId`
/// (matching the Python `url.split(key_trim)[0]`).
#[must_use]
pub fn cache_key(url: &str) -> &str {
    match url.split_once("PlaySessionId") {
        Some((head, _)) => head,
        None => url,
    }
}

#[derive(Debug, Clone)]
struct Entry {
    target: String,
    inserted: Instant,
}

/// A thread-safe redirect cache. Cloning shares the same storage.
#[derive(Debug, Clone, Default)]
pub struct RedirectCache {
    inner: Arc<RwLock<HashMap<String, Entry>>>,
}

impl RedirectCache {
    /// Create an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a cached redirect target for `url`.
    #[must_use]
    pub fn get(&self, url: &str) -> Option<String> {
        let key = cache_key(url);
        self.inner
            .read()
            .ok()
            .and_then(|map| map.get(key).map(|e| e.target.clone()))
    }

    /// Cache `target` as the redirect for `url`, stamped at `Instant::now()`.
    pub fn insert(&self, url: &str, target: String) {
        self.insert_at(url, target, Instant::now());
    }

    /// Cache with an explicit insertion time (used in tests).
    pub fn insert_at(&self, url: &str, target: String, at: Instant) {
        if let Ok(mut map) = self.inner.write() {
            map.insert(
                cache_key(url).to_owned(),
                Entry {
                    target,
                    inserted: at,
                },
            );
        }
    }

    /// Number of cached entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().map(|m| m.len()).unwrap_or(0)
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Remove entries whose key contains a TTL rule keyword and whose age
    /// (`now - inserted`) exceeds that rule's duration. Entries not matching
    /// any rule are kept (the Python loop only ever expires matched hosts).
    ///
    /// Returns the number of entries removed.
    pub fn purge_expired(
        &self,
        rules: &[(String, Duration)],
        now: Instant,
    ) -> usize {
        let Ok(mut map) = self.inner.write() else {
            return 0;
        };
        let before = map.len();
        map.retain(|key, entry| {
            match rules.iter().find(|(kw, _)| key.contains(kw.as_str())) {
                Some((_, ttl)) => {
                    let age = now.saturating_duration_since(entry.inserted);
                    age <= *ttl
                }
                None => true,
            }
        });
        before - map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_truncates_before_play_session_id() {
        let url = "https://h/videos/1/stream.mkv?\
                   MediaSourceId=x&PlaySessionId=abc&api_key=k";
        assert_eq!(
            cache_key(url),
            "https://h/videos/1/stream.mkv?MediaSourceId=x&"
        );
        assert_eq!(cache_key("https://h/no-session"), "https://h/no-session");
    }

    #[test]
    fn get_and_insert_roundtrip_by_key() {
        let cache = RedirectCache::new();
        cache.insert(
            "https://pili.app/s?PlaySessionId=1",
            "https://cdn/real".to_owned(),
        );
        // Different PlaySessionId resolves to the same cached entry.
        assert_eq!(
            cache.get("https://pili.app/s?PlaySessionId=2"),
            Some("https://cdn/real".to_owned())
        );
        assert!(cache.get("https://other.app/s").is_none());
    }

    #[test]
    fn purge_removes_only_expired_matched_hosts() {
        let cache = RedirectCache::new();
        let t0 = Instant::now();
        cache.insert_at("https://pili.app/a?PlaySessionId=1", "x".into(), t0);
        cache.insert_at("https://bili.io/b?PlaySessionId=1", "y".into(), t0);
        cache.insert_at("https://keep.me/c?PlaySessionId=1", "z".into(), t0);

        let rules = vec![
            ("pili".to_owned(), Duration::from_secs(5)),
            ("bili".to_owned(), Duration::from_secs(3600)),
        ];
        let now = t0 + Duration::from_secs(10);
        let removed = cache.purge_expired(&rules, now);

        // pili expired (age 10s > 5s); bili kept (10s < 3600s);
        // keep.me matches no rule and is kept.
        assert_eq!(removed, 1);
        assert!(cache.get("https://pili.app/a?PlaySessionId=9").is_none());
        assert_eq!(
            cache.get("https://bili.io/b?PlaySessionId=9"),
            Some("y".to_owned())
        );
        assert_eq!(
            cache.get("https://keep.me/c?PlaySessionId=9"),
            Some("z".to_owned())
        );
    }
}
