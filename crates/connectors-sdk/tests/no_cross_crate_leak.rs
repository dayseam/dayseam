//! CI guard that keeps the crate graph one-way.
//!
//! `connectors-sdk` deliberately does **not** depend on:
//!
//! * `dayseam-db` — connectors never persist directly. The
//!   orchestrator is responsible for turning a `SyncResult` into DB
//!   rows. Pulling `dayseam-db` in here would make every connector
//!   transitively depend on SQLx and recreate the "everything knows
//!   about the schema" anti-pattern this architecture rejects.
//! * `dayseam-secrets` — secrets are loaded by the orchestrator and
//!   handed to the connector as an opaque `AuthStrategy`. A connector
//!   that imported `dayseam-secrets` directly could stash raw token
//!   bytes outside our `Secret<T>` discipline.
//! * `dayseam-report` — rendering is a downstream step. A connector
//!   that knew about `ReportDraft` would be tempted to produce
//!   report-shaped output directly, bypassing the canonical artifact
//!   layer planned for Phase 2.
//! * `sinks-sdk` — sinks are strictly downstream of connectors. Any
//!   import edge between these two crates is a layering bug.
//!
//! The test parses the workspace's `cargo metadata` output and asserts
//! the forbidden edges are absent. It is fast enough to run in CI
//! without feature flags.

use cargo_metadata::MetadataCommand;

const FORBIDDEN: &[&str] = &[
    "dayseam-db",
    "dayseam-secrets",
    "dayseam-report",
    "sinks-sdk",
];

#[test]
fn connectors_sdk_does_not_depend_on_forbidden_crates() {
    let metadata = MetadataCommand::new()
        .exec()
        .expect("cargo metadata must succeed");

    let sdk = metadata
        .packages
        .iter()
        .find(|p| p.name == "connectors-sdk")
        .expect("connectors-sdk must be a workspace member");

    let direct_deps: Vec<&str> = sdk.dependencies.iter().map(|d| d.name.as_str()).collect();

    for forbidden in FORBIDDEN {
        assert!(
            !direct_deps.contains(forbidden),
            "connectors-sdk must not depend on `{forbidden}` — see tests/no_cross_crate_leak.rs for why.\nObserved direct deps: {direct_deps:?}"
        );
    }
}
