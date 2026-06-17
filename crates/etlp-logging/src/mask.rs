//! Log masking, ported from the Python `MyLogger` mixing behavior.
//!
//! Sensitive values that appear in log lines—the API key, the server host, and
//! the OS user name—are replaced with placeholders so logs can be shared
//! safely. The host is half-redacted (keeping the leading half plus the port)
//! to remain recognizable; mirrors `MyLogger.mix_host_gen`.

use std::sync::{Arc, RwLock};

const HIDE_API_KEY: &str = "_hide_api_key_";
const HIDE_USER: &str = "_hide_user_";

/// Build the half-redacted host placeholder, e.g. `example.com:8096`
/// becomes `exa_mix_host_:8096`.
#[must_use]
pub fn mix_host_gen(netloc: &str) -> String {
    let (host, port) = match netloc.split_once(':') {
        Some((h, p)) => (h, format!(":{p}")),
        None => (netloc, String::new()),
    };
    let keep: String = host.chars().take(host.chars().count() / 2).collect();
    format!("{keep}_mix_host_{port}")
}

/// The set of substrings to redact and their replacements.
#[derive(Debug, Clone, Default)]
struct Rules {
    /// When false, masking is a no-op (the `dev.mix_log = no` case).
    enabled: bool,
    api_key: String,
    netloc: String,
    netloc_replace: String,
    user_name: String,
}

impl Rules {
    fn apply(&self, input: &str) -> String {
        if !self.enabled {
            return input.to_owned();
        }
        let mut out = input.to_owned();
        if !self.api_key.is_empty() {
            out = out.replace(&self.api_key, HIDE_API_KEY);
        }
        if !self.netloc.is_empty() {
            out = out.replace(&self.netloc, &self.netloc_replace);
        }
        if !self.user_name.is_empty() {
            out = out.replace(&self.user_name, HIDE_USER);
        }
        out
    }
}

/// A shareable, mutable masking ruleset.
///
/// Cloning shares the same underlying rules (it is `Arc`-backed), so a logging
/// layer and request handlers see the same redaction settings.
#[derive(Debug, Clone)]
pub struct Masker {
    rules: Arc<RwLock<Rules>>,
}

impl Default for Masker {
    fn default() -> Self {
        Self::new(true)
    }
}

impl Masker {
    /// Create a masker. `enabled` corresponds to `dev.mix_log`.
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self {
            rules: Arc::new(RwLock::new(Rules {
                enabled,
                ..Rules::default()
            })),
        }
    }

    /// Update the per-request secrets (the Python `logger_setup`).
    pub fn set_request(&self, api_key: &str, netloc: &str) {
        if let Ok(mut rules) = self.rules.write() {
            rules.api_key = api_key.to_owned();
            rules.netloc = netloc.to_owned();
            rules.netloc_replace = mix_host_gen(netloc);
        }
    }

    /// Set the OS user name to redact.
    pub fn set_user(&self, user_name: &str) {
        if let Ok(mut rules) = self.rules.write() {
            rules.user_name = user_name.to_owned();
        }
    }

    /// Apply masking to a log line. On lock poisoning, returns the input
    /// unchanged rather than panicking (logging must never crash the process).
    #[must_use]
    pub fn mask(&self, input: &str) -> String {
        match self.rules.read() {
            Ok(rules) => rules.apply(input),
            Err(_) => input.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_host_keeps_leading_half_and_port() {
        assert_eq!(mix_host_gen("example.com:8096"), "examp_mix_host_:8096");
        assert_eq!(mix_host_gen("abcd"), "ab_mix_host_");
        assert_eq!(mix_host_gen("1.2.3.4:443"), "1.2_mix_host_:443");
    }

    #[test]
    fn masking_redacts_all_secrets() {
        let m = Masker::new(true);
        m.set_request("SECRETKEY", "media.example.com:8096");
        m.set_user("alice");
        let line = "user alice hit https://media.example.com:8096/?\
                    api_key=SECRETKEY";
        let masked = m.mask(line);
        assert!(!masked.contains("SECRETKEY"));
        assert!(!masked.contains("alice"));
        assert!(!masked.contains("media.example.com:8096"));
        assert!(masked.contains("_hide_api_key_"));
        assert!(masked.contains("_hide_user_"));
        assert!(masked.contains("_mix_host_"));
    }

    #[test]
    fn disabled_masker_is_identity() {
        let m = Masker::new(false);
        m.set_request("SECRETKEY", "media.example.com:8096");
        assert_eq!(m.mask("api_key=SECRETKEY"), "api_key=SECRETKEY");
    }

    #[test]
    fn empty_rules_leave_input_untouched() {
        let m = Masker::new(true);
        assert_eq!(m.mask("nothing to hide"), "nothing to hide");
    }
}
