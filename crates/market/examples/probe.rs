//! live test of the client + matcher against warframe.market
//!
//! `cargo run -p nasua-market --example probe -- [seconds] [item-slug] [max-price]`
//! defaults to 300s, archon_intensify, 150p. sell orders at or under max-price show as MATCH

use std::time::Duration;

use nasua_core::matcher::matches;
use nasua_core::model::{Filter, OrderType, Platform, Status};
use nasua_market::client::{self, Error};
use serde_json::Value;
use tokio::time::{Instant, timeout_at};

const USER_AGENT: &str = concat!(
    "NasuaFrame-probe/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/GGuedesA/NasuaFrame)"
);
const REST_BASE: &str = "https://api.warframe.market/v2";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let mut args = std::env::args().skip(1);
    let seconds: u64 = args.next().map(|s| s.parse()).transpose()?.unwrap_or(300);
    let slug = args.next().unwrap_or_else(|| "archon_intensify".into());
    let max_price: u32 = args.next().map(|s| s.parse()).transpose()?.unwrap_or(150);

    let filter = match fetch_item_id(&slug) {
        Ok(item_id) => {
            println!("`{slug}` item id: {item_id}");
            Some(Filter {
                item_id,
                intent: OrderType::Buy,
                price_limit: max_price,
                platform: Platform::Pc,
                crossplay: true,
                rank: None,
                subtype: None,
                // show all of them, status is printed anyway
                min_status: Status::Offline,
            })
        }
        Err(e) => {
            println!("REST failed: {e} (watching no item, stream test only)");
            None
        }
    };

    let mut stream = client::connect(Platform::Pc, true).await?;
    println!(
        "connected and subscribed. Listening for {seconds}s, MATCH = sell at or below {max_price}p\n"
    );

    let deadline = Instant::now() + Duration::from_secs(seconds);
    let (mut orders, mut unreadable, mut watched, mut matched) = (0, 0, 0, 0);
    loop {
        let Ok(next) = timeout_at(deadline, stream.next_order()).await else {
            break;
        };
        match next {
            None => {
                println!("server closed the stream");
                break;
            }
            Some(Err(Error::Parse(e))) => {
                unreadable += 1;
                println!("skipped an unreadable message: {e}");
            }
            Some(Err(e)) => return Err(e.into()),
            Some(Ok(order)) => {
                orders += 1;
                let Some(filter) = filter.as_ref().filter(|f| f.item_id == order.item_id) else {
                    continue;
                };
                watched += 1;
                let is_match = matches(filter, &order);
                matched += usize::from(is_match);
                println!(
                    "{} {} {:?} {}p rank={:?} by {} ({:?})",
                    if is_match { "MATCH" } else { "order" },
                    order.id,
                    order.order_type,
                    order.platinum,
                    order.rank,
                    order.user.ingame_name,
                    order.user.status,
                );
            }
        }
    }

    println!("\n== Summary");
    println!("orders: {orders}, unreadable messages: {unreadable}");
    println!("`{slug}` orders seen: {watched}, matches: {matched}");
    Ok(())
}

/// item id from the slug, over REST
fn fetch_item_id(slug: &str) -> Result<String> {
    let body: Value = ureq::get(format!("{REST_BASE}/item/{slug}"))
        .header("User-Agent", USER_AGENT)
        .header("Language", "en")
        .call()?
        .body_mut()
        .read_json()?;
    match body["data"]["id"].as_str() {
        Some(id) => Ok(id.to_owned()),
        None => Err(format!("no data.id in response: {body}").into()),
    }
}
