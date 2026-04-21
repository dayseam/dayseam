//! Atlassian IPC surface (DAY-82).
//!
//! Two commands live here:
//!
//!   1. [`atlassian_validate_credentials`] — a one-shot
//!      `GET /rest/api/3/myself` probe. The `AddAtlassianSourceDialog`
//!      calls it once the user has pasted an email + API token and a
//!      workspace URL so the dialog can show "Connected as …" before
//!      committing to a `sources_add`.
//!   2. [`atlassian_sources_add`] — the transactional persist call
//!      that covers all four Add-Atlassian journeys (shared-PAT,
//!      single-product, reuse-existing-PAT, different-PAT) documented
//!      in the plan at `docs/plan/2026-04-20-v0.2-atlassian.md`. Every
//!      row it writes points at a [`SecretRef`] either freshly minted
//!      here (one new keychain row) or handed in by the caller
//!      (Journey C mode 1, zero new keychain rows). DAY-81's refcount
//!      guard handles clean-up on delete — the shared/not-shared
//!      distinction lives entirely in how many `Source` rows end up
//!      holding a given `secret_ref`.
//!
//! This module does **not** build an auth strategy for the persisted
//! sources the way `commands::build_source_auth` does for GitLab: the
//! Jira and Confluence arms of that function still return
//! `Unsupported` until DAY-84 wires the real walkers. DAY-82's IPC
//! exists so the dialog can drive DAY-81's secret management end-to-
//! end; the walkers that actually *use* those secrets are the next
//! ticket's problem.
//!
//! The keychain keying scheme is deliberate:
//!
//! | GitLab (DAY-70)                    | Atlassian (this file)                    |
//! |------------------------------------|------------------------------------------|
//! | `service = dayseam.gitlab`         | `service = dayseam.atlassian`            |
//! | `account = source:<source_id>`     | `account = slot:<uuid>`                  |
//!
//! GitLab keys by `source_id` because one GitLab source owns exactly
//! one PAT. Atlassian keys by an opaque UUID slot because the
//! shared-PAT flow needs two `Source` rows to address the *same*
//! keychain entry, and `source_id` cannot do that by construction.
//! The slot UUID is independent of either `source_id` so a later
//! rename or reshape (e.g. splitting a shared PAT by rotating one of
//! the two products' credentials) does not require re-keying the
//! keychain.

use chrono::Utc;
use connector_atlassian_common::{
    cloud::{discover_cloud, AtlassianAccountInfo},
    identity::seed_atlassian_identity,
};
use connectors_sdk::{BasicAuth, HttpClient};
use dayseam_core::{
    error_codes, AtlassianValidationResult, DayseamError, SecretRef, Source, SourceConfig,
    SourceHealth, SourceId, SourceKind,
};
use dayseam_db::{PersonRepo, SourceIdentityRepo, SourceRepo};
use dayseam_secrets::Secret;
use tauri::State;
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use crate::ipc::commands::{
    invalid_config_public, persist_restart_required_toast, secret_store_key,
    SELF_DEFAULT_DISPLAY_NAME,
};
use crate::ipc::secret::IpcSecretString;
use crate::state::AppState;

/// Keychain `service` half for every Atlassian API token this app
/// stores. Matches the "Keychain Access readability" rationale that
/// all `dayseam.atlassian` entries live under one heading and is
/// the shape DAY-81's orphan-secret audit on boot walks.
const ATLASSIAN_KEYCHAIN_SERVICE: &str = "dayseam.atlassian";

/// Build a fresh [`SecretRef`] for an Atlassian PAT. Unlike the
/// GitLab variant, the `account` half is a brand-new UUID rather
/// than derived from any one `SourceId` — two sources may point at
/// the same keychain row (shared-PAT mode) and neither one is
/// "canonical".
fn new_atlassian_secret_ref() -> SecretRef {
    SecretRef {
        keychain_service: ATLASSIAN_KEYCHAIN_SERVICE.to_string(),
        keychain_account: format!("slot:{}", Uuid::new_v4()),
    }
}

