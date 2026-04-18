//! The [`SourceConnector`] trait every read-only source implements.
//!
//! This is the plan's key contract: a single `sync` method taking a
//! [`SyncRequest`] so the v0.1 `Day`-only orchestrator and the v0.3
//! incremental scheduler speak the same trait — no rewrites in between.

use async_trait::async_trait;
use dayseam_core::{DayseamError, SourceHealth, SourceKind};

use crate::{
    ctx::ConnCtx,
    sync::{SyncRequest, SyncResult},
};

/// One read-only source of activity (GitLab, local git, and later
/// GitHub/Jira/Slack/…). Implementations live in their own
/// `connector-*` crates; this SDK defines the contract they speak.
#[async_trait]
pub trait SourceConnector: Send + Sync + std::fmt::Debug {
    /// Which high-level source kind this connector serves. The
    /// orchestrator dispatches by this value, so two connectors that
    /// claim the same kind are a bug.
    fn kind(&self) -> SourceKind;

    /// Probe the source to confirm the configuration and credentials
    /// are still valid. Runs on demand from the UI ("test
    /// connection") and periodically from the orchestrator before a
    /// sync. Fast — no bulk fetching.
    async fn healthcheck(&self, ctx: &ConnCtx) -> Result<SourceHealth, DayseamError>;

    /// Fetch activity for `request`. Every [`SyncRequest`] variant is
    /// a legal input; connectors that cannot service `Since(Checkpoint)`
    /// return `Err(DayseamError::Unsupported { code:
    /// CONNECTOR_UNSUPPORTED_SYNC_REQUEST, … })` and the orchestrator
    /// retries with an equivalent `Range`.
    ///
    /// The connector is expected to:
    /// * emit progress events via `ctx.progress`,
    /// * log structured warnings via `ctx.logs`,
    /// * check `ctx.cancel` between batches,
    /// * authenticate outbound calls via `ctx.auth`,
    /// * filter returned events by `ctx.source_identities`.
    async fn sync(&self, ctx: &ConnCtx, request: SyncRequest) -> Result<SyncResult, DayseamError>;
}
