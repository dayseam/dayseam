//! End-to-end wiremock-driven tests for the DAY-77 Jira JQL walker.
//!
//! The plan's Task 2 matrix, reflecting the `CAR-5117` + `KTON-4550`
//! field-data the spike captured (see
//! `docs/spikes/2026-04-20-atlassian-connectors-data-shape.md`):
//!
//! 1. **Happy path** — a single issue with one self-authored status
//!    transition and one self-authored comment produces two normalised
//!    events.
//! 2. **Rapid-transition rollup** — six consecutive status transitions
//!    by the same author within 10 seconds (the `CAR-5117` cascade)
//!    collapse into exactly one `JiraIssueTransitioned` event whose
//!    `metadata.transition_count == 6` and whose `from_status` /
//!    `to_status` span the first and last transition.
//! 3. **ADF comment** — a `KTON-4550`-style comment whose body is an
//!    ADF doc with a `@mention` renders the mention's `@displayName`
//!    (not the `accountId`) in `event.body`.
//! 4. **Self-filter** — a comment authored by someone *other* than the
//!    configured `AtlassianAccountId` is silently dropped.
//! 5. **Pagination** — the walker drives multiple pages via
//!    `nextPageToken` and only stops when the upstream signals
//!    `isLast: true`.
//! 6. **Rate limit** — the walker's `429` path surfaces as
//!    `DayseamError::RateLimited { code: jira.walk.rate_limited, … }`,
//!    never leaking the SDK's internal `http.*` code.
//! 7. **Shape guard** — a JQL response missing the `issues` array
//!    fails loudly with `jira.walk.upstream_shape_changed` (the
//!    DAY-71 invariant: a silent empty report is the worst outcome).
//! 8. **Identity miss** — no `AtlassianAccountId` identity registered
//!    for the source returns an empty outcome without ever issuing a
//!    JQL (exercises the early-bail arm in `walk_day`).
//!
//! These live alongside the inline unit tests in
//! `normalise.rs`/`rollup.rs`/`walk.rs`: the unit tests pin
//! per-function contracts, and these tests pin the full authn →
//! HTTP → paginate → normalise → rollup round-trip the orchestrator
//! will actually invoke.

use std::sync::Arc;

use chrono::{FixedOffset, NaiveDate};
use connector_jira::walk::walk_day;
use connectors_sdk::{AuthStrategy, BasicAuth, HttpClient, RetryPolicy};
use dayseam_core::{
    error_codes, ActivityKind, DayseamError, SourceId, SourceIdentity, SourceIdentityKind,
};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---- Test scaffolding ----------------------------------------------------

const SELF_ACCOUNT: &str = "5d53f3cbc6b9320d9ea5bdc2";

fn utc() -> FixedOffset {
    FixedOffset::east_opt(0).unwrap()
}

fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 4, 20).unwrap()
}

fn source_id() -> SourceId {
    Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap()
}

fn http_for_tests() -> HttpClient {
    HttpClient::new()
        .expect("HttpClient::new")
        .with_policy(RetryPolicy::instant())
}

fn auth_for_tests() -> Arc<dyn AuthStrategy> {
    Arc::new(BasicAuth::atlassian(
        "me@acme.com",
        "api-token",
        "dayseam.jira",
        "acme",
    ))
}

fn self_identity() -> Vec<SourceIdentity> {
    vec![SourceIdentity {
        id: Uuid::new_v4(),
        person_id: Uuid::new_v4(),
        kind: SourceIdentityKind::AtlassianAccountId,
        external_actor_id: SELF_ACCOUNT.into(),
        source_id: Some(source_id()),
    }]
}

fn workspace(server: &MockServer) -> Url {
    // Ensure the workspace URL ends with a trailing slash so the
    // walker's `workspace_url.join("rest/api/3/search/jql")` lands on
    // `/rest/api/3/search/jql` instead of replacing the final segment.
    Url::parse(&format!("{}/", server.uri())).unwrap()
}

/// Minimal single-issue envelope with optional `changelog` + `comment`
/// overlays, matching the shape the spike captured.
fn issue(
    key: &str,
    summary: &str,
    changelog_histories: Value,
    comments: Value,
    created: Option<&str>,
    reporter_account: Option<&str>,
) -> Value {
    let mut issue = json!({
        "id": "10001",
        "key": key,
        "fields": {
            "summary": summary,
            "status": {
                "name": "In Progress",
                "statusCategory": {"key": "indeterminate", "name": "In Progress"}
            },
            "issuetype": {"name": "Task"},
            "project": {"id": "10", "key": key.split('-').next().unwrap(), "name": "Test Project"},
            "priority": {"name": "Medium"},
            "labels": [],
            "updated": "2026-04-20T10:00:00.000+0000",
            "comment": {"comments": comments}
        },
        "changelog": {"histories": changelog_histories}
    });
    if let Some(c) = created {
        issue["fields"]["created"] = json!(c);
    }
    if let Some(acct) = reporter_account {
        issue["fields"]["reporter"] = json!({"accountId": acct, "displayName": "Reporter"});
    }
    issue
}

