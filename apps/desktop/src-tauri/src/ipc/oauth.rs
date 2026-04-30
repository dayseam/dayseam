//! OAuth 2.0 PKCE loopback login IPC surface (DAY-201 PR #2).
//!
//! Three commands live here:
//!
//!   1. [`oauth_begin_login`] — the renderer's "Add Outlook source"
//!      dialog calls this with a stable `provider_id` (today only
//!      `microsoft-outlook`). The command binds a loopback TCP
//!      listener on `127.0.0.1:<config.loopback_port>` (production
//!      Microsoft pins this to [`oauth_config::MICROSOFT_LOOPBACK_PORT`]
//!      so the redirect URI byte-for-byte matches a registered Azure
//!      reply URL — required by Microsoft's legacy MSA endpoint;
//!      see DAY-205 — while integration tests pass `0` for an OS-
//!      assigned ephemeral port so they parallelise cleanly), builds
//!      the full authorize URL including a fresh PKCE challenge and
//!      a one-use `state` nonce, launches the user's default browser
//!      at that URL, and returns an [`OAuthSessionView`] whose
//!      `status` is [`OAuthSessionStatus::Pending`]. The flow then
//!      proceeds entirely on a background task so the IPC returns
//!      immediately; the UI keeps up via polling
//!      [`oauth_session_status`] or by listening for the
//!      `oauth://session-updated` event the background task emits
//!      on every status transition.
//!   2. [`oauth_cancel_login`] — the dialog's "Cancel" button.
//!      Marks the session `Cancelled` and signals the background
//!      task's [`tokio_util::sync::CancellationToken`], which tears
//!      down the listener and drops the browser tab's eventual
//!      callback on the floor.
//!   3. [`oauth_session_status`] — the renderer's polling hook.
//!      Returns the current [`OAuthSessionView`] or `None` if the
//!      session has been torn down (which is the UI's signal to stop
//!      polling).
//!
//! The happy path: user clicks "Connect Outlook" → dialog calls
//! `oauth_begin_login` → browser opens to Microsoft → user consents
//! → Microsoft redirects to `http://127.0.0.1:{port}/oauth/callback?
//! code=...&state=...` → listener wakes, parses the query, verifies
//! the `state` nonce, exchanges the code against the IdP's token
//! endpoint via [`connectors_sdk::exchange_code`], stashes the
//! resulting [`TokenPair`] in the session registry, flips the status
//! to `Completed`, and emits a final event. A subsequent
//! `outlook_sources_add` IPC (DAY-203) lifts the pair out of the
//! registry into a keychain row and persists the `sources` row.
//!
//! The token pair **never** crosses the IPC boundary here. The
//! renderer sees only a session id and a non-sensitive status —
//! enough to render "signed in as Alice, connect?" without handing
//! JavaScript an access token it could leak into a log or devtools
//! panel.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use connectors_sdk::{
    exchange_code, generate_pkce_pair, CodeChallenge, CodeVerifier, SystemClock, TokenPair,
};
use dayseam_core::{
    error_codes, runtime::supervised_spawn, DayseamError, OAuthSessionId, OAuthSessionStatus,
    OAuthSessionView,
};
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use crate::oauth_config::{lookup_provider, OAuthProviderConfig};
use crate::oauth_session::{OAuthSession, OAuthSessionRegistry};
use crate::state::AppState;

/// Default wall-clock ceiling on a single login attempt. Microsoft's
/// "you were too slow" UI is itself bounded at roughly 10 minutes
/// (the `nonce` cookie on their side expires around there), so this
/// matches the user's mental model of "start, consent, confirm" — if
/// they got distracted for longer than that the interactive session
/// has almost certainly moved on. The timeout is enforced inside the
/// background driver's `tokio::select!` rather than on the listener's
/// `accept()` alone, so a half-open TCP connection cannot prolong the
/// wait past this ceiling.
const DEFAULT_LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

