//! Orchestrator startup — the one-shot maintenance sweep every
//! [`crate::Orchestrator`] runs exactly once before accepting new
//! [`crate::Orchestrator::generate_report`] calls.
//!
//! Three responsibilities, executed in this order:
//!
//! 1. **Bootstrap the retention setting.** On first boot the
//!    `settings.retention.days` row does not exist; we write the
//!    default ([`crate::retention::DEFAULT_RETENTION_DAYS`]) so the
//!    Task 7 settings UI has a concrete value to render. On
//!    subsequent boots the row is left alone — the user's choice wins.
//! 2. **Crash-recovery sweep.** Any `sync_runs` row left in
//!    `Running` with `finished_at IS NULL` is evidence of an unclean
//!    shutdown: the process died mid-run, before the orchestrator
//!    could mark the row terminal. We rewrite each such row to
//!    `Failed` with
//!    [`dayseam_core::error_codes::INTERNAL_PROCESS_RESTARTED`],
//!    mapping every `Pending` / `Running` per-source-state entry to
//!    `Failed` with the same recovery code. This is the second half
//!    of Task 5 invariant #5.
//! 3. **Retention sweep.** One pass over `raw_payloads` and
//!    `log_entries` with the cutoff resolved from the settings table.
//!    Idempotent — a re-boot with nothing stale sweeps zero rows.
//!
//! Callers get back a [`StartupReport`] with per-step counts so the
//! Tauri layer (see `apps/desktop/src-tauri/src/startup.rs`) can
//! `tracing::info!` one line summarising the boot maintenance.

use chrono::Utc;
use dayseam_core::{error_codes, DayseamError, PerSourceState, RunStatus, SyncRun};
use dayseam_db::{SettingsRepo, SyncRunRepo};

use crate::retention::{
    resolve_cutoff, sweep, SweepReport, DEFAULT_RETENTION_DAYS, RETENTION_DAYS_SETTING_KEY,
};
use crate::Orchestrator;

/// Aggregate counts from a single startup sweep. Used only for
/// logging — the callers never branch on individual fields, so the
/// struct is kept deliberately flat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StartupReport {
    /// `true` when this is the first boot on this database and the
    /// default retention window was just written.
    pub retention_default_installed: bool,
    /// Number of `sync_runs` rows swept from `Running` → `Failed`
    /// during crash recovery.
    pub crashed_runs_recovered: u64,
    /// Per-table counts from the retention sweep run at the end of
    /// startup.
    pub retention: SweepReport,
}

impl Orchestrator {
    /// Run the full startup maintenance sweep. Idempotent on the
    /// happy path — calling it twice in a row on a clean DB returns
    /// `StartupReport::default()` on the second call.
    ///
    /// Intended call site: `apps/desktop/src-tauri/src/startup.rs`,
    /// immediately after constructing the `Orchestrator` and before
    /// putting it on `AppState`. Tests in
    /// `tests/crash_recovery.rs` exercise the crash-recovery half
    /// directly against a pre-seeded DB.
    pub async fn startup(&self) -> Result<StartupReport, DayseamError> {
        let retention_default_installed = ensure_default_retention_days(self).await?;
        let crashed_runs_recovered = recover_crashed_runs(self).await?;
        let retention = run_retention_sweep(self).await?;
        // Feed the debounce guard so the first post-run sweep of the
        // session waits out the full interval rather than firing on
        // the very next cancel. The startup sweep already covered
        // the same window, so an immediate re-sweep would be pure
        // amplification (Task 7.4).
        self.retention_schedule
            .note_external_sweep(self.clock.now())
            .await;
        Ok(StartupReport {
            retention_default_installed,
            crashed_runs_recovered,
            retention,
        })
    }
}

/// Write the shipping-default retention window if the row does not
/// already exist. Returns `true` when the default was freshly
/// installed (first boot on this DB), `false` when the row was
/// already present.
async fn ensure_default_retention_days(orch: &Orchestrator) -> Result<bool, DayseamError> {
    let settings = SettingsRepo::new(orch.pool.clone());
    let existing = settings
        .get::<u32>(RETENTION_DAYS_SETTING_KEY)
        .await
        .map_err(|e| DayseamError::Internal {
            code: error_codes::ORCHESTRATOR_RETENTION_SWEEP_FAILED.into(),
            message: format!("failed to read {RETENTION_DAYS_SETTING_KEY}: {e}"),
        })?;
    if existing.is_some() {
        return Ok(false);
    }
    settings
        .set(RETENTION_DAYS_SETTING_KEY, &DEFAULT_RETENTION_DAYS)
        .await
        .map_err(|e| DayseamError::Internal {
            code: error_codes::ORCHESTRATOR_RETENTION_SWEEP_FAILED.into(),
            message: format!("failed to write default {RETENTION_DAYS_SETTING_KEY}: {e}"),
        })?;
    Ok(true)
}

/// Find every `sync_runs` row still in `Running` with no
/// `finished_at` and rewrite it to `Failed`. Each non-terminal
/// per-source-state entry is rewritten too so the UI doesn't show a
/// source stuck on "in progress" forever.
async fn recover_crashed_runs(orch: &Orchestrator) -> Result<u64, DayseamError> {
    let repo = SyncRunRepo::new(orch.pool.clone());
    let running = repo
        .list_running()
        .await
        .map_err(|e| DayseamError::Internal {
            code: error_codes::INTERNAL_PROCESS_RESTARTED.into(),
            message: format!("failed to list running sync_runs: {e}"),
        })?;
    let now = Utc::now();
    let mut recovered: u64 = 0;
    for run in running {
        let per_source = remap_per_source_state_for_recovery(&run, now);
        repo.mark_failed(&run.id, now, &per_source)
            .await
            .map_err(|e| DayseamError::Internal {
                code: error_codes::INTERNAL_PROCESS_RESTARTED.into(),
                message: format!(
                    "failed to recover sync_run[{id}] after unclean shutdown: {e}",
                    id = run.id
                ),
            })?;
        recovered += 1;
    }
    Ok(recovered)
}

/// Rewrite non-terminal per-source state entries to `Failed` with the
/// recovery error code. Entries already terminal (`Succeeded`,
/// `Failed`, `Skipped`) are preserved verbatim — they were observed
/// before the crash and re-labeling them would lose fidelity.
fn remap_per_source_state_for_recovery(
    run: &SyncRun,
    finished_at: chrono::DateTime<Utc>,
) -> Vec<PerSourceState> {
    run.per_source_state
        .iter()
        .map(|state| match state.status {
            RunStatus::Pending | RunStatus::Running => PerSourceState {
                source_id: state.source_id,
                status: RunStatus::Failed,
                started_at: state.started_at,
                finished_at: Some(finished_at),
                fetched_count: state.fetched_count,
                error: Some(DayseamError::Internal {
                    code: error_codes::INTERNAL_PROCESS_RESTARTED.into(),
                    message: "process restarted before this source finished".into(),
                }),
            },
            RunStatus::Succeeded | RunStatus::Failed | RunStatus::Skipped => state.clone(),
        })
        .collect()
}

async fn run_retention_sweep(orch: &Orchestrator) -> Result<SweepReport, DayseamError> {
    let cutoff = resolve_cutoff(&orch.pool, Utc::now()).await?;
    sweep(&orch.pool, cutoff).await
}
