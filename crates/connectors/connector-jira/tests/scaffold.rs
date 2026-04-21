//! Smoke tests proving the scaffolding invariants that survive
//! alongside the DAY-77 walker:
//!
//! 1. `JiraConnector::kind() == SourceKind::Jira`.
//! 2. `JiraMux` is object-safe through `Arc<dyn SourceConnector>` —
//!    the orchestrator registry stores it behind exactly that
//!    bound, so a regression here would fail every
//!    `default_registries` call loudly.
//! 3. `JiraMux::upsert` / `remove` round-trip by `source_id`.
//! 4. `SyncRequest::Range` and `SyncRequest::Since` return
//!    [`DayseamError::Unsupported`] (the JQL walker only services
//!    `Day` in v0.2; `Range` waits on v0.3's incremental scheduler).
//! 5. `SyncRequest::Day` is *not* `Unsupported` any more — when no
//!    Atlassian identity is configured for the source the walker
//!    degrades to `Ok(SyncResult { events: [], … })` rather than
//!    failing. End-to-end walker behaviour (pagination, normalise,
//!    rollup, error mapping) is exercised in `tests/walk.rs` via
//!    wiremock.

use std::sync::Arc;

use chrono::NaiveDate;
use connector_jira::{JiraConfig, JiraConnector, JiraMux, JiraSourceCfg};
use connectors_sdk::{
    AuthStrategy, BasicAuth, Checkpoint, ConnCtx, HttpClient, NoneAuth, NoopRawStore,
    SourceConnector, SyncRequest, SystemClock,
};
use dayseam_core::{error_codes, DayseamError, Person, SourceKind};
use dayseam_events::RunStreams;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn test_config() -> JiraConfig {
    JiraConfig::from_raw("https://acme.atlassian.net", "vedanth@acme.com")
        .expect("known-good workspace URL parses")
}

fn mk_ctx(source_id: Uuid) -> ConnCtx {
    let streams = RunStreams::new(dayseam_events::RunId::new());
    let ((ptx, ltx), (_, _)) = streams.split();
    let run_id = ptx.run_id();
    ConnCtx {
        run_id,
        source_id,
        person: Person::new_self("Test"),
        source_identities: Vec::new(),
        auth: Arc::new(NoneAuth) as Arc<dyn AuthStrategy>,
        progress: ptx,
        logs: ltx,
        raw_store: Arc::new(NoopRawStore),
        clock: Arc::new(SystemClock),
        http: HttpClient::new().expect("http client builds"),
        cancel: CancellationToken::new(),
    }
}

#[test]
fn jira_connector_kind_is_jira() {
    let conn = JiraConnector::new(test_config());
    assert_eq!(conn.kind(), SourceKind::Jira);
}

#[test]
fn jira_mux_kind_is_jira() {
    let mux = JiraMux::default();
    assert_eq!(mux.kind(), SourceKind::Jira);
}

#[test]
fn jira_mux_can_be_wrapped_as_arc_dyn_source_connector() {
    // The orchestrator registry stores `Arc<dyn SourceConnector>`
    // per kind; this sanity check catches any future regression that
    // would make `JiraMux` un-wrappable (e.g. a stray `?Sized` bound).
    let mux: Arc<dyn SourceConnector> = Arc::new(JiraMux::default());
    assert_eq!(mux.kind(), SourceKind::Jira);
}

#[tokio::test]
async fn jira_mux_upsert_and_remove_round_trip() {
    let mux = JiraMux::default();
    assert!(mux.is_empty().await);

    let source_id = Uuid::new_v4();
    mux.upsert(JiraSourceCfg {
        source_id,
        config: test_config(),
    })
    .await;
    assert_eq!(mux.len().await, 1);

    mux.remove(source_id).await;
    assert!(mux.is_empty().await);
}

#[tokio::test]
async fn jira_mux_sync_on_unregistered_source_returns_source_not_found() {
    let mux = JiraMux::default();
    let ctx = mk_ctx(Uuid::new_v4());
    let err = mux
        .sync(
            &ctx,
            SyncRequest::Day(NaiveDate::from_ymd_opt(2026, 4, 20).unwrap()),
        )
        .await
        .expect_err("an unregistered source_id has to surface as InvalidConfig");
    assert_eq!(err.code(), error_codes::IPC_SOURCE_NOT_FOUND);
    assert!(matches!(err, DayseamError::InvalidConfig { .. }));
}

#[tokio::test]
async fn sync_day_is_serviced_by_walker_degrading_when_no_identity_configured() {
    // DAY-77 flipped this arm onto the JQL walker. The walker opens
    // with an identity check: if no `SourceIdentityKind::AtlassianAccountId`
    // row is registered for the source, the walker returns an empty
    // outcome + a warn log (rather than failing), matching the
    // DAY-71 invariant that a missing identity is a known-cause empty
    // result, not a surprise panic. This test exercises the
    // connector→walker wiring without needing wiremock; end-to-end
    // walker behaviour lives in `tests/walk.rs`.
    let conn = JiraConnector::new(test_config());
    let ctx = mk_ctx(Uuid::new_v4());
    let result = conn
        .sync(
            &ctx,
            SyncRequest::Day(NaiveDate::from_ymd_opt(2026, 4, 20).unwrap()),
        )
        .await
        .expect("Day is now serviced by the walker");
    assert!(
        result.events.is_empty(),
        "no identity configured → walker returns empty events"
    );
    assert_eq!(result.stats.fetched_count, 0);
}

#[tokio::test]
async fn sync_range_returns_unsupported() {
    let conn = JiraConnector::new(test_config());
    let ctx = mk_ctx(Uuid::new_v4());
    let err = conn
        .sync(
            &ctx,
            SyncRequest::Range {
                start: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
                end: NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
            },
        )
        .await
        .expect_err("Range is unsupported in v0.2 Jira");
    assert_eq!(err.code(), error_codes::CONNECTOR_UNSUPPORTED_SYNC_REQUEST);
}

#[tokio::test]
async fn sync_since_returns_unsupported() {
    let conn = JiraConnector::new(test_config());
    let ctx = mk_ctx(Uuid::new_v4());
    let err = conn
        .sync(
            &ctx,
            SyncRequest::Since(Checkpoint {
                connector: "jira".into(),
                value: serde_json::Value::Null,
            }),
        )
        .await
        .expect_err("Since is unsupported in v0.2 Jira");
    assert_eq!(err.code(), error_codes::CONNECTOR_UNSUPPORTED_SYNC_REQUEST);
}

#[test]
fn basic_auth_builds_for_jira_flow() {
    // This is belt-and-braces: the Add-Source dialog reaches for
    // `BasicAuth::atlassian` when wiring up a `SourceKind::Jira` row,
    // and a future signature change there would break the IPC
    // command in a way that is easy to miss (the IPC path
    // double-wraps the BasicAuth under `Arc<dyn AuthStrategy>` before
    // this crate ever sees it). Keep the invocation exercised here
    // so refactors to `connectors-sdk` fail in *this* PR's test
    // surface too, not only in the distant IPC crate.
    let _basic = BasicAuth::atlassian(
        "vedanth@acme.com",
        "api-token-xyz",
        "dayseam.atlassian",
        "vedanth@acme.com",
    );
}
