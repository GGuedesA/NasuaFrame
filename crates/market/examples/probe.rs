//! Protocol probe for warframe.market.
//!
//! Run: `cargo run -p nasua-market --example probe -- [seconds] [item-slug]`
//! (defaults: 90 seconds, `mirage_prime_set`).
//!
//! It prints the REST response headers and every raw WebSocket message without signing in,
//! to check the message format, which subscriptions need an account, and the rate limits.

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

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("install TLS crypto provider");

    let mut args = std::env::args().skip(1);
    let seconds: u64 = args.next().map(|s| s.parse()).transpose()?.unwrap_or(90);
    let slug = args.next().unwrap_or_else(|| "mirage_prime_set".into());

    println!("== REST: GET /item/{slug}");
    let item_id = match fetch_item_id(&slug) {
        Ok(id) => {
            println!("item id: {id}\n");
            Some(id)
        }
        Err(e) => {
            println!("REST failed: {e}\n(skipping the item subscription)\n");
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

    // No sign-in on purpose: the responses tell which subscriptions need an account.
    let mut subscriptions = vec![(
        "probe-new-orders",
        "@wfm|cmd/subscribe/newOrders",
        json!({ "platform": "pc", "crossplay": true }),
    )];
    if let Some(item_id) = item_id {
        subscriptions.push((
            "probe-item",
            "@wfm|cmd/subscribe/item",
            json!({ "itemId": item_id }),
        ));
    }
    for (id, route, payload) in subscriptions {
        let message = json!({ "route": route, "payload": payload, "id": id }).to_string();
        println!(">> {message}");
        ws.send(Message::text(message)).await?;
    }

    println!("\nListening for {seconds}s (first message per route printed in full)...\n");
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    loop {
        match timeout_at(deadline, ws.next()).await {
            Err(_) => break, // time is up
            Ok(None) => {
                println!("server closed the stream");
                break;
            }
            Ok(Some(Err(e))) => return Err(e.into()),
            Ok(Some(Ok(Message::Text(text)))) => {
                let route = serde_json::from_str::<Value>(text.as_str())
                    .ok()
                    .and_then(|v| v["route"].as_str().map(str::to_owned))
                    .unwrap_or_else(|| "<no route>".into());
                let seen = counts.entry(route.clone()).or_default();
                *seen += 1;
                if *seen == 1 {
                    println!("<< [{route}] {text}\n");
                } else {
                    println!("<< [{route}] #{seen}");
                }
            }
            Ok(Some(Ok(Message::Close(frame)))) => {
                println!("close frame: {frame:?}");
                break;
            }
            Ok(Some(Ok(other))) => println!("<< non-text frame: {other:?}"),
        }
    }

    println!("\n== Summary: messages per route");
    for (route, n) in &counts {
        println!("{n:>6}  {route}");
    }
    Ok(())
}

/// Looks up the item's internal id and prints the response headers (rate limits, caching).
fn fetch_item_id(slug: &str) -> Result<String> {
    let mut response = ureq::get(format!("{REST_BASE}/item/{slug}"))
        .header("User-Agent", USER_AGENT)
        .header("Platform", "pc")
        .header("Language", "en")
        .header("Crossplay", "true")
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
