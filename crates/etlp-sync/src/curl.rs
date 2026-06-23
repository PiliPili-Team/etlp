//! Debug-level `curl` logging for Bangumi / Trakt API requests.
//!
//! Emits the equivalent `curl` command line for an outgoing request so the call
//! can be reproduced and diagnosed straight from the logs. When `dev.mix_log`
//! is on, the logging `Masker` redacts the configured Bangumi / Trakt account
//! names (and the Emby api key / host) from the on-disk file, so a `users/<name>`
//! path or `user=` field never leaks the user's identity. The Logs view's
//! anonymous mode additionally masks Bearer tokens, user names and hosts at
//! *display* time. With masking off, the file keeps the full request verbatim
//! for debugging.

use std::time::Duration;

use reqwest::Method;
use serde::de::DeserializeOwned;

use crate::error::{Result, SyncError};

/// Longest non-JSON error body to log (an HTML gateway page can be huge).
const NON_JSON_LOG_LIMIT: usize = 500;

/// A response paired with the request method that produced it.
///
/// `reqwest::Response` does not carry the request method, but the logging
/// policy depends on it (GET/POST dump the JSON body; other verbs log only the
/// status code), so [`send_logged`] captures the method up-front and threads it
/// through to [`read_logged`].
pub(crate) struct LoggedResponse {
    method: Method,
    resp: reqwest::Response,
}

/// Whether a response body is dumped in full for `method`.
///
/// GET/POST replies carry the data we usually want to inspect (collections,
/// search results, scrobble echoes); other verbs (PUT/DELETE/PATCH) are writes
/// whose body rarely matters, so only their status code is logged.
fn logs_full_body(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::POST)
}

/// How a response should be logged, decided from the method and body.
#[derive(Debug, PartialEq, Eq)]
enum ResponseLog {
    /// Log only the status code: a non-GET/POST verb, or an empty body.
    StatusOnly,
    /// GET/POST with a JSON body — dump it verbatim at debug.
    JsonBody,
    /// GET/POST with a non-empty, non-JSON body — flag it as an error.
    NonJson,
}

/// Classify how to log a response.
///
/// The method is checked before the body so a non-GET/POST reply is always
/// status-only — an empty `204` can never be mistaken for a non-JSON payload.
/// An empty body is status-only for every verb, since there is nothing to dump.
fn classify(method: &Method, body: &str) -> ResponseLog {
    if !logs_full_body(method) || body.trim().is_empty() {
        ResponseLog::StatusOnly
    } else if serde_json::from_str::<serde_json::Value>(body).is_ok() {
        ResponseLog::JsonBody
    } else {
        ResponseLog::NonJson
    }
}

/// Read a response body and log it according to the request method.
///
/// For other verbs (PUT/DELETE/PATCH) only the status code is logged — the
/// method is checked first, so an empty body (e.g. a `204 No Content`) is never
/// mistaken for a non-JSON payload. For GET/POST the raw JSON is logged at debug
/// when the body parses as JSON, or an `error` line when it does not (a
/// gateway/proxy can answer with an HTML page, e.g. on a 5xx).
///
/// `label` (e.g. `"trakt"`/`"bangumi"`) prefixes the log line. The body is
/// logged verbatim so the exact API reply is visible while diagnosing; it is
/// redacted only at display time by the Logs view's anonymous mode (Bearer
/// token, usernames, hosts, …), exactly like the `curl` request lines. Returns
/// the status and body for the caller to interpret.
pub(crate) async fn read_logged(
    label: &str,
    logged: LoggedResponse,
) -> Result<(reqwest::StatusCode, String)> {
    let LoggedResponse { method, resp } = logged;
    let status = resp.status();
    let body = resp.text().await?;
    match classify(&method, &body) {
        ResponseLog::StatusOnly => {
            tracing::debug!("{label}: {method} response {}", status.as_u16());
        }

        ResponseLog::JsonBody => {
            tracing::debug!(
                "{label}: response {} body: {body}",
                status.as_u16()
            );
        }

        ResponseLog::NonJson => {
            let preview: String =
                body.chars().take(NON_JSON_LOG_LIMIT).collect();
            tracing::error!(
                "{label}: non-JSON response {}: {preview}",
                status.as_u16()
            );
        }
    }
    Ok((status, body))
}

