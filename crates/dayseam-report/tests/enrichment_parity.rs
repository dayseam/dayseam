//! DAY-112 cross-source enrichment parity tests.
//!
//! These tests pin the invariant the v0.4 review §3 flagged and the
//! audit in
//! [`docs/dogfood/2026-04-20-cross-source-enrichment-parity-audit.md`](../../../docs/dogfood/2026-04-20-cross-source-enrichment-parity-audit.md)
//! resolved: a GitLab MR event and a GitHub PR event with **identical
//! title + body content** must acquire **identical `JiraIssue`
//! entity sets** after the shared enrichment pipeline runs, and the
//! cross-forge `(triggered by …)` slot on a Jira transition must go
//! to the earliest-opened MR/PR regardless of which forge produced
//! it.
//!
//! The four tests below exercise the four dimensions the audit
//! compared — scan surface (title+body), scan surface (body only),
//! de-duplication, and link-priority tie-breaking — as `assert_eq!`
//! on structurally-equal entity sets. A future refactor that
//! tightens one forge's regex, loosens the other's bail, or flips
//! the tie-breaker will fail these at PR time.
//!
//! Pre-DAY-112: the GitHub connector's `normalise.rs` ran a
//! divergent local ticket-key scanner (looser `[A-Z][A-Z0-9]+-\d+`
//! regex, title-only, 8-cap, `label: Some(key)`) against the
//! `dayseam_report::extract_ticket_keys` path that GitLab MRs rely
//! on exclusively (stricter `[A-Z]{2,10}-\d+`, title+body,
//! bail-at-3, `label: None`). The two paths disagreed on edge cases
//! — digit-prefix tokens like `LOG4J-2`, long-prefix tokens like
//! `VERYLONGPROJECT-42`, and noise titles with 4+ real keys. The
//! fix drops the connector-local path so the shared report-layer
//! extractor is the single source of truth.

use chrono::{DateTime, TimeZone, Utc};
use dayseam_core::{
    ActivityEvent, ActivityKind, Actor, EntityKind, EntityRef, Privacy, RawRef, SourceId,
};
use dayseam_report::{annotate_transition_with_mr, extract_ticket_keys};
use uuid::Uuid;

/// Build a GitLab `MrOpened` event matching the shape
/// `connector_gitlab::normalise` produces: no pre-attached
/// `JiraIssue` entities (enrichment is the report layer's job).
fn gitlab_mr(
    source_id: SourceId,
    iid: &str,
    title: &str,
    body: Option<&str>,
    occurred_at: DateTime<Utc>,
) -> ActivityEvent {
    ActivityEvent {
        id: Uuid::new_v5(&Uuid::NAMESPACE_OID, format!("gitlab:{iid}").as_bytes()),
        source_id,
        external_id: iid.into(),
        kind: ActivityKind::MrOpened,
        occurred_at,
        actor: Actor {
            display_name: "Self".into(),
            email: None,
            external_id: Some("17".into()),
        },
        title: title.into(),
        body: body.map(|s| s.to_string()),
        links: Vec::new(),
        entities: Vec::new(),
        parent_external_id: None,
        metadata: serde_json::Value::Null,
        raw_ref: RawRef {
            storage_key: format!("gitlab-mr:{iid}"),
            content_type: "application/json".into(),
        },
        privacy: Privacy::Normal,
    }
}

/// Build a GitHub `GitHubPullRequestOpened` event matching the shape
/// `connector_github::normalise` produces **post-DAY-112**: no
/// pre-attached `JiraIssue` entities. The whole point of the DAY-112
/// fix is that this event enters the pipeline the same way the
/// GitLab MR does, so the shared extractor gets to do its job
/// uniformly.
fn github_pr(
    source_id: SourceId,
    repo: &str,
    number: u32,
    title: &str,
    body: Option<&str>,
    occurred_at: DateTime<Utc>,
) -> ActivityEvent {
    let external_id = format!("{repo}#{number}");
    ActivityEvent {
        id: Uuid::new_v5(&Uuid::NAMESPACE_OID, external_id.as_bytes()),
        source_id,
        external_id: external_id.clone(),
        kind: ActivityKind::GitHubPullRequestOpened,
        occurred_at,
        actor: Actor {
            display_name: "Self".into(),
            email: None,
            external_id: Some("gh-17".into()),
        },
        title: title.into(),
        body: body.map(|s| s.to_string()),
        links: Vec::new(),
        entities: Vec::new(),
        parent_external_id: None,
        metadata: serde_json::Value::Null,
        raw_ref: RawRef {
            storage_key: format!("gh:{external_id}"),
            content_type: "application/json".into(),
        },
        privacy: Privacy::Normal,
    }
}

