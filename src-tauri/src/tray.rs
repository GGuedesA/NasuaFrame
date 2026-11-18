//! tray icon, the app keeps watching while the window is hidden

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Wry};

use crate::text;

const TRAY_ID: &str = "main";

pub fn create(app: &App) -> tauri::Result<()> {
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("NasuaFrame")
        .menu(&menu(app.handle())?)
        // left click shows the window, the menu is on right click
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// after a language change
pub fn update_menu(app: &AppHandle) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(menu(app)?))?;
    }
    Ok(())
}

fn menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let text = text::current(app);
    let show = MenuItem::with_id(app, "show", text.show, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", text.quit, true, None::<&str>)?;
    Menu::with_items(app, &[&show, &quit])
}

pub fn show_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    // each step can fail on some desktops, the others still help
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}
