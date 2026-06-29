//! Async HTTP client wrapping `reqwest`.
//!
//! Per-request conventions: User-Agent, `Referer` header, JSON headers, a
//! proxy that skips localhost/`plex.direct`, and a bounded retry loop. TLS
//! uses rustls so the binary needs no system OpenSSL.

use std::time::Duration;

use crate::UA_ETLP;
use crate::url_tools::{build_referer, safe_url};
use reqwest::header::{ACCEPT, CONTENT_TYPE, LOCATION, REFERER, USER_AGENT};
use reqwest::redirect::Policy;
use reqwest::{Client, Method, Proxy};
use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;

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
#[derive(Debug, Clone, Default)]
pub struct HttpClientBuilder {
    /// Proxy for HTTP traffic.  Full URL: `http://host:port`.
    proxy_http: Option<String>,
    /// Proxy for HTTPS traffic (CONNECT tunnel).  Usually `http://host:port`.
    proxy_https: Option<String>,
    /// When `false`, all proxy config is ignored and connections are direct.
    proxy_enabled: bool,
    cert_verify: bool,
    timeout: Option<Duration>,
    retry: u32,
    /// User-Agent for normal requests. `None` → [`UA_ETLP`].
    user_agent: Option<String>,
}

impl HttpClientBuilder {
    /// Start from defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            proxy_enabled: true,
            cert_verify: true,
            timeout: None,
            retry: DEFAULT_RETRY,
            ..Default::default()
        }
    }

    /// Proxy for HTTP traffic (full URL: `http://host:port`).
    #[must_use]
    pub fn proxy_http(mut self, proxy: Option<String>) -> Self {
        self.proxy_http = proxy;
        self
    }

    /// Proxy for HTTPS traffic (full URL: `http://host:port` or `https://...`).
    #[must_use]
    pub fn proxy_https(mut self, proxy: Option<String>) -> Self {
        self.proxy_https = proxy;
        self
    }

    /// Whether any configured proxy is active.
    #[must_use]
    pub fn proxy_enabled(mut self, enabled: bool) -> Self {
        self.proxy_enabled = enabled;
        self
    }

    /// Whether to verify TLS certificates.
    #[must_use]
    pub fn cert_verify(mut self, verify: bool) -> Self {
        self.cert_verify = verify;
        self
    }

    /// Default per-request timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
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
    /// Empty strings fall back to [`UA_ETLP`].
    #[must_use]
    pub fn user_agent(mut self, ua: Option<String>) -> Self {
        self.user_agent = ua.filter(|s| !s.is_empty());
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<HttpClient> {
        let timeout = self.timeout.unwrap_or(DEFAULT_TIMEOUT);
        let proxies = if self.proxy_enabled {
            Some((self.proxy_http, self.proxy_https))
        } else {
            tracing::debug!("http_client: proxy disabled");
            None
        };
        let follow = build_inner(&proxies, self.cert_verify, timeout, true)?;
        let no_follow =
            build_inner(&proxies, self.cert_verify, timeout, false)?;
        let ua = self.user_agent.unwrap_or_else(|| UA_ETLP.to_owned());
        Ok(HttpClient {
            follow,
            no_follow,
            retry: self.retry,
            user_agent: ua,
        })
    }
}

/// Return `true` for addresses that should always bypass the proxy:
/// loopback, RFC 1918 private ranges, and Plex-direct hostnames.
fn is_bypass_host(host: &str, full_url: &str) -> bool {
    if host == "localhost" || host == "::1" {
        return true;
    }
    if full_url.contains("plex.direct") {
        return true;
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => {
                // Covers loopback (127.x), private (10.x/172.16-31.x/192.168.x),
                // and link-local (169.254.x).
                v4.is_loopback() || v4.is_private() || v4.is_link_local()
            }
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback() || v6.is_unicast_link_local()
            }
        };
    }
    false
}

type ProxyPair = (Option<String>, Option<String>);

fn normalize_proxy_url(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.contains("://") {
        return Some(raw.to_owned());
    }
    Some(format!("http://{raw}"))
}

fn build_inner(
    proxies: &Option<ProxyPair>,
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
    if let Some((ph, ps)) = proxies {
        if ph.as_deref().or(ps.as_deref()).is_some() {
            tracing::debug!(
                http = ?ph.as_deref(),
                https = ?ps.as_deref(),
                "http_client: proxy configured"
            );
        } else {
            tracing::debug!("http_client: no proxy configured");
        }
        if let Some(custom) = custom_proxy(ph, ps) {
            builder = builder.proxy(custom);
        }
    } else {
        tracing::debug!("http_client: no proxy configured");
    }
    builder.build().map_err(|e| NetError::Build(e.to_string()))
}

