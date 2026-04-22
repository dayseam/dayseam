//! Credential-validation + identity-seed helpers for the GitHub
//! connector.
//!
//! These are the two entry points the IPC layer (DAY-99 Add-Source
//! dialog) calls when a user pastes a GitHub personal access token:
//!
//! 1. [`validate_auth`] — cheap, read-only probe of
//!    `GET <api_base_url>/user`. Returns the `{ id, login, name }`
//!    triple the dialog displays ("Connected as …") and the numeric
//!    `id` the identity seed consumes. 401 / 403 / 404 here are
//!    mapped to the registry-defined `github.*` codes by
//!    [`crate::errors::map_status`] so the dialog can render
//!    actionable messages without peeking at raw HTTP status codes.
//!
//! 2. [`list_identities`] — pure, synchronous transform that converts
//!    the `GithubUserInfo` triple into the
//!    [`dayseam_core::SourceIdentity`] row the activity walker will
//!    later filter by. Kept out of `validate_auth` so the IPC layer
//!    can run the identity seed inside the same DB transaction that
//!    writes the new [`dayseam_core::Source`] row — mirrors
//!    `ensure_gitlab_self_identity` and the atlassian-common
//!    `seed_atlassian_identity` helpers.
//!
//! The PAT is **never** borrowed as a `&str` by this module; it lives
//! inside a [`connectors_sdk::PatAuth`] throughout, so the raw bytes
//! never leave the `SecretString` wrapper. This differs from
//! `connector-gitlab::auth::validate_pat` (which takes a `&str pat`
//! parameter dating from v0.1) and matches `connector-jira::auth`'s
//! `&BasicAuth` shape — the v0.2-established pattern the rest of the
//! connector family converges on.

use connectors_sdk::{AuthStrategy, HttpClient, PatAuth};
use dayseam_core::{DayseamError, SourceIdentity, SourceIdentityKind};
use dayseam_events::LogSender;
use reqwest::StatusCode;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use crate::errors::{map_status, GithubUpstreamError};

/// Shape returned by GitHub's `GET /user` endpoint. The real response
/// carries ~30 fields (avatar URL, company, bio, plan, …); we keep
/// only the three the add-source flow needs.
///
/// `id` is the authoritative identity anchor. A GitHub user's `login`
/// is mutable (users can rename), but `id` is stable for the lifetime
/// of the account — so the self-filter in DAY-96's walker keys off
/// `id`, and the `login` is only used for the bullet's human-readable
/// "by @handle" suffix. This mirrors GitLab's `(user_id, username)`
/// pair.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GithubUserInfo {
    /// Numeric user id (`GET /user` → `.id`). Stable across rename.
    pub id: i64,
    /// Login handle — the `@handle` form, rendered in bullet copy
    /// and stored as a [`SourceIdentityKind::GitHubLogin`] row (the
    /// persist happens on the IPC side; this struct is just the
    /// probe's return type).
    pub login: String,
    /// Display name — optional on GitHub (users can leave it blank).
    /// Surfaced in the Add-Source dialog's "Connected as …" affordance
    /// when present; falls back to `login` otherwise.
    #[serde(default)]
    pub name: Option<String>,
}

/// Probe a GitHub tenant with the supplied [`PatAuth`].
///
/// Runs `GET {api_base_url}/user` and returns the echoed
/// `GithubUserInfo` on success. Non-success statuses are funnelled
/// through [`map_status`] into a typed `DayseamError` with a
/// registered `github.*` code.
///
/// The function does exactly **one** HTTP call — we deliberately do
/// not probe `/rate_limit` or `/meta` to sanity-check reachability
/// first. Failure modes on those endpoints are identical to
/// `/user`'s (the user's PAT is either valid or it isn't), so
/// double-probing would only double the latency and the failure
/// surface.
///
/// `cancel` is honoured end-to-end: a cancellation mid-request
/// returns [`DayseamError::Cancelled`] via the SDK's
/// [`HttpClient::send`] retry loop, the same way the Jira probe
/// behaves.
pub async fn validate_auth(
    http: &HttpClient,
    auth: &PatAuth,
    api_base_url: &Url,
    cancel: &CancellationToken,
    logs: Option<&LogSender>,
) -> Result<GithubUserInfo, DayseamError> {
    let url = api_base_url
        .join("user")
        .map_err(|e| DayseamError::InvalidConfig {
            code: "github.config.bad_api_base_url".to_string(),
            message: format!("cannot join `/user` onto API base URL: {e}"),
        })?;

    // `Accept: application/vnd.github+json` is the documented-forward
    // header; plain `application/json` also works today but GitHub's
    // own docs warn that API changes are versioned through the Accept
    // header. Using the vendor form means a future deprecation notice
    // reaches us the same way it reaches every other documented
    // client.
    let request = http
        .reqwest()
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    let request = auth.authenticate(request).await?;
    let response = http.send(request, cancel, None, logs).await?;

    let status = response.status();
    if status.is_success() {
        let info: GithubUserInfo = response.json().await.map_err(|err| {
            DayseamError::from(GithubUpstreamError::ShapeChanged {
                message: format!("failed to decode /user response: {err}"),
            })
        })?;
        return Ok(info);
    }

    // Pull the body for the error message — bounded to 4 KiB so a
    // pathological upstream cannot balloon the error log. Matches
    // the atlassian-common `discover_cloud` shape.
    let body_snippet = response
        .text()
        .await
        .unwrap_or_else(|_| String::new())
        .chars()
        .take(4096)
        .collect::<String>();

    // CORR-01 compliance is proven upstream in the SDK — a 401 / 403
    // reaches us here as a raw response, never a pre-classified
    // `DayseamError`. Classification lives in this crate, per the
    // v0.2 CORR-01 fix.
    let context = match status {
        StatusCode::NOT_FOUND => format!(
            "GET /user returned 404 for {api_base_url}; the API base URL is likely mistyped \
             (GitHub Enterprise hosts must include `/api/v3`): {body_snippet}"
        ),
        _ => body_snippet,
    };
    Err(map_status(status, context).into())
}