/// Name of the event the background driver emits to the `main`
/// window on every session-status transition. Renderer code binds to
/// this via `@tauri-apps/api/event::listen`. The payload is the
/// updated [`OAuthSessionView`] — same shape `oauth_session_status`
/// would have returned had the UI polled on that instant.
const SESSION_EVENT: &str = "oauth://session-updated";

/// Minimum HTML body the browser tab sees after a successful
/// callback. The user has already been authenticated at the IdP at
/// this point; their next action is to close the tab and go back to
/// the Dayseam window. The success page keeps a zero-dependency,
/// single-screen look — no external fonts or assets — so it loads
/// instantly regardless of network conditions after a redirect.
const SUCCESS_HTML: &str = "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Dayseam - \
     Signed in</title><style>body{font-family:-apple-system,BlinkMacSystemFont,Segoe UI,\
     sans-serif;background:#0f172a;color:#e2e8f0;display:flex;align-items:center;\
     justify-content:center;min-height:100vh;margin:0}div{text-align:center;padding:2rem}\
     h1{margin:0 0 0.5rem}p{margin:0;opacity:0.8}</style></head><body><div>\
     <h1>You can close this tab</h1><p>Return to Dayseam to finish connecting.</p>\
     </div></body></html>";
const FAILURE_HTML: &str = "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Dayseam - \
     Sign-in failed</title><style>body{font-family:-apple-system,BlinkMacSystemFont,Segoe UI,\
     sans-serif;background:#0f172a;color:#e2e8f0;display:flex;align-items:center;\
     justify-content:center;min-height:100vh;margin:0}div{text-align:center;padding:2rem}\
     h1{margin:0 0 0.5rem}p{margin:0;opacity:0.8}</style></head><body><div>\
     <h1>Sign-in didn't complete</h1><p>Return to Dayseam; we kept the details for you.</p>\
     </div></body></html>";

/// Seam the Tauri command uses to launch the user's default browser.
/// Production wires in [`SystemBrowserOpener`] (which shells out to
/// the `opener` crate); integration tests pass a
/// `RecordingBrowserOpener` that stashes the URL and drives the
/// listener itself. The trait lives here because the listener binds
/// on an OS-assigned port that is only known *after*
/// `oauth_begin_login_impl` constructs the redirect URI, so a test
/// that wants to simulate "Microsoft redirected the browser back
/// here" cannot bake the port into a static fixture — it needs to
/// observe the exact URL the production flow would have opened.
pub trait BrowserOpener: Send + Sync {
    fn open(&self, url: &str) -> Result<(), std::io::Error>;
}

/// Production browser-opener: hands the URL to the `opener` crate,
/// which dispatches to `open(1)` on macOS, `xdg-open` on Linux, and
/// `start` on Windows. Transparent to the OAuth flow above it.
pub struct SystemBrowserOpener;

impl BrowserOpener for SystemBrowserOpener {
    fn open(&self, url: &str) -> Result<(), std::io::Error> {
        // `opener::OpenError` does not implement `std::error::Error`'s
        // source chain the way a `std::io::Error` needs, but it does
        // implement `Display` — wrap it in `io::ErrorKind::Other` so
        // the trait's `io::Error` shape is preserved and the
        // underlying message flows unchanged into the logs.
        opener::open_browser(url).map_err(|e| std::io::Error::other(e.to_string()))
    }
}

