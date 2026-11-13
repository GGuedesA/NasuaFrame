pub mod catalog;
pub mod commands;
pub mod storage;

use std::sync::Mutex;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = storage::open(&dir.join("nasuaframe.db"))?;
            app.manage(Mutex::new(conn));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_filters,
            commands::add_filter,
            commands::update_filter,
            commands::delete_filter,
            commands::search_items,
            commands::refresh_catalog,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
