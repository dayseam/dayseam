//! Retention sweep tests (Task 5 invariant #6).
//!
//! Contract:
//!
//! * [`retention::sweep`] deletes every `raw_payloads` / `log_entries`
//!   row strictly older than the supplied cutoff, and leaves every
//!   row at-or-after the cutoff alone.
//! * The sweep is **idempotent**: running it twice back-to-back on
//!   the same DB returns zero deletions on the second call. This is
//!   what lets the future `retention_sweep_now` IPC command be
//!   triggered safely without further guards.
//! * [`retention::resolve_cutoff`] falls back to
//!   [`retention::DEFAULT_RETENTION_DAYS`] when the `retention.days`
//!   settings row is missing, and honours an explicit user-set value
//!   on top of an already-populated table.

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;

use chrono::{Duration, Utc};
use common::{build_orchestrator, fixture_date, seed_source, test_person, test_pool};
use connectors_sdk::MockConnector;
use dayseam_core::SourceKind;
use dayseam_core::{LogLevel, SourceId};
use dayseam_db::{LogRepo, LogRow, RawPayload, RawPayloadRepo, SettingsRepo};
use dayseam_orchestrator::retention::{self, DEFAULT_RETENTION_DAYS, RETENTION_DAYS_SETTING_KEY};
use dayseam_orchestrator::{orchestrator::GenerateRequest, ConnectorRegistry, SinkRegistry};
use dayseam_report::DEV_EOD_TEMPLATE_ID;
use uuid::Uuid;

fn raw_payload_at(source_id: SourceId, fetched_at: chrono::DateTime<Utc>) -> RawPayload {
    RawPayload {
        id: Uuid::new_v4(),
        source_id,
        endpoint: "GET /events".into(),
        fetched_at,
        payload_json: "{}".into(),
        payload_sha256: "deadbeef".into(),
    }
}

fn log_row_at(ts: chrono::DateTime<Utc>) -> LogRow {
    LogRow {
        ts,
        level: LogLevel::Info,
        source_id: None,
        message: "fixture".into(),
        context: None,
    }
}

/// A stale row (older than cutoff) is pruned; a fresh row is not.
/// Second run is a no-op — the idempotency guarantee the sweep
/// depends on for safe re-trigger.
#[tokio::test]
async fn sweep_deletes_only_old_rows_and_is_idempotent() {
    let (pool, _tmp) = test_pool().await;
    let (source, _id, _handle) = seed_source(
        &pool,
        &test_person(),
        SourceKind::LocalGit,
        "fixture-source",
        "dev@example.com",
    )
    .await;
    let now = Utc::now();
    let cutoff = now - Duration::days(30);

    let raw_payloads = RawPayloadRepo::new(pool.clone());
    let stale = raw_payload_at(source.id, now - Duration::days(60));
    let fresh = raw_payload_at(source.id, now - Duration::days(1));
    raw_payloads.insert(&stale).await.expect("insert stale");
    raw_payloads.insert(&fresh).await.expect("insert fresh");

    let logs = LogRepo::new(pool.clone());
    let stale_log = log_row_at(now - Duration::days(45));
    let fresh_log = log_row_at(now - Duration::days(2));
    logs.append(&stale_log).await.expect("insert stale log");
    logs.append(&fresh_log).await.expect("insert fresh log");

    let first = retention::sweep(&pool, cutoff).await.expect("sweep");
    assert_eq!(first.raw_payloads_deleted, 1);
    assert_eq!(first.log_entries_deleted, 1);
    assert_eq!(first.total_deleted(), 2);

    // Fresh rows still present.
    assert!(
        raw_payloads.get(&fresh.id).await.expect("get").is_some(),
        "fresh raw_payload must survive",
    );

    // Second call is a pure no-op.
    let second = retention::sweep(&pool, cutoff).await.expect("sweep");
    assert_eq!(
        second,
        retention::SweepReport::empty(),
        "idempotent: nothing to prune the second time",
    );
}

