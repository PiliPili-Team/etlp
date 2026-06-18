//! Async HTTP client wrapping `reqwest`.
//!
//! Per-request conventions: User-Agent, `Referer` header, JSON headers, a
//! proxy that skips localhost/`plex.direct`, and a bounded retry loop. TLS
//! uses rustls so the binary needs no system OpenSSL.

use std::time::Duration;

use reqwest::header::{ACCEPT, CONTENT_TYPE, LOCATION, REFERER, USER_AGENT};
use reqwest::redirect::Policy;
use reqwest::{Client, Method, Proxy};
use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;
use url::Url;

use crate::UA_ETLP;
use crate::url_tools::{build_referer, safe_url};

/// Emit a DEBUG-level curl equivalent for the outgoing request.
fn log_curl(
    ua: &str,
    method: &str,
    url: &str,
    params: &[(&str, &str)],
    extra_headers: &[(&str, &str)],
    body: Option<&str>,
) {
    if !tracing::enabled!(tracing::Level::DEBUG) {
        return;
    }
    let full_url = if params.is_empty() {
        url.to_owned()
    } else {
        let qs: String = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");
        format!("{url}?{qs}")
    };
    let mut parts = vec![format!("curl -X {method} '{full_url}'")];
    parts.push(format!("-H 'User-Agent: {ua}'"));
    for (k, v) in extra_headers {
        parts.push(format!("-H '{k}: {v}'"));
    }
    if let Some(b) = body {
        parts.push(format!("-d '{b}'"));
    }
    tracing::debug!("{}", parts.join(" "));
}

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_RETRY: u32 = 3;

/// Errors raised by the HTTP client.
#[derive(Debug, Error)]
pub enum NetError {
    /// The underlying client could not be constructed.
    #[error("failed to build http client: {0}")]
    Build(String),

    /// All retry attempts failed at the transport layer.
    #[error("request to {url} failed after {tries} tries: {source}")]
    Request {
        url: String,
        tries: u32,
        #[source]
        source: reqwest::Error,
    },

    /// The server returned a non-success status.
    #[error("http status {status} for {url}")]
    Status { status: u16, url: String },

    /// The response body could not be decoded as the requested type.
    #[error("failed to decode response from {url}: {source}")]
    Decode {
        url: String,
        #[source]
        source: reqwest::Error,
    },
}

/// Convenience alias for client results.
pub type Result<T> = std::result::Result<T, NetError>;

/// Builder for [`HttpClient`].
#[derive(Debug, Clone)]
pub struct HttpClientBuilder {
    proxy: Option<String>,
    cert_verify: bool,
    timeout: Duration,
    retry: u32,
    /// User-Agent for normal requests. `None` → [`UA_ETLP`].
    user_agent: Option<String>,
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self {
            proxy: None,
            cert_verify: true,
            timeout: DEFAULT_TIMEOUT,
            retry: DEFAULT_RETRY,
            user_agent: None,
        }
    }
}

impl HttpClientBuilder {
    /// Start from defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// HTTP proxy as `host:port` (no scheme). `None` disables proxying.
    #[must_use]
    pub fn proxy(mut self, proxy: Option<String>) -> Self {
        self.proxy = proxy;
        self
    }

    /// Whether to verify TLS certificates (`false` mirrors
    /// `dev.skip_certificate_verify`).
    #[must_use]
    pub fn cert_verify(mut self, verify: bool) -> Self {
        self.cert_verify = verify;
        self
    }

    /// Default per-request timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Number of transport-level attempts before giving up.
    #[must_use]
    pub fn retry(mut self, retry: u32) -> Self {
        self.retry = retry.max(1);
        self
    }

    /// Override the User-Agent for normal requests.
    ///
    /// Empty strings are treated the same as `None` — both fall back to
    /// [`UA_ETLP`]. Download and prefetch clients are not affected.
    #[must_use]
    pub fn user_agent(mut self, ua: Option<String>) -> Self {
        self.user_agent = ua.filter(|s| !s.is_empty());
        self
    }

    /// Build the client (constructs one redirect-following and one
    /// redirect-stopping inner client).
    pub fn build(self) -> Result<HttpClient> {
        let follow =
            build_inner(&self.proxy, self.cert_verify, self.timeout, true)?;
        let no_follow =
            build_inner(&self.proxy, self.cert_verify, self.timeout, false)?;
        let ua = self.user_agent.unwrap_or_else(|| UA_ETLP.to_owned());
        Ok(HttpClient {
            follow,
            no_follow,
            retry: self.retry,
            user_agent: ua,
        })
    }
}

