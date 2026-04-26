//! OAuth 2.0 provider registry for the Tauri desktop shell.
//!
//! DAY-201 PR #2 introduces the PKCE loopback login flow. To keep
//! the IPC surface provider-agnostic — a future Slack, Linear, or
//! Google Calendar connector will reuse the same `oauth_begin_login`
//! command — the per-provider knobs (issuer URLs, default scopes,
//! prompt behaviour, the `client_id` we registered with the IdP)
//! live here rather than hard-coded into the flow itself. The flow
//! asks this module for an [`OAuthProviderConfig`] by `provider_id`
//! and stays oblivious to the fact that Microsoft's tenant-common
//! endpoint looks different from, say, Slack's or Google's.
//!
//! # `client_id` resolution
//!
//! Dayseam ships a single binary; the Azure app registration each
//! developer / user's tenant needs is *not* something we can bake in
//! at crate-publish time. We support three resolution layers, in
//! order of precedence:
//!
//!   1. The runtime environment variable `DAYSEAM_MS_CLIENT_ID`.
//!      This is the layer the Tauri dev server and CI pipelines use;
//!      a developer running `pnpm tauri dev` after reading
//!      `docs/setup/azure-app-registration.md` just exports the var
//!      in their shell and the login flow picks it up on the next
//!      `oauth_begin_login` call without a rebuild.
//!   2. The compile-time environment variable of the same name,
//!      evaluated via [`option_env!`]. This is how a release build
//!      embeds the production app registration at bundle time — CI
//!      sets `DAYSEAM_MS_CLIENT_ID` before `cargo tauri build`, the
//!      shipped binary carries the id even on a user's machine that
//!      has no such env var set, and the runtime layer above can
//!      still override it during debugging.
//!   3. The literal placeholder constant [`PLACEHOLDER_CLIENT_ID`].
//!      A build with neither env var set lands on this value and the
//!      IPC refuses to start a login flow, surfacing
//!      [`error_codes::OAUTH_LOGIN_NOT_CONFIGURED`] instead — so a
//!      fresh clone of the repo that has never seen the setup doc
//!      fails loud, not silent.
//!
//! Tests never go through this function; they pass a bespoke
//! [`OAuthProviderConfig`] directly into the flow so the token and
//! authorize endpoints can point at a `wiremock::MockServer`.

use dayseam_core::{error_codes, DayseamError};

/// Sentinel value the resolve functions treat as "not configured".
/// Surfaced in user-facing error messages so the fix is obvious
/// rather than having to grep for a literal UUID.
pub const PLACEHOLDER_CLIENT_ID: &str = "UNSET-DAYSEAM-CLIENT-ID";

/// Stable identifier the frontend's "Add Outlook source" dialog
/// passes to `oauth_begin_login`. Also baked into the `Commands`
/// TypeScript map so a typo in the dropdown doesn't compile.
pub const PROVIDER_MICROSOFT_OUTLOOK: &str = "microsoft-outlook";

/// Everything the PKCE loopback flow needs to talk to one IdP. Owned
/// `String`s rather than `&'static str`s because integration tests
/// construct bespoke instances pointing at `http://127.0.0.1:PORT`
/// wiremock servers; the production resolver builds the same shape
/// from compiled-in constants.
#[derive(Debug, Clone)]
pub struct OAuthProviderConfig {
    /// Opaque id; matches what `oauth_begin_login` takes on the wire.
    pub id: String,
    /// Full authorization endpoint URL (e.g. Microsoft's
    /// `https://login.microsoftonline.com/common/oauth2/v2.0/authorize`).
    pub authorization_endpoint: String,
    /// Full token endpoint URL used by both the initial code
    /// exchange and the later refresh path.
    pub token_endpoint: String,
    /// Delegated scopes the connector needs. Concatenated with
    /// spaces before going onto the `scope=` parameter.
    pub scopes: Vec<String>,
    /// Path component of the loopback redirect URI. Combined at
    /// runtime with `http://127.0.0.1:<os-assigned-port>` so the
    /// full redirect is `http://127.0.0.1:54321/oauth/callback` and
    /// Microsoft's "any port on loopback" matching rule accepts it
    /// against a registered `http://localhost` reply URL.
    pub redirect_path: String,
    /// Optional `prompt=` parameter on the authorize URL. Microsoft
    /// uses `select_account` so the user can pick between personal
    /// and work accounts on every login; other IdPs may leave this
    /// `None`.
    pub prompt: Option<String>,
    /// The public-client `client_id` the IdP knows us by. Resolved
    /// via [`resolve_microsoft_client_id`] for Microsoft; injected
    /// directly in tests.
    pub client_id: String,
}

