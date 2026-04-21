//! End-to-end tests for [`connector_confluence::validate_auth`] +
//! [`connector_confluence::list_identities`].
//!
//! The common crate's own tests prove `discover_cloud` classifies
//! each HTTP status correctly; these tests prove the Confluence
//! wrapper preserves that classification end-to-end, and — the
//! DAY-79-specific invariant — that the Confluence `list_identities`
//! is byte-identical to the Jira one on the `(kind, external_actor_id)`
//! tuple the walker filters by, so a shared Atlassian credential
//! really does "serve both products" at the identity layer.
//!
//! Covers the invariants listed in the plan (§Task 7):
//! * `validate_auth_200` — happy path, account triple round-trips.
//! * `validate_auth_401` — `atlassian.auth.invalid_credentials`.
//! * `validate_auth_403` — `atlassian.auth.missing_scope`.
//! * `list_identities_is_shared_across_products` — Jira + Confluence
//!   helpers produce identity rows with the same
//!   `(kind, external_actor_id)` from the same
//!   [`AtlassianAccountInfo`].

use connector_atlassian_common::AtlassianAccountInfo;
use connector_confluence::{list_identities, validate_auth};
use connector_jira::list_identities as jira_list_identities;
use connectors_sdk::{BasicAuth, HttpClient, RetryPolicy};
use dayseam_core::{error_codes, DayseamError, SourceIdentityKind};
use dayseam_events::{RunId, RunStreams};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;
use wiremock::matchers::{header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn http() -> HttpClient {
    HttpClient::new()
        .expect("build http client")
        .with_policy(RetryPolicy::instant())
}

fn auth() -> BasicAuth {
    BasicAuth::atlassian(
        "vedanth@acme.com",
        "api-token-xyz",
        "dayseam.atlassian",
        "vedanth@acme.com",
    )
}

fn workspace(server: &MockServer) -> Url {
    // Match the real production flow where `ConfluenceConfig::from_raw`
    // pads a trailing slash so `Url::join` preserves the last path
    // segment when appending `/wiki/rest/api/…`.
    Url::parse(&format!("{}/", server.uri())).expect("mock uri parses")
}

#[tokio::test]
async fn validate_auth_200_returns_account_info() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .and(header_exists("Authorization"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "5d53f3cbc6b9320d9ea5bdc2",
            "displayName": "Vedanth Vasudev",
            "emailAddress": "vedanth@acme.com",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let streams = RunStreams::new(RunId::new());
    let ((_ptx, ltx), (_, _)) = streams.split();
    let cancel = CancellationToken::new();

    let cloud = validate_auth(&http(), &auth(), &workspace(&server), &cancel, Some(&ltx))
        .await
        .expect("200 must yield AtlassianCloud");

    assert_eq!(cloud.account.account_id, "5d53f3cbc6b9320d9ea5bdc2");
    assert_eq!(cloud.account.display_name, "Vedanth Vasudev");
    assert_eq!(cloud.account.email.as_deref(), Some("vedanth@acme.com"));
    // Basic-auth flow: `cloud_id` is deliberately absent — the
    // OAuth-era opaque UUID lives at `/_edge/tenant_info` which
    // Basic credentials cannot reach. Same rationale as the Jira
    // variant of this test.
    assert!(cloud.account.cloud_id.is_none());
}

#[tokio::test]
async fn validate_auth_401_maps_to_atlassian_auth_invalid_credentials() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthenticated"))
        .expect(1)
        .mount(&server)
        .await;

    let cancel = CancellationToken::new();
    let err = validate_auth(&http(), &auth(), &workspace(&server), &cancel, None)
        .await
        .expect_err("401 must surface as DayseamError");

    assert_eq!(err.code(), error_codes::ATLASSIAN_AUTH_INVALID_CREDENTIALS);
    assert!(matches!(err, DayseamError::Auth { .. }));
}

#[tokio::test]
async fn validate_auth_403_maps_to_atlassian_auth_missing_scope() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(403).set_body_string("forbidden"))
        .expect(1)
        .mount(&server)
        .await;

    let cancel = CancellationToken::new();
    let err = validate_auth(&http(), &auth(), &workspace(&server), &cancel, None)
        .await
        .expect_err("403 must surface as DayseamError");

    assert_eq!(err.code(), error_codes::ATLASSIAN_AUTH_MISSING_SCOPE);
    assert!(matches!(err, DayseamError::Auth { .. }));
}

