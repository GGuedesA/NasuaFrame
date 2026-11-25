//! cached item catalog + version it came from

use nasua_core::model::Item;
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::Error;

const VERSION_KEY: &str = "items_version";
const LANGUAGE_KEY: &str = "items_language";

/// swaps the whole catalog in one transaction, a crash halfway keeps the old one
pub fn replace_all(
    conn: &mut Connection,
    items: &[Item],
    version: &str,
    language: &str,
) -> Result<(), Error> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM items", [])?;
    {
        let mut insert = tx.prepare(
            "INSERT INTO items (id, slug, name, name_en, max_rank, subtypes, ducats, game_ref)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        for item in items {
            let subtypes = serde_json::to_string(&item.subtypes)?;
            insert.execute(params![
                item.id,
                item.slug,
                item.name,
                item.name_en,
                item.max_rank,
                subtypes,
                item.ducats,
                item.game_ref
            ])?;
        }
    }
    for (key, value) in [(VERSION_KEY, version), (LANGUAGE_KEY, language)] {
        tx.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn load_all(conn: &Connection) -> Result<Vec<Item>, Error> {
    select(conn, "", -1)
}

pub fn search(conn: &Connection, query: &str, limit: u32) -> Result<Vec<Item>, Error> {
    select(conn, query, limit.into())
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<Item>, Error> {
    let found = conn
        .query_row(
            "SELECT id, slug, name, name_en, max_rank, subtypes, ducats, game_ref FROM items WHERE id = ?1",
            [id],
            read_row,
        )
        .optional()?;
    found.map(with_subtypes).transpose()
}

fn select(conn: &Connection, query: &str, limit: i64) -> Result<Vec<Item>, Error> {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let mut stmt = conn.prepare(
        "SELECT id, slug, name, name_en, max_rank, subtypes, ducats, game_ref FROM items
         WHERE name LIKE '%' || ?1 || '%' ESCAPE '\\' ORDER BY name LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![escaped, limit], read_row)?;
    rows.map(|row| with_subtypes(row?)).collect()
}

/// subtypes stay as json text here, the closure can only return sqlite errors
fn read_row(row: &Row) -> rusqlite::Result<(Item, String)> {
    let item = Item {
        id: row.get(0)?,
        slug: row.get(1)?,
        name: row.get(2)?,
        name_en: row.get(3)?,
        max_rank: row.get(4)?,
        subtypes: Vec::new(),
        ducats: row.get(6)?,
        game_ref: row.get(7)?,
    };
    Ok((item, row.get(5)?))
}

fn with_subtypes((mut item, subtypes): (Item, String)) -> Result<Item, Error> {
    item.subtypes = serde_json::from_str(&subtypes)?;
    Ok(item)
}

/// None until the first successful download
pub fn cached_version(conn: &Connection) -> Result<Option<String>, Error> {
    meta(conn, VERSION_KEY)
}

pub fn cached_language(conn: &Connection) -> Result<Option<String>, Error> {
    meta(conn, LANGUAGE_KEY)
}

fn meta(conn: &Connection, key: &str) -> Result<Option<String>, Error> {
    let value = conn
        .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory;

    fn item(id: &str, name: &str, max_rank: Option<u8>, subtypes: &[&str]) -> Item {
        Item {
            id: id.into(),
            slug: name.to_lowercase().replace(' ', "_"),
            name: name.into(),
            name_en: format!("{name} en"),
            max_rank,
            subtypes: subtypes.iter().map(|s| s.to_string()).collect(),
            ducats: None,
            game_ref: String::new(),
        }
    }

    #[test]
    fn round_trip() {
        let mut conn = open_in_memory().unwrap();
        let mut items = vec![
            item("1", "Archon Intensify", Some(10), &[]),
            item("3", "Braton Prime Barrel", None, &[]),
            item("2", "Neo A1 Relic", None, &["intact", "radiant"]),
        ];
        items[1].ducats = Some(15);
        items[1].game_ref = "/Lotus/Types/Recipes/Weapons/WeaponParts/BratonPrimeBarrel".into();
        replace_all(&mut conn, &items, "v1", "pt").unwrap();
        assert_eq!(load_all(&conn).unwrap(), items);
        assert_eq!(cached_version(&conn).unwrap().as_deref(), Some("v1"));
        assert_eq!(cached_language(&conn).unwrap().as_deref(), Some("pt"));
        assert_eq!(get(&conn, "3").unwrap().as_ref(), Some(&items[1]));
        assert_eq!(get(&conn, "nope").unwrap(), None);
    }

    #[test]
    fn replace_drops_old_items_and_updates_version() {
        let mut conn = open_in_memory().unwrap();
        replace_all(&mut conn, &[item("1", "Old Mod", Some(5), &[])], "v1", "en").unwrap();
        let fresh = vec![item("2", "New Mod", Some(3), &[])];
        replace_all(&mut conn, &fresh, "v2", "en").unwrap();
        assert_eq!(load_all(&conn).unwrap(), fresh);
        assert_eq!(cached_version(&conn).unwrap().as_deref(), Some("v2"));
    }

    #[test]
    fn search_is_case_insensitive_and_literal() {
        let mut conn = open_in_memory().unwrap();
        let items = vec![
            item("1", "Archon Intensify", Some(10), &[]),
            item("2", "Intensify", Some(5), &[]),
            item("3", "Serration", Some(10), &[]),
            item("4", "100% Tax", None, &[]),
        ];
        replace_all(&mut conn, &items, "v1", "en").unwrap();
        let names = |q, limit| -> Vec<String> {
            search(&conn, q, limit)
                .unwrap()
                .into_iter()
                .map(|i| i.name)
                .collect()
        };
        assert_eq!(names("intensify", 10), ["Archon Intensify", "Intensify"]);
        assert_eq!(names("INTENSIFY", 1), ["Archon Intensify"]);
        assert_eq!(names("%", 10), ["100% Tax"]);
        assert!(names("_", 10).is_empty());
    }

    #[test]
    fn no_version_before_first_download() {
        let conn = open_in_memory().unwrap();
        assert_eq!(cached_version(&conn).unwrap(), None);
        assert_eq!(cached_language(&conn).unwrap(), None);
        assert!(load_all(&conn).unwrap().is_empty());
    }
}
