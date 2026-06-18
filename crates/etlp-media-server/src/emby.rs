//! Thin Emby/Jellyfin API client.
//!
//! Covers the endpoints used during playback: `PlaybackInfo` (used to backfill
//! strm runtime) and the "continue watching" Resume list. Authentication is via
//! the `X-Emby-Token` query parameter, which Emby accepts for these endpoints.

use etlp_net::{HttpClient, NetError};

use crate::dto::{ItemList, PlaybackInfo};

/// A lightweight Emby client bound to one server + credentials.
#[derive(Debug, Clone)]
pub struct EmbyClient {
    http: HttpClient,
    host: String,
    api_key: String,
    user_id: String,
}

impl EmbyClient {
    /// Create a client. `host` should be the server origin (the Python code
    /// strips any trailing `/web/index...`); a trailing slash is trimmed.
    #[must_use]
    pub fn new(
        http: HttpClient,
        host: impl Into<String>,
        api_key: impl Into<String>,
        user_id: impl Into<String>,
    ) -> Self {
        let host = host.into().trim_end_matches('/').to_owned();
        Self {
            http,
            host,
            api_key: api_key.into(),
            user_id: user_id.into(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/emby/{path}", self.host)
    }

    /// `Items/{item_id}/PlaybackInfo` — fetch the (possibly backfilled) media
    /// sources for an item.
    pub async fn playback_info(
        &self,
        item_id: &str,
    ) -> Result<PlaybackInfo, NetError> {
        let url = self.url(&format!("Items/{item_id}/PlaybackInfo"));
        self.http
            .get_json(&url, &[("X-Emby-Token", self.api_key.as_str())])
            .await
    }

    /// `Shows/{show_id}/Episodes` — the episodes of a season.
    ///
    /// Mirrors `list_episodes`: `show_id` is the season id (falling back to the
    /// series id when the season is unknown), and `season_id` is passed as a
    /// query filter only when it is known.
    pub async fn episodes(
        &self,
        show_id: &str,
        season_id: Option<&str>,
    ) -> Result<ItemList, NetError> {
        let url = self.url(&format!("Shows/{show_id}/Episodes"));
        let mut params = vec![
            ("Fields", "MediaSources,Path,ProviderIds"),
            ("X-Emby-Token", self.api_key.as_str()),
        ];
        if let Some(season_id) = season_id {
            params.push(("SeasonId", season_id));
        }
        self.http.get_json(&url, &params).await
    }

    /// Build the direct-stream URL for an item.
    ///
    /// `container` defaults to `"mkv"` when absent. If `media_source_id` is
    /// provided, it is appended as `MediaSourceId=`.
    #[must_use]
    pub fn stream_url_for_item(
        &self,
        item_id: &str,
        container: Option<&str>,
        media_source_id: Option<&str>,
    ) -> String {
        let ext = container.unwrap_or("mkv");
        let base = format!(
            "{}/emby/Videos/{item_id}/stream.{ext}?api_key={}&Static=true",
            self.host, self.api_key
        );
        match media_source_id {
            Some(id) if !id.is_empty() => format!("{base}&MediaSourceId={id}"),
            _ => base,
        }
    }

    /// `Users/{user_id}/Items/Resume` — the "continue watching" list.
    pub async fn resume_items(&self) -> Result<ItemList, NetError> {
        let url = self.url(&format!("Users/{}/Items/Resume", self.user_id));
        self.http
            .get_json(
                &url,
                &[
                    ("Fields", "MediaStreams,PremiereDate,Path"),
                    ("MediaTypes", "Video"),
                    ("Limit", "12"),
                    ("X-Emby-Token", self.api_key.as_str()),
                ],
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[tokio::test]
    async fn playback_info_parses_media_sources() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/emby/Items/55/PlaybackInfo"))
            .and(query_param("X-Emby-Token", "KEY"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "PlaySessionId": "ps-1",
                    "MediaSources": [
                        {"Id": "src1", "Name": "v1", "Path": "/m/a.mkv",
                         "RunTimeTicks": 12_000_000_000_i64}
                    ]
                }),
            ))
            .mount(&server)
            .await;

        let http = HttpClient::new().expect("client");
        let client = EmbyClient::new(http, server.uri(), "KEY", "U");
        let info = client.playback_info("55").await.expect("playback_info");
        assert_eq!(info.play_session_id.as_deref(), Some("ps-1"));
        assert_eq!(info.media_sources.len(), 1);
        let src = info.media_sources.first().expect("one source");
        assert_eq!(src.id, "src1");
        assert_eq!(src.run_time_ticks, Some(12_000_000_000));
    }

    #[tokio::test]
    async fn resume_items_parses_list() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/emby/Users/U/Items/Resume"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "Items": [
                        {"Id": "1", "Name": "Ep1", "Path": "/m/s01e01.mkv",
                         "IndexNumber": 1, "ParentIndexNumber": 1}
                    ]
                }),
            ))
            .mount(&server)
            .await;

        let http = HttpClient::new().expect("client");
        let client = EmbyClient::new(http, server.uri(), "KEY", "U");
        let list = client.resume_items().await.expect("resume");
        assert_eq!(list.items.len(), 1);
        let item = list.items.first().expect("one item");
        assert_eq!(item.id, "1");
        assert_eq!(item.index_number, Some(1));
    }

    #[test]
    fn host_trailing_slash_trimmed() {
        let http = HttpClient::new().expect("client");
        let client = EmbyClient::new(http, http_host(), "K", "U");
        assert_eq!(client.url("Items/1"), "https://h/emby/Items/1");
    }

    fn http_host() -> String {
        "https://h/".to_owned()
    }
}
