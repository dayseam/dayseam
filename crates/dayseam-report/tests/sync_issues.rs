use dayseam_core::{error_codes, DayseamError, RunStatus, SourceKind, SourceRunState};

mod common;

#[test]
fn render_prepends_sync_issues_section_when_a_source_failed() {
    let mut input = common::fixture_input();

    let src_gitlab = common::source_id(1);
    input.source_kinds.insert(src_gitlab, SourceKind::GitLab);
    input.per_source_state.insert(
        src_gitlab,
        SourceRunState {
            status: RunStatus::Failed,
            started_at: common::generated_at(),
            finished_at: Some(common::generated_at()),
            fetched_count: 0,
            error: Some(DayseamError::Network {
                code: error_codes::GITLAB_URL_DNS.to_string(),
                message: "dns lookup failed".to_string(),
            }),
        },
    );

    // Ensure the day is otherwise empty: groups -> empty_section.
    input.events = Vec::new();
    input.artifacts = Vec::new();
    input.source_identities = Vec::new();

    let draft = dayseam_report::render(input).expect("render must succeed");
    assert!(
        draft.sections.len() >= 2,
        "sync issues section should be prepended ahead of empty commits section"
    );
    assert_eq!(draft.sections[0].id, "sync_issues");
    let sync_bullets = &draft.sections[0].bullets;
    assert!(
        sync_bullets
            .iter()
            .any(|b| b.text.contains("GitLab") && b.text.contains("sync failed")),
        "sync issues bullets should mention the failing forge: {sync_bullets:?}"
    );
    assert!(
        sync_bullets.iter().all(|b| b.source_kind.is_none()),
        "sync diagnostics must not carry source_kind so the day-summary chart ignores them"
    );
}

#[test]
fn render_sync_issues_when_failed_but_error_missing_uses_fallback_message() {
    let mut input = common::fixture_input();
    let src = common::source_id(3);
    input.source_kinds.insert(src, SourceKind::GitHub);
    input.per_source_state.insert(
        src,
        SourceRunState {
            status: RunStatus::Failed,
            started_at: common::generated_at(),
            finished_at: Some(common::generated_at()),
            fetched_count: 0,
            error: None,
        },
    );
    input.events = Vec::new();
    input.artifacts = Vec::new();
    input.source_identities = Vec::new();

    let draft = dayseam_report::render(input).expect("render must succeed");
    let b = draft.sections[0].bullets.first().expect("one bullet");
    assert!(b.text.contains("GitHub"));
    assert!(b.text.contains("without an attached error"));
}

#[test]
fn render_sync_issues_when_kind_unknown_uses_source_id_label() {
    let mut input = common::fixture_input();
    let src = common::source_id(7);
    input.per_source_state.insert(
        src,
        SourceRunState {
            status: RunStatus::Failed,
            started_at: common::generated_at(),
            finished_at: Some(common::generated_at()),
            fetched_count: 0,
            error: Some(DayseamError::Network {
                code: error_codes::GITLAB_URL_DNS.to_string(),
                message: "dns lookup failed".to_string(),
            }),
        },
    );
    input.events = Vec::new();
    input.artifacts = Vec::new();
    input.source_identities = Vec::new();

    let draft = dayseam_report::render(input).expect("render must succeed");
    let b = draft.sections[0].bullets.first().expect("one bullet");
    assert!(b.text.contains("Source `07000000`"));
    assert!(b.text.contains("sync failed"));
}

/// Multiple failed sources must list in [`SourceKind::render_order`]
/// (GitHub before GitLab, etc.), with sources missing from
/// `source_kinds` trailing the list; ties break on [`SourceId`].
#[test]
fn render_sync_issues_sorts_multi_failure_by_kind_order_then_source_id() {
    let mut input = common::fixture_input();

    let id_gitlab = common::source_id(2);
    let id_github = common::source_id(3);
    let id_unknown = common::source_id(9);

    input.source_kinds.insert(id_github, SourceKind::GitHub);
    input.source_kinds.insert(id_gitlab, SourceKind::GitLab);

    let err_network = || DayseamError::Network {
        code: error_codes::GITLAB_URL_DNS.to_string(),
        message: "dns lookup failed".to_string(),
    };

    input.per_source_state.insert(
        id_gitlab,
        SourceRunState {
            status: RunStatus::Failed,
            started_at: common::generated_at(),
            finished_at: Some(common::generated_at()),
            fetched_count: 0,
            error: Some(err_network()),
        },
    );
    input.per_source_state.insert(
        id_github,
        SourceRunState {
            status: RunStatus::Failed,
            started_at: common::generated_at(),
            finished_at: Some(common::generated_at()),
            fetched_count: 0,
            error: Some(err_network()),
        },
    );
    input.per_source_state.insert(
        id_unknown,
        SourceRunState {
            status: RunStatus::Failed,
            started_at: common::generated_at(),
            finished_at: Some(common::generated_at()),
            fetched_count: 0,
            error: Some(err_network()),
        },
    );

    input.events = Vec::new();
    input.artifacts = Vec::new();
    input.source_identities = Vec::new();

    let draft = dayseam_report::render(input).expect("render must succeed");
    let sync = draft
        .sections
        .iter()
        .find(|s| s.id == "sync_issues")
        .expect("sync section");
    assert_eq!(sync.bullets.len(), 3);

    let t0 = sync.bullets[0].text.as_str();
    let t1 = sync.bullets[1].text.as_str();
    let t2 = sync.bullets[2].text.as_str();

    assert!(
        t0.contains("GitHub") && !t0.contains("`"),
        "first row should be labelled GitHub forge, not Source id fallback: {t0}"
    );
    assert!(
        t1.contains("GitLab") && !t1.contains("Source `"),
        "second row GitLab forge: {t1}"
    );
    assert!(
        t2.contains("Source `09000000`"),
        "unknown-kind failure last, short id prefix: {t2}"
    );
}

/// Unknown-kind rows share the trailing sort bucket; distinguish them only
/// by `SourceId` lexical order (`Uuid` ascending).
#[test]
fn render_sync_issues_orders_unknown_kind_bucket_by_source_id() {
    let mut input = common::fixture_input();
    let id_a = common::source_id(0x08);
    let id_b = common::source_id(0x40);

    let err = DayseamError::Network {
        code: error_codes::GITLAB_URL_DNS.to_string(),
        message: "down".to_string(),
    };

    input.per_source_state.insert(
        id_b,
        SourceRunState {
            status: RunStatus::Failed,
            started_at: common::generated_at(),
            finished_at: Some(common::generated_at()),
            fetched_count: 0,
            error: Some(err.clone()),
        },
    );
    input.per_source_state.insert(
        id_a,
        SourceRunState {
            status: RunStatus::Failed,
            started_at: common::generated_at(),
            finished_at: Some(common::generated_at()),
            fetched_count: 0,
            error: Some(err),
        },
    );

    input.events = Vec::new();
    input.artifacts = Vec::new();
    input.source_identities = Vec::new();

    let draft = dayseam_report::render(input).expect("render");
    let bullets = &draft
        .sections
        .iter()
        .find(|s| s.id == "sync_issues")
        .expect("sync section")
        .bullets;

    assert_eq!(bullets.len(), 2);
    assert!(
        bullets[0].text.contains("Source `08000000`"),
        "smaller uuid first: {}",
        bullets[0].text
    );
    assert!(
        bullets[1].text.contains("Source `40000000`"),
        "larger uuid second: {}",
        bullets[1].text
    );
}
