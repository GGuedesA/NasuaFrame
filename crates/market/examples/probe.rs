//! live test of the client + matcher against warframe.market
//!
//! `cargo run -p nasua-market --example probe -- [seconds] [item-slug] [max-price]`
//! defaults to 300s, archon_intensify, 150p. sell orders at or under max-price show as MATCH

use std::time::Duration;

use nasua_core::matcher::matches;
use nasua_core::model::{Filter, Item, OrderType, Platform, Status};
use nasua_core::whisper::build_whisper;
use nasua_market::client::{self, Error};
use nasua_market::rest;
use tokio::time::{Instant, timeout_at};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let seconds: u64 = args.next().map(|s| s.parse()).transpose()?.unwrap_or(300);
    let slug = args.next().unwrap_or_else(|| "archon_intensify".into());
    let max_price: u32 = args.next().map(|s| s.parse()).transpose()?.unwrap_or(150);

    let watched_item = match find_item(&slug) {
        Ok(item) => {
            println!("`{slug}`: {item:?}");
            Some(item)
        }
        Err(e) => {
            println!("item lookup failed: {e} (watching no item, stream test only)");
            None
        }
    };
    let filter = watched_item.as_ref().map(|item| Filter {
        item_id: item.id.clone(),
        intent: OrderType::Buy,
        price_limit: max_price,
        platform: Platform::Pc,
        crossplay: true,
        rank: None,
        subtype: None,
        // show all of them, status is printed anyway
        min_status: Status::Offline,
    });

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
                if let (true, Some(item)) = (is_match, &watched_item) {
                    println!("  {}", build_whisper(&order, &item.name));
                }
            }
        }
    }

    println!("\n== Summary");
    println!("orders: {orders}, unreadable messages: {unreadable}");
    println!("`{slug}` orders seen: {watched}, matches: {matched}");
    Ok(())
}

/// loads the whole catalog, so this also tests `rest` against the live api
fn find_item(slug: &str) -> Result<Item> {
    println!("catalog version: {}", rest::fetch_items_version()?);
    let items = rest::fetch_items()?;
    println!("catalog: {} items", items.len());
    items
        .into_iter()
        .find(|item| item.slug == slug)
        .ok_or_else(|| format!("no item with slug `{slug}`").into())
}
