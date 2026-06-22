//! Debug-level `curl` logging for Bangumi / Trakt API requests.
//!
//! Emits the equivalent `curl` command line for an outgoing request so the call
//! can be reproduced and diagnosed straight from the logs. The line is written
//! verbatim to the log file; secrets in it (Bearer tokens, api keys, usernames,
//! hosts) are redacted only at *display* time by the Logs view's anonymous
//! mode, so a shared screenshot stays safe while the on-disk file keeps the full
//! request for debugging.

use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::error::{Result, SyncError};

/// Longest non-JSON error body to log (an HTML gateway page can be huge).
const NON_JSON_LOG_LIMIT: usize = 500;

/// Read a response body and log it: the raw JSON at debug when the body parses
/// as JSON, otherwise an `error` line (a gateway/proxy can answer with an HTML
/// page, e.g. on a 5xx, which is not worth dumping as JSON).
///
/// `label` (e.g. `"trakt"`/`"bangumi"`) prefixes the log line. The body is
/// logged verbatim so the exact API reply is visible while diagnosing; it is
/// redacted only at display time by the Logs view's anonymous mode (Bearer
/// token, usernames, hosts, …), exactly like the `curl` request lines. Returns
/// the status and body for the caller to interpret.
pub(crate) async fn read_logged(
    label: &str,
    resp: reqwest::Response,
) -> Result<(reqwest::StatusCode, String)> {
    let status = resp.status();
    let body = resp.text().await?;
    if serde_json::from_str::<serde_json::Value>(&body).is_ok() {
        tracing::debug!("{label}: response {} body: {body}", status.as_u16());
    } else {
        let preview: String = body.chars().take(NON_JSON_LOG_LIMIT).collect();
        tracing::error!(
            "{label}: non-JSON response {}: {preview}",
            status.as_u16()
        );
    }
    Ok((status, body))
}

/// Read and log a response (via [`read_logged`]), then deserialize it.
///
/// A non-success status maps to [`SyncError::Api`] carrying the body; a body
/// that fails to deserialize maps to [`SyncError::Json`].
pub(crate) async fn json_logged<T: DeserializeOwned>(
    label: &str,
    resp: reqwest::Response,
) -> Result<T> {
    let (status, body) = read_logged(label, resp).await?;
    if !status.is_success() {
        return Err(SyncError::Api {
            status: status.as_u16(),
            body,
        });
    }
    serde_json::from_str(&body).map_err(SyncError::Json)
}

/// Maximum retry attempts after the first try, on a transient failure.
const MAX_RETRIES: usize = 5;

/// Fixed delay between retry attempts.
const RETRY_INTERVAL: Duration = Duration::from_secs(1);

/// Send `builder`, first logging the equivalent `curl` command at debug level.
///
/// Kept at `debug`: the line carries the full Bearer token / api key, so it
/// belongs to the diagnostic level the user opts into, not the default. The
/// sync *outcome* (`scrobbled`, skip reasons) is logged at info separately.
///
/// The request is cloned only for logging; the original is sent unchanged. A
/// body that cannot be cloned (e.g. a stream) is still sent — just without a
/// logged line. Use this for data endpoints only, never for OAuth/token
/// exchanges whose bodies carry the client secret.
///
/// `label` (e.g. `"trakt"`/`"bangumi"`) prefixes the retry log lines.
pub(crate) async fn send_logged(
    label: &str,
    builder: reqwest::RequestBuilder,
) -> Result<reqwest::Response> {
    if tracing::enabled!(tracing::Level::DEBUG)
        && let Some(clone) = builder.try_clone()
        && let Ok(req) = clone.build()
    {
        tracing::debug!("{}", curl_command(&req));
    }
    send_retrying(label, builder).await
}

/// Send a request, retrying up to [`MAX_RETRIES`] times on transient failures.
///
/// Gateway errors (`502`/`503`/`504`) and rate limits (`429`), plus connection
/// and timeout errors, are retried at a fixed [`RETRY_INTERVAL`] — Trakt's edge
/// returns `504 upstream timeout` intermittently and a manual re-run usually
/// succeeds, so the same automatic retry keeps the sync from failing on a
/// transient blip. A non-transient response (any other status) and a body that
/// cannot be cloned are returned on the first attempt. Use for idempotent
/// requests only.
pub(crate) async fn send_retrying(
    label: &str,
    builder: reqwest::RequestBuilder,
) -> Result<reqwest::Response> {
    let mut retries = 0usize;
    loop {
        // Clone so the original survives a retry; a non-cloneable (streaming)
        // body is sent once and returned as-is.
        let Some(attempt) = builder.try_clone() else {
            return Ok(builder.send().await?);
        };
        let result = attempt.send().await;
        let transient = match &result {
            Ok(resp) => is_transient_status(resp.status().as_u16()),
            Err(e) => e.is_timeout() || e.is_connect(),
        };
        if transient && retries < MAX_RETRIES {
            retries += 1;
            let reason = match &result {
                Ok(resp) => format!("HTTP {}", resp.status().as_u16()),
                Err(e) => e.to_string(),
            };
            tracing::debug!(
                "{label}: attempt to retry {retries}/{MAX_RETRIES} \
                 after {reason}"
            );
            tokio::time::sleep(RETRY_INTERVAL).await;
            continue;
        }
        return Ok(result?);
    }
}

/// Whether an HTTP status warrants a retry: gateway errors and rate limiting.
fn is_transient_status(status: u16) -> bool {
    matches!(status, 429 | 502 | 503 | 504)
}

/// Format a [`reqwest::Request`] as an equivalent `curl` command line.
///
/// Headers and an in-memory (non-streaming) UTF-8 body are included; query
/// parameters are already part of the URL. Binary header values and bodies are
/// rendered as placeholders rather than dropped.
fn curl_command(req: &reqwest::Request) -> String {
    let mut line =
        format!("curl -X {} '{}'", req.method().as_str(), req.url().as_str());
    for (name, value) in req.headers() {
        let v = value.to_str().unwrap_or("<binary>");
        line.push_str(&format!(" -H '{name}: {v}'"));
    }
    if let Some(body) = req.body().and_then(reqwest::Body::as_bytes) {
        match std::str::from_utf8(body) {
            Ok(text) => line.push_str(&format!(" -d '{text}'")),
            Err(_) => line.push_str(" -d '<binary>'"),
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curl_command_includes_method_url_headers_and_body() {
        let client = reqwest::Client::new();
        let req = client
            .post("https://api.trakt.tv/scrobble/pause")
            .header("Authorization", "Bearer secret-token")
            .json(&serde_json::json!({ "progress": 35.0 }))
            .build()
            .expect("build request");

        let line = curl_command(&req);
        assert!(line.starts_with("curl -X POST "));
        assert!(line.contains("https://api.trakt.tv/scrobble/pause"));
        assert!(line.contains("-H 'authorization: Bearer secret-token'"));
        assert!(line.contains("-d '{\"progress\":35.0}'"));
    }
}