fn build_inner(
    proxy: &Option<String>,
    cert_verify: bool,
    timeout: Duration,
    follow_redirects: bool,
) -> Result<Client> {
    let policy = if follow_redirects {
        Policy::default()
    } else {
        Policy::none()
    };
    let mut builder = Client::builder().timeout(timeout).redirect(policy);
    if !cert_verify {
        builder = builder.danger_accept_invalid_certs(true);
    }
    if let Some(proxy) = proxy {
        let proxy_url = format!("http://{proxy}");
        // Skip the proxy for localhost and plex.direct.
        let custom = Proxy::custom(move |url| {
            let host = url.host_str().unwrap_or("");
            let skip = host.starts_with("127.0.0.1")
                || host == "localhost"
                || url.as_str().contains("plex.direct");
            if skip {
                None
            } else {
                Url::parse(&proxy_url).ok()
            }
        });
        builder = builder.proxy(custom);
    }
    builder.build().map_err(|e| NetError::Build(e.to_string()))
}

/// An async HTTP client with etlp's request conventions baked in.
#[derive(Debug, Clone)]
pub struct HttpClient {
    follow: Client,
    no_follow: Client,
    retry: u32,
    /// Effective User-Agent for normal requests (resolved from config / default).
    user_agent: String,
}

impl HttpClient {
    /// A default client with no proxy, TLS verification enabled, and the
    /// built-in User-Agent.
    pub fn new() -> Result<Self> {
        HttpClientBuilder::new().build()
    }

    fn prepare(
        &self,
        client: &Client,
        method: Method,
        url: &str,
        params: &[(&str, &str)],
    ) -> reqwest::RequestBuilder {
        let safe = safe_url(url);
        let referer = build_referer(&safe);
        let mut rb = client.request(method, safe);
        if !params.is_empty() {
            rb = rb.query(params);
        }
        rb = rb.header(USER_AGENT, &self.user_agent);
        if let Some(referer) = referer {
            rb = rb.header(REFERER, referer);
        }
        rb
    }

    async fn send_with_retry(
        &self,
        url: &str,
        rb: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response> {
        let mut last: Option<reqwest::Error> = None;
        for _ in 0..self.retry {
            let Some(attempt) = rb.try_clone() else {
                // Non-cloneable (streaming) body: send once.
                return rb.send().await.map_err(|source| NetError::Request {
                    url: url.to_owned(),
                    tries: 1,
                    source,
                });
            };
            match attempt.send().await {
                Ok(resp) => return Ok(resp),
                Err(e) => last = Some(e),
            }
        }
        match last {
            Some(source) => Err(NetError::Request {
                url: url.to_owned(),
                tries: self.retry,
                source,
            }),
            None => Err(NetError::Build("retry count was zero".to_owned())),
        }
    }

    /// GET a URL and decode the JSON body into `T`.
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        params: &[(&str, &str)],
    ) -> Result<T> {
        log_curl(
            &self.user_agent,
            "GET",
            url,
            params,
            &[("Accept", "application/json")],
            None,
        );
        let rb = self
            .prepare(&self.follow, Method::GET, url, params)
            .header(ACCEPT, "application/json");
        let resp = self.send_with_retry(url, rb).await?;
        let resp = error_for_status(resp, url)?;
        resp.json::<T>().await.map_err(|source| NetError::Decode {
            url: url.to_owned(),
            source,
        })
    }

    /// GET a URL and return the body as text.
    pub async fn get_text(
        &self,
        url: &str,
        params: &[(&str, &str)],
    ) -> Result<String> {
        log_curl(&self.user_agent, "GET", url, params, &[], None);
        let rb = self.prepare(&self.follow, Method::GET, url, params);
        let resp = self.send_with_retry(url, rb).await?;
        let resp = error_for_status(resp, url)?;
        resp.text().await.map_err(|source| NetError::Decode {
            url: url.to_owned(),
            source,
        })
    }

