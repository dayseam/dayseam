//! Drives `MockConnector` through a real `ConnCtx` built on a real
//! `RunStreams`, and asserts the SDK invariants the Phase 1 plan
//! specifies: ordered progress phases, identity-filtered output, and
//! accurate stats. This is the "a real implementation can live inside
//! the SDK surface" proof.

mod common;

use chrono::{NaiveDate, TimeZone, Utc};
use common::{build_ctx, self_identity};
use connectors_sdk::{MockConnector, SourceConnector, SyncRequest};
use dayseam_core::{ProgressPhase, SourceKind};
use uuid::Uuid;

#[tokio::test]
async fn mock_connector_emits_starting_progress_completed_in_order() {
    let source_id = Uuid::new_v4();
    let actor_email = "me@example.com";
    let day = NaiveDate::from_ymd_opt(2026, 4, 17).unwrap();

    let fixtures = vec![
        MockConnector::fixture_event(
            source_id,
            "1",
            actor_email,
            Utc.with_ymd_and_hms(2026, 4, 17, 9, 0, 0).unwrap(),
        ),
        MockConnector::fixture_event(
            source_id,
            "2",
            actor_email,
            Utc.with_ymd_and_hms(2026, 4, 17, 12, 0, 0).unwrap(),
        ),
        MockConnector::fixture_event(
            source_id,
            "3",
            actor_email,
            Utc.with_ymd_and_hms(2026, 4, 17, 18, 0, 0).unwrap(),
        ),
    ];
    let connector = MockConnector::new(SourceKind::GitLab, fixtures);

    let mut harness = build_ctx(source_id, vec![self_identity(source_id, actor_email)]);
    let run_id = harness.ctx.run_id;

    let result = connector
        .sync(&harness.ctx, SyncRequest::Day(day))
        .await
        .expect("sync ok");

    assert_eq!(result.events.len(), 3);
    assert_eq!(result.stats.fetched_count, 3);
    assert_eq!(result.stats.filtered_by_identity, 0);
    assert_eq!(result.stats.filtered_by_date, 0);

    // Close sender clones so the receiver can terminate.
    drop(harness.progress_tx);
    drop(harness.log_tx);
    drop(harness.ctx);

    let mut phases = Vec::new();
    while let Some(evt) = harness.progress_rx.recv().await {
        assert_eq!(evt.run_id, run_id);
        phases.push(evt.phase);
    }

    assert!(
        matches!(phases.first(), Some(ProgressPhase::Starting { .. })),
        "first phase must be Starting, got {:?}",
        phases.first()
    );
    assert!(
        matches!(phases.last(), Some(ProgressPhase::Completed { .. })),
        "last phase must be Completed, got {:?}",
        phases.last()
    );
    let in_progress_count = phases
        .iter()
        .filter(|p| matches!(p, ProgressPhase::InProgress { .. }))
        .count();
    assert_eq!(
        in_progress_count, 3,
        "expected one InProgress per fetched event"
    );
}

#[tokio::test]
async fn mock_connector_filters_events_outside_the_identity_set() {
    let source_id = Uuid::new_v4();
    let my_email = "me@example.com";
    let someone_else = "coworker@example.com";
    let day = NaiveDate::from_ymd_opt(2026, 4, 17).unwrap();

    let fixtures = vec![
        MockConnector::fixture_event(
            source_id,
            "mine",
            my_email,
            Utc.with_ymd_and_hms(2026, 4, 17, 9, 0, 0).unwrap(),
        ),
        MockConnector::fixture_event(
            source_id,
            "theirs",
            someone_else,
            Utc.with_ymd_and_hms(2026, 4, 17, 10, 0, 0).unwrap(),
        ),
    ];
    let connector = MockConnector::new(SourceKind::GitLab, fixtures);

    let harness = build_ctx(source_id, vec![self_identity(source_id, my_email)]);
    let result = connector
        .sync(&harness.ctx, SyncRequest::Day(day))
        .await
        .expect("sync ok");

    assert_eq!(result.events.len(), 1);
    assert_eq!(result.events[0].external_id, "mine");
    assert_eq!(result.stats.filtered_by_identity, 1);
}

#[tokio::test]
async fn mock_connector_rejects_since_checkpoint_with_unsupported() {
    let source_id = Uuid::new_v4();
    let connector = MockConnector::new(SourceKind::GitLab, vec![]);
    let harness = build_ctx(source_id, vec![]);

    let err = connector
        .sync(
            &harness.ctx,
            SyncRequest::Since(connectors_sdk::Checkpoint {
                connector: "mock".into(),
                value: serde_json::json!({}),
            }),
        )
        .await
        .expect_err("mock does not support Since");
    assert!(matches!(
        err,
        dayseam_core::DayseamError::Unsupported { .. }
    ));
    assert_eq!(
        err.code(),
        dayseam_core::error_codes::CONNECTOR_UNSUPPORTED_SYNC_REQUEST
    );
}

#[tokio::test]
async fn mock_connector_bails_out_when_cancelled_before_sync() {
    let source_id = Uuid::new_v4();
    let connector = MockConnector::new(SourceKind::GitLab, vec![]);
    let harness = build_ctx(source_id, vec![]);
    harness.cancel.cancel();

    let err = connector
        .sync(
            &harness.ctx,
            SyncRequest::Day(NaiveDate::from_ymd_opt(2026, 4, 17).unwrap()),
        )
        .await
        .expect_err("cancelled before sync");
    assert!(matches!(err, dayseam_core::DayseamError::Cancelled { .. }));
}
