//! End-to-end integration test for the PKCE loopback login flow
//! introduced in DAY-201 PR #2.
//!
//! The shape of the flow is:
//!
//! ```text
//!   UI                                  Rust (this crate)               IdP (wiremock)
//!    │  oauth_begin_login               │                                │
//!    │─────────────────────────────────▶│                                │
//!    │                                  │ bind 127.0.0.1:0               │
//!    │                                  │ mint PKCE + state              │
//!    │                                  │ browser.open(authorize_url) ──▶│ (recorded)
//!    │                                  │ spawn driver                   │
//!    │  OAuthSessionView{Pending}       │                                │
//!    │◀─────────────────────────────────│                                │
//!    │                                  │                                │
//!    │  (real browser) GET redirect_uri?code=XYZ&state=NONCE             │
//!    │                                  │ driver validates state         │
//!    │                                  │ POST /token (code+verifier) ──▶│
//!    │                                  │◀──────────────── TokenResponse │
//!    │                                  │ registry.set_token_pair(...)   │
//!    │                                  │ status = Completed             │
//!    │                                  │ emitter.emit(...)              │
//!    │                                  │                                │
//!    │  oauth_session_status            │                                │
//!    │─────────────────────────────────▶│                                │
//!    │  OAuthSessionView{Completed}     │                                │
//!    │◀─────────────────────────────────│                                │
//! ```
//!
//! This test drives the arrows that carry `oauth_begin_login`, the
//! recorded `browser.open`, the fake HTTP GET to the loopback
//! listener, the `POST /token` round trip against a
//! [`wiremock::MockServer`], and the final `oauth_session_status`
//! assertion. We use the `test-helpers` feature gate so the plain
//! `cargo test -p dayseam-desktop` remains fast; CI's
//! `cargo test --workspace --all-features` includes it.

#![cfg(feature = "test-helpers")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use connectors_sdk::{HttpClient, RetryPolicy};
use dayseam_core::{OAuthSessionId, OAuthSessionStatus, OAuthSessionView};
use dayseam_db::open;
use dayseam_desktop::ipc::oauth::{
    oauth_begin_login_impl, oauth_cancel_login_impl, oauth_session_status_impl, BrowserOpener,
    SessionEmitter,
};
use dayseam_desktop::oauth_config::OAuthProviderConfig;
use dayseam_desktop::AppState;
use dayseam_events::AppBus;
use dayseam_orchestrator::{ConnectorRegistry, OrchestratorBuilder, SinkRegistry};
use dayseam_secrets::InMemoryStore;
use serde_json::json;
use tempfile::TempDir;
use url::Url;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------- test fixtures ----------

/// Recording [`BrowserOpener`] — when `oauth_begin_login_impl` would
/// shell out to the system browser, this captures the exact URL
/// instead so the test can parse the redirect URI back out of it and
/// replay the callback itself.
#[derive(Default, Clone)]
struct RecordingBrowserOpener {
    opened: Arc<Mutex<Vec<String>>>,
}

impl RecordingBrowserOpener {
    fn last(&self) -> Option<String> {
        self.opened.lock().unwrap().last().cloned()
    }
}

impl BrowserOpener for RecordingBrowserOpener {
    fn open(&self, url: &str) -> Result<(), std::io::Error> {
        self.opened.lock().unwrap().push(url.to_string());
        Ok(())
    }
}

/// Recording [`SessionEmitter`] — stashes every `OAuthSessionView`
/// the driver would have fired as an `oauth://session-updated` event
/// so the test can assert on the transition sequence.
#[derive(Default, Clone)]
struct RecordingEmitter {
    views: Arc<Mutex<Vec<OAuthSessionView>>>,
}

impl RecordingEmitter {
    fn snapshot(&self) -> Vec<OAuthSessionView> {
        self.views.lock().unwrap().clone()
    }
}

impl SessionEmitter for RecordingEmitter {
    fn emit(&self, view: &OAuthSessionView) {
        self.views.lock().unwrap().push(view.clone());
    }
}

/// Build an `AppState` sufficient for driving the OAuth IPC flow.
/// The Tauri shell's heavier wiring (scheduler, tray) is not
/// reachable from these helpers, which is fine — the OAuth IPC only
/// touches `state.http` and `state.oauth_sessions`.
async fn make_state() -> (AppState, TempDir) {
    let dir = TempDir::new().expect("temp dir");
    let pool = open(&dir.path().join("state.db"))
        .await
        .expect("open sqlite");
    let app_bus = AppBus::new();
    let orchestrator = OrchestratorBuilder::new(
        pool.clone(),
        app_bus.clone(),
        ConnectorRegistry::new(),
        SinkRegistry::new(),
    )
    .build()
    .expect("build orchestrator");
    let http = HttpClient::new()
        .expect("build HttpClient")
        .with_policy(RetryPolicy::instant());
    let state = AppState::with_http_for_test(
        pool,
        app_bus,
        Arc::new(InMemoryStore::new()),
        orchestrator,
        http,
    );
    (state, dir)
}

