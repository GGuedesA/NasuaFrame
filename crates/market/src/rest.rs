//! REST calls for the item catalog
//!
//! blocking (ureq), so call them from `spawn_blocking` inside async code

use std::collections::HashMap;
use std::fmt;

use nasua_core::model::Item;
use serde::Deserialize;

use crate::USER_AGENT;

const REST_BASE: &str = "https://api.warframe.market/v2";

#[derive(Debug)]
pub enum Error {
    Http(Box<ureq::Error>),
    Parse(serde_json::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Http(e) => write!(f, "request failed: {e}"),
            Error::Parse(e) => write!(f, "unexpected response: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<ureq::Error> for Error {
    fn from(e: ureq::Error) -> Self {
        Error::Http(Box::new(e))
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Parse(e)
    }
}

/// every tradable item, big download
pub fn fetch_items() -> Result<Vec<Item>, Error> {
    parse_items(&get("/items")?)
}

/// changes whenever the catalog changes, compare it with the cached one before refetching
pub fn fetch_items_version() -> Result<String, Error> {
    parse_items_version(&get("/versions")?)
}

fn get(path: &str) -> Result<String, Error> {
    crate::install_tls();
    let body = ureq::get(format!("{REST_BASE}{path}"))
        .header("User-Agent", USER_AGENT)
        .header("Language", "en")
        .call()?
        .body_mut()
        .read_to_string()?;
    Ok(body)
}

fn parse_items(body: &str) -> Result<Vec<Item>, Error> {
    let envelope: Envelope<Vec<WireItem>> = serde_json::from_str(body)?;
    Ok(envelope.data.into_iter().map(Item::from).collect())
}

fn parse_items_version(body: &str) -> Result<String, Error> {
    let envelope: Envelope<WireVersions> = serde_json::from_str(body)?;
    Ok(envelope.data.collections.items)
}

#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireItem {
    id: String,
    slug: String,
    max_rank: Option<u8>,
    #[serde(default)]
    subtypes: Vec<String>,
    #[serde(default)]
    i18n: HashMap<String, WireI18n>,
}

#[derive(Deserialize)]
struct WireI18n {
    name: String,
}

#[derive(Deserialize)]
struct WireVersions {
    collections: WireCollections,
}

#[derive(Deserialize)]
struct WireCollections {
    items: String,
}

impl From<WireItem> for Item {
    fn from(mut w: WireItem) -> Self {
        // slug as a fallback so one item missing its name doesn't break the catalog
        let name = w
            .i18n
            .remove("en")
            .map_or_else(|| w.slug.clone(), |en| en.name);
        Item {
            id: w.id,
            slug: w.slug,
            name,
            max_rank: w.max_rank,
            subtypes: w.subtypes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: &str = r#"{"apiVersion":"0.0.0","error":null,"data":[
        {"id":"6322918add457f0072aeb8b1","slug":"archon_intensify","gameRef":"/x","tags":["mod"],"maxRank":10,
         "i18n":{"en":{"name":"Archon Intensify","icon":"a.png","thumb":"a.png"}}},
        {"id":"000000000000000000000002","slug":"neo_a1_relic","gameRef":"/y","tags":["relic"],
         "subtypes":["intact","exceptional","flawless","radiant"],
         "i18n":{"en":{"name":"Neo A1 Relic","icon":"b.png","thumb":"b.png"}}},
        {"id":"000000000000000000000003","slug":"no_name_item","gameRef":"/z","tags":[]}
    ]}"#;

    #[test]
    fn parses_items() {
        let items = parse_items(ITEMS).unwrap();
        assert_eq!(
            items[0],
            Item {
                id: "6322918add457f0072aeb8b1".into(),
                slug: "archon_intensify".into(),
                name: "Archon Intensify".into(),
                max_rank: Some(10),
                subtypes: vec![],
            }
        );
        assert_eq!(items[1].max_rank, None);
        assert_eq!(
            items[1].subtypes,
            ["intact", "exceptional", "flawless", "radiant"]
        );
    }

    #[test]
    fn missing_name_falls_back_to_slug() {
        let items = parse_items(ITEMS).unwrap();
        assert_eq!(items[2].name, "no_name_item");
    }

    #[test]
    fn parses_items_version() {
        let body = r#"{"apiVersion":"0.0.0","error":null,"data":{"apps":{},"collections":{"items":"aGFzaA==","rivens":"x"},"updatedAt":"2026-11-12T00:00:00Z"}}"#;
        assert_eq!(parse_items_version(body).unwrap(), "aGFzaA==");
    }

    #[test]
    fn broken_body_is_an_error() {
        assert!(matches!(
            parse_items("<html>blocked</html>"),
            Err(Error::Parse(_))
        ));
    }
}
