//! Process-wide state held by the Tauri runtime.
//!
//! Everything the IPC layer needs to serve a command lives here: the
//! single SQLite pool, the app-wide broadcast bus, the secret store,
//! and the per-run registry that tracks which sync runs are currently
//! streaming events to the frontend.
//!
//! `AppState` is owned by Tauri via [`tauri::Manager::manage`] and is
//! accessed from every `#[tauri::command]` through
//! `tauri::State<'_, AppState>`. That's the only way state leaks out
//! of this module.

use std::collections::HashMap;
use std::sync::Arc;

use dayseam_core::RunId;
use dayseam_events::AppBus;
use dayseam_secrets::SecretStore;
use sqlx::SqlitePool;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Per-run bookkeeping used by the IPC layer: the cancellation token
/// that aborts the run, and the task handles spawned to pump the
/// per-run streams into Tauri `Channel<T>`s.
#[derive(Debug)]
pub struct RunHandle {
    pub run_id: RunId,
    pub cancel: CancellationToken,
    /// Forwarder tasks (progress + log + producer). Held onto so the
    /// registry can `await` or `abort` them on shutdown.
    pub tasks: Vec<JoinHandle<()>>,
}

/// Registry of currently-live sync runs keyed by [`RunId`].
///
/// Registration happens when a command that starts a run (for Phase 1
/// that's `dev_start_demo_run`; Phase 2 adds `run_start`) allocates a
/// fresh `RunStreams` and spawns forwarder tasks. Deregistration
/// happens when those forwarders observe their receivers returning
/// `None`, which is how run completion is signalled end-to-end.
#[derive(Debug, Default)]
pub struct RunRegistry {
    runs: HashMap<RunId, RunHandle>,
}

impl RunRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a new run handle. Returns the previous handle for
    /// `run_id` if one existed — which should never happen given
    /// `RunId::new()` uses v4 UUIDs, but surfacing the collision is
    /// cheaper than silently clobbering the prior run.
    pub fn insert(&mut self, handle: RunHandle) -> Option<RunHandle> {
        self.runs.insert(handle.run_id, handle)
    }

    pub fn remove(&mut self, run_id: &RunId) -> Option<RunHandle> {
        self.runs.remove(run_id)
    }

    #[must_use]
    pub fn contains(&self, run_id: &RunId) -> bool {
        self.runs.contains_key(run_id)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.runs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// Request every live run to cancel. Used on app shutdown so
    /// in-flight forwarders observe the cancellation signal and exit
    /// promptly instead of stalling the quit.
    pub fn cancel_all(&self) {
        for handle in self.runs.values() {
            handle.cancel.cancel();
        }
    }
}

/// Process-wide state. Cheap to share — every field is either cheaply
/// cloneable (`SqlitePool`, `AppBus`) or behind an `Arc` / `RwLock`.
pub struct AppState {
    pub pool: SqlitePool,
    pub app_bus: AppBus,
    pub secrets: Arc<dyn SecretStore>,
    pub runs: RwLock<RunRegistry>,
}

impl AppState {
    /// Construct an [`AppState`] from its collaborators. Keep this a
    /// plain constructor — wiring the pool and the keychain is the
    /// responsibility of [`crate::startup`].
    #[must_use]
    pub fn new(pool: SqlitePool, app_bus: AppBus, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            pool,
            app_bus,
            secrets,
            runs: RwLock::new(RunRegistry::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle(run_id: RunId) -> RunHandle {
        RunHandle {
            run_id,
            cancel: CancellationToken::new(),
            tasks: Vec::new(),
        }
    }

    #[test]
    fn insert_then_remove_round_trips() {
        let mut reg = RunRegistry::new();
        let id = RunId::new();
        reg.insert(handle(id));
        assert!(reg.contains(&id));
        assert_eq!(reg.len(), 1);
        assert!(reg.remove(&id).is_some());
        assert!(reg.is_empty());
    }

    #[test]
    fn cancel_all_flips_every_token() {
        let mut reg = RunRegistry::new();
        let a = handle(RunId::new());
        let b = handle(RunId::new());
        let tok_a = a.cancel.clone();
        let tok_b = b.cancel.clone();
        reg.insert(a);
        reg.insert(b);

        reg.cancel_all();
        assert!(tok_a.is_cancelled());
        assert!(tok_b.is_cancelled());
    }
}
