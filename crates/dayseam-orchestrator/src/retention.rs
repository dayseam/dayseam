//! Retention sweep — the stateless pruner that keeps the local SQLite
//! database from growing forever.
//!
//! Phase 2 prunes two tables:
//!
//! * `raw_payloads` — the per-fetch on-disk cache of upstream JSON
//!   responses, indexed by `fetched_at`.
//! * `log_entries` — the in-app log drawer feed, indexed by `ts`.
//!
//! Drafts and sync-run rows are **not** swept here; they carry the
//! user's history ("what did I do on 2026-04-01?") and have their own
//! retention window that is much longer (Task 7).
//!
//! The sweep is designed around three properties:
//!
//! 1. **Idempotent** — a sweep that has nothing to do deletes zero rows
//!    and returns `Ok(SweepReport::empty())`. Running it twice in a row
//!    must match the first run's outcome on the second call. This is
//!    what lets Task 6 expose a `retention_sweep_now` command safely.
//! 2. **Single transaction per table** — each table's prune is its own
//!    `DELETE` statement; a failure on `log_entries` never rolls back
//!    the `raw_payloads` prune. That's intentional: a partial success
//!    is still a win for disk pressure.
//! 3. **Never touches rows newer than `cutoff`** — the cutoff the
//!    caller supplies is the upper bound for "old enough to drop";
//!    everything at or after the cutoff stays.
//!
//! The default cutoff comes from the `retention.days` key in
//! `settings`, written once at first startup with value `30` (see
//! [`crate::startup::ensure_default_retention_days`]). Callers that
//! want to skip the DB read (tests, the `retention_sweep_now`
//! command) can pass a literal [`DateTime`] directly.

use chrono::{DateTime, Duration, Utc};
use dayseam_core::{error_codes, DayseamError};
use dayseam_db::{LogRepo, RawPayloadRepo, SettingsRepo};
use sqlx::SqlitePool;

/// Settings key under which the retention window (in days) is
/// persisted. Read at startup and on every manual
/// `retention_sweep_now` call so a user who edits the setting in the
/// Task 7 settings UI sees the new cutoff take effect on the next
/// sweep without a restart.
pub const RETENTION_DAYS_SETTING_KEY: &str = "retention.days";

/// Shipping default: keep the last 30 days of raw payloads / logs.
/// Written to `settings` on first startup only (see
/// [`crate::startup::ensure_default_retention_days`]).
pub const DEFAULT_RETENTION_DAYS: u32 = 30;

/// Per-table counts from one [`sweep`] call. Returned so the caller
/// (typically the Tauri startup hook or the `retention_sweep_now`
/// command) can log "pruned N raw_payloads / M log_entries" without
/// re-querying the DB.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SweepReport {
    pub raw_payloads_deleted: u64,
    pub log_entries_deleted: u64,
}

impl SweepReport {
    /// Convenience constructor for "the sweep had nothing to do".
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Total rows deleted across every table. Used by the log line
    /// the startup hook emits after the sweep completes.
    #[must_use]
    pub fn total_deleted(&self) -> u64 {
        self.raw_payloads_deleted + self.log_entries_deleted
    }
}

/// Delete every `raw_payloads` and `log_entries` row older than
/// `cutoff` (exclusive of `cutoff` itself). Each table is a single
/// `DELETE` statement; a failure on the second table never rolls
/// back the first — partial cleanup still helps.
///
/// Maps every DB-layer error into a [`DayseamError::Internal`]
/// tagged with [`error_codes::ORCHESTRATOR_RETENTION_SWEEP_FAILED`]
/// so the caller gets a single uniform error shape regardless of
/// which table failed. The failing table's name is included in the
/// message for diagnosis.
pub async fn sweep(pool: &SqlitePool, cutoff: DateTime<Utc>) -> Result<SweepReport, DayseamError> {
    let raw_payloads = RawPayloadRepo::new(pool.clone())
        .prune_older_than(cutoff)
        .await
        .map_err(|e| DayseamError::Internal {
            code: error_codes::ORCHESTRATOR_RETENTION_SWEEP_FAILED.into(),
            message: format!("retention sweep failed on raw_payloads: {e}"),
        })?;
    let log_entries = LogRepo::new(pool.clone())
        .prune_older_than(cutoff)
        .await
        .map_err(|e| DayseamError::Internal {
            code: error_codes::ORCHESTRATOR_RETENTION_SWEEP_FAILED.into(),
            message: format!("retention sweep failed on log_entries: {e}"),
        })?;
    Ok(SweepReport {
        raw_payloads_deleted: raw_payloads,
        log_entries_deleted: log_entries,
    })
}

/// Resolve the effective cutoff for a retention sweep from the DB.
/// Reads `retention.days` from `settings`; if the row is absent (the
/// startup bootstrap hasn't run yet, or a test skipped it) falls back
/// to [`DEFAULT_RETENTION_DAYS`]. The caller subtracts the result from
/// `now` to get the cutoff used in [`sweep`].
pub async fn resolve_cutoff(
    pool: &SqlitePool,
    now: DateTime<Utc>,
) -> Result<DateTime<Utc>, DayseamError> {
    let days = SettingsRepo::new(pool.clone())
        .get::<u32>(RETENTION_DAYS_SETTING_KEY)
        .await
        .map_err(|e| DayseamError::Internal {
            code: error_codes::ORCHESTRATOR_RETENTION_SWEEP_FAILED.into(),
            message: format!("failed to read retention.days: {e}"),
        })?
        .unwrap_or(DEFAULT_RETENTION_DAYS);
    Ok(now - Duration::days(i64::from(days)))
}