/// Read a response and log only the status code; the body is drained but
/// never written to the log.
///
/// Use for endpoints (e.g. `GET /me`) whose response body may contain PII
/// (email address, display name, registration date) that no masker rule
/// covers.  The body is still returned so error paths can surface the raw
/// API reply to the caller, but it never reaches the log sink.
pub(crate) async fn read_status_only(
    label: &str,
    logged: LoggedResponse,
) -> Result<(reqwest::StatusCode, String)> {
    let LoggedResponse { method: _, resp } = logged;
    let status = resp.status();
    tracing::debug!("{label}: response {}", status.as_u16());
    let body = resp.text().await?;
    Ok((status, body))
}

/// Read and log a response (via [`read_logged`]), then deserialize it.
///
/// A non-success status maps to [`SyncError::Api`] carrying the body; a body
/// that fails to deserialize maps to [`SyncError::Json`].
pub(crate) async fn json_logged<T: DeserializeOwned>(
    label: &str,
    logged: LoggedResponse,
) -> Result<T> {
    let (status, body) = read_logged(label, logged).await?;
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
) -> Result<LoggedResponse> {
    // Capture the method now: the response will not carry it, yet read_logged
    // needs it to decide whether to dump the JSON body. Building a clone is the
    // only way to read the verb off a RequestBuilder; a non-cloneable
    // (streaming) body falls back to GET, which simply keeps the old "dump the
    // body" behaviour for that rare case.
    let built = builder.try_clone().and_then(|c| c.build().ok());
    let method = built
        .as_ref()
        .map_or(Method::GET, |req| req.method().clone());
    if tracing::enabled!(tracing::Level::DEBUG)
        && let Some(req) = built.as_ref()
    {
        tracing::debug!("{}", curl_command(req));
    }
    let resp = send_retrying(label, builder).await?;
    Ok(LoggedResponse { method, resp })
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

    #[test]
    fn full_body_is_logged_only_for_get_and_post() {
        // GET/POST replies carry data worth inspecting; writes do not.
        assert!(logs_full_body(&Method::GET));
        assert!(logs_full_body(&Method::POST));
        assert!(!logs_full_body(&Method::PUT));
        assert!(!logs_full_body(&Method::DELETE));
        assert!(!logs_full_body(&Method::PATCH));
    }

    #[test]
    fn classify_routes_by_method_then_body() {
        // GET/POST: JSON bodies are dumped, other payloads are errors.
        assert_eq!(classify(&Method::GET, r#"{"a":1}"#), ResponseLog::JsonBody);
        assert_eq!(
            classify(&Method::POST, r#"{"ok":true}"#),
            ResponseLog::JsonBody
        );
        assert_eq!(
            classify(&Method::GET, "<html>oops</html>"),
            ResponseLog::NonJson
        );

        // Non GET/POST verbs are status-only regardless of the body — a PUT
        // returning an empty 204 must never be flagged as a non-JSON error.
        assert_eq!(classify(&Method::PUT, ""), ResponseLog::StatusOnly);
        assert_eq!(
            classify(&Method::PUT, r#"{"type":2}"#),
            ResponseLog::StatusOnly
        );
        assert_eq!(classify(&Method::DELETE, "x"), ResponseLog::StatusOnly);

        // An empty body is status-only even for GET/POST: nothing to dump.
        assert_eq!(classify(&Method::POST, ""), ResponseLog::StatusOnly);
        assert_eq!(classify(&Method::GET, "   "), ResponseLog::StatusOnly);
    }

    #[tokio::test]
    async fn read_status_only_returns_status_and_body_without_logging() {
        // Verify that read_status_only returns the correct status and the body
        // is available for error paths, even though it is never logged.
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let pii_body = serde_json::json!({
            "id": 12345,
            "username": "someuser",
            "email": "private@example.com",
            "nickname": "Nick",
        });
        Mock::given(method("GET"))
            .and(path("/me"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(pii_body.clone()),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{}/me", server.uri()))
            .send()
            .await
            .unwrap();
        let logged = LoggedResponse {
            method: Method::GET,
            resp,
        };
        let (status, body) = read_status_only("bangumi", logged).await.unwrap();
        assert_eq!(status.as_u16(), 200);
        // The body is still available so error paths can include it.
        assert!(body.contains("private@example.com"));
    }
}
