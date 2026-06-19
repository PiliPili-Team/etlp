//! End-to-end smoke tests.
// Integration tests live outside #[cfg(test)] modules, so clippy.toml's
// `allow-expect-in-tests` does not cover non-test helper functions here.
#![allow(clippy::expect_used)]
//!
//! Covers three layers:
//!   1. HTTP route returns 200 with a realistic Emby / Plex fixture payload.
//!   2. `parse_received_data_emby` produces correct `PlaybackData` fields when
//!      given a well-formed payload that points to a `wiremock` Emby server.
//!   3. `EmbyClient::episodes` and `realtime_progress` make the expected HTTP
//!      calls to a mock server.
//!
//! Fixture JSON files live in `crates/etlp-media-server/tests/fixtures/` for
//! reference; the tests inline the relevant portions directly.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use etlp_core::{PlaybackData, Server};
use etlp_media_server::received::ReceivedData;
use etlp_media_server::{
    EmbyClient, EmbyParseConfig, parse_received_data_emby,
};
use etlp_net::{HttpClient, PlaybackEvent, RedirectCache, realtime_progress};
use tower::ServiceExt as _;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use etlp_config::Config;
use etlp_download::{
    DEFAULT_MAX_CONCURRENT, DEFAULT_MAX_PER_DOMAIN, DownloadManager,
};
use etlp_net::HttpClientBuilder;
use std::io::Write as _;
use std::sync::Arc;
use tempfile::TempDir;

use etlp_server::{AppState, SharedState, build_router};

fn make_test_state() -> (SharedState, TempDir) {
    const TOML: &str = "\
[emby]\nplayer = \"mpv\"\n\
[dev]\nskip_certificate_verify = false\n\
[trakt]\nenable_host = \"\"\n\
";
    let dir = tempfile::tempdir().expect("tempdir");
    let toml_path = dir.path().join("config.toml");
    {
        let mut f = std::fs::File::create(&toml_path).expect("create toml");
        f.write_all(TOML.as_bytes()).expect("write toml");
    }
    let config = Config::load_file(&toml_path).expect("load config");
    let client = reqwest::Client::builder().build().expect("reqwest client");
    let dl_manager = DownloadManager::new(
        dir.path().to_path_buf(),
        0,
        DEFAULT_MAX_CONCURRENT,
        DEFAULT_MAX_PER_DOMAIN,
        client,
    );
    let http_client = HttpClientBuilder::new().build().expect("http client");
    let state = Arc::new(AppState::new(
        config,
        dl_manager,
        http_client,
        dir.path().to_path_buf(),
    ));
    (state, dir)
}

// ── Route-level smoke tests ───────────────────────────────────────────────────

