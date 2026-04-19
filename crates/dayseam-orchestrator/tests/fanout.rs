//! Invariant #1 (full) + Invariant #4: fan-out.
//!
//! Two `MockConnector`s (registered under different `SourceKind`s
//! because the registry is keyed by kind) run concurrently for a
//! single run. The orchestrator must:
//! * share a single `run_id` across both per-source calls,
//! * cap parallelism at `min(n, 4)`,
//! * capture per-source state for both sources,
//! * render the draft from whatever succeeded even when a source
//!   fails (Invariant #4).

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;

use common::{
    build_orchestrator, fixture_date, fixture_event, seed_source, test_person, test_pool,
};
use connectors_sdk::MockConnector;
use dayseam_core::{error_codes, DayseamError, RunStatus, SourceKind, SyncRunStatus};
use dayseam_orchestrator::{orchestrator::GenerateRequest, ConnectorRegistry, SinkRegistry};
use dayseam_report::DEV_EOD_TEMPLATE_ID;

#[tokio::test]
async fn two_sources_complete_with_shared_run_id_and_per_source_state() {
    let (pool, _tmp) = test_pool().await;
    let person = test_person();
    let date = fixture_date();

    // Seed two sources, each with its own identity. Use different
    // `SourceKind`s because the registry holds one connector per
    // kind — this matches how the default registry is populated in
    // production (`LocalGit` + `GitLab`).
    let (src_a, _id_a, handle_a) = seed_source(
        &pool,
        &person,
        SourceKind::LocalGit,
        "git fixture",
        "dev@example.com",
    )
    .await;
    let (src_b, _id_b, handle_b) = seed_source(
        &pool,
        &person,
        SourceKind::GitLab,
        "gitlab fixture",
        "dev@example.com",
    )
    .await;

    let event_a = fixture_event(src_a.id, "a-1", "dev@example.com", date);
    let event_b = fixture_event(src_b.id, "b-1", "dev@example.com", date);
    let conn_a = Arc::new(MockConnector::new(SourceKind::LocalGit, vec![event_a]));
    let conn_b = Arc::new(MockConnector::new(SourceKind::GitLab, vec![event_b]));

    let mut connectors = ConnectorRegistry::default();
    connectors.insert(SourceKind::LocalGit, conn_a);
    connectors.insert(SourceKind::GitLab, conn_b);

    let orch = build_orchestrator(pool.clone(), connectors, SinkRegistry::default());

    let request = GenerateRequest {
        person: person.clone(),
        sources: vec![handle_a, handle_b],
        date,
        template_id: DEV_EOD_TEMPLATE_ID.to_string(),
        template_version: "0.0.1".to_string(),
        verbose_mode: false,
    };
    let handle = orch.generate_report(request).await;
    let run_id = handle.run_id;
    let outcome = handle.completion.await.expect("join");

    assert_eq!(outcome.status, SyncRunStatus::Completed);
    let draft_id = outcome.draft_id.expect("draft id");

    // Per-source state: one entry per source, both Succeeded.
    let syncrun = dayseam_db::SyncRunRepo::new(pool.clone())
        .get(&run_id)
        .await
        .expect("sync_runs lookup")
        .expect("row present");
    assert_eq!(syncrun.per_source_state.len(), 2);
    for ps in &syncrun.per_source_state {
        assert_eq!(ps.status, RunStatus::Succeeded, "per-source: {ps:?}");
        assert_eq!(ps.fetched_count, 1, "per-source fetch count: {ps:?}");
        assert!(ps.error.is_none());
    }

    // Draft carries the same per-source entries.
    let draft = dayseam_db::DraftRepo::new(pool.clone())
        .get(&draft_id)
        .await
        .expect("drafts lookup")
        .expect("draft persisted");
    assert_eq!(draft.per_source_state.len(), 2);
    let succeeded_sources: Vec<_> = draft
        .per_source_state
        .values()
        .filter(|s| s.status == RunStatus::Succeeded)
        .collect();
    assert_eq!(succeeded_sources.len(), 2);
}

