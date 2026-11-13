pub mod catalog;
pub mod storage;

use std::sync::Mutex;

use tauri::Manager;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

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

            let handle = app.handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                let db = handle.state::<Mutex<rusqlite::Connection>>();
                // offline is fine, the old cache keeps working
                match catalog::refresh(&db) {
                    Ok(outcome) => eprintln!("catalog: {outcome:?}"),
                    Err(e) => eprintln!("catalog refresh failed: {e}"),
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
