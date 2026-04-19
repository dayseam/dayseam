//! `save_report` lifecycle — dispatch a persisted [`ReportDraft`] to a
//! configured [`Sink`] and return the resulting [`WriteReceipt`].
//!
//! The contract, verbatim from Task 5 invariant #7 in
//! `docs/plan/2026-04-18-v0.1-phase-2-local-git.md`:
//!
//! > `save_report` is atomic from the orchestrator's point of view. A
//! > failed sink write does not mutate `report_drafts.sections_json`;
//! > only `WriteReceipt` rows are appended (see Task 4 invariant #5).
//!
//! The atomicity is structural, not transactional: the orchestrator
//! never writes to `report_drafts` here. The draft row was persisted
//! at the end of [`crate::generate`] and stays exactly as it was. A
//! sink write that fails propagates a [`DayseamError`] back to the
//! caller without touching either the draft row or any other state.
//!
//! Receipts are returned as a `Vec` rather than a single value for
//! forward compatibility with the Task 6 `report_save(draft_id,
//! sink_id) -> Vec<WriteReceipt>` IPC command shape. In v0.1 the vec
//! always contains exactly one entry because `SinkAdapter::write`
//! returns one receipt per call.

use dayseam_core::{error_codes, DayseamError, ReportDraft, RunId, Sink, WriteReceipt};
use dayseam_events::RunStreams;
use sinks_sdk::SinkCtx;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::Orchestrator;

/// Run the save lifecycle for `draft_id` against `sink`.
///
/// Implementation outline:
///
/// 1. Load the draft from [`dayseam_db::DraftRepo`]. A missing draft
///    returns [`DayseamError::InvalidConfig`] with code
///    [`error_codes::ORCHESTRATOR_SAVE_DRAFT_NOT_FOUND`]; the Task 6
///    save dialog surfaces this inline rather than as a toast.
/// 2. Look up the adapter via
///    [`crate::SinkRegistry::get`](crate::SinkRegistry::get). An
///    unregistered kind returns
///    [`error_codes::ORCHESTRATOR_SINK_NOT_REGISTERED`] — usually a
///    feature-flag mismatch rather than user error.
/// 3. Build a fresh [`SinkCtx`]. `run_id` is `None` because saves are
///    not tied to a generate run; the Phase 1 dev-run shape stays
///    untouched, and the receipt's `run_id` field reflects that.
/// 4. Call `adapter.write(ctx, sink.config, draft)`. Any error is
///    returned unchanged — the adapter has already tagged it with a
///    `sink.*` error code (Task 4 invariant #3).
pub(crate) async fn run(
    orch: &Orchestrator,
    draft_id: Uuid,
    sink: &Sink,
) -> Result<Vec<WriteReceipt>, DayseamError> {
    let draft = load_draft(orch, draft_id).await?;
    let adapter = orch
        .sinks
        .get(sink.kind)
        .ok_or_else(|| DayseamError::InvalidConfig {
            code: error_codes::ORCHESTRATOR_SINK_NOT_REGISTERED.into(),
            message: format!("no sink adapter registered for kind {:?}", sink.kind),
        })?;

    // Save is an ad-hoc action, not part of a generate run. We still
    // build a fresh `RunStreams` so the sink has progress / log
    // channels to emit on; Task 6 wires them into the save dialog's
    // progress indicator.
    let streams = RunStreams::new(RunId::new());
    let ctx = SinkCtx::new(
        None,
        streams.progress_tx,
        streams.log_tx,
        CancellationToken::new(),
    );

    let receipt = adapter.write(&ctx, &sink.config, &draft).await?;
    Ok(vec![receipt])
}

async fn load_draft(orch: &Orchestrator, draft_id: Uuid) -> Result<ReportDraft, DayseamError> {
    let repo = dayseam_db::DraftRepo::new(orch.pool.clone());
    let draft = repo
        .get(&draft_id)
        .await
        .map_err(|e| DayseamError::Internal {
            code: error_codes::ORCHESTRATOR_SAVE_DRAFT_NOT_FOUND.into(),
            message: format!("failed to read report_drafts[{draft_id}]: {e}"),
        })?;
    draft.ok_or_else(|| DayseamError::InvalidConfig {
        code: error_codes::ORCHESTRATOR_SAVE_DRAFT_NOT_FOUND.into(),
        message: format!("no report_drafts row with id {draft_id}"),
    })
}
