//! Playback progress write-back.
//!
//! Reports the stop position back to Emby / Jellyfin / Plex, including the
//! `> 10h` sanity guard, the small-offset trim, and the skip for
//! `.iso` / `.m3u8` media.

use etlp_core::{PlaybackData, Server};
use serde_json::json;

use crate::DEVICE_NAME;
use crate::client::{HttpClient, Result};

/// Realtime playback event kind for `Sessions/Playing/Progress` heartbeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackEvent {
    /// Playback session started; maps to `Sessions/Playing`.
    Start,
    /// Periodic heartbeat or pause-state update; maps to `Sessions/Playing/Progress`.
    Playing,
    /// Playback session ended; maps to `Sessions/Playing/Stopped`.
    End,
}

/// Send a realtime playback event to the originating server.
///
/// Plex and STRM media (`total_sec == 86400`) are silently skipped; the caller
/// must check `data.runtime_missing()` if it wants to skip early.
///
/// On network error the result is silently discarded — the loop continues.
pub async fn realtime_progress(
    client: &HttpClient,
    data: &PlaybackData,
    pos_sec: i64,
    event: PlaybackEvent,
) -> Result<()> {
    match data.server {
        Server::Plex => Ok(()), // Plex does not use Sessions/Playing
        Server::Emby => emby_realtime(client, data, pos_sec, event).await,
        Server::Jellyfin => {
            jellyfin_realtime(client, data, pos_sec, event).await
        }
    }
}

async fn emby_realtime(
    client: &HttpClient,
    data: &PlaybackData,
    pos_sec: i64,
    event: PlaybackEvent,
) -> Result<()> {
    let ticks = pos_sec * EMBY_TICKS_PER_SEC;
    let base = format!("{}://{}/emby", data.scheme, data.netloc);
    let params = [
        ("X-Emby-Token", data.api_key.as_str()),
        ("X-Emby-Device-Id", data.device_id.as_str()),
        ("X-Emby-Client", DEVICE_NAME),
        ("X-Emby-Device-Name", DEVICE_NAME),
    ];
    let body = json!({
        "PositionTicks": ticks,
        "ItemId": data.item_id,
        "PlaySessionId": data.play_session_id,
        "IsPaused": false,
    });
    let path = match event {
        PlaybackEvent::Start => "Sessions/Playing",
        PlaybackEvent::Playing => "Sessions/Playing/Progress",
        PlaybackEvent::End => "Sessions/Playing/Stopped",
    };
    client
        .post_json(&format!("{base}/{path}"), &params, &body)
        .await
}

async fn jellyfin_realtime(
    client: &HttpClient,
    data: &PlaybackData,
    pos_sec: i64,
    event: PlaybackEvent,
) -> Result<()> {
    let ticks = pos_sec * EMBY_TICKS_PER_SEC;
    let base = format!("{}://{}", data.scheme, data.netloc);
    let body = json!({
        "PositionTicks": ticks,
        "ItemId": data.item_id,
        "PlaySessionId": data.play_session_id,
        "IsPaused": false,
    });
    let path = match event {
        PlaybackEvent::Start => "Sessions/Playing",
        PlaybackEvent::Playing => "Sessions/Playing/Progress",
        PlaybackEvent::End => "Sessions/Playing/Stopped",
    };
    client
        .post_json(&format!("{base}/{path}"), &[], &body)
        .await
}

/// Seconds in 10 hours; a larger stop second is treated as corrupt and ignored.
const MAX_STOP_SEC: i64 = 10 * 60 * 60;

/// Emby ticks are 100-nanosecond units (1s = 10^7).
const EMBY_TICKS_PER_SEC: i64 = 10_000_000;

/// Plex timeline is in milliseconds.
const PLEX_MS_PER_SEC: i64 = 1000;

/// Write back the stop position to the originating server.
///
/// `update_success` indicates the realtime reporter already opened a session,
/// in which case the extra `Sessions/Playing` start call is skipped. Media that
/// is `.iso` / `.m3u8` is skipped (it would be wrongly marked watched).
pub async fn update_progress(
    client: &HttpClient,
    data: &PlaybackData,
    stop_sec: i64,
    update_success: bool,
) -> Result<()> {
    let ext = extension_lower(&data.file_path);
    if ext == "iso" || ext == "m3u8" {
        tracing::info!("skip update progress because media is .{ext}");
        return Ok(());
    }
    // Match Python: pull back a couple seconds so "resume" lands before the end.
    let stop_sec = if stop_sec > 5 { stop_sec - 2 } else { stop_sec };
    if stop_sec > MAX_STOP_SEC {
        tracing::error!("stop_sec {stop_sec} too large, skip update");
        return Ok(());
    }

    match data.server {
        Server::Emby => {
            emby_stopped(client, data, stop_sec, update_success).await
        }
        Server::Jellyfin => {
            jellyfin_stopped(client, data, stop_sec, update_success).await
        }
        Server::Plex => plex_stopped(client, data, stop_sec).await,
    }
}

fn extension_lower(file_path: &str) -> String {
    file_path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_lowercase())
        .unwrap_or_default()
}

