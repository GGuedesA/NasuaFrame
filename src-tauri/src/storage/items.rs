//! cached item catalog + version it came from

use nasua_core::model::Item;
use rusqlite::{Connection, OptionalExtension, params};

use super::Error;

const VERSION_KEY: &str = "items_version";

/// swaps the whole catalog in one transaction, a crash halfway keeps the old one
pub fn replace_all(conn: &mut Connection, items: &[Item], version: &str) -> Result<(), Error> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM items", [])?;
    {
        let mut insert = tx.prepare(
            "INSERT INTO items (id, slug, name, max_rank, subtypes) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for item in items {
            let subtypes = serde_json::to_string(&item.subtypes)?;
            insert.execute(params![
                item.id,
                item.slug,
                item.name,
                item.max_rank,
                subtypes
            ])?;
        }
    }
    tx.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![VERSION_KEY, version],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn load_all(conn: &Connection) -> Result<Vec<Item>, Error> {
    select(conn, "", -1)
}

pub fn search(conn: &Connection, query: &str, limit: u32) -> Result<Vec<Item>, Error> {
    select(conn, query, limit.into())
}

fn select(conn: &Connection, query: &str, limit: i64) -> Result<Vec<Item>, Error> {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let mut stmt = conn.prepare(
        "SELECT id, slug, name, max_rank, subtypes FROM items
         WHERE name LIKE '%' || ?1 || '%' ESCAPE '\\' ORDER BY name LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![escaped, limit], |row| {
        let item = Item {
            id: row.get(0)?,
            slug: row.get(1)?,
            name: row.get(2)?,
            max_rank: row.get(3)?,
            subtypes: Vec::new(),
        };
        Ok((item, row.get::<_, String>(4)?))
    })?;
    rows.map(|row| {
        let (mut item, subtypes) = row?;
        item.subtypes = serde_json::from_str(&subtypes)?;
        Ok(item)
    })
    .collect()
}

/// None until the first successful download
pub fn cached_version(conn: &Connection) -> Result<Option<String>, Error> {
    let version = conn
        .query_row(
            "SELECT value FROM meta WHERE key = ?1",
            [VERSION_KEY],
            |row| row.get(0),
        )
        .optional()?;
    Ok(version)
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
            max_rank,
            subtypes: subtypes.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn round_trip() {
        let mut conn = open_in_memory().unwrap();
        let items = vec![
            item("1", "Archon Intensify", Some(10), &[]),
            item("2", "Neo A1 Relic", None, &["intact", "radiant"]),
        ];
        replace_all(&mut conn, &items, "v1").unwrap();
        assert_eq!(load_all(&conn).unwrap(), items);
        assert_eq!(cached_version(&conn).unwrap().as_deref(), Some("v1"));
    }

    #[test]
    fn replace_drops_old_items_and_updates_version() {
        let mut conn = open_in_memory().unwrap();
        replace_all(&mut conn, &[item("1", "Old Mod", Some(5), &[])], "v1").unwrap();
        let fresh = vec![item("2", "New Mod", Some(3), &[])];
        replace_all(&mut conn, &fresh, "v2").unwrap();
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
        replace_all(&mut conn, &items, "v1").unwrap();
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
        assert!(load_all(&conn).unwrap().is_empty());
    }
}