#[tokio::test]
async fn partial_failure_renders_draft_and_records_error_in_per_source_state() {
    let (pool, _tmp) = test_pool().await;
    let person = test_person();
    let date = fixture_date();

    let (src_good, _id_good, handle_good) = seed_source(
        &pool,
        &person,
        SourceKind::LocalGit,
        "healthy git",
        "dev@example.com",
    )
    .await;
    let (_src_bad, _id_bad, handle_bad) = seed_source(
        &pool,
        &person,
        SourceKind::GitLab,
        "sick gitlab",
        "dev@example.com",
    )
    .await;

    let good_event = fixture_event(src_good.id, "good-1", "dev@example.com", date);
    let conn_good = Arc::new(MockConnector::new(SourceKind::LocalGit, vec![good_event]));

    // The bad connector is `MockConnector` with a forced error so it
    // short-circuits before producing any events. Mirrors a real
    // GitLab outage as seen from the orchestrator: `sync` returns
    // `Err`, no `Completed` progress from that connector.
    let bad_err = DayseamError::Auth {
        code: error_codes::GITLAB_AUTH_INVALID_TOKEN.to_string(),
        message: "fixture outage".into(),
        retryable: false,
        action_hint: None,
    };
    let conn_bad =
        Arc::new(MockConnector::new(SourceKind::GitLab, vec![]).with_always_err(bad_err));

    let mut connectors = ConnectorRegistry::default();
    connectors.insert(SourceKind::LocalGit, conn_good);
    connectors.insert(SourceKind::GitLab, conn_bad);

    let orch = build_orchestrator(pool.clone(), connectors, SinkRegistry::default());

    let request = GenerateRequest {
        person: person.clone(),
        sources: vec![handle_good, handle_bad],
        date,
        template_id: DEV_EOD_TEMPLATE_ID.to_string(),
        template_version: "0.0.1".to_string(),
        verbose_mode: false,
    };
    let handle = orch.generate_report(request).await;
    let run_id = handle.run_id;
    let outcome = handle.completion.await.expect("join");

    // Even with a failed source, the run is Completed: a partial
    // draft is more useful than no draft.
    assert_eq!(
        outcome.status,
        SyncRunStatus::Completed,
        "partial failure must still render a draft",
    );
    let draft_id = outcome.draft_id.expect("draft id");

    let syncrun = dayseam_db::SyncRunRepo::new(pool.clone())
        .get(&run_id)
        .await
        .expect("sync_runs lookup")
        .expect("row present");
    assert_eq!(syncrun.per_source_state.len(), 2);
    let statuses: Vec<RunStatus> = syncrun.per_source_state.iter().map(|p| p.status).collect();
    assert!(statuses.contains(&RunStatus::Succeeded), "{statuses:?}");
    assert!(statuses.contains(&RunStatus::Failed), "{statuses:?}");

    let failed = syncrun
        .per_source_state
        .iter()
        .find(|p| p.status == RunStatus::Failed)
        .expect("one failed entry");
    let err = failed.error.as_ref().expect("error captured");
    assert_eq!(err.code(), error_codes::GITLAB_AUTH_INVALID_TOKEN);

    // Draft renders from the healthy source only: one event, one
    // per-source state row with Succeeded, one with Failed.
    let draft = dayseam_db::DraftRepo::new(pool.clone())
        .get(&draft_id)
        .await
        .expect("drafts lookup")
        .expect("draft persisted");
    assert_eq!(draft.per_source_state.len(), 2);
    let draft_statuses: Vec<RunStatus> =
        draft.per_source_state.values().map(|s| s.status).collect();
    assert!(draft_statuses.contains(&RunStatus::Succeeded));
    assert!(draft_statuses.contains(&RunStatus::Failed));
}
