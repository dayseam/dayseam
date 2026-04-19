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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use dayseam_core::{error_codes, DayseamError};
use dayseam_db::{LogRepo, RawPayloadRepo, SettingsRepo};
use sqlx::SqlitePool;
use tokio::sync::Mutex;

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

/// Minimum wall-clock gap between two post-run retention sweeps.
/// Ten back-to-back `report_generate` → `report_cancel` pairs must not
/// fan out into ten `DELETE` bursts — that's the PERF-08 cancel-storm
/// amplification Task 7.4 closes. Fifteen minutes is short enough that
/// a user who runs the app every morning still sees a sweep on the
/// first cancel of the day, and long enough that a retry burst is
/// coalesced to a single prune.
pub const POST_RUN_SWEEP_MIN_INTERVAL: Duration = Duration::minutes(15);

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

/// Debounce guard for the post-run retention sweep hook.
///
/// The [`Orchestrator`](crate::Orchestrator) calls
/// [`RetentionSchedule::claim_sweep_slot`] after every terminal
/// `generate_report` transition. The guard returns `true` at most
/// once per [`POST_RUN_SWEEP_MIN_INTERVAL`] so a cancel storm is
/// coalesced into a single `DELETE`. The startup sweep and the
/// manual `retention_sweep_now` IPC command both feed the guard via
/// [`RetentionSchedule::note_external_sweep`] so the post-run hook
/// does not double-fire immediately after them.
///
/// `Clone` returns a handle to the same underlying state — cheap and
/// safe across tasks; internally an [`Arc`] wraps the mutex + counter.
#[derive(Clone, Debug)]
pub struct RetentionSchedule {
    inner: Arc<RetentionScheduleInner>,
}

#[derive(Debug)]
struct RetentionScheduleInner {
    min_interval: Duration,
    last: Mutex<Option<DateTime<Utc>>>,
    /// Number of times [`RetentionSchedule::claim_sweep_slot`] has
    /// returned `true`. Tests use this to assert the debounce:
    /// observing the counter directly is deterministic, whereas
    /// counting deleted rows is not (an empty DB always deletes 0).
    sweeps_performed: AtomicU64,
}

impl RetentionSchedule {
    /// Fresh guard with the shipping default interval
    /// ([`POST_RUN_SWEEP_MIN_INTERVAL`]).
    #[must_use]
    pub fn new() -> Self {
        Self::with_interval(POST_RUN_SWEEP_MIN_INTERVAL)
    }

    /// Fresh guard with a caller-chosen interval. Exposed for tests
    /// that want to exercise the debounce without sleeping 15 minutes.
    #[must_use]
    pub fn with_interval(min_interval: Duration) -> Self {
        Self {
            inner: Arc::new(RetentionScheduleInner {
                min_interval,
                last: Mutex::new(None),
                sweeps_performed: AtomicU64::new(0),
            }),
        }
    }

    /// Minimum gap between two claimed sweep slots. Exposed so the
    /// Tauri layer can log "next post-run sweep in N minutes" without
    /// peeking at internals.
    #[must_use]
    pub fn min_interval(&self) -> Duration {
        self.inner.min_interval
    }

    /// Try to claim the next sweep slot. Returns `true` exactly when
    /// enough time has passed since the last claim (or there has been
    /// no claim yet) and records `now` as the new "last sweep at".
    /// Returns `false` otherwise without mutating state, so the
    /// caller can skip the sweep cheaply.
    ///
    /// Atomic across tasks because every call takes the internal
    /// mutex before reading-and-updating `last`.
    pub async fn claim_sweep_slot(&self, now: DateTime<Utc>) -> bool {
        let mut guard = self.inner.last.lock().await;
        match *guard {
            Some(prev) if now - prev < self.inner.min_interval => false,
            _ => {
                *guard = Some(now);
                self.inner.sweeps_performed.fetch_add(1, Ordering::Relaxed);
                true
            }
        }
    }

    /// Mark that an external caller (startup, manual
    /// `retention_sweep_now`) just swept. Does not increment the
    /// "post-run sweeps performed" counter — that counter exists to
    /// answer "did the debounce hook fire?", not "did the DB get
    /// pruned?". Callers that want total sweep history should consult
    /// the DB directly.
    pub async fn note_external_sweep(&self, now: DateTime<Utc>) {
        *self.inner.last.lock().await = Some(now);
    }

    /// Number of times [`Self::claim_sweep_slot`] has returned `true`
    /// over the lifetime of this guard. Exposed for tests and for the
    /// Tauri layer's "maintenance" logging.
    #[must_use]
    pub fn sweeps_performed(&self) -> u64 {
        self.inner.sweeps_performed.load(Ordering::Relaxed)
    }
}

impl Default for RetentionSchedule {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience: [`resolve_cutoff`] + [`sweep`] in one call, for
/// callers that don't need the cutoff separately. Used by the
/// startup sweep and by the orchestrator's post-run debounce hook.
pub async fn sweep_with_resolved_cutoff(
    pool: &SqlitePool,
    now: DateTime<Utc>,
) -> Result<SweepReport, DayseamError> {
    let cutoff = resolve_cutoff(pool, now).await?;
    sweep(pool, cutoff).await
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
