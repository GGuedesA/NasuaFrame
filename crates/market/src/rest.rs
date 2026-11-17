//! REST calls for the item catalog and current orders
//!
//! blocking (ureq), so call them from `spawn_blocking` inside async code

use std::collections::HashMap;
use std::fmt;

use nasua_core::model::{Item, Order, Platform};
use serde::Deserialize;

use crate::USER_AGENT;
use crate::client::platform_slug;
use crate::events::WireOrder;

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

/// best sell and buy orders (up to 5 each) from online traders, sells first
///
/// `rank` asks for that exact rank, None gets any rank
pub fn fetch_top_orders(
    slug: &str,
    platform: Platform,
    crossplay: bool,
    rank: Option<u8>,
    subtype: Option<&str>,
) -> Result<Vec<Order>, Error> {
    let mut request = request(&format!("/orders/item/{slug}/top"))
        .header("Platform", platform_slug(platform))
        .header("Crossplay", if crossplay { "true" } else { "false" });
    if let Some(rank) = rank {
        request = request.query("rank", rank.to_string());
    }
    if let Some(subtype) = subtype {
        request = request.query("subtype", subtype);
    }
    parse_top_orders(&read(request)?)
}

fn get(path: &str) -> Result<String, Error> {
    read(request(path))
}

fn request(path: &str) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
    crate::install_tls();
    ureq::get(format!("{REST_BASE}{path}"))
        .header("User-Agent", USER_AGENT)
        .header("Language", "en")
}

fn read(request: ureq::RequestBuilder<ureq::typestate::WithoutBody>) -> Result<String, Error> {
    Ok(request.call()?.body_mut().read_to_string()?)
}

fn parse_items(body: &str) -> Result<Vec<Item>, Error> {
    let envelope: Envelope<Vec<WireItem>> = serde_json::from_str(body)?;
    Ok(envelope.data.into_iter().map(Item::from).collect())
}

fn parse_items_version(body: &str) -> Result<String, Error> {
    let envelope: Envelope<WireVersions> = serde_json::from_str(body)?;
    Ok(envelope.data.collections.items)
}

fn parse_top_orders(body: &str) -> Result<Vec<Order>, Error> {
    let envelope: Envelope<WireTop> = serde_json::from_str(body)?;
    let WireTop { sell, buy } = envelope.data;
    Ok(sell.into_iter().chain(buy).map(Order::from).collect())
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
struct WireTop {
    sell: Vec<WireOrder>,
    buy: Vec<WireOrder>,
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
    use nasua_core::model::{OrderType, Status};

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

    const TOP: &str = r#"{"apiVersion":"0.25.0","data":{"sell":[
        {"id":"000000000000000000000001","type":"sell","platinum":15,"quantity":1,"perTrade":1,"rank":0,"visible":true,"createdAt":"2026-11-17T01:49:03Z","updatedAt":"2026-11-17T01:49:03Z","itemId":"6322918add457f0072aeb8b1",
         "user":{"id":"000000000000000000000002","ingameName":"TestTrader","slug":"testtrader","avatar":"user/avatar/x.webp","reputation":9,"platform":"pc","crossplay":true,"locale":"en","status":"ingame","activity":{"type":"UNKNOWN","details":"unknown","startedAt":"2026-11-17T14:59:13Z"},"lastSeen":"2026-11-17T14:59:13Z"}},
        {"id":"000000000000000000000003","type":"sell","platinum":20,"quantity":1,"perTrade":1,"rank":0,"visible":true,"createdAt":"2026-11-17T12:40:49Z","updatedAt":"2026-11-17T12:40:49Z","itemId":"6322918add457f0072aeb8b1",
         "user":{"id":"000000000000000000000004","ingameName":"TestSeller","slug":"testseller","reputation":0,"platform":"pc","crossplay":true,"locale":"zh-hans","status":"online","activity":{"type":"UNKNOWN","details":"unknown","startedAt":"2026-11-17T13:25:25Z"},"lastSeen":"2026-11-17T13:25:25Z"}}
    ],"buy":[
        {"id":"000000000000000000000005","type":"buy","platinum":62,"quantity":1,"perTrade":1,"rank":10,"visible":true,"createdAt":"2026-11-15T07:10:14Z","updatedAt":"2026-11-15T07:10:14Z","itemId":"6322918add457f0072aeb8b1",
         "user":{"id":"000000000000000000000006","ingameName":"TestBuyer","slug":"testbuyer","reputation":4,"platform":"pc","crossplay":true,"locale":"en","status":"ingame","activity":{"type":"UNKNOWN","details":"unknown","startedAt":"2026-11-17T11:53:45Z"},"lastSeen":"2026-11-17T11:53:45Z"}}
    ]},"error":null}"#;

    #[test]
    fn parses_top_orders_sells_first() {
        let orders = parse_top_orders(TOP).unwrap();
        let summary: Vec<_> = orders
            .iter()
            .map(|o| (o.order_type, o.platinum, o.user.ingame_name.as_str()))
            .collect();
        assert_eq!(
            summary,
            [
                (OrderType::Sell, 15, "TestTrader"),
                (OrderType::Sell, 20, "TestSeller"),
                (OrderType::Buy, 62, "TestBuyer"),
            ]
        );
        assert_eq!(orders[1].user.status, Status::Online);
        assert_eq!(orders[2].rank, Some(10));
        assert_eq!(orders[0].item_id, "6322918add457f0072aeb8b1");
    }

    #[test]
    fn no_orders_is_empty() {
        let body = r#"{"apiVersion":"0.25.0","data":{"sell":[],"buy":[]},"error":null}"#;
        assert!(parse_top_orders(body).unwrap().is_empty());
    }

    #[test]
    fn broken_body_is_an_error() {
        assert!(matches!(
            parse_items("<html>blocked</html>"),
            Err(Error::Parse(_))
        ));
    }
}
