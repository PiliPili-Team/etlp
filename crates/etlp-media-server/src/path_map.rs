//! Server-to-local path translation, ported from `tools.translate_path_by_ini`.
//!
//! Maps an Emby server-side file path to a local (mounted) path using the
//! ordered `[src]` / `[dst]` prefix pairs from the config. The first matching
//! `src` prefix wins. HTTP paths are returned unchanged.
//!
//! The `path_check` NFC/NFD existence probing is not reproduced here (it is a
//! filesystem-dependent refinement, off by default); only the prefix
//! substitution is performed.

/// Translate `file_path` using ordered `(src_prefix, dst_prefix)` pairs.
///
/// Returns the input unchanged when it is an HTTP URL or no prefix matches.
#[must_use]
pub fn translate_path(file_path: &str, pairs: &[(String, String)]) -> String {
    if file_path.starts_with("http") {
        return file_path.to_owned();
    }
    for (src, dst) in pairs {
        if !src.is_empty() && file_path.starts_with(src.as_str()) {
            return file_path.replacen(src.as_str(), dst, 1);
        }
    }
    file_path.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs() -> Vec<(String, String)> {
        vec![
            ("/mnt/disk1".to_owned(), "E:".to_owned()),
            ("/mnt/disk2/media".to_owned(), "F:/media".to_owned()),
        ]
    }

    #[test]
    fn translates_first_matching_prefix() {
        assert_eq!(
            translate_path("/mnt/disk1/movie/a.mkv", &pairs()),
            "E:/movie/a.mkv"
        );
        assert_eq!(
            translate_path("/mnt/disk2/media/show/b.mkv", &pairs()),
            "F:/media/show/b.mkv"
        );
    }

    #[test]
    fn unmatched_and_http_unchanged() {
        assert_eq!(translate_path("/other/c.mkv", &pairs()), "/other/c.mkv");
        assert_eq!(
            translate_path("https://h/stream.mkv", &pairs()),
            "https://h/stream.mkv"
        );
    }

    #[test]
    fn only_first_occurrence_replaced() {
        let p = vec![("/a".to_owned(), "/x".to_owned())];
        assert_eq!(translate_path("/a/a/file", &p), "/x/a/file");
    }
}