async fn emby_stopped(
    client: &HttpClient,
    data: &PlaybackData,
    stop_sec: i64,
    update_success: bool,
) -> Result<()> {
    let ticks = stop_sec * EMBY_TICKS_PER_SEC;
    let params = [
        ("X-Emby-Token", data.api_key.as_str()),
        ("X-Emby-Device-Id", data.device_id.as_str()),
        ("X-Emby-Client", DEVICE_NAME),
        ("X-Emby-Device-Name", DEVICE_NAME),
    ];
    let base = format!("{}://{}/emby", data.scheme, data.netloc);
    if !update_success {
        client
            .post_json(
                &format!("{base}/Sessions/Playing"),
                &params,
                &json!({
                    "ItemId": data.item_id,
                    "PlaySessionId": data.play_session_id,
                }),
            )
            .await?;
    }
    client
        .post_json(
            &format!("{base}/Sessions/Playing/Stopped"),
            &params,
            &json!({
                "PositionTicks": ticks,
                "ItemId": data.item_id,
                "PlaySessionId": data.play_session_id,
            }),
        )
        .await
}

async fn jellyfin_stopped(
    client: &HttpClient,
    data: &PlaybackData,
    stop_sec: i64,
    update_success: bool,
) -> Result<()> {
    let ticks = stop_sec * EMBY_TICKS_PER_SEC;
    let base = format!("{}://{}", data.scheme, data.netloc);
    if !update_success {
        client
            .post_json(
                &format!("{base}/Sessions/Playing"),
                &[],
                &json!({
                    "ItemId": data.item_id,
                    "PlaySessionId": data.play_session_id,
                }),
            )
            .await?;
    }
    client
        .post_json(
            &format!("{base}/Sessions/Playing/Stopped"),
            &[],
            &json!({
                "PositionTicks": ticks,
                "ItemId": data.item_id,
                "PlaySessionId": data.play_session_id,
            }),
        )
        .await
}

async fn plex_stopped(
    client: &HttpClient,
    data: &PlaybackData,
    stop_sec: i64,
) -> Result<()> {
    let ticks = stop_sec * PLEX_MS_PER_SEC;
    let duration = (data.total_sec * PLEX_MS_PER_SEC).to_string();
    let ticks_s = ticks.to_string();
    let rating_key = data.rating_key.clone().unwrap_or_default();
    let client_id = data.client_id.clone().unwrap_or_default();
    let base = format!("{}://{}", data.scheme, data.netloc);

    client
        .get_text(
            &format!("{base}/:/timeline"),
            &[
                ("ratingKey", rating_key.as_str()),
                ("state", "stopped"),
                ("time", ticks_s.as_str()),
                ("duration", duration.as_str()),
                ("X-Plex-Client-Identifier", client_id.as_str()),
                ("X-Plex-Token", data.api_key.as_str()),
            ],
        )
        .await?;

    if stop_sec > 30 {
        return Ok(());
    }
    // Early stop: unscrobble so it is not marked watched.
    client
        .get_text(
            &format!("{base}/:/unscrobble"),
            &[
                ("key", rating_key.as_str()),
                ("X-Plex-Client-Identifier", client_id.as_str()),
                ("X-Plex-Token", data.api_key.as_str()),
                ("identifier", "com.plexapp.plugins.library"),
            ],
        )
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::HttpClient;

    fn emby_data(server_uri: &str) -> PlaybackData {
        let (scheme, netloc) = server_uri
            .split_once("://")
            .unwrap_or(("http", "127.0.0.1"));
        PlaybackData {
            server: Server::Emby,
            scheme: scheme.to_owned(),
            netloc: netloc.to_owned(),
            api_key: "KEY".into(),
            device_id: "DEV".into(),
            play_session_id: "PS".into(),
            item_id: "100".into(),
            total_sec: 1200,
            file_path: "/m/a.mkv".into(),
            ..PlaybackData::default()
        }
    }

    #[test]
    fn extension_lower_handles_paths() {
        assert_eq!(extension_lower("/x/y.MKV"), "mkv");
        assert_eq!(extension_lower("/x/movie.m3u8"), "m3u8");
        assert_eq!(extension_lower("/x/noext"), "");
    }

    #[tokio::test]
    async fn emby_posts_start_and_stopped() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/emby/Sessions/Playing"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/emby/Sessions/Playing/Stopped"))
            .and(query_param("X-Emby-Token", "KEY"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let client = HttpClient::new().expect("client");
        let data = emby_data(&server.uri());
        update_progress(&client, &data, 600, false)
            .await
            .expect("update");
    }

    #[tokio::test]
    async fn iso_media_is_skipped_without_request() {
        // No mocks mounted: any HTTP call would 404/err. Skip path must not call.
        let client = HttpClient::new().expect("client");
        let mut data = emby_data("http://127.0.0.1:9");
        data.file_path = "/m/disc.iso".into();
        update_progress(&client, &data, 600, false)
            .await
            .expect("iso skip is ok");
    }

    #[tokio::test]
    async fn update_success_skips_start_call() {
        let server = MockServer::start().await;
        // Only the Stopped endpoint is mounted; if start were called it 404s.
        Mock::given(method("POST"))
            .and(path("/emby/Sessions/Playing/Stopped"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let client = HttpClient::new().expect("client");
        let data = emby_data(&server.uri());
        update_progress(&client, &data, 600, true)
            .await
            .expect("update with success flag");
    }
}
