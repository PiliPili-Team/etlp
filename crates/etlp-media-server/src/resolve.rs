//! Stream/disk-mode resolution, ported from the heart of
//! `parse_received_data_emby` (the strm / direct-play / read-disk decision and
//! the resulting `media_path`).
//!
//! This is the trickiest part of payload parsing, so it is isolated as a pure
//! function with exhaustive tests. The redirect/stream-url rewriting that the
//! Python does inline (network + config) is handled by the async caller; this
//! function takes the already-built `stream_url`.

use crate::path_map::translate_path;

/// Inputs needed to decide how to play a source.
#[derive(Debug, Clone)]
pub struct ResolveInput<'a> {
    /// `media_source_info['Path']` — for strm this is the URL/text inside it.
    pub source_path: &'a str,
    /// `main_ep_info['Path']`.
    pub main_ep_path: &'a str,
    /// `main_ep_info['Type']` (e.g. `"TvChannel"`).
    pub item_type: Option<&'a str>,
    /// `media_source_info['Container']`.
    pub container: Option<&'a str>,
    /// `mountDiskEnable == "true"`.
    pub mount_disk_enable: bool,
    /// Whether `dev.strm_direct_host` matched the server.
    pub strm_direct: bool,
    /// Already-built stream URL (Emby transcode/direct URL).
    pub stream_url: &'a str,
    /// `dev.force_disk_mode_path` prefixes.
    pub force_disk_prefixes: &'a [String],
    /// `[src]`/`[dst]` path translation pairs.
    pub path_pairs: &'a [(String, String)],
}

/// The resolved play decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamResolution {
    pub file_path: String,
    pub is_strm: bool,
    pub is_http_source: bool,
    pub is_http_direct_strm: bool,
    pub mount_disk_mode: bool,
    pub media_path: String,
}

/// `force_disk_mode_by_path`: whether the path starts with any configured
/// force-disk prefix.
fn force_disk_mode_by_path(file_path: &str, prefixes: &[String]) -> bool {
    prefixes
        .iter()
        .any(|p| !p.is_empty() && file_path.starts_with(p.as_str()))
}

