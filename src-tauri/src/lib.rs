pub mod catalog;
pub mod commands;
pub mod scan;
pub mod storage;
pub mod text;
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
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let db = window.state::<commands::Db>();
                // a broken settings row still keeps the app alive in the tray
                let to_tray =
                    storage::settings::load(&db.lock().unwrap()).map_or(true, |s| s.close_to_tray);
                if to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    window.app_handle().exit(0);
                }
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
            commands::get_settings,
            commands::save_settings,
            commands::open_log_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
