pub mod catalog;
pub mod commands;
pub mod storage;
pub mod tray;
pub mod watcher;

use std::sync::Mutex;

use tauri::{Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_window(app);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .max_file_size(1_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(3))
                .build(),
        )
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = storage::open(&dir.join("nasuaframe.db"))?;
            let saved = storage::filters::list(&conn)?;
            app.manage(Mutex::new(conn));
            let watcher = watcher::Watcher::new(app.handle().clone());
            watcher.reload(saved);
            app.manage(watcher);
            tray::create(app)?;
            Ok(())
        })
        // closing hides to the tray
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_filters,
            commands::add_filter,
            commands::update_filter,
            commands::delete_filter,
            commands::search_items,
            commands::refresh_catalog,
            commands::list_best_orders,
            commands::open_log_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