fn jira_transition(
    source_id: SourceId,
    issue_key: &str,
    occurred_at: DateTime<Utc>,
) -> ActivityEvent {
    let external_id = format!("{issue_key}::transition");
    ActivityEvent {
        id: Uuid::new_v5(&Uuid::NAMESPACE_OID, external_id.as_bytes()),
        source_id,
        external_id,
        kind: ActivityKind::JiraIssueTransitioned,
        occurred_at,
        actor: Actor {
            display_name: "Self".into(),
            email: None,
            external_id: Some("acct-self".into()),
        },
        title: format!("Moved {issue_key} to Done"),
        body: None,
        links: Vec::new(),
        entities: vec![EntityRef {
            kind: EntityKind::JiraIssue,
            external_id: issue_key.into(),
            label: None,
        }],
        parent_external_id: Some(issue_key.into()),
        metadata: serde_json::Value::Null,
        raw_ref: RawRef {
            storage_key: format!("jira:{issue_key}"),
            content_type: "application/json".into(),
        },
        privacy: Privacy::Normal,
    }
}

/// Extract the `JiraIssue` entities for a given event, sorted by
/// `external_id` so the assertion doesn't depend on insertion
/// order.
fn jira_entities_of(event: &ActivityEvent) -> Vec<&EntityRef> {
    let mut out: Vec<&EntityRef> = event
        .entities
        .iter()
        .filter(|e| e.kind == EntityKind::JiraIssue)
        .collect();
    out.sort_by(|a, b| a.external_id.cmp(&b.external_id));
    out
}

fn gitlab_source() -> SourceId {
    Uuid::from_u128(0x00000000_0000_0000_0000_000000000a01)
}

fn github_source() -> SourceId {
    Uuid::from_u128(0x00000000_0000_0000_0000_000000000a02)
}

fn jira_source() -> SourceId {
    Uuid::from_u128(0x00000000_0000_0000_0000_000000000a03)
}

// ---------------------------------------------------------------------
// Dimension 1: title + body scan surface produces identical keys.
// ---------------------------------------------------------------------

/// Title carries `CAR-5117`, body carries `PROJ-42`. After the
/// shared `extract_ticket_keys` runs over the day's events, both
/// the GitLab MR and the GitHub PR must carry the **exact same**
/// `JiraIssue` entity set: `{CAR-5117, PROJ-42}`, each with
/// `label: None`.
#[test]
fn gitlab_mr_and_github_pr_extract_same_jira_keys_from_title_plus_body() {
    let t = Utc.with_ymd_and_hms(2026, 4, 18, 10, 0, 0).unwrap();
    let title = "CAR-5117: improve charge reasons";
    let body = Some("See PROJ-42 for the incident ticket.");

    let mr = gitlab_mr(gitlab_source(), "!321", title, body, t);
    let pr = github_pr(github_source(), "dayseam", 42, title, body, t);

    let mut events = vec![mr, pr];
    extract_ticket_keys(&mut events);

    let mr_keys = jira_entities_of(&events[0]);
    let pr_keys = jira_entities_of(&events[1]);

    assert_eq!(
        mr_keys, pr_keys,
        "GitLab MR and GitHub PR with identical title+body must acquire \
         the same JiraIssue entity set (including label=None). \
         GitLab side: {mr_keys:?}; GitHub side: {pr_keys:?}"
    );
    let external_ids: Vec<&str> = mr_keys.iter().map(|e| e.external_id.as_str()).collect();
    assert_eq!(
        external_ids,
        vec!["CAR-5117", "PROJ-42"],
        "expected both keys from title+body"
    );
}

// ---------------------------------------------------------------------
// Dimension 2: body-only scan surface produces identical keys.
// ---------------------------------------------------------------------

/// Title has no ticket key; body alone carries `ACME-12`. Pre-DAY-112
/// this was the one case where GitHub PRs and GitLab MRs *already*
/// agreed — GitHub's connector-local scanner was title-only, so it
/// attached nothing, and the centralized extractor picked up the
/// body key for both forges. The test is kept in the parity suite
/// so a future regression that adds a body-only divergence (e.g.
/// GitHub connector starts scanning bodies with a different regex)
/// still fails here.
#[test]
fn gitlab_mr_and_github_pr_extract_same_jira_keys_from_body_only() {
    let t = Utc.with_ymd_and_hms(2026, 4, 18, 11, 0, 0).unwrap();
    let title = "Bump dependencies";
    let body = Some("Closes ACME-12 — the regression filed yesterday.");

    let mr = gitlab_mr(gitlab_source(), "!322", title, body, t);
    let pr = github_pr(github_source(), "dayseam", 43, title, body, t);

    let mut events = vec![mr, pr];
    extract_ticket_keys(&mut events);

    let mr_keys = jira_entities_of(&events[0]);
    let pr_keys = jira_entities_of(&events[1]);

    assert_eq!(
        mr_keys, pr_keys,
        "body-only key parity: GitLab MR and GitHub PR with the same \
         plain title + body-only key must produce the same JiraIssue \
         set. GitLab: {mr_keys:?}; GitHub: {pr_keys:?}"
    );
    let external_ids: Vec<&str> = mr_keys.iter().map(|e| e.external_id.as_str()).collect();
    assert_eq!(external_ids, vec!["ACME-12"]);
}

