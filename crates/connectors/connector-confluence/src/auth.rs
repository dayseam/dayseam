//! Credential-validation + identity-seed helpers for the Confluence
//! connector.
//!
//! The shape mirrors [`connector_jira::auth`] one-for-one. Both
//! products authenticate through the same Atlassian Cloud endpoint
//! (`GET /rest/api/3/myself`) and seed the same
//! [`dayseam_core::SourceIdentityKind::AtlassianAccountId`] row; the
//! only reason these helpers live in a Confluence-specific crate is
//! so the IPC layer (DAY-82) can import
//! `connector_confluence::validate_auth` rather than reaching into
//! [`connector_atlassian_common`] directly. That keeps the Jira /
//! Confluence scaffolds symmetric and makes the sharing intentional.
//!
//! These are the two entry points the IPC layer calls when a user
//! pastes an Atlassian email + API token for a Confluence source:
//!
//! 1. [`validate_auth`] — cheap, read-only probe of
//!    `GET <workspace>/rest/api/3/myself`. Returns the account
//!    triple the dialog displays ("Connected as …") and the
//!    `accountId` the next step consumes. A 401/403/404 here is
//!    mapped to the registry-defined `atlassian.*` codes by
//!    [`connector_atlassian_common::discover_cloud`] so the dialog
//!    can render actionable messages without peeking at raw HTTP
//!    status codes.
//! 2. [`list_identities`] — pure, synchronous transform that converts
//!    the account triple into the [`SourceIdentity`] row the
//!    activity walker will later filter by. Kept out of
//!    [`validate_auth`] so the IPC layer can run the identity seed
//!    inside the same DB transaction that writes the new
//!    [`dayseam_core::Source`] row.
//!
//! ## Shared-identity invariant
//!
//! A single email + API-token credential authenticates one Jira row
//! and one Confluence row for the same workspace. The
//! [`list_identities`] helper is a pure mapping from
//! [`AtlassianAccountInfo`] → [`SourceIdentity`], so calling it from
//! the Confluence crate for the same Atlassian account yields a row
//! with the **same** `(kind, external_actor_id)` Jira produced.
//! Row-level dedup across sources happens at the DB layer
//! (`SourceIdentityRepo::ensure` keys on
//! `(person_id, source_id, kind, external_actor_id)`), so each source
//! gets its own row — but the row the walker filters by (kind +
//! external_actor_id) is the same for both products, which is the
//! point of "one credential serves both".

use connector_atlassian_common::{
    discover_cloud, seed_atlassian_identity, AtlassianAccountInfo, AtlassianCloud,
};
use connectors_sdk::{BasicAuth, HttpClient};
use dayseam_core::{DayseamError, SourceIdentity};
use dayseam_events::LogSender;
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

/// Probe a Confluence Cloud workspace with the supplied Basic-auth
/// credential.
///
/// Thin wrapper around
/// [`connector_atlassian_common::discover_cloud`]; kept in this crate
/// so the IPC layer has a single `connector_confluence::validate_auth`
/// symbol to import, exactly parallel to
/// [`connector_jira::validate_auth`]. The probe endpoint is the same
/// `GET /rest/api/3/myself` Jira uses — an Atlassian Cloud credential
/// that can read `/myself` can also reach `/wiki/rest/api/*` content
/// the same workspace exposes.
///
/// Errors surface verbatim from the common crate; see
/// [`discover_cloud`] for the full taxonomy.
pub async fn validate_auth(
    http: &HttpClient,
    auth: &BasicAuth,
    workspace_url: &Url,
    cancel: &CancellationToken,
    logs: Option<&LogSender>,
) -> Result<AtlassianCloud, DayseamError> {
    discover_cloud(http, auth, workspace_url, cancel, logs).await
}

/// Build the [`SourceIdentity`] rows a freshly-added Confluence source
/// needs. Today that is exactly one row (the self-identity), so the
/// return shape is `Vec<SourceIdentity>` with a single element; the
/// `Vec` type is chosen so a future extension (e.g. also mirror the
/// reporter's alternate account) doesn't require a signature change
/// at every IPC caller.
///
/// Pure helper — no I/O, no DB writes. The caller (DAY-82 IPC
/// command) is responsible for persisting the returned rows inside
/// the same transaction that writes the source. Matches
/// [`connector_jira::list_identities`] exactly so the shared-PAT flow
/// is provably consistent: given the same [`AtlassianAccountInfo`],
/// both helpers emit a row with identical
/// `(kind, external_actor_id)`.
pub fn list_identities(
    info: &AtlassianAccountInfo,
    source_id: Uuid,
    person_id: Uuid,
    logs: Option<&LogSender>,
) -> Result<Vec<SourceIdentity>, DayseamError> {
    let identity = seed_atlassian_identity(info, source_id, person_id, logs)?;
    Ok(vec![identity])
}

#[cfg(test)]
mod tests {
    use super::*;
    use dayseam_core::{error_codes, SourceIdentityKind};

    fn sample_info(account_id: &str) -> AtlassianAccountInfo {
        AtlassianAccountInfo {
            account_id: account_id.into(),
            display_name: "Vedanth Vasudev".into(),
            email: Some("vedanth@acme.com".into()),
            cloud_id: None,
        }
    }

    #[test]
    fn list_identities_returns_exactly_one_row_on_happy_path() {
        let source = Uuid::new_v4();
        let person = Uuid::new_v4();
        let info = sample_info("5d53f3cbc6b9320d9ea5bdc2");
        let identities = list_identities(&info, source, person, None).unwrap();
        assert_eq!(identities.len(), 1);
        let row = &identities[0];
        assert_eq!(row.person_id, person);
        assert_eq!(row.source_id, Some(source));
        assert_eq!(row.kind, SourceIdentityKind::AtlassianAccountId);
        assert_eq!(row.external_actor_id, "5d53f3cbc6b9320d9ea5bdc2");
    }

    #[test]
    fn list_identities_propagates_malformed_account_id_error() {
        let source = Uuid::new_v4();
        let person = Uuid::new_v4();
        let info = sample_info("");
        let err = list_identities(&info, source, person, None).unwrap_err();
        assert_eq!(
            err.code(),
            error_codes::ATLASSIAN_IDENTITY_MALFORMED_ACCOUNT_ID
        );
    }
}