#[tokio::test]
async fn validate_auth_404_maps_to_atlassian_cloud_resource_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;

    let cancel = CancellationToken::new();
    let err = validate_auth(&http(), &auth(), &workspace(&server), &cancel, None)
        .await
        .expect_err("404 must surface as DayseamError");

    assert_eq!(err.code(), error_codes::ATLASSIAN_CLOUD_RESOURCE_NOT_FOUND);
    assert!(matches!(err, DayseamError::Network { .. }));
}

#[test]
fn list_identities_is_shared_across_products() {
    // Plan invariant 4 (§Task 7):
    //
    //     If a Jira source already exists for the same workspace +
    //     secret_id, calling `list_identities` on a new Confluence
    //     source for the same workspace returns the existing
    //     AtlassianAccountId row, not a duplicate.
    //
    // Row-level dedup happens at the DB layer
    // (`SourceIdentityRepo::ensure` unique-indexes on
    // `(person_id, source_id, kind, external_actor_id)`), so each
    // source legitimately gets its own row — what the walker
    // filters by is `(kind, external_actor_id)`, and that tuple
    // is what must be stable across the two products. This test
    // pins exactly that invariant: given the same
    // `AtlassianAccountInfo`, the Jira helper and the Confluence
    // helper emit rows with identical
    // `(kind, external_actor_id)`. A drift here would silently
    // break the "one credential serves both" promise.
    let info = AtlassianAccountInfo {
        account_id: "5d53f3cbc6b9320d9ea5bdc2".into(),
        display_name: "Vedanth Vasudev".into(),
        email: Some("vedanth@acme.com".into()),
        cloud_id: None,
    };
    let person = Uuid::new_v4();

    let jira_source = Uuid::new_v4();
    let jira = &jira_list_identities(&info, jira_source, person, None).unwrap()[0];

    let confluence_source = Uuid::new_v4();
    let confluence = &list_identities(&info, confluence_source, person, None).unwrap()[0];

    assert_eq!(jira.kind, SourceIdentityKind::AtlassianAccountId);
    assert_eq!(confluence.kind, jira.kind);
    assert_eq!(confluence.external_actor_id, jira.external_actor_id);
    assert_eq!(confluence.person_id, jira.person_id);
    // The `source_id` differs — that is the whole point of a
    // per-source identity row — but the walker key does not.
    assert_ne!(confluence.source_id, jira.source_id);
}

#[tokio::test]
async fn list_identities_seeds_one_row_per_successful_probe() {
    // Round-trip the happy path of the IPC add-source flow: probe
    // the workspace, then seed the identity row the DAY-80 walker
    // will filter by. Mirrors the Jira auth test so the Atlassian
    // side stays symmetric across the two connectors.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "5d53f3cbc6b9320d9ea5bdc2",
            "displayName": "Vedanth Vasudev",
            "emailAddress": "vedanth@acme.com",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let cancel = CancellationToken::new();
    let cloud = validate_auth(&http(), &auth(), &workspace(&server), &cancel, None)
        .await
        .expect("validation succeeds");

    let source_id = Uuid::new_v4();
    let person_id = Uuid::new_v4();
    let identities =
        list_identities(&cloud.account, source_id, person_id, None).expect("identity seed is pure");

    assert_eq!(identities.len(), 1);
    let row = &identities[0];
    assert_eq!(row.person_id, person_id);
    assert_eq!(row.source_id, Some(source_id));
    assert_eq!(row.kind, SourceIdentityKind::AtlassianAccountId);
    assert_eq!(row.external_actor_id, "5d53f3cbc6b9320d9ea5bdc2");
}