    /// POST a JSON body. The response body is ignored (only success is
    /// checked), matching the common Python progress-write-back usage.
    pub async fn post_json<B: Serialize + ?Sized>(
        &self,
        url: &str,
        params: &[(&str, &str)],
        body: &B,
    ) -> Result<()> {
        let body_str = serde_json::to_string(body).unwrap_or_default();
        log_curl(
            &self.user_agent,
            "POST",
            url,
            params,
            &[
                ("Content-Type", "application/json; charset=utf-8"),
                ("Accept", "application/json"),
            ],
            Some(&body_str),
        );
        let rb = self
            .prepare(&self.follow, Method::POST, url, params)
            .header(CONTENT_TYPE, "application/json; charset=utf-8")
            .header(ACCEPT, "application/json")
            .json(body);
        let resp = self.send_with_retry(url, rb).await?;
        error_for_status(resp, url)?;
        Ok(())
    }

    /// Resolve a single redirect hop without following it, returning the
    /// `Location` target, or the original URL when there is no redirect
    /// (mirrors `net_tools.get_redirect_url`).
    pub async fn resolve_redirect(&self, url: &str) -> Result<String> {
        log_curl(&self.user_agent, "GET", url, &[], &[], None);
        let rb = self.prepare(&self.no_follow, Method::GET, url, &[]);
        let resp = self.send_with_retry(url, rb).await?;
        if resp.status().is_redirection()
            && let Some(loc) = resp.headers().get(LOCATION)
            && let Ok(target) = loc.to_str()
        {
            return Ok(target.to_owned());
        }
        Ok(url.to_owned())
    }
}

fn error_for_status(
    resp: reqwest::Response,
    url: &str,
) -> Result<reqwest::Response> {
    let status = resp.status();
    if status.is_success() {
        Ok(resp)
    } else {
        Err(NetError::Status {
            status: status.as_u16(),
            url: url.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use wiremock::matchers::{header_exists, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct Payload {
        a: i32,
        name: String,
    }

    #[tokio::test]
    async fn get_json_decodes_and_sends_browser_ua() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/item"))
            .and(header_exists("user-agent"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"a": 7, "name": "x"})),
            )
            .mount(&server)
            .await;

        let client = HttpClient::new().expect("client");
        let url = format!("{}/item", server.uri());
        let got: Payload = client.get_json(&url, &[]).await.expect("get_json");
        assert_eq!(
            got,
            Payload {
                a: 7,
                name: "x".to_owned()
            }
        );
    }

    #[tokio::test]
    async fn custom_user_agent_is_sent() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/item"))
            .and(wiremock::matchers::header("user-agent", "MyCustomUA/1.0"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"a": 1, "name": "y"})),
            )
            .mount(&server)
            .await;

        let client = HttpClientBuilder::new()
            .user_agent(Some("MyCustomUA/1.0".to_owned()))
            .build()
            .expect("client");
        let url = format!("{}/item", server.uri());
        let got: Payload = client.get_json(&url, &[]).await.expect("get_json");
        assert_eq!(got.name, "y");
    }

    #[tokio::test]
    async fn empty_user_agent_falls_back_to_default() {
        let client = HttpClientBuilder::new()
            .user_agent(Some(String::new()))
            .build()
            .expect("client");
        // Verify the effective UA is the built-in default, not empty.
        assert_eq!(client.user_agent, UA_ETLP);
    }

    #[tokio::test]
    async fn post_json_checks_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/Sessions/Playing/Stopped"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let client = HttpClient::new().expect("client");
        let url = format!("{}/Sessions/Playing/Stopped", server.uri());
        client
            .post_json(&url, &[], &serde_json::json!({"PositionTicks": 1}))
            .await
            .expect("post_json");
    }

    #[tokio::test]
    async fn non_success_status_is_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = HttpClient::new().expect("client");
        let url = format!("{}/missing", server.uri());
        let err = client.get_text(&url, &[]).await.unwrap_err();
        assert!(matches!(err, NetError::Status { status: 404, .. }));
    }

    #[tokio::test]
    async fn resolve_redirect_returns_location() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/stream"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("Location", "https://cdn.example.com/real"),
            )
            .mount(&server)
            .await;

        let client = HttpClient::new().expect("client");
        let url = format!("{}/stream", server.uri());
        let target = client.resolve_redirect(&url).await.expect("redirect");
        assert_eq!(target, "https://cdn.example.com/real");
    }

    #[tokio::test]
    async fn resolve_redirect_without_redirect_returns_original() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/direct"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = HttpClient::new().expect("client");
        let url = format!("{}/direct", server.uri());
        let target = client.resolve_redirect(&url).await.expect("redirect");
        assert_eq!(target, url);
    }
}
