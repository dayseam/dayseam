//! Wiremock-driven integration test for
//! [`connector_github::pagination::next_link`].
//!
//! The unit tests in `pagination.rs` exercise the parser against
//! synthetic `HeaderMap`s — they are fast, exhaustive, and pinpoint a
//! parse regression. What they do *not* cover is the real-world
//! "fetch the page, read `response.headers()`, feed it to
//! `next_link`" flow, because no actual HTTP response ever transits
//! them. A bug where `reqwest` normalises the header name, joins
//! multiple header lines with `, `, or drops a trailing comma would
//! pass every unit test but break the walker at runtime.
//!
//! This file pins that seam. We stand up a wiremock, return a
//! documented two-entry `Link` header, call it through the real
//! `HttpClient::send` path, and confirm `next_link` recovers the
//! `rel="next"` URL. That is the smallest test that would catch a
//! reqwest header-handling regression without requiring the (not yet
//! written) walker.
//!
//! DAY-96 will add the end-to-end walker integration test that loops
//! until `next_link` returns `None`; this test only verifies the
//! HTTP ↔ parser boundary in isolation.

use connector_github::pagination::next_link;
use connectors_sdk::{AuthStrategy, HttpClient, PatAuth};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn next_link_recovers_rel_next_from_real_reqwest_response() {
    let server = MockServer::start().await;
    // Two-entry `Link` header mirroring the shape documented at
    // https://docs.github.com/en/rest/guides/using-pagination-in-the-rest-api
    // — `rel="next"` first, `rel="last"` second. We assert on the
    // `next` URL coming back verbatim, including the `page=2` query.
    let raw_link = r#"<https://api.github.com/user/events?page=2>; rel="next", <https://api.github.com/user/events?page=17>; rel="last""#;
    Mock::given(method("GET"))
        .and(path("/user/events"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Link", raw_link)
                .set_body_json(serde_json::json!([])),
        )
        .mount(&server)
        .await;

    let http = HttpClient::new().expect("http client");
    let auth = PatAuth::github("ghp-test", "dayseam.github", "probe");
    let url = format!("{}/user/events", server.uri().trim_end_matches('/'));
    let req = http
        .reqwest()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    let req = auth.authenticate(req).await.expect("auth applies");

    let resp = http
        .send(req, &CancellationToken::new(), None, None)
        .await
        .expect("send succeeds");
    assert!(resp.status().is_success(), "expected 200 from mock server");

    let got = next_link(resp.headers()).expect("next link present in real response");
    assert_eq!(
        got.as_str(),
        "https://api.github.com/user/events?page=2",
        "next_link must return the rel=\"next\" URL verbatim from a real reqwest response"
    );
}

#[tokio::test]
async fn next_link_returns_none_when_response_omits_link_header() {
    // The last page of a multi-page walk (and every single-page walk)
    // returns no `Link` header at all. The walker depends on this
    // returning `None` to terminate its loop without an extra probe.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user/events"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let http = HttpClient::new().expect("http client");
    let auth = PatAuth::github("ghp-test", "dayseam.github", "probe");
    let url = format!("{}/user/events", server.uri().trim_end_matches('/'));
    let req = http.reqwest().get(&url);
    let req = auth.authenticate(req).await.expect("auth applies");

    let resp = http
        .send(req, &CancellationToken::new(), None, None)
        .await
        .expect("send succeeds");
    assert!(resp.status().is_success());
    assert_eq!(
        next_link(resp.headers()),
        None,
        "absent Link header must yield None so the walker terminates"
    );
}
