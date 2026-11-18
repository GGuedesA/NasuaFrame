//! keeps the cached item catalog in sync with warframe.market

use std::fmt;
use std::sync::Mutex;

use nasua_core::model::Item;
use nasua_market::rest;
use rusqlite::Connection;

use crate::storage::{self, items, settings};

#[derive(Debug, PartialEq)]
pub enum Outcome {
    UpToDate,
    Updated(usize),
}

#[derive(Debug)]
pub enum Error {
    Fetch(rest::Error),
    Storage(storage::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Fetch(e) => write!(f, "catalog download: {e}"),
            Error::Storage(e) => write!(f, "catalog cache: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rest::Error> for Error {
    fn from(e: rest::Error) -> Self {
        Error::Fetch(e)
    }
}

impl From<storage::Error> for Error {
    fn from(e: storage::Error) -> Self {
        Error::Storage(e)
    }
}

/// blocking, run it in spawn_blocking
pub fn refresh(db: &Mutex<Connection>) -> Result<Outcome, Error> {
    let language = settings::load(&db.lock().unwrap())?.language;
    refresh_with(db, &language, rest::fetch_items_version, rest::fetch_items)
}

/// lock is never held during a download so the ui isn't stuck waiting on the network
fn refresh_with(
    db: &Mutex<Connection>,
    language: &str,
    fetch_version: impl FnOnce() -> Result<String, rest::Error>,
    fetch_items: impl FnOnce(&str) -> Result<Vec<Item>, rest::Error>,
) -> Result<Outcome, Error> {
    let (cached, cached_language) = {
        let conn = db.lock().unwrap();
        (
            items::cached_version(&conn)?,
            items::cached_language(&conn)?,
        )
    };
    let latest = fetch_version()?;
    // the version is the same for every language
    if cached.as_deref() == Some(latest.as_str()) && cached_language.as_deref() == Some(language) {
        return Ok(Outcome::UpToDate);
    }
    let fresh = fetch_items(language)?;
    items::replace_all(&mut db.lock().unwrap(), &fresh, &latest, language)?;
    Ok(Outcome::Updated(fresh.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory;

    fn item(id: &str) -> Item {
        Item {
            id: id.into(),
            slug: format!("item_{id}"),
            name: format!("Item {id}"),
            name_en: format!("Item {id}"),
            max_rank: None,
            subtypes: Vec::new(),
        }
    }

    fn network_down() -> rest::Error {
        rest::Error::Parse(serde_json::from_str::<()>("offline").unwrap_err())
    }

    fn db_with(version: &str, cached: &[Item]) -> Mutex<Connection> {
        let mut conn = open_in_memory().unwrap();
        items::replace_all(&mut conn, cached, version, "en").unwrap();
        Mutex::new(conn)
    }

    #[test]
    fn first_run_downloads_everything() {
        let db = Mutex::new(open_in_memory().unwrap());
        let outcome = refresh_with(&db, "en", || Ok("v1".into()), |_| Ok(vec![item("1")])).unwrap();
        assert_eq!(outcome, Outcome::Updated(1));
        let conn = db.lock().unwrap();
        assert_eq!(items::load_all(&conn).unwrap(), vec![item("1")]);
        assert_eq!(items::cached_version(&conn).unwrap().as_deref(), Some("v1"));
    }

    #[test]
    fn same_version_skips_the_download() {
        let db = db_with("v1", &[item("1")]);
        let outcome = refresh_with(
            &db,
            "en",
            || Ok("v1".into()),
            |_| panic!("should not download"),
        )
        .unwrap();
        assert_eq!(outcome, Outcome::UpToDate);
    }

    #[test]
    fn new_version_replaces_the_cache() {
        let db = db_with("v1", &[item("1")]);
        let outcome = refresh_with(&db, "en", || Ok("v2".into()), |_| Ok(vec![item("2")])).unwrap();
        assert_eq!(outcome, Outcome::Updated(1));
        let conn = db.lock().unwrap();
        assert_eq!(items::load_all(&conn).unwrap(), vec![item("2")]);
        assert_eq!(items::cached_version(&conn).unwrap().as_deref(), Some("v2"));
    }

    #[test]
    fn language_change_downloads_again() {
        let db = db_with("v1", &[item("1")]);
        let outcome = refresh_with(
            &db,
            "pt",
            || Ok("v1".into()),
            |language| {
                assert_eq!(language, "pt");
                Ok(vec![item("2")])
            },
        )
        .unwrap();
        assert_eq!(outcome, Outcome::Updated(1));
        let conn = db.lock().unwrap();
        assert_eq!(items::load_all(&conn).unwrap(), vec![item("2")]);
        assert_eq!(
            items::cached_language(&conn).unwrap().as_deref(),
            Some("pt")
        );
    }

    #[test]
    fn network_error_keeps_the_old_cache() {
        let db = db_with("v1", &[item("1")]);
        let result = refresh_with(&db, "en", || Ok("v2".into()), |_| Err(network_down()));
        assert!(matches!(result, Err(Error::Fetch(_))));
        let conn = db.lock().unwrap();
        assert_eq!(items::load_all(&conn).unwrap(), vec![item("1")]);
        assert_eq!(items::cached_version(&conn).unwrap().as_deref(), Some("v1"));
    }
}
