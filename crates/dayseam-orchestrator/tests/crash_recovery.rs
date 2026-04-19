//! Crash recovery + startup orchestration (Task 5 invariants #5, #6).
//!
//! `Orchestrator::startup` bundles three jobs:
//!
//! 1. Bootstrap the `retention.days` setting with
//!    [`retention::DEFAULT_RETENTION_DAYS`] on first boot.
//! 2. Rewrite every `Running` `sync_runs` row with
//!    `finished_at IS NULL` to `Failed` with the
//!    `INTERNAL_PROCESS_RESTARTED` code. This is the only public
//!    entry point for that transition — the orchestrator never
//!    produces this code during a live run.
//! 3. Run one retention sweep with the resolved cutoff.
//!
//! The invariants under test:
//!
//! * A crashed run (status=`Running`, `finished_at IS NULL`) is
//!   rewritten to `Failed`; a `Pending`/`Running` per-source entry is
//!   remapped to `Failed` with the recovery error code; terminal
//!   entries (`Succeeded`/`Failed`/`Skipped`) are left verbatim.
//! * Rows already terminal before startup are untouched.
//! * Startup is idempotent: a second call right after the first
//!   recovers zero runs and installs zero defaults.
//! * On a clean first boot, `retention.days` is installed with
//!   [`retention::DEFAULT_RETENTION_DAYS`] and
//!   `retention_default_installed` is `true` exactly once.

#[path = "common/mod.rs"]
mod common;

use std::collections::HashMap;

use chrono::{Duration, Utc};
use common::{build_orchestrator, seed_source, test_person, test_pool};
use dayseam_core::{
    error_codes, DayseamError, PerSourceState, RunId, RunStatus, SourceKind, SyncRun,
    SyncRunStatus, SyncRunTrigger,
};
use dayseam_db::{SettingsRepo, SyncRunRepo};
use dayseam_orchestrator::{
    retention::{DEFAULT_RETENTION_DAYS, RETENTION_DAYS_SETTING_KEY},
    ConnectorRegistry, SinkRegistry,
};

fn fixture_sync_run(per_source: Vec<PerSourceState>) -> SyncRun {
    SyncRun {
        id: RunId::new(),
        started_at: Utc::now() - Duration::minutes(5),
        finished_at: None,
        trigger: SyncRunTrigger::User,
        status: SyncRunStatus::Running,
        cancel_reason: None,
        superseded_by: None,
        per_source_state: per_source,
    }
}

/// The canonical crashed-run shape: one `Running` sync_runs row with
/// a mix of per-source states. The sweep must flip the run to
/// `Failed`, rewrite `Pending`/`Running` entries to `Failed` with the
/// recovery code, and leave already-terminal entries byte-for-byte.
#[tokio::test]
async fn startup_recovers_crashed_run_and_remaps_per_source_states() {
    let (pool, _tmp) = test_pool().await;
    let (source, _id, _handle) = seed_source(
        &pool,
        &test_person(),
        SourceKind::LocalGit,
        "fixture",
        "dev@example.com",
    )
    .await;

    // One `Running` + `Pending` per-source entry plus one already-
    // terminal `Succeeded` entry so we can assert the sweep doesn't
    // clobber the latter.
    let now = Utc::now();
    let running_entry = PerSourceState {
        source_id: source.id,
        status: RunStatus::Running,
        started_at: now - Duration::minutes(5),
        finished_at: None,
        fetched_count: 0,
        error: None,
    };
    let terminal_entry = PerSourceState {
        source_id: source.id,
        status: RunStatus::Succeeded,
        started_at: now - Duration::minutes(10),
        finished_at: Some(now - Duration::minutes(9)),
        fetched_count: 3,
        error: None,
    };
    let run = fixture_sync_run(vec![running_entry.clone(), terminal_entry.clone()]);
    let repo = SyncRunRepo::new(pool.clone());
    repo.insert(&run).await.expect("insert crashed run");

    let orch = build_orchestrator(
        pool.clone(),
        ConnectorRegistry::default(),
        SinkRegistry::default(),
    );
    let report = orch.startup().await.expect("startup");

    assert_eq!(report.crashed_runs_recovered, 1);
    assert!(
        report.retention_default_installed,
        "first boot must install the default retention window",
    );

    // The run is now `Failed`, carries a `finished_at`, and the
    // per-source states are remapped correctly.
    let reread = repo.get(&run.id).await.expect("get").expect("row present");
    assert_eq!(reread.status, SyncRunStatus::Failed);
    assert!(reread.finished_at.is_some());
    assert_eq!(reread.per_source_state.len(), 2);

    let by_status: HashMap<_, _> = reread
        .per_source_state
        .iter()
        .map(|s| (s.status, s))
        .collect();
    let recovered = by_status
        .get(&RunStatus::Failed)
        .expect("Running → Failed after recovery");
    assert!(recovered.finished_at.is_some());
    match &recovered.error {
        Some(DayseamError::Internal { code, .. }) => {
            assert_eq!(code, error_codes::INTERNAL_PROCESS_RESTARTED);
        }
        other => panic!("expected Internal with recovery code, got {other:?}"),
    }
    let preserved = by_status
        .get(&RunStatus::Succeeded)
        .expect("terminal entry must be preserved");
    assert_eq!(preserved.fetched_count, terminal_entry.fetched_count);
    assert_eq!(preserved.finished_at, terminal_entry.finished_at);
}

