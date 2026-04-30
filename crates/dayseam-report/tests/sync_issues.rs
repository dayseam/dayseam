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