#[tokio::test]
async fn smoke_route_emby_ok() {
    let (state, _dir) = make_test_state();
    let app = build_router(state);

    let body = serde_json::json!({
        "playbackUrl":
            "http://emby.smoke:8096/emby/Items/ep001/PlaybackInfo\
             ?X-Emby-Token=smkkey&UserId=usr1",
        "ApiClient": {
            "_serverAddress": "http://emby.smoke:8096",
            "_serverVersion": "4.9"
        },
        "request": {"headers": {}},
        "mountDiskEnable": "false",
        "playbackData": {
            "PlaySessionId": "smk-session",
            "MediaSources": [{
                "Id": "src001", "Name": "1080p",
                "Path": "/media/show/s01e01.mkv",
                "RunTimeTicks": 27000000000_i64,
                "MediaStreams": [
                    {"Type": "Video", "Index": 0},
                    {"Type": "Audio", "Index": 1}
                ]
            }]
        },
        "extraData": {
            "mainEpInfo": {
                "Id": "ep001", "Name": "Pilot",
                "Path": "/media/show/s01e01.mkv",
                "Type": "Episode", "SeriesName": "Smoke Show",
                "SeasonId": "season001", "SeriesId": "series001",
                "IndexNumber": 1, "ParentIndexNumber": 1
            },
            "episodesInfo": [
                {"Id": "ep001", "IndexNumber": 1},
                {"Id": "ep002", "IndexNumber": 2}
            ],
            "playlistInfo": []
        }
    });

    let req = Request::builder()
        .method(Method::POST)
        .uri("/embyToLocalPlayer")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).expect("serialize")))
        .expect("request");

    let res = app.oneshot(req).await.expect("oneshot");
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn smoke_route_plex_ok() {
    let (state, _dir) = make_test_state();
    let app = build_router(state);

    let body = serde_json::json!({
        "playbackUrl":
            "http://plex.smoke:32400/library/metadata/42\
             ?X-Plex-Token=plextoken",
        "mountDiskEnable": "false",
        "playbackData": {
            "MediaContainer": {
                "Metadata": [{
                    "ratingKey": "42",
                    "title": "Smoke Movie",
                    "type": "movie",
                    "Media": [{
                        "Part": [{
                            "key": "/library/parts/42/file.mkv",
                            "file": "/media/smoke.mkv",
                            "duration": 7200000,
                            "size": 4294967296_i64
                        }]
                    }]
                }]
            }
        }
    });

    let req = Request::builder()
        .method(Method::POST)
        .uri("/plexToLocalPlayer")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).expect("serialize")))
        .expect("request");

    let res = app.oneshot(req).await.expect("oneshot");
    assert_eq!(res.status(), StatusCode::OK);
}

// ── Parse chain smoke test ────────────────────────────────────────────────────

/// Build a `ReceivedData` payload pointing to `host` with pre-filled
/// `MediaSources` so no HTTP call is needed during parsing.
fn emby_received_data(host: &str) -> ReceivedData {
    let json = serde_json::json!({
        "playbackUrl": format!(
            "http://{host}/emby/Items/ep001/PlaybackInfo\
             ?X-Emby-Token=smkkey&UserId=usr1"
        ),
        "ApiClient": {
            "_serverAddress": format!("http://{host}"),
            "_serverVersion": "4.9"
        },
        "request": {"headers": {}},
        "mountDiskEnable": "false",
        "playbackData": {
            "PlaySessionId": "smk-session",
            "MediaSources": [{
                "Id": "src001", "Name": "1080p",
                "Path": "/media/show/s01e01.mkv",
                "RunTimeTicks": 27000000000_i64,
                "Size": 4294967296_i64,
                "MediaStreams": [
                    {"Type": "Video", "Index": 0},
                    {"Type": "Audio", "Index": 1},
                    {"Type": "Subtitle", "Index": 2,
                     "IsExternal": false, "DisplayTitle": "Chinese"}
                ]
            }]
        },
        "extraData": {
            "mainEpInfo": {
                "Id": "ep001", "Name": "Pilot",
                "Path": "/media/show/s01e01.mkv",
                "Type": "Episode", "SeriesName": "Smoke Show",
                "SeasonId": "season001", "SeriesId": "series001",
                "IndexNumber": 1, "ParentIndexNumber": 1
            },
            "episodesInfo": [
                {"Id": "ep001", "IndexNumber": 1},
                {"Id": "ep002", "IndexNumber": 2}
            ],
            "playlistInfo": []
        }
    });
    serde_json::from_value(json).expect("build ReceivedData")
}

