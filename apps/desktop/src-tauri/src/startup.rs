//! App startup helpers — everything that needs to happen exactly once
//! between "Tauri is about to call `setup`" and "the window is
//! allowed to make IPC calls".
//!
//! Factored out of `main.rs` so integration tests can exercise the
//! same code path without running a real Tauri runtime.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dayseam_core::{DayseamError, LogLevel};
use dayseam_db::{open, LogRepo, LogRow};
use dayseam_events::AppBus;
use dayseam_secrets::{KeychainStore, SecretStore};
use sqlx::SqlitePool;

use crate::state::AppState;

/// Fixed subdirectory inside the OS "app data" dir that Dayseam owns.
/// Matches the Tauri bundle identifier prefix so multiple installs
/// (stable, alpha, custom) can coexist without stepping on one
/// another.
const DATA_SUBDIR: &str = "dev.dayseam.desktop";
const DB_FILENAME: &str = "state.db";

/// Resolve the per-platform application-data directory Dayseam writes
/// to. Uses the same logic as Tauri so the database sits next to the
/// updater cache, the logs, and anything else the runtime may add in
/// a future phase.
///
/// Falls back to `./<DATA_SUBDIR>/` when no platform directory can be
/// resolved (should only happen in very unusual headless CI setups).
#[must_use]
pub fn default_data_dir() -> PathBuf {
    if let Some(base) = dirs_like_app_data() {
        return base.join(DATA_SUBDIR);
    }
    PathBuf::from(DATA_SUBDIR)
}

#[cfg(target_os = "macos")]
fn dirs_like_app_data() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn dirs_like_app_data() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        return Some(PathBuf::from(xdg));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
}

#[cfg(target_os = "windows")]
fn dirs_like_app_data() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

#[cfg(not(any(unix, target_os = "windows")))]
fn dirs_like_app_data() -> Option<PathBuf> {
    None
}

/// Build an [`AppState`] from a data directory. Creates the directory
/// if missing, opens the database (running migrations), writes a
/// single startup log row so the empty-state of the log drawer is
/// informative ("Dayseam started at {ts}"), and returns the populated
/// state.
pub async fn build_app_state(data_dir: &Path) -> Result<AppState, DayseamError> {
    tokio::fs::create_dir_all(data_dir)
        .await
        .map_err(|e| DayseamError::Io {
            code: "startup.data_dir".into(),
            path: Some(data_dir.to_path_buf()),
            message: e.to_string(),
        })?;

    let pool = open(&data_dir.join(DB_FILENAME))
        .await
        .map_err(|e| DayseamError::Io {
            code: "startup.db_open".into(),
            path: Some(data_dir.join(DB_FILENAME)),
            message: e.to_string(),
        })?;

    record_startup_log(&pool).await;

    let app_bus = AppBus::new();
    let secrets: Arc<dyn SecretStore> = Arc::new(KeychainStore::new());
    Ok(AppState::new(pool, app_bus, secrets))
}

async fn record_startup_log(pool: &SqlitePool) {
    let repo = LogRepo::new(pool.clone());
    // Best-effort — a startup log failing to write is not worth
    // refusing to boot. The next successful write still gives the user
    // a sensible log drawer.
    let _ = repo
        .append(&LogRow {
            ts: chrono::Utc::now(),
            level: LogLevel::Info,
            source_id: None,
            message: "Dayseam started".into(),
            context: Some(serde_json::json!({ "source": "startup" })),
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn build_app_state_writes_the_startup_log_entry() {
        let dir = TempDir::new().expect("temp dir");
        let state = build_app_state(dir.path()).await.expect("build state");
        let repo = LogRepo::new(state.pool.clone());
        let rows = repo
            .tail(chrono::DateTime::<chrono::Utc>::MIN_UTC, 10)
            .await
            .expect("tail");
        assert!(
            rows.iter().any(|r| r.message == "Dayseam started"),
            "startup log missing: {:?}",
            rows.iter().map(|r| &r.message).collect::<Vec<_>>()
        );
    }
}