/// Tauri wrapper for [`oauth_begin_login_impl`]. The impl function
/// carries a trait-object browser opener + callback emitter so
/// integration tests can drive the same flow without a live webview.
#[tauri::command]
pub async fn oauth_begin_login(
    provider_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<OAuthSessionView, DayseamError> {
    let config = lookup_provider(&provider_id)?;
    let emitter = TauriSessionEmitter { app };
    let opener_impl: Arc<dyn BrowserOpener> = Arc::new(SystemBrowserOpener);
    oauth_begin_login_impl(&state, config, opener_impl, Arc::new(emitter)).await
}

/// Tauri wrapper for [`oauth_cancel_login_impl`].
#[tauri::command]
pub async fn oauth_cancel_login(
    session_id: OAuthSessionId,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Option<OAuthSessionView>, DayseamError> {
    let emitter = TauriSessionEmitter { app };
    Ok(oauth_cancel_login_impl(&state, &session_id, &emitter).await)
}

/// Tauri wrapper for [`oauth_session_status_impl`].
#[tauri::command]
pub async fn oauth_session_status(
    session_id: OAuthSessionId,
    state: State<'_, AppState>,
) -> Result<Option<OAuthSessionView>, DayseamError> {
    Ok(oauth_session_status_impl(&state, &session_id).await)
}

/// Shape the background driver uses to emit session-updated events.
/// Production wires in a [`TauriSessionEmitter`]; integration tests
/// pass a `Vec<OAuthSessionView>`-backed recorder so they can assert
/// on the exact event stream the UI would have observed. Kept as a
/// trait rather than a concrete `AppHandle` so the impl function is
/// callable without the Tauri runtime.
pub trait SessionEmitter: Send + Sync {
    fn emit(&self, view: &OAuthSessionView);
}

/// Production emitter — fires a single event per status change onto
/// the app-wide Tauri event bus, where any window that's interested
/// can listen for it via `@tauri-apps/api/event`. Failures to emit
/// (e.g. window already gone on shutdown) are logged at `warn` but
/// do not propagate: the session registry is the durable record of
/// state, and a missed event is strictly a UI freshness problem.
pub struct TauriSessionEmitter {
    pub app: AppHandle,
}

impl SessionEmitter for TauriSessionEmitter {
    fn emit(&self, view: &OAuthSessionView) {
        if let Err(e) = self.app.emit(SESSION_EVENT, view) {
            tracing::warn!(error = %e, "oauth: failed to emit session-updated event");
        }
    }
}

/// Core "start a PKCE flow" function. Publicly callable from the
/// integration test suite (via the crate's `pub` re-export) so the
/// whole loopback dance can be exercised against a wiremock token
/// endpoint without booting Tauri.
///
/// Steps, in order:
///
///   1. Bind a TCP listener on `127.0.0.1:<config.loopback_port>`.
///      Production Microsoft pins this to a fixed port — see
///      [`oauth_config::MICROSOFT_LOOPBACK_PORT`] — so the redirect
///      URI byte-for-byte matches a registered Azure reply URL,
///      which is required by Microsoft's legacy MSA endpoint
///      (DAY-205). Tests pass `0` for an OS-assigned ephemeral
///      port so they parallelise cleanly. Binding here rather than
///      in the background task means we fail fast on a "port
///      already claimed" edge case (e.g. another Dayseam window
///      already mid-login) before spending a session id, with a
///      remediation message that names the offending port.
///   2. Mint a fresh [`OAuthSessionId`], PKCE pair, and `state`
///      nonce. `state` is a v4 UUID — plenty of entropy to rule
///      out the CSRF attack the parameter exists to prevent.
///   3. Compose the authorize URL and hand it to the [`BrowserOpener`].
///      A failure here is surfaced as `OAUTH_LOGIN_BROWSER_OPEN_FAILED`
///      *before* registering the session so the caller can retry
///      cleanly without an orphan `Pending` row.
///   4. Register the session and spawn the background driver.
///   5. Return the view. The driver will take it from here.
pub async fn oauth_begin_login_impl(
    state: &AppState,
    config: OAuthProviderConfig,
    browser: Arc<dyn BrowserOpener>,
    emitter: Arc<dyn SessionEmitter>,
) -> Result<OAuthSessionView, DayseamError> {
    let bind_addr = SocketAddr::from(([127, 0, 0, 1], config.loopback_port));
    let listener = TcpListener::bind(bind_addr).await.map_err(|e| {
        // A fixed-port bind failure is almost always "another login
        // is already in flight in a second window" or "another tool
        // happens to claim this port". The remediation message names
        // the port explicitly so a power user can `lsof -i tcp:53691`
        // without hunting through source. The `0` (OS-assigned) path
        // can never hit `EADDRINUSE` in practice; it only fails on
        // exhausted ephemeral ranges, which is rare enough that the
        // generic message is fine.
        DayseamError::Internal {
            code: error_codes::OAUTH_LOGIN_LOOPBACK_BIND_FAILED.to_string(),
            message: if config.loopback_port == 0 {
                format!("failed to bind loopback listener on 127.0.0.1:0: {e}")
            } else {
                format!(
                    "failed to bind OAuth loopback on 127.0.0.1:{port}: {e}. \
                     Another process is using the port — close any other \
                     Dayseam window mid-login and retry, or run `lsof -i \
                     tcp:{port}` to identify the conflicting process.",
                    port = config.loopback_port,
                )
            },
        }
    })?;
    let local_addr = listener.local_addr().map_err(|e| DayseamError::Internal {
        code: error_codes::OAUTH_LOGIN_LOOPBACK_BIND_FAILED.to_string(),
        message: format!("listener bound but local_addr() failed: {e}"),
    })?;
    let port = local_addr.port();
    let redirect_uri = format!("http://127.0.0.1:{port}{path}", path = config.redirect_path);

    let (verifier, challenge) = generate_pkce_pair(&mut rand::thread_rng());
    let csrf_state = Uuid::new_v4().to_string();
    let session_id = OAuthSessionId::new();
    let created_at = Utc::now();
    let cancel = CancellationToken::new();

    let authorize_url = build_authorize_url(&config, &redirect_uri, &challenge, &csrf_state)?;

    // Open the browser *before* persisting the session — a failure
    // here is a terminal error the UI wants surfaced immediately,
    // and registering first would leak a `Pending` row no driver
    // will ever update.
    if let Err(e) = browser.open(&authorize_url) {
        return Err(DayseamError::Internal {
            code: error_codes::OAUTH_LOGIN_BROWSER_OPEN_FAILED.to_string(),
            message: format!("failed to open system browser: {e}"),
        });
    }

    let session = OAuthSession {
        id: session_id,
        provider_id: config.id.clone(),
        created_at,
        status: OAuthSessionStatus::Pending,
        token_pair: None,
        cancel: cancel.clone(),
    };
    let initial_view = session.to_view();
    state.oauth_sessions.insert(session).await;

    let driver = LoopbackDriver {
        session_id,
        config,
        redirect_uri,
        csrf_state,
        verifier,
        cancel,
        timeout: DEFAULT_LOGIN_TIMEOUT,
        registry: state.oauth_sessions.clone(),
        http: state.http.reqwest().clone(),
        emitter,
    };
    let _handle = supervised_spawn("ipc::oauth::loopback_driver", async move {
        driver.run(listener).await
    });

    Ok(initial_view)
}

/// Core "cancel a pending flow" function. Returns the updated view
/// if the session existed, `None` if the UI held a stale id. The
/// background driver observes the cancellation on its next select
/// and exits without completing the token exchange.
pub async fn oauth_cancel_login_impl(
    state: &AppState,
    id: &OAuthSessionId,
    emitter: &dyn SessionEmitter,
) -> Option<OAuthSessionView> {
    let updated = state.oauth_sessions.cancel(id).await?;
    emitter.emit(&updated);
    Some(updated)
}

/// Core "what's this session up to" function.
pub async fn oauth_session_status_impl(
    state: &AppState,
    id: &OAuthSessionId,
) -> Option<OAuthSessionView> {
    state.oauth_sessions.get_view(id).await
}

/// Compose the full authorize URL. Separated out for readability and
/// so a test can assert on the exact query parameters without
/// spinning up a driver.
fn build_authorize_url(
    config: &OAuthProviderConfig,
    redirect_uri: &str,
    challenge: &CodeChallenge,
    csrf_state: &str,
) -> Result<String, DayseamError> {
    let mut url =
        Url::parse(&config.authorization_endpoint).map_err(|e| DayseamError::Internal {
            code: error_codes::OAUTH_LOGIN_NOT_CONFIGURED.to_string(),
            message: format!("authorize endpoint is not a valid URL: {e}"),
        })?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("client_id", &config.client_id);
        q.append_pair("response_type", "code");
        q.append_pair("redirect_uri", redirect_uri);
        q.append_pair("response_mode", "query");
        q.append_pair("scope", &config.scope_string());
        q.append_pair("state", csrf_state);
        q.append_pair("code_challenge", challenge.as_str());
        q.append_pair("code_challenge_method", "S256");
        if let Some(prompt) = &config.prompt {
            q.append_pair("prompt", prompt);
        }
    }
    Ok(url.to_string())
}

/// Self-contained background driver. Owns every moving part of one
/// PKCE flow after the IPC command returns: the listener, the
/// HTTP client, the cancellation handle, the session registry, the
/// emitter. Destructured once per login into a `tokio::spawn` body.
struct LoopbackDriver {
    session_id: OAuthSessionId,
    config: OAuthProviderConfig,
    redirect_uri: String,
    csrf_state: String,
    verifier: CodeVerifier,
    cancel: CancellationToken,
    timeout: Duration,
    registry: OAuthSessionRegistry,
    http: reqwest::Client,
    emitter: Arc<dyn SessionEmitter>,
}

impl LoopbackDriver {
    async fn run(self, listener: TcpListener) {
        let outcome = tokio::select! {
            biased;
            _ = self.cancel.cancelled() => {
                tracing::info!(session = %self.session_id, "oauth: login cancelled before callback arrived");
                // `oauth_cancel_login_impl` already flipped the
                // status to `Cancelled`; nothing else to do here.
                return;
            }
            _ = tokio::time::sleep(self.timeout) => {
                DriverOutcome::Failed {
                    code: error_codes::OAUTH_LOGIN_TIMEOUT.to_string(),
                    message: format!(
                        "no OAuth callback received within {}s",
                        self.timeout.as_secs(),
                    ),
                }
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _peer)) => self.handle_callback(stream).await,
                    Err(e) => DriverOutcome::Failed {
                        code: error_codes::OAUTH_LOGIN_LOOPBACK_BIND_FAILED.to_string(),
                        message: format!("listener accept() failed: {e}"),
                    },
                }
            }
        };

        match outcome {
            DriverOutcome::Completed(pair) => {
                self.registry.set_token_pair(&self.session_id, pair).await;
                if let Some(view) = self
                    .registry
                    .update_status(&self.session_id, OAuthSessionStatus::Completed)
                    .await
                {
                    self.emitter.emit(&view);
                }
            }
            DriverOutcome::Failed { code, message } => {
                if let Some(view) = self
                    .registry
                    .update_status(
                        &self.session_id,
                        OAuthSessionStatus::Failed {
                            code: code.clone(),
                            message: message.clone(),
                        },
                    )
                    .await
                {
                    self.emitter.emit(&view);
                }
                tracing::warn!(
                    session = %self.session_id,
                    code = %code,
                    message = %message,
                    "oauth: login failed",
                );
            }
        }
    }

    /// Handle the first accepted TCP connection as the OAuth
    /// callback. Reads the HTTP request head, extracts the query
    /// params, verifies the `state` nonce, exchanges the code, and
    /// writes a terminal HTML body to the browser tab — all on the
    /// same socket.
    async fn handle_callback(&self, mut stream: TcpStream) -> DriverOutcome {
        let request_line = match read_request_line(&mut stream).await {
            Ok(line) => line,
            Err(e) => {
                let _ = write_response(&mut stream, FAILURE_HTML, 400).await;
                return DriverOutcome::Failed {
                    code: error_codes::OAUTH_LOGIN_AUTHORIZATION_ERROR.to_string(),
                    message: format!("failed to read callback HTTP request: {e}"),
                };
            }
        };

        let query = match extract_query(&request_line) {
            Some(q) => q,
            None => {
                let _ = write_response(&mut stream, FAILURE_HTML, 400).await;
                return DriverOutcome::Failed {
                    code: error_codes::OAUTH_LOGIN_AUTHORIZATION_ERROR.to_string(),
                    message: format!("callback request line has no query: `{request_line}`"),
                };
            }
        };

        let params = parse_query(&query);

        if let Some(err) = params.iter().find(|(k, _)| k == "error") {
            let description = params
                .iter()
                .find(|(k, _)| k == "error_description")
                .map(|(_, v)| v.as_str())
                .unwrap_or_default();
            let _ = write_response(&mut stream, FAILURE_HTML, 200).await;
            return DriverOutcome::Failed {
                code: error_codes::OAUTH_LOGIN_AUTHORIZATION_ERROR.to_string(),
                message: format!(
                    "IdP reported `error={code}`{desc}",
                    code = err.1,
                    desc = if description.is_empty() {
                        String::new()
                    } else {
                        format!(" ({description})")
                    }
                ),
            };
        }

        let received_state = match params.iter().find(|(k, _)| k == "state") {
            Some((_, v)) => v.clone(),
            None => {
                let _ = write_response(&mut stream, FAILURE_HTML, 400).await;
                return DriverOutcome::Failed {
                    code: error_codes::OAUTH_LOGIN_STATE_MISMATCH.to_string(),
                    message: "callback query missing `state` parameter".to_string(),
                };
            }
        };
        if received_state != self.csrf_state {
            let _ = write_response(&mut stream, FAILURE_HTML, 400).await;
            return DriverOutcome::Failed {
                code: error_codes::OAUTH_LOGIN_STATE_MISMATCH.to_string(),
                message: "`state` parameter does not match expected CSRF nonce".to_string(),
            };
        }

        let code = match params.iter().find(|(k, _)| k == "code") {
            Some((_, v)) => v.clone(),
            None => {
                let _ = write_response(&mut stream, FAILURE_HTML, 400).await;
                return DriverOutcome::Failed {
                    code: error_codes::OAUTH_LOGIN_AUTHORIZATION_ERROR.to_string(),
                    message: "callback query missing `code` parameter".to_string(),
                };
            }
        };

        // Respond to the browser tab *before* hitting the token
        // endpoint. The user's tab becomes useful the moment they
        // see the "close this" page; making them watch a spinner
        // that depends on the IdP's response time adds nothing.
        let _ = write_response(&mut stream, SUCCESS_HTML, 200).await;
        let _ = stream.shutdown().await;

        let clock = SystemClock;
        match exchange_code(
            &self.http,
            &self.config.token_endpoint,
            &self.config.client_id,
            &code,
            &self.verifier,
            &self.redirect_uri,
            &clock,
        )
        .await
        {
            Ok(pair) => DriverOutcome::Completed(pair),
            Err(e) => DriverOutcome::Failed {
                code: e.code().to_string(),
                message: e.to_string(),
            },
        }
    }
}