/// Parse a caller-supplied `workspace_url` into an absolute
/// `https://<host>` URL (no path, no query, no trailing slash). The
/// dialog normalises client-side first (see the TS
/// `atlassian-workspace-url.ts` helper); this server-side check is a
/// defence-in-depth so a bespoke caller cannot round-trip a malformed
/// URL into `SourceConfig::{Jira, Confluence}`.
fn parse_workspace_url(input: &str) -> Result<Url, DayseamError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(invalid_config_public(
            error_codes::IPC_ATLASSIAN_INVALID_WORKSPACE_URL,
            "workspace_url must not be empty",
        ));
    }
    // `Url::parse` refuses scheme-less input, which is exactly what
    // we want here: the dialog adds `https://` client-side, so any
    // input that still lacks a scheme is a caller bug.
    let parsed = Url::parse(trimmed).map_err(|e| {
        invalid_config_public(
            error_codes::IPC_ATLASSIAN_INVALID_WORKSPACE_URL,
            format!("workspace_url `{trimmed}` is not a valid URL: {e}"),
        )
    })?;
    if parsed.scheme() != "https" {
        return Err(invalid_config_public(
            error_codes::IPC_ATLASSIAN_INVALID_WORKSPACE_URL,
            format!(
                "workspace_url scheme must be `https`; got `{}`",
                parsed.scheme()
            ),
        ));
    }
    if parsed.host_str().unwrap_or("").is_empty() {
        return Err(invalid_config_public(
            error_codes::IPC_ATLASSIAN_INVALID_WORKSPACE_URL,
            "workspace_url has no host component",
        ));
    }
    if parsed.path() != "/" && !parsed.path().is_empty() {
        return Err(invalid_config_public(
            error_codes::IPC_ATLASSIAN_INVALID_WORKSPACE_URL,
            format!(
                "workspace_url must be origin-only; got path `{}`",
                parsed.path()
            ),
        ));
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(invalid_config_public(
            error_codes::IPC_ATLASSIAN_INVALID_WORKSPACE_URL,
            "workspace_url must not carry a query string or fragment",
        ));
    }
    Ok(parsed)
}

/// Canonicalise a parsed `Url` to the string shape
/// `SourceConfig::{Jira, Confluence}.workspace_url` stores: scheme +
/// host (+ port) with no trailing slash and no path.
fn canonical_workspace_url(url: &Url) -> String {
    let mut s = url.origin().ascii_serialization();
    while s.ends_with('/') {
        s.pop();
    }
    s
}

fn require_nonempty_email(email: &str) -> Result<(), DayseamError> {
    if email.trim().is_empty() {
        return Err(invalid_config_public(
            error_codes::IPC_ATLASSIAN_CREDENTIALS_MISSING,
            "email must not be empty or whitespace-only",
        ));
    }
    Ok(())
}

fn require_nonempty_token(token: &str) -> Result<(), DayseamError> {
    if token.trim().is_empty() {
        return Err(invalid_config_public(
            error_codes::IPC_ATLASSIAN_CREDENTIALS_MISSING,
            "api_token must not be empty or whitespace-only",
        ));
    }
    Ok(())
}

/// Build a minimal [`AtlassianAccountInfo`] wrapper around a bare
/// `account_id`. `seed_atlassian_identity` only reads the id field —
/// display_name / email are placeholders here because the dialog
/// already used them at validate time for the "Connected as …"
/// ribbon.
fn account_info_from_id(account_id: &str) -> AtlassianAccountInfo {
    AtlassianAccountInfo {
        account_id: account_id.to_string(),
        display_name: String::new(),
        email: None,
        cloud_id: None,
    }
}