impl OAuthProviderConfig {
    /// Build the Microsoft Outlook flavour of the config with an
    /// explicit `client_id`. Used by both the production resolver
    /// (with a resolved id) and tests (with a fabricated id against
    /// wiremock-backed endpoints).
    #[must_use]
    pub fn microsoft_outlook(client_id: impl Into<String>) -> Self {
        Self {
            id: PROVIDER_MICROSOFT_OUTLOOK.to_string(),
            authorization_endpoint:
                "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string(),
            token_endpoint: "https://login.microsoftonline.com/common/oauth2/v2.0/token"
                .to_string(),
            // `offline_access` is what gives us the refresh token;
            // `Calendars.Read` is the Outlook connector's core need;
            // `User.Read` backs the "Connected as <upn>" ribbon on
            // the Add Source dialog. The walker's narrower scope
            // requirements live next to the walker itself so this
            // list never silently drifts — see DAY-202.
            scopes: vec![
                "offline_access".to_string(),
                "Calendars.Read".to_string(),
                "User.Read".to_string(),
            ],
            redirect_path: "/oauth/callback".to_string(),
            prompt: Some("select_account".to_string()),
            client_id: client_id.into(),
        }
    }

    /// Join the scopes list with single-space separators, the shape
    /// both the authorize URL and the token endpoint expect.
    #[must_use]
    pub fn scope_string(&self) -> String {
        self.scopes.join(" ")
    }
}

/// Resolve the Microsoft Outlook `client_id` following the three-
/// layer precedence described at the top of this module. Returns
/// `None` when every layer lands on the placeholder so the caller
/// can emit [`error_codes::OAUTH_LOGIN_NOT_CONFIGURED`] instead of
/// kicking off a browser flow that can only fail.
#[must_use]
pub fn resolve_microsoft_client_id() -> Option<String> {
    if let Ok(runtime) = std::env::var("DAYSEAM_MS_CLIENT_ID") {
        let trimmed = runtime.trim();
        if !trimmed.is_empty() && trimmed != PLACEHOLDER_CLIENT_ID {
            return Some(trimmed.to_string());
        }
    }
    if let Some(compile_time) = option_env!("DAYSEAM_MS_CLIENT_ID") {
        let trimmed = compile_time.trim();
        if !trimmed.is_empty() && trimmed != PLACEHOLDER_CLIENT_ID {
            return Some(trimmed.to_string());
        }
    }
    None
}

/// Top-level resolver called by `oauth_begin_login`. Maps a wire
/// `provider_id` onto a fully-populated [`OAuthProviderConfig`] or a
/// precise `DayseamError` code the frontend's error-card copy can
/// branch on (`provider_unknown` vs `not_configured` surface
/// different UI affordances — "that's a bug" vs "finish setup").
pub fn lookup_provider(provider_id: &str) -> Result<OAuthProviderConfig, DayseamError> {
    match provider_id {
        PROVIDER_MICROSOFT_OUTLOOK => {
            let client_id =
                resolve_microsoft_client_id().ok_or_else(|| DayseamError::InvalidConfig {
                    code: error_codes::OAUTH_LOGIN_NOT_CONFIGURED.to_string(),
                    message: "Microsoft OAuth `client_id` is not configured. \
                              Register an Azure app following `docs/setup/azure-app-registration.md` \
                              and export `DAYSEAM_MS_CLIENT_ID=<your-app-id>` before launching the app."
                        .to_string(),
                })?;
            Ok(OAuthProviderConfig::microsoft_outlook(client_id))
        }
        other => Err(DayseamError::InvalidConfig {
            code: error_codes::OAUTH_LOGIN_PROVIDER_UNKNOWN.to_string(),
            message: format!("unknown OAuth provider `{other}`"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microsoft_outlook_populates_known_endpoints() {
        let cfg = OAuthProviderConfig::microsoft_outlook("abc-123");
        assert_eq!(cfg.id, PROVIDER_MICROSOFT_OUTLOOK);
        assert!(cfg
            .authorization_endpoint
            .starts_with("https://login.microsoftonline.com/"));
        assert!(cfg
            .token_endpoint
            .starts_with("https://login.microsoftonline.com/"));
        assert_eq!(cfg.redirect_path, "/oauth/callback");
        assert_eq!(cfg.prompt.as_deref(), Some("select_account"));
        assert_eq!(cfg.client_id, "abc-123");
    }

    #[test]
    fn scope_string_joins_with_single_spaces() {
        let cfg = OAuthProviderConfig::microsoft_outlook("x");
        let s = cfg.scope_string();
        assert_eq!(s, "offline_access Calendars.Read User.Read");
    }

    #[test]
    fn lookup_unknown_provider_returns_typed_error() {
        let err = lookup_provider("definitely-not-real").unwrap_err();
        assert!(matches!(err, DayseamError::InvalidConfig { .. }));
        assert_eq!(err.code(), error_codes::OAUTH_LOGIN_PROVIDER_UNKNOWN);
    }
}