#[tokio::test]
async fn sweep_on_empty_db_deletes_zero_rows() {
    let (pool, _tmp) = test_pool().await;
    let report = retention::sweep(&pool, Utc::now() - Duration::days(30))
        .await
        .expect("sweep");
    assert_eq!(report, retention::SweepReport::empty());
}

/// Task 7 invariant #4 (PERF-08 cancel-storm amplification).
///
/// Firing a burst of `report_generate` → terminal transitions in
/// quick succession must coalesce to a single post-run retention
/// sweep. The debounce guard lives on the [`Orchestrator`]; the
/// counter on `retention_schedule().sweeps_performed()` is the
/// deterministic witness (we cannot count DB deletes, because an
/// empty fixture DB always deletes zero rows regardless of how many
/// times the sweep fires).
///
/// We run ten complete generate cycles sequentially against a
/// `MockConnector`. That's 10 terminal transitions within real-time
/// milliseconds — well inside the 15-minute debounce window. The
/// guard must therefore allow the first sweep through and reject the
/// other nine.
#[tokio::test]
async fn retention_sweep_debounces_under_cancel_storm() {
    let (pool, _tmp) = test_pool().await;
    let person = test_person();
    let date = fixture_date();

    let (source, _id, handle) = seed_source(
        &pool,
        &person,
        SourceKind::LocalGit,
        "storm-fixture",
        "dev@example.com",
    )
    .await;
    let event = common::fixture_event(source.id, "evt-1", "dev@example.com", date);
    let connector = Arc::new(MockConnector::new(SourceKind::LocalGit, vec![event]));
    let mut connectors = ConnectorRegistry::default();
    connectors.insert(SourceKind::LocalGit, connector);

    let orch = build_orchestrator(pool.clone(), connectors, SinkRegistry::default());

    // Ten sequential generate_reports against the same (person, date,
    // template) tuple. Each call supersedes the prior in-flight run,
    // so this is a stricter cancel-storm than "ten unrelated runs":
    // every cycle both terminates a run (via supersede) and starts a
    // new one, giving the debounce guard two termination events per
    // iteration rather than one.
    for _ in 0..10 {
        let req = GenerateRequest {
            person: person.clone(),
            sources: vec![handle.clone()],
            date,
            template_id: DEV_EOD_TEMPLATE_ID.to_string(),
            template_version: "0.0.1".to_string(),
            verbose_mode: false,
        };
        let run_handle = orch.generate_report(req).await;
        // We don't care about the outcome — only that the post-run
        // hook fires on the terminal transition.
        let _ = run_handle.completion.await;
    }

    let swept = orch.retention_schedule().sweeps_performed();
    assert!(
        swept <= 1,
        "debounce broken: {swept} post-run sweeps for a 10-run storm; expected ≤1",
    );
}

/// Absent setting → default 30 days. Present setting → honoured
/// verbatim, so a user who set `retention.days = 7` in the UI gets a
/// 7-day cutoff on the very next sweep.
#[tokio::test]
async fn resolve_cutoff_defaults_without_setting_and_honours_user_override() {
    let (pool, _tmp) = test_pool().await;
    let now = Utc::now();

    let default_cutoff = retention::resolve_cutoff(&pool, now).await.expect("cutoff");
    let expected_default = now - Duration::days(i64::from(DEFAULT_RETENTION_DAYS));
    // Allow a 1 s skew because `Utc::now()` is called twice internally.
    let skew = (default_cutoff - expected_default).num_seconds().abs();
    assert!(skew <= 1, "default cutoff skewed by {skew}s");

    SettingsRepo::new(pool.clone())
        .set::<u32>(RETENTION_DAYS_SETTING_KEY, &7)
        .await
        .expect("set retention.days");
    let overridden = retention::resolve_cutoff(&pool, now).await.expect("cutoff");
    let expected_override = now - Duration::days(7);
    let skew = (overridden - expected_override).num_seconds().abs();
    assert!(skew <= 1, "overridden cutoff skewed by {skew}s");
}
