//! listens to new orders and alerts the ui when a filter matches

use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use nasua_core::matcher::matches;
use nasua_core::model::{Order, Platform};
use nasua_core::whisper::build_whisper;
use nasua_market::client;
use serde::Serialize;
use tauri::async_runtime::{self, JoinHandle};
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::Db;
use crate::storage::filters::StoredFilter;
use crate::storage::items;

/// the stream is subscribed per platform + crossplay
type Feed = (Platform, bool);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub filter_id: i64,
    pub item_name: String,
    pub platinum: u32,
    pub trader: String,
    pub whisper: String,
}

/// one stream task per feed that an enabled filter needs
pub struct Watcher {
    app: AppHandle,
    filters: Arc<RwLock<Vec<StoredFilter>>>,
    feeds: Mutex<HashMap<Feed, JoinHandle<()>>>,
}

impl Watcher {
    pub fn new(app: AppHandle) -> Self {
        Watcher {
            app,
            filters: Arc::default(),
            feeds: Mutex::default(),
        }
    }

    /// call with every saved filter after any change
    pub fn reload(&self, all: Vec<StoredFilter>) {
        let wanted: HashSet<Feed> = all.iter().filter(|s| s.enabled).map(feed_of).collect();
        *self.filters.write().unwrap() = all;

        let mut feeds = self.feeds.lock().unwrap();
        // dead feeds get dropped too, so they start again below
        feeds.retain(|feed, task| {
            let keep = wanted.contains(feed) && !task.inner().is_finished();
            if !keep {
                task.abort();
            }
            keep
        });
        for feed in wanted {
            feeds.entry(feed).or_insert_with(|| {
                async_runtime::spawn(run_feed(feed, self.filters.clone(), self.app.clone()))
            });
        }
    }
}

fn feed_of(stored: &StoredFilter) -> Feed {
    (stored.filter.platform, stored.filter.crossplay)
}

// no reconnect yet, a dropped feed only restarts on the next filter change
async fn run_feed(feed: Feed, filters: Arc<RwLock<Vec<StoredFilter>>>, app: AppHandle) {
    let mut stream = match client::connect(feed.0, feed.1).await {
        Ok(stream) => stream,
        Err(e) => return report(&app, e),
    };
    let mut last_alert = HashMap::new();
    while let Some(next) = stream.next_order().await {
        let order = match next {
            Ok(order) => order,
            Err(client::Error::Parse(_)) => continue,
            Err(e) => return report(&app, e),
        };
        let due = due_filters(
            &filters.read().unwrap(),
            feed,
            &order,
            &mut last_alert,
            Instant::now(),
        );
        if !due.is_empty() {
            send_alerts(&app, &due, &order);
        }
    }
    report(&app, "connection closed");
}

/// enabled filters on this feed that match and are past their cooldown
fn due_filters(
    filters: &[StoredFilter],
    feed: Feed,
    order: &Order,
    last_alert: &mut HashMap<i64, Instant>,
    now: Instant,
) -> Vec<i64> {
    let mut due = Vec::new();
    for stored in filters {
        if !stored.enabled || feed_of(stored) != feed || !matches(&stored.filter, order) {
            continue;
        }
        let cooldown = Duration::from_secs(stored.cooldown_secs.into());
        let ready = last_alert
            .get(&stored.id)
            .is_none_or(|&at| now.duration_since(at) >= cooldown);
        if ready {
            last_alert.insert(stored.id, now);
            due.push(stored.id);
        }
    }
    due
}

fn send_alerts(app: &AppHandle, filter_ids: &[i64], order: &Order) {
    let name = {
        let db = app.state::<Db>();
        let conn = db.lock().unwrap();
        items::name_of(&conn, &order.item_id).ok().flatten()
    };
    // item missing from the cache, the id still tells the user something
    let name = name.unwrap_or_else(|| order.item_id.clone());
    let whisper = build_whisper(order, &name);
    for &filter_id in filter_ids {
        let alert = Alert {
            filter_id,
            item_name: name.clone(),
            platinum: order.platinum,
            trader: order.user.ingame_name.clone(),
            whisper: whisper.clone(),
        };
        if let Err(e) = app.emit("alert", alert) {
            eprintln!("alert not sent: {e}");
        }
    }
}

fn report(app: &AppHandle, error: impl Display) {
    if let Err(e) = app.emit("watcher-error", error.to_string()) {
        eprintln!("watcher error not sent: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nasua_core::model::{Filter, OrderType, Status, Trader};

    const PC: Feed = (Platform::Pc, true);

    fn stored(id: i64) -> StoredFilter {
        StoredFilter {
            id,
            enabled: true,
            cooldown_secs: 60,
            filter: Filter {
                item_id: "item".into(),
                intent: OrderType::Buy,
                price_limit: 50,
                platform: Platform::Pc,
                crossplay: true,
                rank: None,
                subtype: None,
                min_status: Status::Ingame,
            },
        }
    }

    fn order() -> Order {
        Order {
            id: "000000000000000000000001".into(),
            item_id: "item".into(),
            order_type: OrderType::Sell,
            platinum: 40,
            rank: None,
            subtype: None,
            user: Trader {
                ingame_name: "TestTrader".into(),
                platform: Platform::Pc,
                crossplay: true,
                status: Status::Ingame,
            },
        }
    }

    #[test]
    fn cooldown_holds_back_repeats() {
        let filters = [stored(1)];
        let mut last = HashMap::new();
        let start = Instant::now();
        let at = |secs| start + Duration::from_secs(secs);
        assert_eq!(due_filters(&filters, PC, &order(), &mut last, at(0)), [1]);
        assert!(due_filters(&filters, PC, &order(), &mut last, at(59)).is_empty());
        assert_eq!(due_filters(&filters, PC, &order(), &mut last, at(60)), [1]);
    }

    #[test]
    fn skips_disabled_other_feeds_and_non_matches() {
        let disabled = StoredFilter {
            enabled: false,
            ..stored(1)
        };
        let mut no_crossplay = stored(2);
        no_crossplay.filter.crossplay = false;
        let mut too_cheap = stored(3);
        too_cheap.filter.price_limit = 30;
        let filters = [disabled, no_crossplay, too_cheap, stored(4)];
        let due = due_filters(&filters, PC, &order(), &mut HashMap::new(), Instant::now());
        assert_eq!(due, [4]);
    }
}
