#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use dayseam_db::LogRepo;
use dayseam_desktop::ipc::{broadcast_forwarder, commands};
use dayseam_desktop::startup;
use tauri::Manager;

fn main() {
    // One multi-threaded Tokio runtime powers the whole app: the
    // database pool, the broadcast forwarder, and per-run forwarders
    // all share it. `tauri::async_runtime` wraps the same machinery
    // so there's no second reactor to keep in sync.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    let _guard = runtime.enter();

    let builder = tauri::Builder::default().setup(|app| {
        let data_dir = startup::default_data_dir();
        let state = tauri::async_runtime::block_on(startup::build_app_state(&data_dir))
            .expect("build AppState");
        let pool = state.pool.clone();
        let app_bus = state.app_bus.clone();
        app.manage(state);

        let handle = app.handle().clone();
        let logs = LogRepo::new(pool);
        let _broadcast_task = broadcast_forwarder::spawn(handle, app_bus, logs);
        Ok(())
    });

    // Release builds compile the dev commands out entirely so the
    // binary ships with a minimal IPC surface.
    #[cfg(feature = "dev-commands")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::settings_get,
        commands::settings_update,
        commands::logs_tail,
        commands::dev_emit_toast,
        commands::dev_start_demo_run,
    ]);

    #[cfg(not(feature = "dev-commands"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::settings_get,
        commands::settings_update,
        commands::logs_tail,
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running the Dayseam desktop app");
}