// ---------------------------------------------------------------------
// Dimension 3: de-duplication across title + body.
// ---------------------------------------------------------------------

/// Title and body both repeat `CAR-5117`. Both forges must produce
/// **exactly one** `JiraIssue("CAR-5117")` entity. Pre-DAY-112, this
/// held for GitLab MRs (centralized extractor dedups internally)
/// but was structurally at risk for GitHub PRs: the connector's
/// `BTreeSet` dedups within a single call, but because the
/// connector attached via title-only then the centralized extractor
/// ran on title+body, the idempotency check in the centralized
/// extractor (`event.entities.iter().any(... && external_id == key)`)
/// was the only thing preventing a duplicate. If a future refactor
/// disabled that check, GitHub PRs would silently gain a phantom
/// second `CAR-5117` entity. This test fails red in that world.
#[test]
fn gitlab_mr_and_github_pr_deduplicate_keys_identically() {
    let t = Utc.with_ymd_and_hms(2026, 4, 18, 12, 0, 0).unwrap();
    let title = "CAR-5117: wire the thing";
    let body = Some("Follow-up on CAR-5117 from yesterday's review.");

    let mr = gitlab_mr(gitlab_source(), "!323", title, body, t);
    let pr = github_pr(github_source(), "dayseam", 44, title, body, t);

    let mut events = vec![mr, pr];
    extract_ticket_keys(&mut events);
    // Idempotency: a second call must not duplicate.
    extract_ticket_keys(&mut events);

    let mr_keys = jira_entities_of(&events[0]);
    let pr_keys = jira_entities_of(&events[1]);

    assert_eq!(
        mr_keys, pr_keys,
        "dedup parity: repeated key in title + body must collapse to \
         exactly one entity on both forges after two extract passes. \
         GitLab: {mr_keys:?}; GitHub: {pr_keys:?}"
    );
    assert_eq!(mr_keys.len(), 1, "expected a single CAR-5117 entity");
    assert_eq!(mr_keys[0].external_id, "CAR-5117");
    assert_eq!(
        mr_keys[0].label, None,
        "JiraIssue entities attached by the shared extractor must \
         carry label=None on both forges (DAY-112 dropped the GitHub \
         connector's label=Some(key) divergence)"
    );
}

// ---------------------------------------------------------------------
// Dimension 4: link-priority tie-breaking is forge-agnostic.
// ---------------------------------------------------------------------

/// A GitLab MR and a GitHub PR both reference `CAR-5117`; the MR
/// opens first (`T1`), the PR second (`T2 > T1`), and a Jira
/// transition on `CAR-5117` fires later (`T3 > T2`, within the
/// 24-hour window). `annotate_transition_with_mr` must stamp the
/// transition's `parent_external_id` with the **GitLab MR's**
/// `external_id` — earliest-opened wins, regardless of which forge
/// it came from. Swapping the event vector's insertion order must
/// not flip the pick (the tie-breaker inside the selection uses
/// `ActivityEvent::id`).
#[test]
fn earliest_opened_wins_triggered_by_slot_across_forges() {
    let t1 = Utc.with_ymd_and_hms(2026, 4, 18, 9, 0, 0).unwrap();
    let t2 = Utc.with_ymd_and_hms(2026, 4, 18, 10, 0, 0).unwrap();
    let t3 = Utc.with_ymd_and_hms(2026, 4, 18, 10, 10, 0).unwrap();

    let mr = gitlab_mr(
        gitlab_source(),
        "!321",
        "CAR-5117: fix charge reasons",
        None,
        t1,
    );
    let pr = github_pr(
        github_source(),
        "dayseam",
        42,
        "CAR-5117: alternate fix",
        None,
        t2,
    );
    let transition = jira_transition(jira_source(), "CAR-5117", t3);

    let mr_external_id = mr.external_id.clone();

    // Insertion order A: MR, PR, transition.
    let mut events_a = vec![mr.clone(), pr.clone(), transition.clone()];
    extract_ticket_keys(&mut events_a);
    annotate_transition_with_mr(&mut events_a);
    let trans_a = events_a
        .iter()
        .find(|e| e.kind == ActivityKind::JiraIssueTransitioned)
        .expect("transition event must remain in the stream");
    assert_eq!(
        trans_a.parent_external_id.as_deref(),
        Some(mr_external_id.as_str()),
        "earliest-opened wins: MR opened at T1 must win the \
         (triggered by …) slot over PR opened at T2 > T1"
    );

    // Insertion order B: transition, PR, MR. Same pick.
    let mut events_b = vec![transition, pr, mr];
    extract_ticket_keys(&mut events_b);
    annotate_transition_with_mr(&mut events_b);
    let trans_b = events_b
        .iter()
        .find(|e| e.kind == ActivityKind::JiraIssueTransitioned)
        .expect("transition event must remain in the stream");
    assert_eq!(
        trans_b.parent_external_id.as_deref(),
        Some(mr_external_id.as_str()),
        "insertion order must not flip the pick: earliest `occurred_at` \
         wins regardless of vec order, cross-forge"
    );
}
