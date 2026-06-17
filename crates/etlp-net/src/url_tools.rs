//! URL helpers ported from `net_tools.py` (`safe_url`, Referer building).

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use url::Url;

/// Characters left unescaped when encoding a URL path: the RFC 3986 unreserved
/// set plus `/` (path separator) and `%` (so already-encoded input is kept),
/// matching Python's `quote(path, safe="/%")`.
const PATH_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'_')
    .remove(b'.')
    .remove(b'-')
    .remove(b'~')
    .remove(b'/')
    .remove(b'%');

/// Percent-encode only the path component of a URL, leaving scheme, authority,
/// query and fragment untouched. Mirrors `net_tools.safe_url`, making URLs with
/// spaces or non-ASCII characters in the path safe to request.
#[must_use]
pub fn safe_url(url: &str) -> String {
    let (before_frag, fragment) = match url.split_once('#') {
        Some((a, b)) => (a, Some(b)),
        None => (url, None),
    };
    let (before_query, query) = match before_frag.split_once('?') {
        Some((a, b)) => (a, Some(b)),
        None => (before_frag, None),
    };

    let (prefix, path) = match before_query.split_once("://") {
        Some((scheme, rest)) => match rest.split_once('/') {
            Some((authority, path_rest)) => {
                (format!("{scheme}://{authority}/"), path_rest)
            }
            None => (format!("{scheme}://{rest}"), ""),
        },
        None => (String::new(), before_query),
    };

    let mut out = prefix;
    out.push_str(&utf8_percent_encode(path, PATH_SET).to_string());
    if let Some(q) = query {
        out.push('?');
        out.push_str(q);
    }
    if let Some(f) = fragment {
        out.push('#');
        out.push_str(f);
    }
    out
}

/// Build the `Referer` header value used by the Python client:
/// `<scheme>://<host>[:<port>]/web/index.html`. Returns `None` if the URL
/// cannot be parsed.
#[must_use]
pub fn build_referer(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    let mut referer = format!("{}://{}", parsed.scheme(), host);
    if let Some(port) = parsed.port() {
        referer.push_str(&format!(":{port}"));
    }
    referer.push_str("/web/index.html");
    Some(referer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_url_encodes_spaces_and_unicode_in_path() {
        let got = safe_url("https://h:8096/videos/movie (2020).mkv?x=1");
        assert_eq!(got, "https://h:8096/videos/movie%20%282020%29.mkv?x=1");
    }

    #[test]
    fn safe_url_keeps_already_encoded_and_query() {
        // '%' is preserved, query and fragment are untouched.
        let got = safe_url("https://h/a%20b/c?q=a b#frag x");
        assert_eq!(got, "https://h/a%20b/c?q=a b#frag x");
    }

    #[test]
    fn safe_url_without_path_or_scheme() {
        assert_eq!(safe_url("https://host:8096"), "https://host:8096");
        assert_eq!(safe_url("/local path/x"), "/local%20path/x");
    }

    #[test]
    fn referer_includes_explicit_port_only() {
        assert_eq!(
            build_referer("https://media.example.com:8096/emby/x"),
            Some("https://media.example.com:8096/web/index.html".to_owned())
        );
        assert_eq!(
            build_referer("https://media.example.com/emby/x"),
            Some("https://media.example.com/web/index.html".to_owned())
        );
    }

    #[test]
    fn referer_none_for_unparseable() {
        assert_eq!(build_referer("not a url"), None);
    }
}
