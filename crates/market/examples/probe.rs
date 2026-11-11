//! Protocol probe for warframe.market.
//!
//! Run: `cargo run -p nasua-market --example probe -- [seconds] [item-slug] [max-price]`
//! (defaults: 300 seconds, `archon_intensify`, 150).
//!
//! It subscribes to the global new-orders stream, prints the first raw
//! message of each route, and highlights orders for the watched item. Sell orders at or
//! below `max-price` are flagged as matches, like a buyer's alert would be.

use std::collections::BTreeMap;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::time::{Instant, timeout_at};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

const USER_AGENT: &str = concat!(
    "NasuaFrame-probe/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/GGuedesA/NasuaFrame)"
);
const REST_BASE: &str = "https://api.warframe.market/v2";
const WS_URL: &str = "wss://ws.warframe.market/socket";
const NEW_ORDER_EVENT: &str = "@wfm|event/subscriptions/newOrder";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("install TLS crypto provider");

    let mut args = std::env::args().skip(1);
    let seconds: u64 = args.next().map(|s| s.parse()).transpose()?.unwrap_or(300);
    let slug = args.next().unwrap_or_else(|| "archon_intensify".into());
    let max_price: i64 = args.next().map(|s| s.parse()).transpose()?.unwrap_or(150);

    println!("== REST: GET /item/{slug}");
    let item_id = match fetch_item_id(&slug) {
        Ok(id) => {
            println!("item id: {id}\n");
            Some(id)
        }
        Err(e) => {
            println!("REST failed: {e}\n(watching no item, stream test only)\n");
            None
        }
    };

    println!("== WebSocket: {WS_URL}");
    let mut request = WS_URL.into_client_request()?;
    request
        .headers_mut()
        .insert("Sec-WebSocket-Protocol", "wfm".parse()?);
    request
        .headers_mut()
        .insert("User-Agent", USER_AGENT.parse()?);
    let (mut ws, response) = tokio_tungstenite::connect_async(request).await?;
    println!("connected: HTTP {}", response.status());

    let subscribe = json!({
        "route": "@wfm|cmd/subscribe/newOrders",
        "id": "probe-new-orders",
        "payload": { "platform": "pc", "crossplay": true },
    })
    .to_string();
    println!(">> {subscribe}");
    ws.send(Message::text(subscribe)).await?;

    println!(
        "\nListening for {seconds}s. Watching `{slug}`: sell orders at or below {max_price}p are MATCH.\n"
    );
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let (mut watched, mut matches) = (0, 0);
    loop {
        match timeout_at(deadline, ws.next()).await {
            Err(_) => break, // time is up
            Ok(None) => {
                println!("server closed the stream");
                break;
            }
            Ok(Some(Err(e))) => return Err(e.into()),
            Ok(Some(Ok(Message::Text(text)))) => {
                let message: Value = serde_json::from_str(text.as_str()).unwrap_or_default();
                let route = message["route"].as_str().unwrap_or("<no route>").to_owned();
                let seen = counts.entry(route.clone()).or_default();
                *seen += 1;
                if *seen == 1 {
                    println!("<< first [{route}] {text}\n");
                }

                let order = &message["payload"];
                let is_watched = route == NEW_ORDER_EVENT
                    && item_id.is_some()
                    && order["itemId"].as_str() == item_id.as_deref();
                if is_watched {
                    watched += 1;
                    let is_match =
                        order["type"] == "sell" && order["platinum"].as_i64() <= Some(max_price);
                    matches += usize::from(is_match);
                    println!(
                        "{} {} {}p rank={} qty={} by {} ({})",
                        if is_match { "MATCH" } else { "order" },
                        order["type"],
                        order["platinum"],
                        order["rank"],
                        order["quantity"],
                        order["user"]["ingameName"],
                        order["user"]["status"],
                    );
                }
            }
            Ok(Some(Ok(Message::Close(frame)))) => {
                println!("close frame: {frame:?}");
                break;
            }
            Ok(Some(Ok(other))) => println!("<< non-text frame: {other:?}"),
        }
    }

    println!("\n== Summary");
    for (route, n) in &counts {
        println!("{n:>6}  {route}");
    }
    println!("`{slug}` orders seen: {watched}, matches: {matches}");
    Ok(())
}

/// Looks up the item's internal id and prints the response headers (rate limits, caching).
fn fetch_item_id(slug: &str) -> Result<String> {
    let mut response = ureq::get(format!("{REST_BASE}/item/{slug}"))
        .header("User-Agent", USER_AGENT)
        .header("Language", "en")
        .call()?;
    println!("HTTP {}", response.status());
    for (name, value) in response.headers() {
        println!("  {name}: {}", value.to_str().unwrap_or("<binary>"));
    }
    let body: Value = response.body_mut().read_json()?;
    match body["data"]["id"].as_str() {
        Some(id) => Ok(id.to_owned()),
        None => Err(format!("no data.id in response: {body}").into()),
    }
}
