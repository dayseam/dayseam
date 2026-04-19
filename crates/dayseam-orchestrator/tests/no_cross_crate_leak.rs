//! CI guard that keeps the crate graph one-way.
//!
//! `dayseam-orchestrator` is intentionally a pure Rust crate so
//! headless integration tests (and any future CLI) can drive the
//! generate lifecycle without pulling the Tauri runtime. It must
//! therefore never depend on:
//!
//! * `tauri` — pulling Tauri into the orchestrator would mean every
//!   non-desktop consumer has to drag WebView, bundler scripts, and
//!   the Tauri build tooling behind it.
//! * `dayseam-desktop` (or any other app-layer crate) — the
//!   orchestrator is upstream of the app, not the other way around.
//!   An edge the wrong direction is an architectural bug and
//!   instantly creates a cycle the moment `dayseam-desktop` adds
//!   `dayseam-orchestrator` as a dependency (which it will in PR-B
//!   when the IPC layer wires the two together).
//!
//! The test parses the workspace's `cargo metadata` output and
//! asserts the forbidden edges are absent. It is fast enough to run
//! in CI without feature flags.

use cargo_metadata::MetadataCommand;

const FORBIDDEN: &[&str] = &["tauri", "tauri-build", "dayseam-desktop"];

#[test]
fn orchestrator_does_not_depend_on_app_or_tauri_crates() {
    let metadata = MetadataCommand::new()
        .exec()
        .expect("cargo metadata must succeed");

    let orch = metadata
        .packages
        .iter()
        .find(|p| p.name == "dayseam-orchestrator")
        .expect("dayseam-orchestrator must be a workspace member");

    let direct_deps: Vec<&str> = orch.dependencies.iter().map(|d| d.name.as_str()).collect();

    for forbidden in FORBIDDEN {
        assert!(
            !direct_deps.contains(forbidden),
            "dayseam-orchestrator must not depend on `{forbidden}` — \
             see tests/no_cross_crate_leak.rs for why.\n\
             Observed direct deps: {direct_deps:?}",
        );
    }
}
