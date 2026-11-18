//! the few strings rust shows, the ui has its own in src/lib/locales

use tauri::{AppHandle, Manager};

use crate::commands::Db;
use crate::storage::settings;

pub struct Text {
    pub show: &'static str,
    pub quit: &'static str,
    pub open_to_copy: &'static str,
}

const EN: Text = Text {
    show: "Show",
    quit: "Quit",
    open_to_copy: "open NasuaFrame to copy the whisper",
};

const PT: Text = Text {
    show: "Mostrar",
    quit: "Sair",
    open_to_copy: "abra o NasuaFrame para copiar o whisper",
};

pub fn text(language: &str) -> &'static Text {
    match language {
        "pt" => &PT,
        _ => &EN,
    }
}

pub fn current(app: &AppHandle) -> &'static Text {
    let db = app.state::<Db>();
    let language = settings::load(&db.lock().unwrap())
        .map(|s| s.language)
        .unwrap_or_default();
    text(&language)
}
