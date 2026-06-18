//! Background prefetch of Emby "continue watching" items.
//!
//! Downloads the first 5 % and last 2 % of each resume item into the OS temp
//! directory so that subsequent playback can start with minimal buffering.
//! All errors are logged and silently skipped — prefetch is best-effort.
//! Uses the `etlp-prefetch` User-Agent to distinguish prefetch traffic.

use std::path::PathBuf;

use etlp_media_server::EmbyClient;
use tracing::{error, info, warn};

use crate::downloader::Downloader;

/// Pre-warm the first 5 % and last 2 % of every Emby resume item.
///
/// Uses the OS temp directory (`$TMPDIR/etlp-prefetch/`) as scratch space;
/// files are deleted after each item. Pass `speed_bps = 0` for unlimited.
pub async fn prefetch_resume_tv(
    emby: &EmbyClient,
    http: reqwest::Client,
    speed_bps: u64,
) {
    let cache_path = std::env::temp_dir().join("etlp-prefetch");
    if let Err(e) = std::fs::create_dir_all(&cache_path) {
        error!("prefetch: cannot create temp dir: {e}");
        return;
    }

    let list = match emby.resume_items().await {
        Ok(l) => l,
        Err(e) => {
            error!("prefetch: resume_items: {e}");
            return;
        }
    };
    info!("prefetch: {} resume items", list.items.len());

    for item in &list.items {
        let container = item
            .media_sources
            .first()
            .and_then(|s| s.container.as_deref());
        let url = emby.stream_url_for_item(&item.id, container, None);
        let id = format!("prefetch-{}", item.id);

        let mut dl = match Downloader::new(
            url,
            id.clone(),
            http.clone(),
            &cache_path,
            Some(&tmp_save_path(&cache_path, &item.id)),
        ) {
            Ok(d) => d,
            Err(e) => {
                warn!("prefetch: init {}: {e}", item.id);
                continue;
            }
        };

        if let Err(e) = dl.percent_download(0.0, 0.05, speed_bps, false).await {
            warn!("prefetch: head 5% {}: {e}", item.id);
        }
        if let Err(e) = dl.percent_download(0.98, 1.0, speed_bps, false).await {
            warn!("prefetch: tail 2% {}: {e}", item.id);
        }

        // Remove scratch data; ignore errors (file may not exist on /dev/null-like paths).
        dl.cancel_download().await;
        info!("prefetch: warmed {}", item.id);
    }

    let _ = std::fs::remove_dir_all(&cache_path);
}

fn tmp_save_path(base: &std::path::Path, item_id: &str) -> PathBuf {
    base.join(format!("data-{item_id}"))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use etlp_net::HttpClient;

    use super::*;

    #[tokio::test]
    async fn prefetch_no_items_is_noop() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/emby/Users/U/Items/Resume"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "Items": [] })),
            )
            .mount(&server)
            .await;

        let http = HttpClient::new().expect("client");
        let emby = EmbyClient::new(http, server.uri(), "KEY", "U");
        prefetch_resume_tv(&emby, reqwest::Client::new(), 0).await;
    }

    #[tokio::test]
    async fn prefetch_head_failure_is_skipped() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/emby/Users/U/Items/Resume"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "Items": [{"Id": "42", "Name": "Ep1", "MediaSources": []}]
                }),
            ))
            .mount(&server)
            .await;
        // No HEAD or GET mock for the stream URL → prefetch must not panic.

        let http = HttpClient::new().expect("client");
        let emby = EmbyClient::new(http, server.uri(), "KEY", "U");
        prefetch_resume_tv(&emby, reqwest::Client::new(), 0).await;
    }

    #[test]
    fn stream_url_for_item_format() {
        let http = HttpClient::new().expect("client");
        let emby = EmbyClient::new(http, "http://srv:8096", "MYKEY", "U");
        let url = emby.stream_url_for_item("99", Some("mkv"), None);
        assert!(url.starts_with("http://srv:8096/emby/Videos/99/stream.mkv"));
        assert!(url.contains("api_key=MYKEY"));
        assert!(url.contains("Static=true"));
    }

    #[test]
    fn stream_url_defaults_to_mkv() {
        let http = HttpClient::new().expect("client");
        let emby = EmbyClient::new(http, "http://srv:8096", "K", "U");
        let url = emby.stream_url_for_item("1", None, None);
        assert!(url.contains("stream.mkv"));
    }
}
