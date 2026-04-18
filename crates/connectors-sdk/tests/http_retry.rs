//! End-to-end retry behaviour of [`connectors_sdk::HttpClient`].
//!
//! The wiremock server is the only "upstream" that matters here. Each
//! test asserts three things:
//!
//! 1. The client actually retried the configured number of times.
//! 2. The retry loop emitted a progress event per backoff (the "never
//!    fail silently" rule).
//! 3. The final response is either a success or a well-typed
//!    `DayseamError` — no silent swallowing.

use std::time::Duration;

use connectors_sdk::{HttpClient, RetryPolicy};
use dayseam_core::{DayseamError, ProgressPhase};
use dayseam_events::{RunId, RunStreams};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn drain_progress(
    mut rx: dayseam_events::ProgressReceiver,
) -> Vec<dayseam_events::ProgressEvent> {
    let mut out = Vec::new();
    while let Some(evt) = rx.recv().await {
        out.push(evt);
    }
    out
}

#[tokio::test]
async fn client_retries_until_success_after_429s() {
    let server = MockServer::start().await;

    // First two requests get 429, third gets 200.
    Mock::given(method("GET"))
        .and(path("/flaky"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .up_to_n_times(2)
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/flaky"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&server)
        .await;

    let client = HttpClient::new()
        .expect("build")
        .with_policy(RetryPolicy::instant());
    let streams = RunStreams::new(RunId::new());
    let ((progress_tx, log_tx), (progress_rx, _log_rx)) = streams.split();
    let cancel = CancellationToken::new();

    let res = client
        .send(
            client.reqwest().get(server.uri() + "/flaky"),
            &cancel,
            Some(&progress_tx),
            Some(&log_tx),
        )
        .await
        .expect("eventually succeeds");
    assert_eq!(res.status(), 200);

    // Close senders so `drain_progress` can terminate.
    drop(progress_tx);
    drop(log_tx);
    let events = drain_progress(progress_rx).await;
    assert_eq!(
        events.len(),
        2,
        "expected one InProgress event per retry, got {events:#?}"
    );
    assert!(matches!(events[0].phase, ProgressPhase::InProgress { .. }));
    assert!(matches!(events[1].phase, ProgressPhase::InProgress { .. }));
}

#[tokio::test]
async fn client_gives_up_with_rate_limited_error_after_budget_exhausted() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/always429"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
        .mount(&server)
        .await;

    let client = HttpClient::new().expect("build").with_policy(RetryPolicy {
        max_attempts: 3,
        base_backoff: Duration::from_millis(0),
        max_backoff: Duration::from_millis(0),
        jitter_frac: 0.0,
    });
    let streams = RunStreams::new(RunId::new());
    let ((progress_tx, log_tx), _rx) = streams.split();

    let err = client
        .send(
            client.reqwest().get(server.uri() + "/always429"),
            &CancellationToken::new(),
            Some(&progress_tx),
            Some(&log_tx),
        )
        .await
        .expect_err("should give up");
    match err {
        DayseamError::RateLimited {
            retry_after_secs, ..
        } => assert_eq!(retry_after_secs, 1),
        other => panic!("expected RateLimited, got {other:?}"),
    }
}

#[tokio::test]
async fn client_retries_5xx_and_eventually_returns_network_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/always500"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client = HttpClient::new()
        .expect("build")
        .with_policy(RetryPolicy::instant());
    let streams = RunStreams::new(RunId::new());
    let ((progress_tx, log_tx), _rx) = streams.split();

    let err = client
        .send(
            client.reqwest().get(server.uri() + "/always500"),
            &CancellationToken::new(),
            Some(&progress_tx),
            Some(&log_tx),
        )
        .await
        .expect_err("should give up");
    assert!(matches!(err, DayseamError::Network { .. }));
    assert_eq!(
        err.code(),
        dayseam_core::error_codes::HTTP_RETRY_BUDGET_EXHAUSTED
    );
}

#[tokio::test]
async fn non_retriable_status_returns_immediately_without_retries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/gone"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;

    let client = HttpClient::new()
        .expect("build")
        .with_policy(RetryPolicy::instant());
    let streams = RunStreams::new(RunId::new());
    let ((progress_tx, log_tx), (progress_rx, _log_rx)) = streams.split();

    let err = client
        .send(
            client.reqwest().get(server.uri() + "/gone"),
            &CancellationToken::new(),
            Some(&progress_tx),
            Some(&log_tx),
        )
        .await
        .expect_err("404 is a hard failure");
    assert!(matches!(err, DayseamError::Network { .. }));

    drop(progress_tx);
    drop(log_tx);
    let events = drain_progress(progress_rx).await;
    assert!(
        events.is_empty(),
        "non-retriable error must not emit InProgress events"
    );
}
