//! Shared helpers for the connectors-sdk integration tests.
//!
//! Each test file pulls in `mod common;` and uses the builders below
//! to avoid duplicating `ConnCtx` plumbing.

use std::sync::Arc;

use dayseam_core::{Person, SourceIdentity, SourceIdentityKind};
use dayseam_events::{LogReceiver, LogSender, ProgressReceiver, ProgressSender, RunStreams};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use connectors_sdk::{
    AuthStrategy, ConnCtx, HttpClient, NoneAuth, NoopRawStore, RetryPolicy, SystemClock,
};

#[allow(dead_code)] // Fields are used by individual integration tests; each test uses a subset.
pub struct CtxHarness {
    pub ctx: ConnCtx,
    pub cancel: CancellationToken,
    pub progress_rx: ProgressReceiver,
    pub log_rx: LogReceiver,
    pub progress_tx: ProgressSender,
    pub log_tx: LogSender,
}

pub fn build_ctx(source_id: Uuid, identities: Vec<SourceIdentity>) -> CtxHarness {
    let streams = RunStreams::new(dayseam_events::RunId::new());
    let ((progress_tx, log_tx), (progress_rx, log_rx)) = streams.split();
    let run_id = progress_tx.run_id();
    let cancel = CancellationToken::new();

    let ctx = ConnCtx {
        run_id,
        source_id,
        person: Person::new_self("Test"),
        source_identities: identities,
        auth: Arc::new(NoneAuth) as Arc<dyn AuthStrategy>,
        progress: progress_tx.clone(),
        logs: log_tx.clone(),
        raw_store: Arc::new(NoopRawStore),
        clock: Arc::new(SystemClock),
        http: HttpClient::new()
            .expect("build http client")
            .with_policy(RetryPolicy::instant()),
        cancel: cancel.clone(),
    };

    CtxHarness {
        ctx,
        cancel,
        progress_rx,
        log_rx,
        progress_tx,
        log_tx,
    }
}

pub fn self_identity(source_id: Uuid, actor_email: &str) -> SourceIdentity {
    SourceIdentity {
        id: Uuid::new_v4(),
        person_id: Uuid::new_v4(),
        source_id: Some(source_id),
        kind: SourceIdentityKind::GitEmail,
        external_actor_id: actor_email.to_string(),
    }
}
