//! Cancellation behaviour for [`connectors_sdk::HttpClient`].
//!
//! Connectors poll `ctx.cancel` between batches, but the SDK itself
//! must also wake up promptly when a cancellation arrives during a
//! retry sleep or an in-flight request. Otherwise a user pressing
//! "Cancel" while a 429 backoff is pending would sit through the full
//! sleep — exactly the silent-delay failure mode the architecture
//! prohibits.

use std::time::Duration;

use connectors_sdk::{HttpClient, RetryPolicy};
use dayseam_core::DayseamError;
use dayseam_events::RunStreams;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn cancellation_during_backoff_aborts_retry_loop() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/always429"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    // The backoff is long (30 s) so the test would hang for 30 s if
    // cancellation were ignored. The timeout guard below confirms we
    // actually short-circuit.
    let client = HttpClient::new().expect("build").with_policy(RetryPolicy {
        max_attempts: 10,
        base_backoff: Duration::from_secs(30),
        max_backoff: Duration::from_secs(60),
        jitter_frac: 0.0,
    });
    let cancel = CancellationToken::new();
    let streams = RunStreams::new(dayseam_events::RunId::new());
    let ((progress_tx, log_tx), _rx) = streams.split();

    let send_fut = {
        let cancel = cancel.clone();
        let client = client.clone();
        let url = format!("{}/always429", server.uri());
        let progress_tx = progress_tx.clone();
        let log_tx = log_tx.clone();
        tokio::spawn(async move {
            client
                .send(
                    client.reqwest().get(&url),
                    &cancel,
                    Some(&progress_tx),
                    Some(&log_tx),
                )
                .await
        })
    };

    // Give the first attempt a moment to hit 429 and enter the sleep,
    // then cancel.
    tokio::time::sleep(Duration::from_millis(200)).await;
    cancel.cancel();

    let result = tokio::time::timeout(Duration::from_secs(5), send_fut)
        .await
        .expect("send must return within 5s of cancel (not wait for the 30s backoff)")
        .expect("join ok");

    match result {
        Err(DayseamError::Cancelled { .. }) => {}
        other => panic!("expected Cancelled, got {other:?}"),
    }
}

#[tokio::test]
async fn cancellation_before_send_aborts_without_network_call() {
    let client = HttpClient::new()
        .expect("build")
        .with_policy(RetryPolicy::instant());
    let cancel = CancellationToken::new();
    cancel.cancel();

    // URL is intentionally invalid — if the client didn't short-circuit
    // on the pre-cancelled token, it would either error as
    // DayseamError::Network or panic trying to resolve the host.
    let err = client
        .send(
            client.reqwest().get("http://must-not-be-reached.invalid/"),
            &cancel,
            None,
            None,
        )
        .await
        .expect_err("pre-cancelled run must not issue HTTP");
    assert!(matches!(err, DayseamError::Cancelled { .. }));
}