/// Build the [`SourceIdentity`] rows a freshly-added GitHub source
/// needs. Today that is exactly one row (the self-identity keyed off
/// the numeric user id), so the return shape is
/// `Vec<SourceIdentity>` with a single element; the `Vec` type is
/// chosen so a future "also seed the `GitHubLogin` row for
/// human-readable attribution" extension doesn't require a signature
/// change at every IPC caller.
///
/// Pure helper — no I/O, no DB writes. The caller (DAY-99 IPC command)
/// is responsible for persisting the returned rows inside the same
/// transaction that writes the source. Mirrors
/// `ensure_gitlab_self_identity` in spirit:
/// identity-on-credentials, not identity-on-first-sync, because the
/// DAY-71 post-mortem showed the delayed-seed flow silently drops
/// every event produced before the first seed commits.
pub fn list_identities(
    info: &GithubUserInfo,
    source_id: Uuid,
    person_id: Uuid,
    _logs: Option<&LogSender>,
) -> Result<Vec<SourceIdentity>, DayseamError> {
    if info.id <= 0 {
        // GitHub never issues non-positive ids — 0 or negative is a
        // shape corruption (upstream bug or a test fixture that
        // forgot to populate the field). Refuse rather than persist
        // a row that would match nothing at filter time.
        return Err(DayseamError::from(GithubUpstreamError::ShapeChanged {
            message: format!(
                "GitHub /user returned a non-positive user id ({}); refusing to \
                 seed an identity row that would silently match no events",
                info.id
            ),
        }));
    }
    let identity = SourceIdentity {
        id: Uuid::new_v4(),
        person_id,
        source_id: Some(source_id),
        kind: SourceIdentityKind::GitHubUserId,
        external_actor_id: info.id.to_string(),
    };
    Ok(vec![identity])
}

#[cfg(test)]
mod tests {
    use super::*;
    use dayseam_core::error_codes;

    fn sample_info(id: i64) -> GithubUserInfo {
        GithubUserInfo {
            id,
            login: "vedanth".into(),
            name: Some("Vedanth Vasudev".into()),
        }
    }

    #[test]
    fn github_user_info_decodes_and_drops_extra_fields() {
        // GitHub's real `/user` response returns avatar_url, bio,
        // company, and many other fields. We only keep id + login +
        // name and must silently drop the rest.
        let padded = r#"{
            "login": "vedanth",
            "id": 17,
            "node_id": "MDQ6VXNlcjE3",
            "avatar_url": "https://avatars.githubusercontent.com/u/17",
            "type": "User",
            "name": "Vedanth Vasudev",
            "company": "@acme-corp",
            "blog": "https://example.com",
            "plan": {"name": "pro"}
        }"#;
        let back: GithubUserInfo =
            serde_json::from_str(padded).expect("serde should ignore unknown fields by default");
        assert_eq!(back.id, 17);
        assert_eq!(back.login, "vedanth");
        assert_eq!(back.name.as_deref(), Some("Vedanth Vasudev"));
    }

    #[test]
    fn github_user_info_name_is_optional() {
        // Users who have not set a display name yield `name: null`
        // on the wire. `#[serde(default)]` + `Option<String>` makes
        // that round-trip without error.
        let no_name = r#"{"login": "nameless", "id": 99, "name": null}"#;
        let back: GithubUserInfo = serde_json::from_str(no_name).expect("null name deserialises");
        assert!(back.name.is_none());

        let missing_name = r#"{"login": "nameless", "id": 99}"#;
        let back: GithubUserInfo =
            serde_json::from_str(missing_name).expect("missing name deserialises");
        assert!(back.name.is_none());
    }

    #[test]
    fn list_identities_returns_exactly_one_row_on_happy_path() {
        let source = Uuid::new_v4();
        let person = Uuid::new_v4();
        let info = sample_info(17);
        let identities = list_identities(&info, source, person, None).unwrap();
        assert_eq!(identities.len(), 1);
        let row = &identities[0];
        assert_eq!(row.person_id, person);
        assert_eq!(row.source_id, Some(source));
        assert_eq!(row.kind, SourceIdentityKind::GitHubUserId);
        assert_eq!(row.external_actor_id, "17");
    }

    #[test]
    fn list_identities_rejects_non_positive_user_id_as_shape_change() {
        // A GitHub response with `"id": 0` (or negative) is a shape
        // corruption — GitHub never mints those. Persisting would
        // create an identity row that silently matches zero events
        // at filter time; refuse loudly so the IPC layer can show
        // the "upstream shape changed" card.
        let source = Uuid::new_v4();
        let person = Uuid::new_v4();
        let info = sample_info(0);
        let err = list_identities(&info, source, person, None).unwrap_err();
        assert_eq!(err.code(), error_codes::GITHUB_UPSTREAM_SHAPE_CHANGED);
        assert_eq!(err.variant(), "UpstreamChanged");
    }
}