/// One-shot `GET /rest/api/3/myself` probe. Returns the account
/// triple the dialog renders in its "Connected as …" ribbon. See the
/// [`crate::ipc::atlassian`] module docs for the failure modes.
#[tauri::command]
pub async fn atlassian_validate_credentials(
    workspace_url: String,
    email: String,
    api_token: IpcSecretString,
) -> Result<AtlassianValidationResult, DayseamError> {
    require_nonempty_email(&email)?;
    require_nonempty_token(api_token.expose())?;
    let parsed = parse_workspace_url(&workspace_url)?;

    let http = HttpClient::new()?;
    // The `BasicAuth` constructed here lives only for the duration
    // of this call — the descriptor's keychain handle is a synthetic
    // `"probe"` slot because no keychain row actually backs this
    // transient credential. The probe path never hydrates a
    // `SecretRef` from the descriptor, so the placeholder is invisible
    // to everything downstream.
    let auth = BasicAuth::atlassian(
        email.as_str(),
        api_token.expose(),
        ATLASSIAN_KEYCHAIN_SERVICE,
        "probe",
    );
    let cloud = discover_cloud(&http, &auth, &parsed, &CancellationToken::new(), None).await?;

    Ok(AtlassianValidationResult {
        account_id: cloud.account.account_id,
        display_name: cloud.account.display_name,
        email: cloud.account.email,
    })
}

/// Unwind the partial work a failing [`atlassian_sources_add`] left
/// behind. Called at every failure point after the first keychain
/// write / row insert. We log but do not propagate secondary errors
/// — the primary failure the caller returns is the one that matters;
/// a keychain row we failed to delete is picked up by the boot-time
/// orphan audit (DAY-81).
async fn rollback_sources_add(
    state: &AppState,
    source_repo: &SourceRepo,
    inserted: &[SourceId],
    wrote_new_secret: bool,
    secret_ref: &SecretRef,
) {
    for id in inserted {
        if let Err(e) = source_repo.delete(id).await {
            tracing::warn!(
                error = %e,
                source_id = %id,
                "atlassian rollback: sources.delete failed; row may linger",
            );
        }
    }
    if wrote_new_secret {
        let key = secret_store_key(secret_ref);
        if let Err(e) = state.secrets.delete(&key) {
            tracing::warn!(
                error = %e,
                %key,
                "atlassian rollback: keychain delete failed; row may linger",
            );
        }
    }
}

/// Persist one or two Atlassian `Source` rows. See the module docs
/// for the four journeys this single command implements and the
/// keychain-write invariants each enforces.
///
/// This is the thin Tauri wrapper — the real work lives in
/// [`atlassian_sources_add_impl`] so integration tests can drive the
/// same logic without building a [`State`].
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn atlassian_sources_add(
    workspace_url: String,
    email: String,
    api_token: Option<IpcSecretString>,
    account_id: String,
    enable_jira: bool,
    enable_confluence: bool,
    reuse_secret_ref: Option<SecretRef>,
    state: State<'_, AppState>,
) -> Result<Vec<Source>, DayseamError> {
    atlassian_sources_add_impl(
        &state,
        workspace_url,
        email,
        api_token,
        account_id,
        enable_jira,
        enable_confluence,
        reuse_secret_ref,
    )
    .await
}

