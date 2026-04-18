//! The sync request / response shapes every connector speaks.
//!
//! These types are intentionally **Rust-only**: they never cross the
//! Tauri IPC boundary (the frontend sees `ReportDraft`s, `ProgressEvent`s,
//! and `DayseamError`s — not raw connector output), so they do not
//! derive `ts_rs::TS`.
//!
//! The one-`sync`-method contract is the key lesson from the
//! architecture review: having [`SyncRequest::Day`] / [`Range`] /
//! [`Since`] as variants of a single request type means v0.1 connectors
//! only ever service `Day`, while v0.2 (multi-day reporting) and v0.3
//! (scheduler + incremental fetch) can extend the trait without any
//! signature change.

use chrono::NaiveDate;
use dayseam_core::{ActivityEvent, LogEvent, RawRef};
use serde::{Deserialize, Serialize};

/// What the orchestrator is asking the connector to fetch.
///
/// Only [`SyncRequest::Day`] is exercised in v0.1. Connectors are still
/// expected to handle the other variants — either by servicing them or
/// by returning
/// `Err(DayseamError::Unsupported { code: CONNECTOR_UNSUPPORTED_SYNC_REQUEST, … })`
/// so the orchestrator can fall back. This avoids a trait rewrite when
/// v0.2 / v0.3 arrive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncRequest {
    /// Fetch every event whose `occurred_at` falls on `date` in the
    /// user's local timezone. This is the v0.1 shape and the one the
    /// UI's date picker feeds directly.
    Day(NaiveDate),
    /// Fetch every event whose `occurred_at` is in `[start, end]`
    /// inclusive. Used by v0.2 weekly / multi-day reports.
    Range { start: NaiveDate, end: NaiveDate },
    /// Fetch everything the connector has produced since `checkpoint`.
    /// Used by v0.3's incremental scheduler. Connectors without a
    /// meaningful checkpoint return `Err(DayseamError::Unsupported)`
    /// and the orchestrator rewrites the request as an equivalent
    /// `Range`.
    Since(Checkpoint),
}

/// Opaque per-connector cursor persisted inside a `SyncRun` row. The
/// shape is whatever the connector wants — ETag string, last-modified
/// timestamp, GitLab `updated_after` cursor, git `rev-list` tip — the
/// orchestrator only stores and replays the bytes verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Machine id for the producing connector, e.g. `"gitlab"` or
    /// `"local-git"`. Used to refuse a checkpoint from a different
    /// connector without silently misinterpreting its bytes.
    pub connector: String,
    /// Opaque connector-defined payload. Kept as `serde_json::Value`
    /// so connectors can persist structured cursors (a cursor token
    /// alongside its high-water mark) without needing to stringify.
    pub value: serde_json::Value,
}

/// Everything a successful `sync` call produced.
///
/// `artifacts` is intentionally absent from v0.1: the canonical
/// artifact layer lands in Phase 2 alongside multi-source
/// correlation. Connectors therefore return events + optional
/// checkpoint + stats for now; the orchestrator promotes sets of
/// events into artifacts downstream.
#[derive(Debug, Clone, Default)]
pub struct SyncResult {
    /// Normalised evidence records. Every `ActivityEvent::raw_ref`
    /// points at a `RawRef` listed in `raw_refs` below (or at an
    /// already-persisted row for connectors that wrote to the raw
    /// store in-line).
    pub events: Vec<ActivityEvent>,
    /// Checkpoint to persist in the `SyncRun` row so the next
    /// incremental call can resume from here. `None` means the
    /// connector does not support incremental sync.
    pub checkpoint: Option<Checkpoint>,
    /// Counters the UI shows to reassure the user that the sync
    /// actually did something. See [`SyncStats`].
    pub stats: SyncStats,
    /// Non-fatal things the UI should surface (e.g. "skipped a repo
    /// because it is marked private"). Reuse [`LogEvent`] for shape
    /// parity with everything else that flows through the log drawer.
    pub warnings: Vec<LogEvent>,
    /// Raw payloads the connector wants the orchestrator to persist
    /// (subject to retention policy). Connectors that persist
    /// in-line leave this empty.
    pub raw_refs: Vec<RawRef>,
}

/// Small counters exposed to the UI and to the log drawer. Not a
/// performance metric — just enough signal for the user to notice when
/// a sync returns zero events unexpectedly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncStats {
    /// Total events returned to the orchestrator.
    pub fetched_count: u64,
    /// Events the connector filtered out because their actor did not
    /// resolve to any of the `SourceIdentity` rows in `ConnCtx`.
    pub filtered_by_identity: u64,
    /// Events the connector filtered out because their `occurred_at`
    /// fell outside the requested window.
    pub filtered_by_date: u64,
    /// How many HTTP retries the connector performed end-to-end
    /// across the whole sync (429 backoffs, 5xx retries, etc.).
    pub http_retries: u32,
}