/// A row already in `Completed` before startup must not be touched —
/// `list_running` only looks at `Running + finished_at IS NULL`, and
/// the sweep must respect that filter.
#[tokio::test]
async fn startup_leaves_already_terminal_runs_untouched() {
    let (pool, _tmp) = test_pool().await;
    let (source, _id, _handle) = seed_source(
        &pool,
        &test_person(),
        SourceKind::LocalGit,
        "fixture",
        "dev@example.com",
    )
    .await;

    let now = Utc::now();
    let terminal_entry = PerSourceState {
        source_id: source.id,
        status: RunStatus::Succeeded,
        started_at: now - Duration::minutes(10),
        finished_at: Some(now - Duration::minutes(9)),
        fetched_count: 5,
        error: None,
    };
    let run = fixture_sync_run(vec![terminal_entry]);
    let run_id = run.id;
    let repo = SyncRunRepo::new(pool.clone());
    repo.insert(&run).await.expect("insert");
    repo.mark_finished(&run_id, now - Duration::minutes(8), &run.per_source_state)
        .await
        .expect("mark completed");

    let orch = build_orchestrator(
        pool.clone(),
        ConnectorRegistry::default(),
        SinkRegistry::default(),
    );
    let report = orch.startup().await.expect("startup");
    assert_eq!(
        report.crashed_runs_recovered, 0,
        "terminal runs must not be recovered",
    );

    let reread = repo.get(&run_id).await.expect("get").expect("row present");
    assert_eq!(reread.status, SyncRunStatus::Completed);
}

/// Second startup is a no-op: zero runs to recover, the retention
/// default already present, and the retention sweep has nothing to
/// prune on the empty DB.
#[tokio::test]
async fn startup_is_idempotent_on_a_clean_db() {
    let (pool, _tmp) = test_pool().await;
    let orch = build_orchestrator(
        pool.clone(),
        ConnectorRegistry::default(),
        SinkRegistry::default(),
    );

    let first = orch.startup().await.expect("first startup");
    assert!(first.retention_default_installed);
    assert_eq!(first.crashed_runs_recovered, 0);
    assert_eq!(first.retention.total_deleted(), 0);

    let second = orch.startup().await.expect("second startup");
    assert!(
        !second.retention_default_installed,
        "retention default must not be re-installed",
    );
    assert_eq!(second.crashed_runs_recovered, 0);
    assert_eq!(second.retention.total_deleted(), 0);

    let installed: u32 = SettingsRepo::new(pool.clone())
        .get(RETENTION_DAYS_SETTING_KEY)
        .await
        .expect("settings get")
        .expect("retention.days present");
    assert_eq!(installed, DEFAULT_RETENTION_DAYS);
}
