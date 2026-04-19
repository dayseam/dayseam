//! Invariant #7: `save_report` is atomic from the orchestrator's
//! point of view.
//!
//! The two paths we exercise here:
//!
//! * **Happy path** — a persisted draft + a `MockSink` round-trips a
//!   single [`dayseam_core::WriteReceipt`], the draft row is
//!   byte-identical to its pre-save state, and the mock records one
//!   write against its config.
//! * **Failure path** — a sink that returns an error does *not*
//!   mutate `report_drafts.sections_json` or any other draft column.
//!   The error propagates unchanged.
//!
//! Two guard-rail paths on the error taxonomy are also covered: an
//! unknown draft id and an unknown sink kind both produce the Task 6
//! "surface this inline, not as a toast" shape
//! ([`DayseamError::InvalidConfig`]) with the right error code.

#[path = "common/mod.rs"]
mod common;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use common::{build_orchestrator, fixture_date, test_pool};
use dayseam_core::{error_codes, DayseamError, ReportDraft, Sink, SinkConfig, SinkKind};
use dayseam_orchestrator::{ConnectorRegistry, SinkRegistry};
use sinks_sdk::MockSink;
use uuid::Uuid;

/// Build a minimal persisted draft in `pool` and return its
/// `(draft_id, untouched snapshot)`. The snapshot is what we diff the
/// post-save row against to prove the orchestrator did not touch the
/// draft on either happy or failure path.
async fn seed_draft(pool: &sqlx::SqlitePool) -> (Uuid, ReportDraft) {
    let draft = ReportDraft {
        id: Uuid::new_v4(),
        date: fixture_date(),
        template_id: "dev-eod".to_string(),
        template_version: "0.0.1".to_string(),
        sections: Vec::new(),
        evidence: Vec::new(),
        per_source_state: HashMap::new(),
        verbose_mode: false,
        generated_at: Utc::now(),
    };
    dayseam_db::DraftRepo::new(pool.clone())
        .insert(&draft)
        .await
        .expect("seed draft");
    (draft.id, draft)
}

fn markdown_sink_pointing_at(dir: &std::path::Path) -> Sink {
    Sink {
        id: Uuid::new_v4(),
        kind: SinkKind::MarkdownFile,
        label: "test sink".into(),
        config: SinkConfig::MarkdownFile {
            config_version: 1,
            dest_dirs: vec![PathBuf::from(dir)],
            frontmatter: false,
        },
        created_at: Utc::now(),
        last_write_at: None,
    }
}

#[tokio::test]
async fn save_happy_path_returns_one_receipt_and_leaves_draft_untouched() {
    let (pool, tmp) = test_pool().await;
    let (draft_id, snapshot) = seed_draft(&pool).await;

    let mock_sink = Arc::new(MockSink::new());
    let mut sinks = SinkRegistry::default();
    sinks.insert(SinkKind::MarkdownFile, mock_sink.clone());

    let orch = build_orchestrator(pool.clone(), ConnectorRegistry::default(), sinks);
    let sink = markdown_sink_pointing_at(tmp.path());

    let receipts = orch
        .save_report(draft_id, &sink)
        .await
        .expect("save succeeds");

    assert_eq!(receipts.len(), 1, "expect exactly one receipt in v0.1");
    let receipt = &receipts[0];
    assert_eq!(receipt.sink_kind, SinkKind::MarkdownFile);
    assert_eq!(
        receipt.destinations_written,
        vec![PathBuf::from(tmp.path())]
    );
    assert!(
        receipt.run_id.is_none(),
        "save is ad-hoc — receipt.run_id must be None",
    );

    // The mock recorded exactly one write with our sink config.
    let writes = mock_sink.writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].draft_id, draft_id);

    // Draft row is byte-identical to what we seeded.
    let reread = dayseam_db::DraftRepo::new(pool.clone())
        .get(&draft_id)
        .await
        .expect("read draft")
        .expect("row present");
    assert_eq!(reread, snapshot, "save must not mutate report_drafts");
}

#[tokio::test]
async fn save_on_failing_sink_leaves_draft_untouched() {
    let (pool, tmp) = test_pool().await;
    let (draft_id, snapshot) = seed_draft(&pool).await;

    let mock_sink = Arc::new(MockSink::new());
    mock_sink.fail_next_with(DayseamError::Io {
        code: error_codes::SINK_FS_NOT_WRITABLE.into(),
        path: Some(tmp.path().to_path_buf()),
        message: "simulated write failure".into(),
    });
    let mut sinks = SinkRegistry::default();
    sinks.insert(SinkKind::MarkdownFile, mock_sink.clone());

    let orch = build_orchestrator(pool.clone(), ConnectorRegistry::default(), sinks);
    let sink = markdown_sink_pointing_at(tmp.path());

    let err = orch
        .save_report(draft_id, &sink)
        .await
        .expect_err("arm_fail_next should propagate");
    match err {
        DayseamError::Io { code, .. } => {
            assert_eq!(code, error_codes::SINK_FS_NOT_WRITABLE);
        }
        other => panic!("expected Io error from sink, got {other:?}"),
    }

    // Invariant #7 proper: the draft row is byte-identical.
    let reread = dayseam_db::DraftRepo::new(pool.clone())
        .get(&draft_id)
        .await
        .expect("read draft")
        .expect("row present");
    assert_eq!(
        reread, snapshot,
        "failed save must not mutate report_drafts.sections_json",
    );

    // No write was recorded — the mock short-circuited before
    // reaching its push.
    assert!(mock_sink.writes().is_empty());
}

#[tokio::test]
async fn save_unknown_draft_returns_invalid_config_with_stable_code() {
    let (pool, tmp) = test_pool().await;

    let mut sinks = SinkRegistry::default();
    sinks.insert(SinkKind::MarkdownFile, Arc::new(MockSink::new()));
    let orch = build_orchestrator(pool, ConnectorRegistry::default(), sinks);
    let sink = markdown_sink_pointing_at(tmp.path());

    let err = orch
        .save_report(Uuid::new_v4(), &sink)
        .await
        .expect_err("missing draft must fail");
    match err {
        DayseamError::InvalidConfig { code, .. } => {
            assert_eq!(code, error_codes::ORCHESTRATOR_SAVE_DRAFT_NOT_FOUND);
        }
        other => panic!("expected InvalidConfig, got {other:?}"),
    }
}

#[tokio::test]
async fn save_with_unregistered_sink_kind_returns_invalid_config() {
    let (pool, tmp) = test_pool().await;
    let (draft_id, _) = seed_draft(&pool).await;

    // Deliberately no sink registered at all.
    let orch = build_orchestrator(pool, ConnectorRegistry::default(), SinkRegistry::default());
    let sink = markdown_sink_pointing_at(tmp.path());

    let err = orch
        .save_report(draft_id, &sink)
        .await
        .expect_err("unregistered sink kind must fail");
    match err {
        DayseamError::InvalidConfig { code, .. } => {
            assert_eq!(code, error_codes::ORCHESTRATOR_SINK_NOT_REGISTERED);
        }
        other => panic!("expected InvalidConfig, got {other:?}"),
    }
}
