//! Invariant #2: supersede-on-retry.
//!
//! Starting a second `generate_report` for the same
//! `(person_id, date, template_id)` tuple while the first is still
//! in-flight must:
//! * fire the first run's cancel token,
//! * transition the first run's `sync_runs` row to
//!   `Cancelled { SupersededBy(new_run_id) }`,
//! * let the second run run to completion on its own terms.
//!
//! The persist-time guard ensures a late writer (a slow first run
//! that finishes after we already wrote its terminal row) doesn't
//! re-transition or silently persist a stale draft.

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use common::{build_orchestrator, fixture_date, seed_source, test_person, test_pool};
use connectors_sdk::{ConnCtx, MockConnector, SourceConnector, SyncRequest, SyncResult};
use dayseam_core::{DayseamError, SourceHealth, SourceKind, SyncRunCancelReason, SyncRunStatus};
use dayseam_orchestrator::{orchestrator::GenerateRequest, ConnectorRegistry, SinkRegistry};
use dayseam_report::DEV_EOD_TEMPLATE_ID;

/// Test connector that parks on `ctx.cancel` and *then* returns an
/// empty `SyncResult`. This simulates a slow first run that is in
/// the middle of fetching when the supersede call arrives.
#[derive(Debug)]
struct SlowConnector;

#[async_trait]
impl SourceConnector for SlowConnector {
    fn kind(&self) -> SourceKind {
        SourceKind::LocalGit
    }

    async fn healthcheck(&self, _ctx: &ConnCtx) -> Result<SourceHealth, DayseamError> {
        Ok(SourceHealth::unchecked())
    }

    async fn sync(&self, ctx: &ConnCtx, _request: SyncRequest) -> Result<SyncResult, DayseamError> {
        ctx.cancel.cancelled().await;
        // Return an empty result on cancel rather than `Err` so
        // the supersede path exercises the "slow run finishes after
        // supersede, in-flight entry already replaced" branch of
        // the persist-time guard.
        Ok(SyncResult::default())
    }
}

#[tokio::test]
async fn second_generate_supersedes_first_for_same_tuple() {
    let (pool, _tmp) = test_pool().await;
    let person = test_person();
    let date = fixture_date();
    let template_id = DEV_EOD_TEMPLATE_ID.to_string();

    let (src, _id, handle) = seed_source(
        &pool,
        &person,
        SourceKind::LocalGit,
        "local-git fixture",
        "dev@example.com",
    )
    .await;

    // Two connector registries: the first uses a slow connector that
    // parks the run in `sync`; the second uses a fast MockConnector
    // that returns immediately. We build two orchestrators sharing
    // the same pool / in-flight map by handing both runs to the
    // *same* orchestrator — the registries are per-orchestrator, so
    // we register both and dispatch by source kind.
    //
    // Trick: we register `SlowConnector` under `LocalGit`, kick off
    // the first run, then swap the registry? — no, registries aren't
    // hot-swappable. Instead: the slow connector does the blocking
    // work, and the second `generate_report` call comes in while the
    // first is parked. The second call also routes to the same slow
    // connector — but its cancel token fires for its own reasons
    // (there are none yet), so it would also park. We avoid that by
    // using `MockConnector::with_always_err` on the second? No —
    // we want both to use the same connector and both to unblock.
    //
    // Cleaner path: use a single orchestrator where `SlowConnector`
    // parks on `ctx.cancel`. The supersede path fires the first
    // run's cancel token; the second run's `ctx.cancel` is a fresh
    // token so it also needs to be fired to unblock. We fire that
    // via the handle.cancel token *after* observing the supersede
    // has happened — but that'd make the second run Cancelled too.
    //
    // Practical compromise: swap to a fast connector for the second
    // run by using a new `SourceKind::GitLab` for the second source
    // (different registry entry) and register a `MockConnector`
    // there. That keeps the test honest about the scenario and
    // avoids the race.
    let (src_fast, _id_fast, handle_fast) = seed_source(
        &pool,
        &person,
        SourceKind::GitLab,
        "gitlab fixture",
        "dev@example.com",
    )
    .await;

    let slow = Arc::new(SlowConnector);
    let fast_event = common::fixture_event(src_fast.id, "fast-1", "dev@example.com", date);
    let fast = Arc::new(MockConnector::new(SourceKind::GitLab, vec![fast_event]));

    let mut connectors = ConnectorRegistry::default();
    connectors.insert(SourceKind::LocalGit, slow);
    connectors.insert(SourceKind::GitLab, fast);

    let orch = build_orchestrator(pool.clone(), connectors, SinkRegistry::default());

    // First run: `LocalGit` source, parks forever on cancel.
    let first_req = GenerateRequest {
        person: person.clone(),
        sources: vec![handle.clone()],
        date,
        template_id: template_id.clone(),
        template_version: "0.0.1".to_string(),
        verbose_mode: false,
    };
    let first = orch.generate_report(first_req).await;
    let first_run_id = first.run_id;

    // Give the first run time to reach `sync`.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(orch.in_flight_count().await, 1);

    // Second run for the same tuple, but with the fast source. The
    // supersede logic keys on `(person, date, template_id)`, not on
    // the source set, so the fast run replaces the slow one in the
    // in-flight map and cancels the slow run's token.
    let second_req = GenerateRequest {
        person: person.clone(),
        sources: vec![handle_fast],
        date,
        template_id: template_id.clone(),
        template_version: "0.0.1".to_string(),
        verbose_mode: false,
    };
    let second = orch.generate_report(second_req).await;
    let second_run_id = second.run_id;
    assert_ne!(first_run_id, second_run_id);

    // Both runs resolve: the first because supersede fired its
    // cancel token, the second because MockConnector returns
    // immediately.
    let first_outcome = first.completion.await.expect("first join");
    let second_outcome = second.completion.await.expect("second join");

    // The second run wins; the first is marked superseded.
    assert_eq!(second_outcome.status, SyncRunStatus::Completed);
    assert!(second_outcome.draft_id.is_some());

    assert_eq!(first_outcome.status, SyncRunStatus::Cancelled);
    assert_eq!(
        first_outcome.cancel_reason,
        Some(SyncRunCancelReason::SupersededBy {
            run_id: second_run_id
        }),
        "first run should be cancelled with SupersededBy, got {:?}",
        first_outcome.cancel_reason,
    );

    // The row the repo holds for the first run should agree with the
    // outcome: terminal, `Cancelled`, `superseded_by = second`. The
    // persist-time guard must have suppressed the slow task's own
    // terminal write.
    let first_row = dayseam_db::SyncRunRepo::new(pool.clone())
        .get(&first_run_id)
        .await
        .expect("lookup")
        .expect("row present");
    assert_eq!(first_row.status, SyncRunStatus::Cancelled);
    assert_eq!(first_row.superseded_by, Some(second_run_id));
    assert_eq!(
        first_row.cancel_reason,
        Some(SyncRunCancelReason::SupersededBy {
            run_id: second_run_id
        }),
    );

    let second_row = dayseam_db::SyncRunRepo::new(pool.clone())
        .get(&second_run_id)
        .await
        .expect("lookup")
        .expect("row present");
    assert_eq!(second_row.status, SyncRunStatus::Completed);

    // In-flight map is clean after both runs terminate.
    assert_eq!(orch.in_flight_count().await, 0);

    // `src` is used above only for the handle; this reference is to
    // anchor the variable (rustc dead-code check would otherwise
    // ignore the `.id` path).
    let _ = src;
}
