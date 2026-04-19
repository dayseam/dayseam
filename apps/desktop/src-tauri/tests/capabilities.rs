//! Capability parity tests.
//!
//! Tauri 2 denies every command that is not listed in the active
//! capability's `permissions` array. This is great when we remember
//! to add new entries and painful when we forget — the symptom is a
//! silent "command not allowed" at runtime. These tests fail in CI
//! the moment the capability file falls out of step with the
//! `#[tauri::command]` surface declared in
//! [`dayseam_desktop::ipc::commands::PROD_COMMANDS`] /
//! [`dayseam_desktop::ipc::commands::DEV_COMMANDS`].
//!
//! The check is symmetric: every command must have a matching
//! `allow-*` permission, and every `allow-*` permission must map to
//! a command we actually ship. That catches both kinds of drift —
//! adding a command without granting it, and leaving a stale permit
//! behind after deleting a command.

use std::collections::BTreeSet;

use dayseam_desktop::ipc::commands::{DEV_COMMANDS, PROD_COMMANDS};
use serde::Deserialize;

/// Matches the subset of the Tauri capability schema we care about
/// here. `serde` ignores unknown fields by default so the schema can
/// grow without breaking the test.
#[derive(Debug, Deserialize)]
struct Capability {
    permissions: Vec<String>,
}

fn load_capability(relative_path: &str) -> Capability {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

/// `snake_case` command identifier -> `allow-kebab-case` permission
/// identifier. The `tauri-build` autogenerator produces permits with
/// underscores rewritten to dashes — the JSON files have to match.
fn allow_permission(command: &str) -> String {
    format!("allow-{}", command.replace('_', "-"))
}

#[test]
fn default_capability_covers_every_production_command() {
    let capability = load_capability("capabilities/default.json");
    let granted: BTreeSet<String> = capability.permissions.into_iter().collect();

    let mut missing = Vec::new();
    for command in PROD_COMMANDS {
        let expected = allow_permission(command);
        if !granted.contains(&expected) {
            missing.push(format!("`{command}` (expected permit `{expected}`)"));
        }
    }
    assert!(
        missing.is_empty(),
        "capabilities/default.json does not grant these production commands: {}",
        missing.join(", ")
    );

    let expected_allows: BTreeSet<String> =
        PROD_COMMANDS.iter().map(|c| allow_permission(c)).collect();
    let stale: Vec<_> = granted
        .iter()
        .filter(|p| p.starts_with("allow-"))
        .filter(|p| !expected_allows.contains(p.as_str()))
        .cloned()
        .collect();
    assert!(
        stale.is_empty(),
        "capabilities/default.json grants commands that no longer exist: {stale:?}"
    );
}

/// Dev capability is written by `build.rs` only when the
/// `dev-commands` feature is on, so the test is gated on the same
/// flag. Without the gate the file isn't on disk during a plain
/// `cargo test` and the harness would fail to load it.
#[cfg(feature = "dev-commands")]
#[test]
fn dev_capability_covers_every_dev_command() {
    let capability = load_capability("capabilities/dev.json");
    let granted: BTreeSet<String> = capability.permissions.into_iter().collect();

    let mut missing = Vec::new();
    for command in DEV_COMMANDS {
        let expected = allow_permission(command);
        if !granted.contains(&expected) {
            missing.push(format!("`{command}` (expected permit `{expected}`)"));
        }
    }
    assert!(
        missing.is_empty(),
        "capabilities/dev.json does not grant these dev commands: {}",
        missing.join(", ")
    );
}

/// Even when the `dev-commands` feature is off we still want the
/// `DEV_COMMANDS` list referenced somewhere so the symbol doesn't
/// get stripped and the parity invariant is preserved at the source
/// level. This check is a no-op but anchors the constant.
#[test]
fn dev_commands_list_is_nonempty() {
    assert!(!DEV_COMMANDS.is_empty());
}
