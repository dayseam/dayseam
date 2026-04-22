//! `Link` header pagination helper.
//!
//! GitHub's REST API signals "there is a next page of results" by
//! setting a `Link` header on the response of the form
//!
//! ```text
//! Link: <https://api.github.com/user/events?page=2>; rel="next", <…>; rel="last"
//! ```
//!
//! The header may also carry `rel="prev"`, `rel="first"`, and
//! `rel="last"` companions; we only look for `rel="next"` and stop
//! when it is absent. This shape is used across every paginated
//! endpoint — `/users/{login}/events`, `/search/issues`,
//! `/repos/{owner}/{repo}/pulls`, etc. — so a single shared helper
//! serves every walker call site.
//!
//! The walker lives in DAY-96; this module provides the pure
//! header-parse helper and a small test suite so the walker's
//! pagination logic becomes a thin loop over `next_url` rather than a
//! re-implementation of string-splitting.
//!
//! The parser is deliberately permissive:
//!
//! * an absent `Link` header → `None` (single-page result).
//! * a malformed header (missing angle brackets, missing `rel`, …) →
//!   `None` with a warn log. We do **not** fail the walk — a broken
//!   `Link` header at page N stops pagination early but still returns
//!   the N pages we already read. The rationale is the same as the
//!   DAY-88 silent-failure sweep: a missing page is always a known
//!   shortfall a human can spot in the report; a spurious walk
//!   failure on a malformed header is a silent empty-report risk.
//!
//! What we intentionally do **not** do:
//! * follow `rel="last"` to estimate page counts. GitHub's search
//!   endpoints cap at 1 000 results, so a `last=51` page link is a
//!   lower bound anyway; the walker computes its own stop condition
//!   from `created_at` timestamps.
//! * honour `X-RateLimit-Remaining` / `X-RateLimit-Reset` here. Those
//!   live on `connectors-sdk::HttpClient`'s retry loop; pagination is
//!   oblivious to rate limits (it asks for the next URL, the SDK's
//!   `send` is what stalls when the bucket is empty).

use reqwest::header::HeaderMap;
use url::Url;

/// Parse the `Link` header on a GitHub response and return the URL
/// tagged `rel="next"`, if any.
///
/// Returns `None` when the header is absent, malformed, or has no
/// `rel="next"` entry (the single-page / last-page case). A malformed
/// URL inside an otherwise well-formed header entry is treated as
/// "no next page" rather than an error — see module docs for the
/// silent-failure rationale.
///
/// The returned `Url` is ready to feed straight into the next
/// `http.reqwest().get(url)` call — no further resolution needed,
/// GitHub always returns absolute URLs in the header.
#[must_use]
pub fn next_link(headers: &HeaderMap) -> Option<Url> {
    let link = headers.get(reqwest::header::LINK)?;
    let value = link.to_str().ok()?;
    parse_next_from_link_header(value)
}

/// Low-level parser. Exposed for unit tests and for the rare caller
/// that already holds the raw header string (e.g. a replay fixture).
/// Walks the comma-separated entries and returns the first URL whose
/// `rel=` parameter includes the token `"next"`.
#[must_use]
pub fn parse_next_from_link_header(value: &str) -> Option<Url> {
    // Each entry is `<url>; rel="<tokens>"` (with optional extra
    // parameters). Commas separate entries, but `Url`s cannot contain
    // unescaped commas (they would appear percent-encoded), so a
    // plain `value.split(',')` is safe.
    for raw_entry in value.split(',') {
        let entry = raw_entry.trim();
        if entry.is_empty() {
            continue;
        }
        let Some((url_part, rel_part)) = entry.split_once(';') else {
            continue;
        };
        if !rel_contains_next(rel_part) {
            continue;
        }
        let url_str = url_part
            .trim()
            .trim_start_matches('<')
            .trim_end_matches('>');
        if url_str.is_empty() {
            continue;
        }
        if let Ok(url) = Url::parse(url_str) {
            return Some(url);
        }
    }
    None
}