/// Outcome channel the driver produces once before exiting. Keeps
/// the two happy/sad branches compile-time exhaustive so adding a
/// third (e.g. "cancelled mid-exchange") is a single-line
/// modification the compiler walks.
enum DriverOutcome {
    Completed(TokenPair),
    Failed { code: String, message: String },
}

/// Read up to the first `\r\n\r\n` from `stream`, return the full
/// request line (everything up to the first `\r\n`). Parses at most
/// 8 KiB — plenty for an OAuth callback whose query is at most a
/// few hundred bytes — so a malicious peer spamming an infinite
/// stream of headers cannot exhaust the driver's memory.
async fn read_request_line(stream: &mut TcpStream) -> Result<String, std::io::Error> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if buf.len() > 8 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request head exceeded 8 KiB",
            ));
        }
    }
    let end = buf
        .windows(2)
        .position(|w| w == b"\r\n")
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "no request line"))?;
    Ok(String::from_utf8_lossy(&buf[..end]).to_string())
}

/// Extract the query string (everything between the first `?` and
/// the next whitespace) from an HTTP request line of the form
/// `GET /oauth/callback?code=...&state=... HTTP/1.1`.
fn extract_query(request_line: &str) -> Option<String> {
    // A well-formed line is `METHOD SP PATH SP VERSION`; we care
    // only about the PATH component, then split off the query.
    let mut parts = request_line.split_whitespace();
    let _method = parts.next()?;
    let path = parts.next()?;
    let q_start = path.find('?')? + 1;
    Some(path[q_start..].to_string())
}