#[tokio::test]
async fn smoke_emby_parse_playback_data() {
    // Mock server provides the Emby host URL; actual parsing uses the
    // pre-filled MediaSources so no HTTP call reaches the mock.
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/emby/Items/ep001/PlaybackInfo"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                include_str!(
                    "../../etlp-media-server/tests/fixtures/playback_info.json"
                ),
                "application/json",
            ),
        )
        .expect(0) // not called for standard network play
        .mount(&mock_server)
        .await;

    let host = mock_server.uri().trim_start_matches("http://").to_owned();
    let received = emby_received_data(&host);

    let config = EmbyParseConfig {
        // priority_key() lowercases the display title, so "chinese" matches
        // a subtitle with DisplayTitle = "Chinese".
        subtitle_priority: vec!["chinese".to_owned()],
        pretty_title: true,
        last_ep_disable_playlist: true,
        ..EmbyParseConfig::default()
    };
    let http = HttpClient::new().expect("http client");
    let cache = RedirectCache::new();

    let data = parse_received_data_emby(&received, &config, &http, &cache)
        .await
        .expect("parse");

    assert_eq!(data.server, Server::Emby);
    assert_eq!(data.item_id, "ep001");
    assert_eq!(data.media_source_id, "src001");
    assert_eq!(data.api_key, "smkkey");
    assert_eq!(data.user_id, "usr1");
    assert_eq!(data.total_sec, 2700); // 27_000_000_000 ticks / 1e7
    assert_eq!(data.size, 4_294_967_296);
    assert!(data.is_multiple_episodes); // ep001 is first of two
    assert!(
        data.stream_url
            .to_lowercase()
            .contains("/emby/videos/ep001/"),
        "stream_url should be an Emby direct-stream URL: {}",
        data.stream_url
    );
    // Subtitle index 2 is the only embedded sub → inner_index = 1
    assert_eq!(data.sub.inner_index, Some(1));
}

// ── EmbyClient episodes API smoke test ───────────────────────────────────────

#[tokio::test]
async fn smoke_emby_episodes_api() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/emby/Shows/series-001/Episodes"))
        .and(query_param("X-Emby-Token", "apikey"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            include_str!(
                "../../etlp-media-server/tests/fixtures/episodes.json"
            ),
            "application/json",
        ))
        .expect(1)
        .mount(&mock_server)
        .await;

    let http = HttpClient::new().expect("http client");
    let client = EmbyClient::new(http, mock_server.uri(), "apikey", "u1");

    let list = client.episodes("series-001", None).await.expect("episodes");

    assert_eq!(list.items.len(), 2);
    let first = list.items.first().expect("first");
    assert_eq!(first.id, "ep-001");
    assert_eq!(first.index_number, Some(1));
    let second = list.items.get(1).expect("second");
    assert_eq!(second.id, "ep-002");
    assert_eq!(second.index_number, Some(2));
}

// ── Progress write-back smoke test ───────────────────────────────────────────

#[tokio::test]
async fn smoke_progress_writeback_start() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/emby/Sessions/Playing"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock_server)
        .await;

    let host = mock_server.uri().trim_start_matches("http://").to_owned();

    let data = PlaybackData {
        server: Server::Emby,
        scheme: "http".into(),
        netloc: host,
        api_key: "testkey".into(),
        device_id: "testdevice".into(),
        item_id: "ep001".into(),
        play_session_id: "smk-session".into(),
        total_sec: 2700,
        ..PlaybackData::default()
    };

    let http = HttpClient::new().expect("http client");
    realtime_progress(&http, &data, 42, PlaybackEvent::Start)
        .await
        .expect("progress");

    // wiremock verifies the POST was called exactly once on drop
}

#[tokio::test]
async fn smoke_progress_writeback_end() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/emby/Sessions/Playing/Stopped"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock_server)
        .await;

    let host = mock_server.uri().trim_start_matches("http://").to_owned();

    let data = PlaybackData {
        server: Server::Emby,
        scheme: "http".into(),
        netloc: host,
        api_key: "testkey".into(),
        device_id: "testdevice".into(),
        item_id: "ep001".into(),
        play_session_id: "smk-session".into(),
        total_sec: 2700,
        ..PlaybackData::default()
    };

    let http = HttpClient::new().expect("http client");
    realtime_progress(&http, &data, 1200, PlaybackEvent::End)
        .await
        .expect("progress");
}