/// Test-visible implementation of [`atlassian_sources_add`]. Same
/// shape minus the Tauri [`State`] wrapper, which cannot be
/// constructed outside the Tauri runtime.
#[allow(clippy::too_many_arguments)]
pub async fn atlassian_sources_add_impl(
    state: &AppState,
    workspace_url: String,
    email: String,
    api_token: Option<IpcSecretString>,
    account_id: String,
    enable_jira: bool,
    enable_confluence: bool,
    reuse_secret_ref: Option<SecretRef>,
) -> Result<Vec<Source>, DayseamError> {
    // ---- Structural validation -------------------------------------
    if !enable_jira && !enable_confluence {
        return Err(invalid_config_public(
            error_codes::IPC_ATLASSIAN_NO_PRODUCT_SELECTED,
            "at least one of enable_jira / enable_confluence must be true",
        ));
    }
    require_nonempty_email(&email)?;
    if account_id.trim().is_empty() {
        return Err(invalid_config_public(
            error_codes::IPC_ATLASSIAN_CREDENTIALS_MISSING,
            "account_id must not be empty; call atlassian_validate_credentials first",
        ));
    }
    let parsed_url = parse_workspace_url(&workspace_url)?;
    let canonical_url = canonical_workspace_url(&parsed_url);

    // ---- Keychain: resolve `SecretRef` -----------------------------
    // Two paths:
    //
    //   * `reuse_secret_ref = Some(_)`  → the caller is adding a
    //     product alongside one that already exists and wants the
    //     two to share a keychain row. Verify the slot is populated
    //     (so a stale dialog state cannot write a source row that
    //     points at an empty slot) and skip the token-required check.
    //   * `reuse_secret_ref = None`     → Journeys A / B / C-mode-2.
    //     `api_token` must be present and non-empty; we write a fresh
    //     keychain row keyed by a new UUID slot.
    //
    // The split happens *before* the DB write so a failure here
    // never leaves a half-created source behind.
    let (secret_ref, wrote_new_secret) = match reuse_secret_ref {
        Some(ref existing) => {
            let key = secret_store_key(existing);
            let present = state
                .secrets
                .get(&key)
                .map_err(|e| DayseamError::Internal {
                    code: error_codes::IPC_ATLASSIAN_KEYCHAIN_WRITE_FAILED.to_string(),
                    message: format!("keychain probe for {key} failed: {e}"),
                })?;
            if present.is_none() {
                return Err(invalid_config_public(
                    error_codes::IPC_ATLASSIAN_REUSE_SECRET_MISSING,
                    format!(
                        "reuse_secret_ref `{key}` is empty in the keychain; the owning source was \
                         likely deleted. Re-open the dialog and paste a fresh token instead."
                    ),
                ));
            }
            (existing.clone(), false)
        }
        None => {
            let token = api_token.as_ref().ok_or_else(|| {
                invalid_config_public(
                    error_codes::IPC_ATLASSIAN_CREDENTIALS_MISSING,
                    "api_token must be provided when reuse_secret_ref is null",
                )
            })?;
            require_nonempty_token(token.expose())?;
            let sr = new_atlassian_secret_ref();
            let key = secret_store_key(&sr);
            state
                .secrets
                .put(&key, Secret::new(token.expose().to_string()))
                .map_err(|e| DayseamError::Internal {
                    code: error_codes::IPC_ATLASSIAN_KEYCHAIN_WRITE_FAILED.to_string(),
                    message: format!("keychain write for {key} failed: {e}"),
                })?;
            (sr, true)
        }
    };

    let source_repo = SourceRepo::new(state.pool.clone());
    let identity_repo = SourceIdentityRepo::new(state.pool.clone());
    let mut inserted: Vec<SourceId> = Vec::new();

    // ---- Person (self) for identity seeding ------------------------
    let self_person = match PersonRepo::new(state.pool.clone())
        .bootstrap_self(SELF_DEFAULT_DISPLAY_NAME)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            rollback_sources_add(
                state,
                &source_repo,
                &inserted,
                wrote_new_secret,
                &secret_ref,
            )
            .await;
            return Err(DayseamError::Internal {
                code: "ipc.persons.bootstrap_self".to_string(),
                message: e.to_string(),
            });
        }
    };

    // ---- Insert each enabled product row ---------------------------
    let now = Utc::now();

    if enable_jira {
        let source_id = Uuid::new_v4();
        let source = Source {
            id: source_id,
            kind: SourceKind::Jira,
            label: format!(
                "Jira — {}",
                parsed_url.host_str().unwrap_or(canonical_url.as_str())
            ),
            config: SourceConfig::Jira {
                workspace_url: canonical_url.clone(),
                email: email.clone(),
            },
            secret_ref: Some(secret_ref.clone()),
            created_at: now,
            last_sync_at: None,
            last_health: SourceHealth::unchecked(),
        };
        if let Err(e) = source_repo.insert(&source).await {
            rollback_sources_add(
                state,
                &source_repo,
                &inserted,
                wrote_new_secret,
                &secret_ref,
            )
            .await;
            return Err(DayseamError::Internal {
                code: "ipc.sources.insert".to_string(),
                message: format!("sources.insert(jira) failed: {e}"),
            });
        }
        inserted.push(source_id);

        let info = account_info_from_id(&account_id);
        let identity = match seed_atlassian_identity(&info, source_id, self_person.id, None) {
            Ok(i) => i,
            Err(e) => {
                rollback_sources_add(
                    state,
                    &source_repo,
                    &inserted,
                    wrote_new_secret,
                    &secret_ref,
                )
                .await;
                return Err(e);
            }
        };
        if let Err(e) = identity_repo.ensure(&identity).await {
            rollback_sources_add(
                state,
                &source_repo,
                &inserted,
                wrote_new_secret,
                &secret_ref,
            )
            .await;
            return Err(DayseamError::Internal {
                code: "ipc.source_identities.ensure".to_string(),
                message: e.to_string(),
            });
        }
    }

    if enable_confluence {
        let source_id = Uuid::new_v4();
        let source = Source {
            id: source_id,
            kind: SourceKind::Confluence,
            label: format!(
                "Confluence — {}",
                parsed_url.host_str().unwrap_or(canonical_url.as_str())
            ),
            config: SourceConfig::Confluence {
                workspace_url: canonical_url.clone(),
            },
            secret_ref: Some(secret_ref.clone()),
            created_at: now,
            last_sync_at: None,
            last_health: SourceHealth::unchecked(),
        };
        if let Err(e) = source_repo.insert(&source).await {
            rollback_sources_add(
                state,
                &source_repo,
                &inserted,
                wrote_new_secret,
                &secret_ref,
            )
            .await;
            return Err(DayseamError::Internal {
                code: "ipc.sources.insert".to_string(),
                message: format!("sources.insert(confluence) failed: {e}"),
            });
        }
        inserted.push(source_id);

        let info = account_info_from_id(&account_id);
        let identity = match seed_atlassian_identity(&info, source_id, self_person.id, None) {
            Ok(i) => i,
            Err(e) => {
                rollback_sources_add(
                    state,
                    &source_repo,
                    &inserted,
                    wrote_new_secret,
                    &secret_ref,
                )
                .await;
                return Err(e);
            }
        };
        if let Err(e) = identity_repo.ensure(&identity).await {
            rollback_sources_add(
                state,
                &source_repo,
                &inserted,
                wrote_new_secret,
                &secret_ref,
            )
            .await;
            return Err(DayseamError::Internal {
                code: "ipc.source_identities.ensure".to_string(),
                message: e.to_string(),
            });
        }
    }

    // ---- Commit, re-read, toast ------------------------------------
    persist_restart_required_toast(state);

    let mut rows = Vec::with_capacity(inserted.len());
    for id in &inserted {
        match source_repo.get(id).await {
            Ok(Some(src)) => rows.push(src),
            Ok(None) => {
                return Err(invalid_config_public(
                    error_codes::IPC_SOURCE_NOT_FOUND,
                    format!("source {id} disappeared immediately after insert"),
                ));
            }
            Err(e) => {
                return Err(DayseamError::Internal {
                    code: "ipc.sources.get".to_string(),
                    message: e.to_string(),
                });
            }
        }
    }

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dayseam_db::open;
    use dayseam_events::AppBus;
    use dayseam_orchestrator::{ConnectorRegistry, OrchestratorBuilder, SinkRegistry};
    use dayseam_secrets::InMemoryStore;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn make_state() -> (AppState, TempDir) {
        let dir = TempDir::new().expect("temp dir");
        let pool = open(&dir.path().join("state.db")).await.expect("open db");
        let app_bus = AppBus::new();
        let orchestrator = OrchestratorBuilder::new(
            pool.clone(),
            app_bus.clone(),
            ConnectorRegistry::new(),
            SinkRegistry::new(),
        )
        .build()
        .expect("build orchestrator");
        let state = AppState::new(pool, app_bus, Arc::new(InMemoryStore::new()), orchestrator);
        (state, dir)
    }

    fn token() -> IpcSecretString {
        IpcSecretString::new("atlassian-pat-token")
    }

    #[test]
    fn parse_workspace_url_accepts_canonical_form() {
        let url = parse_workspace_url("https://modulrfinance.atlassian.net").unwrap();
        assert_eq!(
            canonical_workspace_url(&url),
            "https://modulrfinance.atlassian.net"
        );
    }

    #[test]
    fn parse_workspace_url_strips_trailing_slash() {
        let url = parse_workspace_url("https://modulrfinance.atlassian.net/").unwrap();
        assert_eq!(
            canonical_workspace_url(&url),
            "https://modulrfinance.atlassian.net"
        );
    }

    #[test]
    fn parse_workspace_url_rejects_non_https() {
        let err = parse_workspace_url("http://modulrfinance.atlassian.net").unwrap_err();
        assert_eq!(err.code(), error_codes::IPC_ATLASSIAN_INVALID_WORKSPACE_URL);
    }

    #[test]
    fn parse_workspace_url_rejects_path_segments() {
        let err = parse_workspace_url("https://modulrfinance.atlassian.net/wiki").unwrap_err();
        assert_eq!(err.code(), error_codes::IPC_ATLASSIAN_INVALID_WORKSPACE_URL);
    }

    #[test]
    fn parse_workspace_url_rejects_empty() {
        let err = parse_workspace_url("   ").unwrap_err();
        assert_eq!(err.code(), error_codes::IPC_ATLASSIAN_INVALID_WORKSPACE_URL);
    }

    #[test]
    fn parse_workspace_url_rejects_scheme_missing() {
        // URLs without a scheme fail `Url::parse` with "relative URL without a base".
        let err = parse_workspace_url("modulrfinance.atlassian.net").unwrap_err();
        assert_eq!(err.code(), error_codes::IPC_ATLASSIAN_INVALID_WORKSPACE_URL);
    }

    #[test]
    fn new_atlassian_secret_ref_is_unique_per_call() {
        let a = new_atlassian_secret_ref();
        let b = new_atlassian_secret_ref();
        assert_eq!(a.keychain_service, ATLASSIAN_KEYCHAIN_SERVICE);
        assert_eq!(b.keychain_service, ATLASSIAN_KEYCHAIN_SERVICE);
        assert_ne!(a.keychain_account, b.keychain_account);
        assert!(a.keychain_account.starts_with("slot:"));
    }

    // -------------------------------------------------------------------
    // `atlassian_sources_add_impl` IPC integration tests
    //
    // These cover the four journeys the module docs enumerate (A, B,
    // C-mode-1, C-mode-2) plus the rejection paths the frontend relies
    // on to short-circuit before firing the IPC at all.
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn journey_a_shared_pat_writes_two_sources_and_one_keychain_row() {
        // Shared-PAT default: one `sources_add` call with both products
        // enabled must land two `Source` rows pointing at the *same*
        // `SecretRef`, and the keychain must hold exactly one entry.
        let (state, _dir) = make_state().await;

        let rows = atlassian_sources_add_impl(
            &state,
            "https://acme.atlassian.net".into(),
            "user@acme.com".into(),
            Some(token()),
            "5d53f3cbc6b9320d9ea5bdc2".into(),
            true,
            true,
            None,
        )
        .await
        .expect("shared-PAT add succeeds");

        assert_eq!(rows.len(), 2, "shared PAT adds both products");
        let jira = rows.iter().find(|r| r.kind == SourceKind::Jira).unwrap();
        let conf = rows
            .iter()
            .find(|r| r.kind == SourceKind::Confluence)
            .unwrap();
        assert_eq!(
            jira.secret_ref, conf.secret_ref,
            "shared-PAT journey must write one SecretRef, reused across rows"
        );
        let sr = jira.secret_ref.clone().unwrap();
        assert_eq!(sr.keychain_service, ATLASSIAN_KEYCHAIN_SERVICE);
        assert!(sr.keychain_account.starts_with("slot:"));

        // Exactly one keychain row, and it holds the token we sent.
        let value = state
            .secrets
            .get(&secret_store_key(&sr))
            .expect("keychain get")
            .expect("keychain row present");
        assert_eq!(value.expose_secret(), "atlassian-pat-token");
    }

    #[tokio::test]
    async fn journey_b_single_product_writes_one_source() {
        // Single-product add: user enables exactly one product; the
        // other arm must not be touched.
        let (state, _dir) = make_state().await;

        let rows = atlassian_sources_add_impl(
            &state,
            "https://acme.atlassian.net".into(),
            "user@acme.com".into(),
            Some(token()),
            "acc-1".into(),
            true,
            false,
            None,
        )
        .await
        .expect("jira-only add succeeds");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, SourceKind::Jira);
    }

    #[tokio::test]
    async fn journey_c_mode_1_reuse_secret_ref_writes_zero_keychain_rows() {
        // Add Jira first, then Confluence reusing the same SecretRef.
        // The second call must NOT write a new keychain row — the
        // dialog's "reuse existing token" path is how DAY-81's
        // refcount guard actually gets shared keychain rows in
        // practice.
        let (state, _dir) = make_state().await;

        let jira_rows = atlassian_sources_add_impl(
            &state,
            "https://acme.atlassian.net".into(),
            "user@acme.com".into(),
            Some(token()),
            "acc-1".into(),
            true,
            false,
            None,
        )
        .await
        .expect("jira add succeeds");
        let shared_ref = jira_rows[0]
            .secret_ref
            .clone()
            .expect("jira has secret_ref");

        // Now add Confluence reusing the exact same SecretRef, with
        // `api_token = None` — reuse mode must not demand a token.
        let conf_rows = atlassian_sources_add_impl(
            &state,
            "https://acme.atlassian.net".into(),
            "user@acme.com".into(),
            None,
            "acc-1".into(),
            false,
            true,
            Some(shared_ref.clone()),
        )
        .await
        .expect("confluence reuse-PAT add succeeds");

        assert_eq!(conf_rows.len(), 1);
        assert_eq!(conf_rows[0].kind, SourceKind::Confluence);
        assert_eq!(
            conf_rows[0].secret_ref.as_ref(),
            Some(&shared_ref),
            "reuse mode must point at the same SecretRef"
        );
    }

    #[tokio::test]
    async fn journey_c_mode_2_separate_pat_writes_new_keychain_row() {
        // Second product with a *different* PAT: must write a brand-
        // new keychain slot uncoupled from the first source's row.
        let (state, _dir) = make_state().await;

        let jira_rows = atlassian_sources_add_impl(
            &state,
            "https://acme.atlassian.net".into(),
            "user@acme.com".into(),
            Some(token()),
            "acc-1".into(),
            true,
            false,
            None,
        )
        .await
        .expect("jira add");
        let jira_ref = jira_rows[0].secret_ref.clone().unwrap();

        let conf_rows = atlassian_sources_add_impl(
            &state,
            "https://acme.atlassian.net".into(),
            "user@acme.com".into(),
            Some(IpcSecretString::new("different-token")),
            "acc-1".into(),
            false,
            true,
            None,
        )
        .await
        .expect("confluence separate-PAT add");
        let conf_ref = conf_rows[0].secret_ref.clone().unwrap();

        assert_ne!(
            jira_ref.keychain_account, conf_ref.keychain_account,
            "separate-PAT mode must mint a fresh keychain slot"
        );
        let conf_token = state
            .secrets
            .get(&secret_store_key(&conf_ref))
            .expect("get")
            .expect("present");
        assert_eq!(conf_token.expose_secret(), "different-token");
    }

    #[tokio::test]
    async fn rejects_when_both_products_disabled() {
        let (state, _dir) = make_state().await;
        let err = atlassian_sources_add_impl(
            &state,
            "https://acme.atlassian.net".into(),
            "user@acme.com".into(),
            Some(token()),
            "acc-1".into(),
            false,
            false,
            None,
        )
        .await
        .expect_err("must reject when nothing is enabled");
        assert_eq!(err.code(), error_codes::IPC_ATLASSIAN_NO_PRODUCT_SELECTED);
    }

    #[tokio::test]
    async fn rejects_reuse_when_keychain_slot_is_empty() {
        // Stale dialog state: the user had two sources, deleted the
        // owner, then tried to add the other product. The secret_ref
        // the dialog kept in state is pointing at an empty slot, and
        // we must refuse to persist a row that would reference a
        // missing key.
        let (state, _dir) = make_state().await;
        let stale = SecretRef {
            keychain_service: ATLASSIAN_KEYCHAIN_SERVICE.into(),
            keychain_account: "slot:deadbeef".into(),
        };
        let err = atlassian_sources_add_impl(
            &state,
            "https://acme.atlassian.net".into(),
            "user@acme.com".into(),
            None,
            "acc-1".into(),
            true,
            false,
            Some(stale),
        )
        .await
        .expect_err("must reject reuse of empty slot");
        assert_eq!(err.code(), error_codes::IPC_ATLASSIAN_REUSE_SECRET_MISSING);
    }

    #[tokio::test]
    async fn rejects_fresh_add_without_api_token() {
        // `reuse_secret_ref = None` and `api_token = None` is a caller
        // bug: the dialog should have either collected a token or
        // passed a SecretRef. Refuse before touching sqlite.
        let (state, _dir) = make_state().await;
        let err = atlassian_sources_add_impl(
            &state,
            "https://acme.atlassian.net".into(),
            "user@acme.com".into(),
            None,
            "acc-1".into(),
            true,
            false,
            None,
        )
        .await
        .expect_err("must reject fresh add without token");
        assert_eq!(err.code(), error_codes::IPC_ATLASSIAN_CREDENTIALS_MISSING);
    }

    #[tokio::test]
    async fn rejects_empty_email() {
        let (state, _dir) = make_state().await;
        let err = atlassian_sources_add_impl(
            &state,
            "https://acme.atlassian.net".into(),
            "   ".into(),
            Some(token()),
            "acc-1".into(),
            true,
            false,
            None,
        )
        .await
        .expect_err("whitespace email must be rejected");
        assert_eq!(err.code(), error_codes::IPC_ATLASSIAN_CREDENTIALS_MISSING);
    }

    #[tokio::test]
    async fn rejects_malformed_workspace_url() {
        let (state, _dir) = make_state().await;
        let err = atlassian_sources_add_impl(
            &state,
            "not-a-url".into(),
            "user@acme.com".into(),
            Some(token()),
            "acc-1".into(),
            true,
            false,
            None,
        )
        .await
        .expect_err("malformed url must be rejected");
        assert_eq!(err.code(), error_codes::IPC_ATLASSIAN_INVALID_WORKSPACE_URL);
    }

    #[tokio::test]
    async fn rejects_empty_account_id() {
        // `atlassian_sources_add` is a purely transactional op — the
        // frontend is required to call `validate_credentials` first
        // and pass the resulting `account_id` through. An empty
        // account_id here means the dialog skipped validation, which
        // would leave `source_identities` unseeded.
        let (state, _dir) = make_state().await;
        let err = atlassian_sources_add_impl(
            &state,
            "https://acme.atlassian.net".into(),
            "user@acme.com".into(),
            Some(token()),
            "   ".into(),
            true,
            false,
            None,
        )
        .await
        .expect_err("empty account_id must be rejected");
        assert_eq!(err.code(), error_codes::IPC_ATLASSIAN_CREDENTIALS_MISSING);
    }
}