/// Parse a URL-encoded query string into `(key, value)` pairs.
/// Uses `url::form_urlencoded::parse` so the decoder has the right
/// semantics for `+`-as-space and percent-encoding.
fn parse_query(q: &str) -> Vec<(String, String)> {
    url::form_urlencoded::parse(q.as_bytes())
        .into_owned()
        .collect()
}

/// Write a minimal HTTP/1.1 response with the given body and status
/// code. Closes the connection so the user's browser doesn't hang
/// on a keep-alive timeout waiting for more.
async fn write_response(
    stream: &mut TcpStream,
    body: &str,
    status: u16,
) -> Result<(), std::io::Error> {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        _ => "OK",
    };
    let response = format!(
        "HTTP/1.1 {status} {status_text}\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {len}\r\n\
         Connection: close\r\n\
         \r\n{body}",
        len = body.len(),
    );
    stream.write_all(response.as_bytes()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_query_handles_well_formed_request() {
        let line = "GET /oauth/callback?code=abc&state=xyz HTTP/1.1";
        assert_eq!(extract_query(line).as_deref(), Some("code=abc&state=xyz"));
    }

    #[test]
    fn extract_query_returns_none_when_no_question_mark() {
        let line = "GET /oauth/callback HTTP/1.1";
        assert!(extract_query(line).is_none());
    }

    #[test]
    fn parse_query_decodes_percent_encoding() {
        let pairs = parse_query("code=a%20b&state=xyz");
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("code".to_string(), "a b".to_string()));
        assert_eq!(pairs[1], ("state".to_string(), "xyz".to_string()));
    }

    #[test]
    fn build_authorize_url_carries_every_parameter() {
        let cfg = OAuthProviderConfig::microsoft_outlook("client-123");
        let (_, challenge) = generate_pkce_pair(&mut rand::thread_rng());
        let url = build_authorize_url(
            &cfg,
            "http://127.0.0.1:54321/oauth/callback",
            &challenge,
            "csrf-xyz",
        )
        .unwrap();
        let parsed = Url::parse(&url).unwrap();
        let params: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(
            params.get("client_id").map(String::as_str),
            Some("client-123")
        );
        assert_eq!(
            params.get("response_type").map(String::as_str),
            Some("code")
        );
        assert_eq!(
            params.get("response_mode").map(String::as_str),
            Some("query")
        );
        assert_eq!(
            params.get("redirect_uri").map(String::as_str),
            Some("http://127.0.0.1:54321/oauth/callback")
        );
        assert_eq!(params.get("state").map(String::as_str), Some("csrf-xyz"));
        assert_eq!(
            params.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert_eq!(
            params.get("prompt").map(String::as_str),
            Some("select_account")
        );
        let scope = params.get("scope").map(String::as_str).unwrap();
        assert!(scope.contains("Calendars.Read"));
        assert!(scope.contains("offline_access"));
    }
}