/// Resolve the play mode and `media_path` for a source.
#[must_use]
pub fn resolve_stream(input: &ResolveInput) -> StreamResolution {
    let source_path = input.source_path;
    // file_path: live channels use the source path, else the item path.
    let initial = if input.item_type == Some("TvChannel") {
        source_path
    } else {
        input.main_ep_path
    };
    let is_strm = (initial != source_path && initial.ends_with(".strm"))
        || input.container == Some("strm");
    let is_http_source = source_path.starts_with("http");

    // Keep the item path only for an http strm; otherwise use the source path.
    // (`!is_strm || (is_strm && !is_http_source)` simplifies to the below.)
    let file_path = if !is_strm || !is_http_source {
        source_path.to_owned()
    } else {
        initial.to_owned()
    };

    let mut mount_disk_mode = input.mount_disk_enable;
    if (is_strm && !input.strm_direct) || is_http_source {
        mount_disk_mode = false;
    }
    if !is_http_source
        && force_disk_mode_by_path(&file_path, input.force_disk_prefixes)
    {
        mount_disk_mode = true;
    }
    if is_strm && !is_http_source && input.strm_direct {
        mount_disk_mode = true;
    }
    let is_http_direct_strm = is_strm && input.strm_direct && is_http_source;

    let media_path = if mount_disk_mode {
        if is_strm {
            if input.strm_direct {
                translate_path(source_path, input.path_pairs)
            } else {
                // strm files can't be read from disk directly.
                mount_disk_mode = false;
                input.stream_url.to_owned()
            }
        } else {
            translate_path(&file_path, input.path_pairs)
        }
    } else if is_strm && input.strm_direct && !is_http_direct_strm {
        source_path.to_owned()
    } else {
        input.stream_url.to_owned()
    };

    StreamResolution {
        file_path,
        is_strm,
        is_http_source,
        is_http_direct_strm,
        mount_disk_mode,
        media_path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base<'a>(
        source_path: &'a str,
        main_ep_path: &'a str,
        stream_url: &'a str,
    ) -> ResolveInput<'a> {
        ResolveInput {
            source_path,
            main_ep_path,
            item_type: None,
            container: None,
            mount_disk_enable: false,
            strm_direct: false,
            stream_url,
            force_disk_prefixes: &[],
            path_pairs: &[],
        }
    }

    #[test]
    fn plain_file_network_play_uses_stream_url() {
        let input = base("/m/a.mkv", "/m/a.mkv", "https://h/stream.mkv");
        let r = resolve_stream(&input);
        assert!(!r.is_strm);
        assert!(!r.is_http_source);
        assert!(!r.mount_disk_mode);
        assert_eq!(r.file_path, "/m/a.mkv");
        assert_eq!(r.media_path, "https://h/stream.mkv");
    }

    #[test]
    fn plain_file_disk_mode_translates_path() {
        let pairs = vec![("/mnt/d1".to_owned(), "E:".to_owned())];
        let mut input =
            base("/mnt/d1/a.mkv", "/mnt/d1/a.mkv", "https://h/stream.mkv");
        input.mount_disk_enable = true;
        input.path_pairs = &pairs;
        let r = resolve_stream(&input);
        assert!(r.mount_disk_mode);
        assert_eq!(r.media_path, "E:/a.mkv");
    }

    #[test]
    fn http_strm_without_direct_plays_via_stream_url() {
        // strm whose source is an http url; not direct -> network play.
        let mut input =
            base("https://cdn/real.mkv", "/m/a.strm", "https://h/stream.mkv");
        input.container = Some("strm");
        let r = resolve_stream(&input);
        assert!(r.is_strm);
        assert!(r.is_http_source);
        assert!(!r.mount_disk_mode);
        // http strm keeps the item path as file_path.
        assert_eq!(r.file_path, "/m/a.strm");
        assert_eq!(r.media_path, "https://h/stream.mkv");
    }

    #[test]
    fn http_strm_direct_uses_source_url() {
        let mut input =
            base("https://cdn/real.mkv", "/m/a.strm", "https://h/stream.mkv");
        input.container = Some("strm");
        input.strm_direct = true;
        let r = resolve_stream(&input);
        assert!(r.is_http_direct_strm);
        assert!(!r.mount_disk_mode);
        // direct http strm -> media_path is the stream_url (set to source by
        // the caller earlier); here is_http_direct_strm short-circuits the
        // source_path branch, so stream_url is used.
        assert_eq!(r.media_path, "https://h/stream.mkv");
    }

    #[test]
    fn local_strm_direct_reads_disk() {
        let pairs = vec![("/mnt".to_owned(), "E:".to_owned())];
        let mut input =
            base("/mnt/real.mkv", "/m/a.strm", "https://h/stream.mkv");
        input.container = Some("strm");
        input.strm_direct = true;
        input.path_pairs = &pairs;
        let r = resolve_stream(&input);
        assert!(r.is_strm);
        assert!(!r.is_http_source);
        assert!(r.mount_disk_mode);
        assert_eq!(r.media_path, "E:/real.mkv");
    }

    #[test]
    fn local_strm_without_direct_falls_back_to_network() {
        // local strm, disk enabled, but not direct -> can't read, network.
        let mut input =
            base("/mnt/real.mkv", "/m/a.strm", "https://h/stream.mkv");
        input.container = Some("strm");
        input.mount_disk_enable = true;
        let r = resolve_stream(&input);
        assert!(!r.mount_disk_mode);
        assert_eq!(r.media_path, "https://h/stream.mkv");
    }

    #[test]
    fn force_disk_prefix_forces_disk_mode() {
        let prefixes = vec!["/disk/p".to_owned()];
        let pairs = vec![("/disk/p".to_owned(), "P:".to_owned())];
        let mut input =
            base("/disk/p/a.mkv", "/disk/p/a.mkv", "https://h/s.mkv");
        input.force_disk_prefixes = &prefixes;
        input.path_pairs = &pairs;
        let r = resolve_stream(&input);
        assert!(r.mount_disk_mode);
        assert_eq!(r.media_path, "P:/a.mkv");
    }
}