/// Construct an [`OAuthProviderConfig`] pointing the authorize and
/// token endpoints at a local wiremock. The authorize endpoint is
/// not actually fetched from the Rust side — the test simulates the
/// browser tab by replaying the callback directly — so it only has
/// to parse as a URL.
fn config_against(mock: &MockServer) -> OAuthProviderConfig {
    OAuthProviderConfig {
        id: "microsoft-outlook".to_string(),
        authorization_endpoint: format!("{}/authorize", mock.uri()),
        token_endpoint: format!("{}/token", mock.uri()),
        scopes: vec!["offline_access".into(), "Calendars.Read".into()],
        redirect_path: "/oauth/callback".to_string(),
        prompt: Some("select_account".to_string()),
        client_id: "test-client-id".to_string(),
    }
}

/// Block until `emitter` has captured a view whose status matches
/// `predicate`. Polled at 10 ms so the test doesn't leave wall-clock
/// jitter on the table. Tests use this rather than a fixed sleep so
/// they remain deterministic on slow CI runners.
async fn wait_for_status<F>(
    emitter: &RecordingEmitter,
    predicate: F,
    timeout: Duration,
) -> OAuthSessionView
where
    F: Fn(&OAuthSessionStatus) -> bool,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(view) = emitter
            .snapshot()
            .into_iter()
            .find(|v| predicate(&v.status))
        {
            return view;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!(
                "timed out waiting for session-status; observed views: {:?}",
                emitter.snapshot()
            );
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Parse the `redirect_uri` and `state` params out of an authorize
/// URL the flow just opened. The driver composes them in
/// `build_authorize_url`; reading them back here is how a real
/// browser would know where to redirect once the user consents.
fn redirect_and_state_from(authorize_url: &str) -> (String, String) {
    let parsed = Url::parse(authorize_url).expect("authorize URL parses");
    let mut redirect = None;
    let mut state = None;
    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "redirect_uri" => redirect = Some(v.into_owned()),
            "state" => state = Some(v.into_owned()),
            _ => {}
        }
    }
    (
        redirect.expect("redirect_uri present"),
        state.expect("state present"),
    )
}

// ---------- scenarios ----------

/// Happy path: begin → simulate callback → assert Completed and a
/// token pair lives in the registry. Would catch any regression that
/// dropped the `exchange_code` call, the `set_token_pair` write, or
/// the final status flip.
#[tokio::test(flavor = "multi_thread")]
async fn full_loopback_roundtrip_completes_and_stores_tokens() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=authorization_code"))
        .and(body_string_contains("code=fake-auth-code"))
        .and(body_string_contains("client_id=test-client-id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "fake-access-token",
            "refresh_token": "fake-refresh-token",
            "expires_in": 3600,
            "scope": "offline_access Calendars.Read",
            "token_type": "Bearer",
        })))
        .mount(&mock)
        .await;

    let (state, _tmp) = make_state().await;
    let browser = Arc::new(RecordingBrowserOpener::default());
    let emitter = Arc::new(RecordingEmitter::default());

    let view = oauth_begin_login_impl(
        &state,
        config_against(&mock),
        browser.clone(),
        emitter.clone(),
    )
    .await
    .expect("begin_login succeeds");
    assert_eq!(view.status, OAuthSessionStatus::Pending);

    let authorize_url = browser.last().expect("browser was opened");
    let (redirect_uri, state_nonce) = redirect_and_state_from(&authorize_url);

    let callback = format!("{redirect_uri}?code=fake-auth-code&state={state_nonce}");
    let response = reqwest::Client::new()
        .get(&callback)
        .send()
        .await
        .expect("callback GET succeeds");
    assert!(response.status().is_success());

    let completed = wait_for_status(
        &emitter,
        |s| matches!(s, OAuthSessionStatus::Completed),
        Duration::from_secs(5),
    )
    .await;
    assert_eq!(completed.id, view.id);

    let stored = state
        .oauth_sessions
        .take_token_pair(&view.id)
        .await
        .expect("token pair was stored");
    assert_eq!(stored.access_token, "fake-access-token");
    assert_eq!(stored.refresh_token, "fake-refresh-token");
    assert_eq!(
        stored.granted_scopes,
        vec!["offline_access", "Calendars.Read"]
    );
}

