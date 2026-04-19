//! Invariant #3: cancellation.
//!
//! Firing `orchestrator.cancel(run_id)` must propagate to the
//! `ConnCtx` cancellation token the fan-out hands to each connector.
//! The orchestrator then:
//! * transitions the `sync_runs` row `Running → Cancelled { User }`,
//! * emits a terminal `ProgressPhase::Cancelled` on the per-run
//!   stream,
//! * never renders or persists a draft,
//! * clears its in-flight entry,
//! * returns `GenerateOutcome::Cancelled`.

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use common::{build_orchestrator, fixture_date, seed_source, test_person, test_pool};
use connectors_sdk::{ConnCtx, SourceConnector, SyncRequest, SyncResult};
use dayseam_core::{
    DayseamError, ProgressPhase, SourceHealth, SourceKind, SyncRunCancelReason, SyncRunStatus,
};
use dayseam_orchestrator::{orchestrator::GenerateRequest, ConnectorRegistry, SinkRegistry};
use dayseam_report::DEV_EOD_TEMPLATE_ID;

/// Purpose-built test connector that parks until `ctx.cancel` fires.
/// `MockConnector` returns synchronously, which would race with the
/// orchestrator's cancel path — this one guarantees the `generate`
/// task is still executing `sync` when we call `.cancel()`.
#[derive(Debug)]
struct BlockingConnector {
    kind: SourceKind,
}

#[async_trait]
impl SourceConnector for BlockingConnector {
    fn kind(&self) -> SourceKind {
        self.kind
    }

    async fn healthcheck(&self, _ctx: &ConnCtx) -> Result<SourceHealth, DayseamError> {
        Ok(SourceHealth::unchecked())
    }

    async fn sync(&self, ctx: &ConnCtx, _request: SyncRequest) -> Result<SyncResult, DayseamError> {
        // Park the task on the cancel token. If the orchestrator
        // wires `ctx.cancel` correctly, the caller's `cancel()` will
        // unblock us here; otherwise the test times out after
        // several seconds and fails loudly.
        ctx.cancel.cancelled().await;
        ctx.bail_if_cancelled()?;
        unreachable!("bail_if_cancelled must return Err after cancel fires")
    }
}

#[tokio::test]
async fn cancel_propagates_to_connector_and_run_terminates_cancelled() {
    let (pool, _tmp) = test_pool().await;
    let person = test_person();
    let date = fixture_date();

    let (_src, _id, handle) = seed_source(
        &pool,
        &person,
        SourceKind::LocalGit,
        "blocking source",
        "dev@example.com",
    )
    .await;

    let mut connectors = ConnectorRegistry::default();
    connectors.insert(
        SourceKind::LocalGit,
        Arc::new(BlockingConnector {
            kind: SourceKind::LocalGit,
        }),
    );

    let orch = build_orchestrator(pool.clone(), connectors, SinkRegistry::default());

    let request = GenerateRequest {
        person: person.clone(),
        sources: vec![handle],
        date,
        template_id: DEV_EOD_TEMPLATE_ID.to_string(),
        template_version: "0.0.1".to_string(),
        verbose_mode: false,
    };
    let run_handle = orch.generate_report(request).await;
    let run_id = run_handle.run_id;

    // Give the background task a chance to reach `sync` on the
    // connector. Without this, `cancel` might race ahead of the
    // in-flight entry being observable; 50ms is plenty of headroom.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        orch.cancel(run_id).await,
        "cancel should locate a live in-flight run",
    );

    let outcome = run_handle.completion.await.expect("join");
    assert_eq!(outcome.status, SyncRunStatus::Cancelled);
    assert_eq!(outcome.cancel_reason, Some(SyncRunCancelReason::User));
    assert!(
        outcome.draft_id.is_none(),
        "cancelled runs never persist drafts"
    );

    let progress = common::drain_progress(run_handle.progress_rx).await;
    assert!(
        progress
            .iter()
            .any(|p| matches!(p.phase, ProgressPhase::Cancelled { .. })),
        "expected ProgressPhase::Cancelled in transcript: {progress:#?}",
    );
    assert!(
        !progress
            .iter()
            .any(|p| matches!(p.phase, ProgressPhase::Completed { .. })),
        "cancelled runs must not emit Completed",
    );

    let syncrun = dayseam_db::SyncRunRepo::new(pool.clone())
        .get(&run_id)
        .await
        .expect("sync_runs lookup")
        .expect("row present");
    assert_eq!(syncrun.status, SyncRunStatus::Cancelled);
    assert_eq!(syncrun.cancel_reason, Some(SyncRunCancelReason::User));
    assert!(syncrun.finished_at.is_some());

    assert_eq!(orch.in_flight_count().await, 0);
}
