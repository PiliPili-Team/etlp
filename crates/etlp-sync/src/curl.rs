//! Debug-level `curl` logging for Bangumi / Trakt API requests.
//!
//! Emits the equivalent `curl` command line for an outgoing request so the call
//! can be reproduced and diagnosed straight from the logs. The line is written
//! verbatim to the log file; secrets in it (Bearer tokens, api keys, usernames,
//! hosts) are redacted only at *display* time by the Logs view's anonymous
//! mode, so a shared screenshot stays safe while the on-disk file keeps the full
//! request for debugging.

use crate::error::Result;

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
pub(crate) async fn send_logged(
    builder: reqwest::RequestBuilder,
) -> Result<reqwest::Response> {
    if tracing::enabled!(tracing::Level::DEBUG)
        && let Some(clone) = builder.try_clone()
        && let Ok(req) = clone.build()
    {
        tracing::debug!("{}", curl_command(&req));
    }
    Ok(builder.send().await?)
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
