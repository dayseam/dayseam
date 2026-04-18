//! Hook app-defined Tauri commands into the permission system.
//!
//! Tauri 2 denies every command unless it is listed both in the
//! runtime `invoke_handler!` and in a capability's `permissions` array
//! as `allow-<command-name>`. Declaring the command set on the
//! [`tauri_build::AppManifest`] makes `tauri-build` autogenerate the
//! matching permission files under the crate's `OUT_DIR`, so adding a
//! new command in `src/ipc/commands.rs` only requires touching three
//! places:
//!
//!   1. the `#[tauri::command]` function,
//!   2. the list below, and
//!   3. `capabilities/default.json`.
//!
//! Any of the three missing surfaces a loud error — either at compile
//! time (capability entry references an unknown permission) or at
//! runtime (webview call for a command not in the handler).

fn main() {
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "settings_get",
            "settings_update",
            "logs_tail",
            #[cfg(feature = "dev-commands")]
            "dev_emit_toast",
            #[cfg(feature = "dev-commands")]
            "dev_start_demo_run",
        ]));
    tauri_build::try_build(attributes).expect("tauri-build failed");
}
