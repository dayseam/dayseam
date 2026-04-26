//! IPC-facing shapes for the Tauri PKCE loopback login flow.
//!
//! DAY-201 PR #2 lands the browser-side OAuth 2.0 ceremony (the
//! `connectors-sdk` half shipped in PR #1). The implementation lives
//! inside `apps/desktop/src-tauri/src/ipc/oauth.rs`; the *wire*
//! shapes the frontend binds against live here so `ts-rs` picks them
//! up through the same `#[ts(export)]` pipeline every other IPC
//! type rides.
//!
//! The invariant we care about most: **tokens never leave the
//! Rust process as part of these types**. Access / refresh tokens
//! sit inside a server-side session registry keyed by
//! [`OAuthSessionId`], and the UI only ever sees the public
//! `OAuthSessionView` variants below — enough to render "waiting",
//! "signed in as …", or "something failed, try again" without
//! handing the renderer bytes it can leak into a log or a devtools
//! panel. A later ticket (DAY-203) will add a bespoke
//! `outlook_sources_add` IPC command that takes an
//! [`OAuthSessionId`] and promotes the server-side `TokenPair` into
//! a keychain row + `sources` row in one transactional call; the
//! TokenPair itself never rides an IPC response.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Opaque identifier for an in-flight or recently-completed OAuth
/// login session. Backed by a v4 UUID so two tabs racing the "Add
/// Outlook source" dialog cannot collide, and so a session id is
/// safe to log / surface in error telemetry without leaking
/// anything about the user's tenant.
///
/// Wrapped as a newtype rather than a bare `Uuid` so a future
/// refactor can add validation (e.g. "must come from the current
/// process's registry") without touching every call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(transparent)]
pub struct OAuthSessionId(#[ts(type = "string")] pub Uuid);

impl OAuthSessionId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for OAuthSessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for OAuthSessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Public projection of one OAuth login session. Returned directly
/// by `oauth_begin_login` / `oauth_session_status` and emitted on
/// the `oauth://session-updated` event whenever the session's
/// status changes.
///
/// Contains only non-sensitive fields — the server-side
/// `TokenPair` is addressed by [`OAuthSessionId`] in a follow-up
/// IPC call (DAY-203) rather than shipped here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OAuthSessionView {
    pub id: OAuthSessionId,
    /// Stable provider identifier the UI originally passed to
    /// `oauth_begin_login` (e.g. `"microsoft-outlook"`). Echoed
    /// back so a UI watching the event stream can route updates to
    /// the right dialog without round-tripping through local state.
    pub provider_id: String,
    /// When the session started. Included so the UI can render a
    /// "this has been waiting 45s" hint and so a debugging log can
    /// correlate a session id with the tail of the event log.
    pub created_at: DateTime<Utc>,
    pub status: OAuthSessionStatus,
}

/// The four terminal and pre-terminal states an `OAuthSession` can
/// be in from the frontend's perspective. `Pending` covers the
/// whole "browser tab is open, user is consenting" window;
/// `Completed` is terminal and promises a subsequent IPC call can
/// retrieve the resulting `TokenPair`; the two failure variants are
/// terminal and clear the next retry path.
///
/// Serialised as an `externally-tagged` JSON union (`{ "kind":
/// "pending" }` / `{ "kind": "completed" }` / …) so the TypeScript
/// narrowing is ergonomic on the UI side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OAuthSessionStatus {
    /// Listener is bound, browser was asked to open the authorize
    /// URL, and we are waiting for the callback GET to arrive. Also
    /// the state that covers "callback arrived, exchanging code for
    /// tokens" because we do not split that sub-phase out at the
    /// wire level — the whole "make coffee" UX is one spinner.
    Pending,
    /// The code was exchanged for an access/refresh pair and the
    /// TokenPair now lives in the server-side session registry. A
    /// follow-up IPC call (DAY-203) finalises it into a keychain
    /// row + `sources` row; the UI should offer the "Connect this
    /// account" confirmation at this point.
    Completed,
    /// The flow failed at some observable point — listener bind,
    /// state mismatch, IdP-returned `error=…` on the callback,
    /// token endpoint refusal, timeout. The `code` is one of the
    /// stable `oauth.login.*` codes in
    /// `dayseam_core::error_codes`; the `message` is a human-
    /// readable one-liner the UI can show inline.
    Failed { code: String, message: String },
    /// The user (or the app, on shutdown) explicitly cancelled the
    /// session via `oauth_cancel_login`. Distinct from `Failed`
    /// because the remediation copy is different — "try again" vs
    /// "here is what went wrong".
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_is_unique_per_call() {
        let a = OAuthSessionId::new();
        let b = OAuthSessionId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn session_id_round_trips_through_json() {
        let id = OAuthSessionId::new();
        let serialised = serde_json::to_string(&id).unwrap();
        let round_tripped: OAuthSessionId = serde_json::from_str(&serialised).unwrap();
        assert_eq!(id, round_tripped);
    }

    #[test]
    fn status_serialises_with_snake_case_kind() {
        let pending = serde_json::to_value(OAuthSessionStatus::Pending).unwrap();
        assert_eq!(pending, serde_json::json!({ "kind": "pending" }));
        let failed = serde_json::to_value(OAuthSessionStatus::Failed {
            code: "oauth.login.timeout".to_string(),
            message: "timed out".to_string(),
        })
        .unwrap();
        assert_eq!(
            failed,
            serde_json::json!({
                "kind": "failed",
                "code": "oauth.login.timeout",
                "message": "timed out"
            })
        );
    }
}
