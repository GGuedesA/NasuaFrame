//! sqlite storage for filters and the item catalog

pub mod filters;
pub mod items;

use std::fmt;
use std::path::Path;

use rusqlite::Connection;

/// append only, never edit one that already shipped
const MIGRATIONS: &[&str] = &[include_str!("../../migrations/001_init.sql")];

#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    Corrupt(String),
    NotFound(i64),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Sqlite(e) => write!(f, "database: {e}"),
            Error::Corrupt(what) => write!(f, "bad value in database: {what}"),
            Error::NotFound(id) => write!(f, "no row with id {id}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sqlite(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Corrupt(e.to_string())
    }
}

pub fn open(path: &Path) -> Result<Connection, Error> {
    let mut conn = Connection::open(path)?;
    migrate(&mut conn)?;
    Ok(conn)
}

#[cfg(test)]
pub fn open_in_memory() -> Result<Connection, Error> {
    let mut conn = Connection::open_in_memory()?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// sqlite's user_version holds how many migrations already ran
fn migrate(conn: &mut Connection) -> Result<(), Error> {
    let done: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let latest = MIGRATIONS.len() as i64;
    // newer means the app was downgraded, running on would mess up their data
    if !(0..=latest).contains(&done) {
        return Err(Error::Corrupt(format!(
            "database schema {done}, this app only knows up to {latest}"
        )));
    }
    let tx = conn.transaction()?;
    for sql in MIGRATIONS.iter().skip(done as usize) {
        tx.execute_batch(sql)?;
    }
    tx.pragma_update(None, "user_version", latest)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_run_once() {
        let mut conn = open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
    }

    #[test]
    fn refuses_a_db_from_a_newer_app() {
        let mut conn = open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", MIGRATIONS.len() as i64 + 1)
            .unwrap();
        assert!(matches!(migrate(&mut conn), Err(Error::Corrupt(_))));
    }
}
