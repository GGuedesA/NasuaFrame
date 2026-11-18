//! app settings, kept as one json value in meta

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::Error;

const KEY: &str = "settings";

/// fields missing from an older save get their default
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// warframe.market language code, used for the ui and item names
    pub language: String,
    /// closing the window hides it instead of quitting
    pub close_to_tray: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            language: "en".into(),
            close_to_tray: true,
        }
    }
}

pub fn load(conn: &Connection) -> Result<Settings, Error> {
    let saved: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = ?1", [KEY], |row| {
            row.get(0)
        })
        .optional()?;
    match saved {
        Some(json) => Ok(serde_json::from_str(&json)?),
        None => Ok(Settings::default()),
    }
}

pub fn save(conn: &Connection, settings: &Settings) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![KEY, serde_json::to_string(settings)?],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory;

    #[test]
    fn defaults_until_saved() {
        let conn = open_in_memory().unwrap();
        assert_eq!(load(&conn).unwrap(), Settings::default());
    }

    #[test]
    fn round_trip() {
        let conn = open_in_memory().unwrap();
        let settings = Settings {
            language: "pt".into(),
            close_to_tray: false,
        };
        save(&conn, &settings).unwrap();
        save(&conn, &settings).unwrap();
        assert_eq!(load(&conn).unwrap(), settings);
    }

    #[test]
    fn missing_fields_get_defaults() {
        let conn = open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)",
            params![KEY, r#"{"language":"pt"}"#],
        )
        .unwrap();
        let loaded = load(&conn).unwrap();
        assert_eq!(loaded.language, "pt");
        assert!(loaded.close_to_tray);
    }
}
