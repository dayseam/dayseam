//! Dayseam desktop app library root.
//!
//! The desktop crate is split into a thin `main.rs` binary and a
//! testable library (this file) so integration tests can exercise the
//! IPC plumbing without booting an actual Tauri runtime.

/// Compile-time distribution profile (`direct` vs `mas`).
///
/// **MAS-1a:** `mas` is enabled only for Mac App Store bundle builds
/// (`--features mas` + `tauri.mas.conf.json` merge). The default release
/// binary is still the direct-download SKU.
#[cfg(feature = "mas")]
pub const DISTRIBUTION_PROFILE: &str = "mas";
#[cfg(not(feature = "mas"))]
pub const DISTRIBUTION_PROFILE: &str = "direct";

pub mod ipc;
pub mod keychain_profile;
pub mod local_git_scan;
pub mod oauth_config;
pub mod oauth_persister;
pub mod oauth_session;
pub mod scheduler_task;
pub mod security_scoped;
pub mod startup;
pub mod state;
pub mod tracing_init;

pub use oauth_session::{OAuthSession, OAuthSessionRegistry};
pub use state::{AppState, RunHandle, RunRegistry};

#[cfg(test)]
mod distribution_tests {
    #[test]
    fn distribution_profile_is_known_sku() {
        assert!(
            matches!(super::DISTRIBUTION_PROFILE, "direct" | "mas"),
            "unexpected DISTRIBUTION_PROFILE: {}",
            super::DISTRIBUTION_PROFILE
        );
    }
}