async fn mount_jql_returning(server: &MockServer, body: Value) {
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(header("Content-Type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

// ---- 1. Happy path -------------------------------------------------------

#[tokio::test]
async fn walk_day_returns_normalised_events_for_happy_path() {
    let server = MockServer::start().await;

    let histories = json!([
        {
            "id": "1",
            "author": {"accountId": SELF_ACCOUNT, "displayName": "Me"},
            "created": "2026-04-20T10:00:00.000+0000",
            "items": [
                {"field": "status", "fromString": "To Do", "toString": "In Progress",
                 "from": "1", "to": "3"}
            ]
        }
    ]);
    let comments = json!([
        {
            "id": "900",
            "author": {"accountId": SELF_ACCOUNT, "displayName": "Me"},
            "created": "2026-04-20T11:30:00.000+0000",
            "body": {
                "type": "doc",
                "content": [{"type": "paragraph",
                             "content": [{"type": "text", "text": "looks good"}]}]
            }
        }
    ]);

    let body = json!({
        "issues": [issue("CAR-5117", "Fix review findings", histories, comments, None, None)],
        "isLast": true
    });
    mount_jql_returning(&server, body).await;

    let outcome = walk_day(
        &http_for_tests(),
        auth_for_tests(),
        &workspace(&server),
        source_id(),
        &self_identity(),
        day(),
        utc(),
        &CancellationToken::new(),
        None,
        None,
    )
    .await
    .expect("happy path should succeed");

    assert_eq!(outcome.fetched_count, 1, "one issue fetched");
    assert_eq!(outcome.events.len(), 2, "transition + comment");
    // Events come back sorted by occurred_at ascending.
    assert_eq!(outcome.events[0].kind, ActivityKind::JiraIssueTransitioned);
    assert_eq!(outcome.events[1].kind, ActivityKind::JiraIssueCommented);
    assert_eq!(
        outcome.events[1].body.as_deref(),
        Some("looks good"),
        "ADF paragraph renders as plain text"
    );
}

// ---- 2. CAR-5117 rapid-transition collapse ------------------------------

#[tokio::test]
async fn walk_day_collapses_six_rapid_transitions_into_one_event() {
    let server = MockServer::start().await;

    // The CAR-5117 cascade: six transitions by the same author in a
    // 10-second window. The rollup should collapse these into exactly
    // one `JiraIssueTransitioned` event.
    let histories = json!([
        {"id": "1", "author": {"accountId": SELF_ACCOUNT, "displayName": "Me"},
         "created": "2026-04-20T08:48:09.000+0000",
         "items": [{"field": "status", "fromString": "Work In Progress", "toString": "Awaiting Review"}]},
        {"id": "2", "author": {"accountId": SELF_ACCOUNT, "displayName": "Me"},
         "created": "2026-04-20T08:48:11.000+0000",
         "items": [{"field": "status", "fromString": "Awaiting Review", "toString": "In Review"}]},
        {"id": "3", "author": {"accountId": SELF_ACCOUNT, "displayName": "Me"},
         "created": "2026-04-20T08:48:13.000+0000",
         "items": [{"field": "status", "fromString": "In Review", "toString": "Release Testing"}]},
        {"id": "4", "author": {"accountId": SELF_ACCOUNT, "displayName": "Me"},
         "created": "2026-04-20T08:48:15.000+0000",
         "items": [{"field": "status", "fromString": "Release Testing", "toString": "Awaiting Deployment"}]},
        {"id": "5", "author": {"accountId": SELF_ACCOUNT, "displayName": "Me"},
         "created": "2026-04-20T08:48:17.000+0000",
         "items": [{"field": "status", "fromString": "Awaiting Deployment", "toString": "Regression Testing"}]},
        {"id": "6", "author": {"accountId": SELF_ACCOUNT, "displayName": "Me"},
         "created": "2026-04-20T08:48:19.000+0000",
         "items": [{"field": "status", "fromString": "Regression Testing", "toString": "Production Verification"}]}
    ]);

    let body = json!({
        "issues": [issue("CAR-5117", "Run services easy mode", histories, json!([]), None, None)],
        "isLast": true
    });
    mount_jql_returning(&server, body).await;

    let outcome = walk_day(
        &http_for_tests(),
        auth_for_tests(),
        &workspace(&server),
        source_id(),
        &self_identity(),
        day(),
        utc(),
        &CancellationToken::new(),
        None,
        None,
    )
    .await
    .expect("rapid cascade should walk cleanly");

    assert_eq!(
        outcome.events.len(),
        1,
        "six rapid transitions collapse into one event, got {} events",
        outcome.events.len()
    );
    let ev = &outcome.events[0];
    assert_eq!(ev.kind, ActivityKind::JiraIssueTransitioned);
    assert_eq!(ev.metadata["transition_count"], json!(6));
    assert_eq!(ev.metadata["from_status"], json!("Work In Progress"));
    assert_eq!(ev.metadata["to_status"], json!("Production Verification"));
    assert!(
        ev.title.contains("rolled up"),
        "rolled-up transition title should hint at the collapse: {}",
        ev.title
    );
}

// ---- 3. KTON-4550 ADF comment --------------------------------------------

#[tokio::test]
async fn walk_day_renders_adf_mention_as_display_name_in_comment_body() {
    let server = MockServer::start().await;

    // KTON-4550 — a comment asking a colleague to update replication
    // steps. Privacy invariant: the ADF mention must render the
    // displayName, never the raw accountId (which is PII-adjacent
    // and, more importantly, never what the author wrote).
    let histories = json!([]);
    let comments = json!([
        {
            "id": "490883",
            "author": {"accountId": SELF_ACCOUNT, "displayName": "Me"},
            "created": "2026-04-20T14:16:00.000+0000",
            "body": {
                "type": "doc",
                "content": [{
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "hey "},
                        {"type": "mention",
                         "attrs": {"id": "colleague-account-id", "text": "@Saravanan"}},
                        {"type": "text", "text": " could you update the replication steps?"}
                    ]
                }]
            }
        }
    ]);

    let body = json!({
        "issues": [issue("KTON-4550", "Replication steps stale", histories, comments, None, None)],
        "isLast": true
    });
    mount_jql_returning(&server, body).await;

    let outcome = walk_day(
        &http_for_tests(),
        auth_for_tests(),
        &workspace(&server),
        source_id(),
        &self_identity(),
        day(),
        utc(),
        &CancellationToken::new(),
        None,
        None,
    )
    .await
    .expect("ADF comment should normalise cleanly");

    assert_eq!(outcome.events.len(), 1);
    let ev = &outcome.events[0];
    assert_eq!(ev.kind, ActivityKind::JiraIssueCommented);
    let body = ev.body.as_deref().expect("comment body rendered");
    assert!(body.contains("@Saravanan"), "mention renders display name");
    assert!(
        !body.contains("colleague-account-id"),
        "mention must never leak the accountId: {body}"
    );
    assert!(body.contains("replication steps"));
}

// ---- 4. Self-filter — other author's comment drops ----------------------

#[tokio::test]
async fn walk_day_drops_comments_authored_by_other_users() {
    let server = MockServer::start().await;

    let comments = json!([
        {
            "id": "900",
            "author": {"accountId": "colleague-account-id", "displayName": "Colleague"},
            "created": "2026-04-20T15:00:00.000+0000",
            "body": {
                "type": "doc",
                "content": [{"type": "paragraph",
                             "content": [{"type": "text", "text": "reply"}]}]
            }
        }
    ]);

    let body = json!({
        "issues": [issue("KTON-4550", "Replication steps stale",
                         json!([]), comments, None, None)],
        "isLast": true
    });
    mount_jql_returning(&server, body).await;

    let outcome = walk_day(
        &http_for_tests(),
        auth_for_tests(),
        &workspace(&server),
        source_id(),
        &self_identity(),
        day(),
        utc(),
        &CancellationToken::new(),
        None,
        None,
    )
    .await
    .expect("should not fail on colleague-authored comment");

    assert!(
        outcome.events.is_empty(),
        "colleague's comment must not surface as a self-event: got {:#?}",
        outcome.events
    );
    assert_eq!(
        outcome.fetched_count, 1,
        "we still observed the issue on the wire"
    );
}

// ---- 5. Pagination --------------------------------------------------------

#[tokio::test]
async fn walk_day_paginates_via_next_page_token() {
    let server = MockServer::start().await;

    // Page 1 — returns one issue and a nextPageToken.
    let page1 = json!({
        "issues": [issue(
            "CAR-1", "one",
            json!([{
                "id": "1",
                "author": {"accountId": SELF_ACCOUNT, "displayName": "Me"},
                "created": "2026-04-20T10:00:00.000+0000",
                "items": [{"field": "status", "fromString": "To Do", "toString": "In Progress"}]
            }]),
            json!([]), None, None
        )],
        "isLast": false,
        "nextPageToken": "page-2"
    });
    // Page 2 — returns the last issue with isLast: true.
    let page2 = json!({
        "issues": [issue(
            "CAR-2", "two",
            json!([{
                "id": "1",
                "author": {"accountId": SELF_ACCOUNT, "displayName": "Me"},
                "created": "2026-04-20T11:00:00.000+0000",
                "items": [{"field": "status", "fromString": "To Do", "toString": "In Progress"}]
            }]),
            json!([]), None, None
        )],
        "isLast": true
    });

    // wiremock matches the first unmatched mock, and mocks match in
    // LIFO insertion order by default. Mount page 2 first so that
    // when the walker sends the second request (with `nextPageToken`
    // in the body), page-2's matcher takes it; the first call falls
    // through to page 1.
    //
    // We use a body-JSON matcher (`BodyContainsToken`) so the server
    // responds with page 2 only when the walker includes the token in
    // the POST body.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(BodyContainsToken("page-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page2))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page1))
        .expect(1)
        .mount(&server)
        .await;

    let outcome = walk_day(
        &http_for_tests(),
        auth_for_tests(),
        &workspace(&server),
        source_id(),
        &self_identity(),
        day(),
        utc(),
        &CancellationToken::new(),
        None,
        None,
    )
    .await
    .expect("pagination should succeed");

    assert_eq!(outcome.fetched_count, 2, "both pages observed");
    assert_eq!(outcome.events.len(), 2, "one transition per issue");
}

/// Matcher that asserts the POST body contains `"nextPageToken": "<tok>"`.
struct BodyContainsToken(&'static str);

impl wiremock::Match for BodyContainsToken {
    fn matches(&self, request: &wiremock::Request) -> bool {
        match serde_json::from_slice::<Value>(&request.body) {
            Ok(v) => v.get("nextPageToken").and_then(Value::as_str) == Some(self.0),
            Err(_) => false,
        }
    }
}

// ---- 6. Rate-limit 429 ---------------------------------------------------

#[tokio::test]
async fn walk_day_maps_429_to_jira_walk_rate_limited() {
    let server = MockServer::start().await;

    // Always-429. With `RetryPolicy::instant()` the SDK retries 5
    // times then surfaces `RateLimited { code: http.retry_budget_exhausted }`;
    // the walker remaps that to `jira.walk.rate_limited`.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
        .mount(&server)
        .await;

    let err = walk_day(
        &http_for_tests(),
        auth_for_tests(),
        &workspace(&server),
        source_id(),
        &self_identity(),
        day(),
        utc(),
        &CancellationToken::new(),
        None,
        None,
    )
    .await
    .expect_err("429 should surface as a rate-limit error");

    assert_eq!(err.code(), error_codes::JIRA_WALK_RATE_LIMITED);
    assert!(
        matches!(err, DayseamError::RateLimited { .. }),
        "expected RateLimited variant, got: {err:?}"
    );
}

// ---- 7. Shape guard — missing `issues` array -----------------------------

#[tokio::test]
async fn walk_day_flags_missing_issues_array_as_upstream_shape_changed() {
    let server = MockServer::start().await;

    // A 200 with a syntactically valid JSON object that's missing
    // the `issues` key. The walker must refuse to paper over this —
    // the DAY-71 invariant is that a silent empty report is the
    // worst outcome.
    let body = json!({"isLast": true});
    mount_jql_returning(&server, body).await;

    let err = walk_day(
        &http_for_tests(),
        auth_for_tests(),
        &workspace(&server),
        source_id(),
        &self_identity(),
        day(),
        utc(),
        &CancellationToken::new(),
        None,
        None,
    )
    .await
    .expect_err("missing issues should error, not succeed silently");

    assert_eq!(err.code(), error_codes::JIRA_WALK_UPSTREAM_SHAPE_CHANGED);
}

// ---- 8. No identity — early bail ----------------------------------------

#[tokio::test]
async fn walk_day_returns_empty_outcome_when_no_atlassian_identity_configured() {
    let server = MockServer::start().await;

    // The walker must not issue a JQL at all when there's no
    // `AtlassianAccountId` identity in scope — every event would be
    // filtered out and the request would burn rate-limit budget for
    // no reason.
    //
    // We mount a mock that would *panic on match* if the walker did
    // issue a request, via `.expect(0)`.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"issues": [], "isLast": true})),
        )
        .expect(0)
        .mount(&server)
        .await;

    // Hand in a non-Atlassian identity only (e.g. a GitLab identity
    // accidentally scoped to this Jira source). The walker must not
    // treat it as a match.
    let wrong_identities = vec![SourceIdentity {
        id: Uuid::new_v4(),
        person_id: Uuid::new_v4(),
        kind: SourceIdentityKind::GitLabUserId,
        external_actor_id: "17".into(),
        source_id: Some(source_id()),
    }];

    let outcome = walk_day(
        &http_for_tests(),
        auth_for_tests(),
        &workspace(&server),
        source_id(),
        &wrong_identities,
        day(),
        utc(),
        &CancellationToken::new(),
        None,
        None,
    )
    .await
    .expect("missing identity should bail with an empty outcome, not error");

    assert!(outcome.events.is_empty());
    assert_eq!(outcome.fetched_count, 0);
}