/// Build the bypass-aware custom proxy, or `None` when no proxy URL is set.
///
/// The returned [`Proxy`] routes by request scheme and skips
/// local/private/`plex.direct` hosts via [`is_bypass_host`].
fn custom_proxy(
    proxy_http: &Option<String>,
    proxy_https: &Option<String>,
) -> Option<Proxy> {
    if proxy_http.is_none() && proxy_https.is_none() {
        return None;
    }
    let ph = proxy_http.clone();
    let ps = proxy_https.clone();
    Some(Proxy::custom(move |url| {
        let host = url.host_str().unwrap_or("");
        if is_bypass_host(host, url.as_str()) {
            tracing::debug!(
                target_host = %host,
                "http_client: proxy bypassed (local/private/plex)"
            );
            return None;
        }
        // Route by request scheme.
        let candidate: Option<&str> = match url.scheme() {
            "http" => ph.as_deref(),
            "https" => ps.as_deref(),
            _ => None,
        };
        candidate.and_then(|proxy_url| {
            tracing::debug!(
                target_url = %url,
                proxy = %proxy_url,
                "http_client: routing via proxy"
            );
            normalize_proxy_url(proxy_url)
                .and_then(|url| url.parse::<url::Url>().ok())
        })
    }))
}

/// Build a single proxied [`reqwest::Client`] for streaming media downloads.
///
/// Applies the same local/private-host bypass as [`HttpClient`] and the
/// `UA_DOWNLOAD` user agent. No total request timeout is set so large transfers
/// are not cut off. When `proxy_enabled` is `false` the client connects
/// directly regardless of the configured proxy URLs.
pub fn build_media_download_client(
    proxy_http: Option<String>,
    proxy_https: Option<String>,
    proxy_enabled: bool,
    cert_verify: bool,
) -> Result<Client> {
    let mut builder = Client::builder().user_agent(crate::UA_DOWNLOAD);
    if !cert_verify {
        builder = builder.danger_accept_invalid_certs(true);
    }
    if proxy_enabled
        && let Some(proxy) = custom_proxy(&proxy_http, &proxy_https)
    {
        builder = builder.proxy(proxy);
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
        let net_start = std::time::Instant::now();
        let rb = self
            .prepare(&self.follow, Method::GET, url, params)
            .header(ACCEPT, "application/json");
        let resp = self.send_with_retry(url, rb).await;
        let elapsed = net_start.elapsed().as_millis();
        tracing::debug!(
            method = "GET_JSON",
            url,
            elapsed_ms = elapsed,
            ok = resp.is_ok(),
            "http_elapsed"
        );
        let resp = error_for_status(resp?, url)?;
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
        let net_start = std::time::Instant::now();
        let rb = self.prepare(&self.follow, Method::GET, url, params);
        let resp = self.send_with_retry(url, rb).await;
        let elapsed = net_start.elapsed().as_millis();
        tracing::debug!(
            method = "GET_TEXT",
            url,
            elapsed_ms = elapsed,
            ok = resp.is_ok(),
            "http_elapsed"
        );
        let resp = error_for_status(resp?, url)?;
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
        let net_start = std::time::Instant::now();
        let rb = self
            .prepare(&self.follow, Method::POST, url, params)
            .header(CONTENT_TYPE, "application/json; charset=utf-8")
            .header(ACCEPT, "application/json")
            .json(body);
        let resp = self.send_with_retry(url, rb).await;
        let elapsed = net_start.elapsed().as_millis();
        tracing::debug!(
            method = "POST_JSON",
            url,
            elapsed_ms = elapsed,
            ok = resp.is_ok(),
            "http_elapsed"
        );
        error_for_status(resp?, url)?;
        Ok(())
    }

    /// Resolve a single redirect hop without following it, returning the
    /// `Location` target, or the original URL when there is no redirect
    /// (mirrors `net_tools.get_redirect_url`).
    pub async fn resolve_redirect(&self, url: &str) -> Result<String> {
        log_curl(&self.user_agent, "GET", url, &[], &[], None);
        let net_start = std::time::Instant::now();
        let rb = self.prepare(&self.no_follow, Method::GET, url, &[]);
        let resp = self.send_with_retry(url, rb).await;
        let elapsed = net_start.elapsed().as_millis();
        tracing::debug!(
            method = "GET_REDIRECT",
            url,
            elapsed_ms = elapsed,
            ok = resp.is_ok(),
            "http_elapsed"
        );
        let resp = resp?;
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

    #[test]
    fn media_download_client_builds_with_and_without_proxy() {
        // Proxy disabled: direct client builds even with URLs present.
        assert!(
            build_media_download_client(
                Some("127.0.0.1:7890".to_owned()),
                None,
                false,
                true,
            )
            .is_ok()
        );
        // Proxy enabled with valid URLs builds successfully.
        assert!(
            build_media_download_client(
                Some("127.0.0.1:7890".to_owned()),
                Some("http://127.0.0.1:7890".to_owned()),
                true,
                true,
            )
            .is_ok()
        );
        // Enabled but no URLs is a direct client (no proxy attached).
        assert!(build_media_download_client(None, None, true, true).is_ok());
    }

    #[test]
    fn proxy_url_accepts_host_port_and_full_url() {
        assert_eq!(
            normalize_proxy_url("127.0.0.1:7890").as_deref(),
            Some("http://127.0.0.1:7890")
        );
        assert_eq!(
            normalize_proxy_url("http://127.0.0.1:7890").as_deref(),
            Some("http://127.0.0.1:7890")
        );
        assert_eq!(normalize_proxy_url("   "), None);
    }

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
