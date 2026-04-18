//! The context handed to every [`crate::SourceConnector`] call.
//!
//! `ConnCtx` is deliberately not `Clone`: each sync run gets exactly
//! one, and the per-run channels close when it is dropped. Inner fields
//! that do need to be shared across tasks (`HttpClient`, `RawStore`,
//! `Clock`) are already cheap to clone on their own.
//!
//! The per-run `ProgressSender` and `LogSender` come from
//! [`dayseam_events::RunStreams`]; connectors emit through them
//! without knowing or caring whether the consumer is a Tauri
//! `Channel<T>`, a test harness, or a future CLI. The
//! `CancellationToken` is checked at loop boundaries by well-behaved
//! connectors — see the `MockConnector` in `crate::mock` for a
//! reference implementation.

use std::sync::Arc;

use dayseam_core::{Person, SourceId, SourceIdentity};
use dayseam_events::{LogSender, ProgressSender, RunId};
use tokio_util::sync::CancellationToken;

use crate::{auth::AuthStrategy, clock::Clock, http::HttpClient, raw_store::RawStore};

/// Per-run connector context. One instance is built by the orchestrator
/// for every `sync` call and threaded into the connector.
#[derive(Debug)]
pub struct ConnCtx {
    /// The run this connector call is bound to. Stamped onto every
    /// emitted [`dayseam_events::ProgressEvent`] and
    /// [`dayseam_events::LogEvent`] so stale events from superseded
    /// runs can be filtered out downstream.
    pub run_id: RunId,

    /// The configured source this connector instance is serving.
    pub source_id: SourceId,

    /// The canonical human the orchestrator is filtering for. The
    /// connector uses `source_identities` below (not `person.id`) to
    /// answer "is this event mine?"; `person` is here for display
    /// strings and for future multi-person runs.
    pub person: Person,

    /// External actor ids that resolve to `person` for this source. A
    /// connector keeps an event iff at least one
    /// `SourceIdentity::external_actor_id` matches the event's actor
    /// key. v0.1 populates this from git `user.email` and the GitLab
    /// `/user` endpoint.
    pub source_identities: Vec<SourceIdentity>,

    /// How to authenticate outbound requests. `Arc` because the HTTP
    /// retry loop clones the strategy handle across attempts.
    pub auth: Arc<dyn AuthStrategy>,

    /// Progress stream for this run. Emitting on a closed sender is
    /// safe but silent — `is_closed()` is the cancellation
    /// fast-path for pure compute loops.
    pub progress: ProgressSender,

    /// Log stream for this run — the structured sibling of `progress`.
    /// Warnings the connector wants to surface go here.
    pub logs: LogSender,

    /// Raw payload persistence. v0.1 pairs this with [`crate::NoopRawStore`].
    pub raw_store: Arc<dyn RawStore>,

    /// Injectable wall clock. Real runs use
    /// [`crate::SystemClock`]; tests install a fake.
    pub clock: Arc<dyn Clock>,

    /// Shared HTTP client. `HttpClient` is cheap to clone so the
    /// orchestrator hands every run the same configured instance.
    pub http: HttpClient,

    /// Cancellation token the connector is expected to poll between
    /// requests. `cancel.is_cancelled()` flips to `true` when the user
    /// cancels the run, when the app is shutting down, or when a newer
    /// run has superseded this one.
    pub cancel: CancellationToken,
}

impl ConnCtx {
    /// Convenience: bail out with [`dayseam_core::DayseamError::Cancelled`]
    /// if the run has been cancelled. Connectors call this at loop
    /// boundaries (per-repo, per-page, per-batch).
    pub fn bail_if_cancelled(&self) -> Result<(), dayseam_core::DayseamError> {
        if self.cancel.is_cancelled() {
            Err(dayseam_core::DayseamError::Cancelled {
                code: dayseam_core::error_codes::RUN_CANCELLED_BY_USER.to_string(),
                message: "run cancelled".to_string(),
            })
        } else {
            Ok(())
        }
    }
}