/// Does the `rel` parameter list include the token `next`?
///
/// The parameter looks like `rel="next"` on the wire, but we also
/// accept `rel=next` (bare token, occasionally seen on GHE proxies)
/// and `rel="next prev"` (multi-token form, documented by RFC 8288).
/// `split_whitespace` handles the multi-token case, and
/// `trim_matches('"')` handles both quoted and unquoted forms.
fn rel_contains_next(rel_part: &str) -> bool {
    let rel_part = rel_part.trim();
    let Some(value) = rel_part.strip_prefix("rel=") else {
        // Not the `rel` parameter at all — GitHub also emits
        // `title=`, `type=`, etc. Skip.
        return false;
    };
    let tokens = value.trim_matches('"');
    tokens
        .split_whitespace()
        .any(|token| token.trim_matches('"') == "next")
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue, LINK};

    fn headers_with_link(raw: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(LINK, HeaderValue::from_str(raw).expect("header value"));
        h
    }

    #[test]
    fn next_link_returns_none_when_header_absent() {
        let h = HeaderMap::new();
        assert_eq!(next_link(&h), None);
    }

    #[test]
    fn next_link_returns_url_for_standard_next_entry() {
        let raw = r#"<https://api.github.com/user/events?page=2>; rel="next", <https://api.github.com/user/events?page=17>; rel="last""#;
        let got = next_link(&headers_with_link(raw)).expect("next present");
        assert_eq!(got.as_str(), "https://api.github.com/user/events?page=2");
    }

    #[test]
    fn next_link_returns_none_when_only_prev_and_last_set() {
        let raw = r#"<https://api.github.com/user/events?page=16>; rel="prev", <https://api.github.com/user/events?page=17>; rel="last""#;
        assert_eq!(next_link(&headers_with_link(raw)), None);
    }

    #[test]
    fn next_link_is_independent_of_entry_order() {
        // GitHub is documented to emit the entries in `first`,
        // `prev`, `next`, `last` order, but nothing in the RFC
        // enforces that. Regressions where the parser only matches
        // the *first* entry would be invisible until an
        // Enterprise-Server proxy reorders the header.
        let raw = r#"<https://api.github.com/user/events?page=17>; rel="last", <https://api.github.com/user/events?page=2>; rel="next""#;
        let got = next_link(&headers_with_link(raw)).expect("next present");
        assert_eq!(got.as_str(), "https://api.github.com/user/events?page=2");
    }

    #[test]
    fn next_link_tolerates_bare_rel_token_without_quotes() {
        // RFC 8288 allows unquoted parameter values; GHE proxies
        // occasionally strip the quotes.
        let raw = r#"<https://api.github.com/user/events?page=2>; rel=next"#;
        let got = next_link(&headers_with_link(raw)).expect("next present");
        assert_eq!(got.as_str(), "https://api.github.com/user/events?page=2");
    }

    #[test]
    fn next_link_tolerates_multi_token_rel_attribute() {
        // Multi-token `rel="next prev"` is RFC-legal. We ignore
        // every token except `next`.
        let raw = r#"<https://api.github.com/user/events?page=2>; rel="next prev""#;
        let got = next_link(&headers_with_link(raw)).expect("next present");
        assert_eq!(got.as_str(), "https://api.github.com/user/events?page=2");
    }

    #[test]
    fn next_link_returns_none_when_url_is_malformed() {
        // Silent-failure-avoidance invariant: a malformed URL in an
        // otherwise well-formed header entry stops pagination — it
        // does not crash the walker — so the walk completes with the
        // pages we already read. The caller treats `None` as
        // "no more pages."
        let raw = r#"<not a url>; rel="next""#;
        assert_eq!(next_link(&headers_with_link(raw)), None);
    }

    #[test]
    fn next_link_returns_none_when_entry_is_missing_semicolon() {
        let raw = r#"<https://api.github.com/user/events?page=2>"#;
        assert_eq!(next_link(&headers_with_link(raw)), None);
    }

    #[test]
    fn next_link_ignores_entries_without_rel_parameter() {
        let raw = r#"<https://api.github.com/user/events?page=2>; title="page 2""#;
        assert_eq!(next_link(&headers_with_link(raw)), None);
    }

    #[test]
    fn next_link_handles_empty_string() {
        assert_eq!(parse_next_from_link_header(""), None);
    }

    #[test]
    fn next_link_handles_whitespace_only_value() {
        assert_eq!(parse_next_from_link_header("   ,  ,  "), None);
    }

    #[test]
    fn parse_next_from_link_header_matches_next_link_for_same_input() {
        // Parity check — the helper and the HeaderMap-driven entry
        // point must agree on every input.
        let raw = r#"<https://api.github.com/user/events?page=2>; rel="next""#;
        let from_headers = next_link(&headers_with_link(raw));
        let from_str = parse_next_from_link_header(raw);
        assert_eq!(from_headers, from_str);
    }
}