/// State-mismatch path: simulate an attacker who redirected back to
/// the loopback with a different `state` than we minted. The driver
/// must refuse the exchange and flip to `Failed` with the stable
/// `OAUTH_LOGIN_STATE_MISMATCH` code. The token endpoint must never
/// be hit — we enforce that by not mounting a matching mock and
/// asserting the mock saw zero requests.
#[tokio::test(flavor = "multi_thread")]
async fn callback_with_wrong_state_is_rejected_without_hitting_token_endpoint() {
    let mock = MockServer::start().await;
    // No `/token` mock mounted on purpose — a request landing there
    // would be a wire-level bug.

    let (state, _tmp) = make_state().await;
    let browser = Arc::new(RecordingBrowserOpener::default());
    let emitter = Arc::new(RecordingEmitter::default());

    let view = oauth_begin_login_impl(
        &state,
        config_against(&mock),
        browser.clone(),
        emitter.clone(),
    )
    .await
    .expect("begin_login succeeds");

    let authorize_url = browser.last().expect("browser was opened");
    let (redirect_uri, _real_state) = redirect_and_state_from(&authorize_url);

    let callback = format!("{redirect_uri}?code=fake-auth-code&state=attacker-nonce");
    let _ = reqwest::Client::new().get(&callback).send().await;

    let failed = wait_for_status(
        &emitter,
        |s| matches!(s, OAuthSessionStatus::Failed { .. }),
        Duration::from_secs(5),
    )
    .await;
    assert_eq!(failed.id, view.id);
    match failed.status {
        OAuthSessionStatus::Failed { code, .. } => {
            assert_eq!(code, "oauth.login.state_mismatch");
        }
        other => panic!("unexpected terminal status: {other:?}"),
    }

    assert_eq!(
        mock.received_requests()
            .await
            .expect("mock tracks requests")
            .len(),
        0,
        "state-mismatch path must not hit the token endpoint",
    );
}

/// IdP-returned error path: the callback URL carries `error=...`
/// rather than a `code=...`. The driver surfaces an
/// `OAUTH_LOGIN_AUTHORIZATION_ERROR` code with the IdP's description
/// inlined into the message so the UI can render it verbatim.
#[tokio::test(flavor = "multi_thread")]
async fn callback_with_idp_error_flips_to_failed() {
    let mock = MockServer::start().await;
    let (state, _tmp) = make_state().await;
    let browser = Arc::new(RecordingBrowserOpener::default());
    let emitter = Arc::new(RecordingEmitter::default());

    let view = oauth_begin_login_impl(
        &state,
        config_against(&mock),
        browser.clone(),
        emitter.clone(),
    )
    .await
    .expect("begin_login succeeds");
    let authorize_url = browser.last().expect("browser was opened");
    let (redirect_uri, state_nonce) = redirect_and_state_from(&authorize_url);

    let callback = format!(
        "{redirect_uri}?error=access_denied&error_description=User+declined&state={state_nonce}"
    );
    let _ = reqwest::Client::new().get(&callback).send().await;

    let failed = wait_for_status(
        &emitter,
        |s| matches!(s, OAuthSessionStatus::Failed { .. }),
        Duration::from_secs(5),
    )
    .await;
    assert_eq!(failed.id, view.id);
    match failed.status {
        OAuthSessionStatus::Failed { code, message } => {
            assert_eq!(code, "oauth.login.authorization_error");
            assert!(message.contains("access_denied"));
            assert!(message.contains("User declined"));
        }
        other => panic!("unexpected terminal status: {other:?}"),
    }
}

/// Cancel path: `oauth_cancel_login_impl` before the callback
/// arrives flips the session to `Cancelled`, trips the cancellation
/// token, and causes the driver to exit without emitting another
/// event. The registry keeps the Cancelled view around so a UI that
/// was mid-poll still sees the terminal state once.
#[tokio::test(flavor = "multi_thread")]
async fn cancel_before_callback_flips_to_cancelled() {
    let mock = MockServer::start().await;
    let (state, _tmp) = make_state().await;
    let browser = Arc::new(RecordingBrowserOpener::default());
    let emitter = Arc::new(RecordingEmitter::default());

    let view = oauth_begin_login_impl(
        &state,
        config_against(&mock),
        browser.clone(),
        emitter.clone(),
    )
    .await
    .expect("begin_login succeeds");
    assert_eq!(view.status, OAuthSessionStatus::Pending);

    let cancelled = oauth_cancel_login_impl(&state, &view.id, emitter.as_ref())
        .await
        .expect("cancel finds session");
    assert_eq!(cancelled.status, OAuthSessionStatus::Cancelled);

    let post = oauth_session_status_impl(&state, &view.id)
        .await
        .expect("session still visible");
    assert_eq!(post.status, OAuthSessionStatus::Cancelled);
}

/// Unknown-session path: `oauth_session_status_impl` on an id the
/// registry has never seen returns `None`, which the UI treats as
/// "stop polling". Pins the "no ghost sessions" invariant that makes
/// the polling contract safe — if a future change leaked a
/// `Some(Pending)` for an unknown id, the UI would loop forever.
#[tokio::test(flavor = "multi_thread")]
async fn session_status_on_unknown_id_is_none() {
    let (state, _tmp) = make_state().await;
    assert!(oauth_session_status_impl(&state, &OAuthSessionId::new())
        .await
        .is_none());
}
