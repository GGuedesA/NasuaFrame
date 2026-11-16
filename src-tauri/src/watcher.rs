//! listens to new orders and alerts the ui when a filter matches

use std::collections::{HashMap, HashSet};
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
use crate::storage::filters::{StoredFilter, platform_text};
use crate::storage::items;

/// the stream is subscribed per platform + crossplay
type Feed = (Platform, bool);

const FIRST_RETRY: Duration = Duration::from_secs(1);
const MAX_RETRY: Duration = Duration::from_secs(60);
const STABLE: Duration = Duration::from_secs(60);

/// problem None means connected
#[derive(Debug, Clone, Serialize)]
pub struct FeedStatus {
    pub feed: String,
    pub problem: Option<String>,
}

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
        // a feed only ends by panicking, start it again below
        feeds.retain(|&feed, task| {
            let keep = wanted.contains(&feed) && !task.inner().is_finished();
            if !keep {
                task.abort();
                // its last problem would stay on screen otherwise
                report(&self.app, feed, None);
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

/// keeps one feed connected for as long as the app runs
async fn run_feed(feed: Feed, filters: Arc<RwLock<Vec<StoredFilter>>>, app: AppHandle) {
    let mut last_alert = HashMap::new();
    let mut failures = 0;
    loop {
        let started = Instant::now();
        let problem = match client::connect(feed.0, feed.1).await {
            Ok(mut stream) => {
                report(&app, feed, None);
                read_orders(&mut stream, feed, &filters, &app, &mut last_alert).await
            }
            Err(e) => e.to_string(),
        };
        if started.elapsed() >= STABLE {
            failures = 0;
        }
        let wait = retry_delay(failures);
        failures = failures.saturating_add(1);
        let message = format!("{problem}, retrying in {}s", wait.as_secs());
        report(&app, feed, Some(message));
        tokio::time::sleep(wait).await;
    }
}

/// returns why the connection ended
async fn read_orders(
    stream: &mut client::OrderStream,
    feed: Feed,
    filters: &RwLock<Vec<StoredFilter>>,
    app: &AppHandle,
    last_alert: &mut HashMap<i64, Instant>,
) -> String {
    while let Some(next) = stream.next_order().await {
        let order = match next {
            Ok(order) => order,
            Err(client::Error::Parse(_)) => continue,
            Err(e) => return e.to_string(),
        };
        let due = due_filters(
            &filters.read().unwrap(),
            feed,
            &order,
            last_alert,
            Instant::now(),
        );
        if !due.is_empty() {
            send_alerts(app, &due, &order);
        }
    }
    "connection closed".into()
}

fn retry_delay(failures: u32) -> Duration {
    FIRST_RETRY
        .saturating_mul(2u32.saturating_pow(failures))
        .min(MAX_RETRY)
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
        items::get(&conn, &order.item_id)
            .ok()
            .flatten()
            .map(|item| item.name)
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

fn report(app: &AppHandle, feed: Feed, problem: Option<String>) {
    let crossplay = if feed.1 { " + crossplay" } else { "" };
    let status = FeedStatus {
        feed: format!("{}{crossplay}", platform_text(feed.0)),
        problem,
    };
    if let Err(e) = app.emit("feed-status", status) {
        eprintln!("feed status not sent: {e}");
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
    fn retries_back_off_up_to_a_minute() {
        let secs: Vec<u64> = [0, 1, 2, 5, 6, 40]
            .map(|failures| retry_delay(failures).as_secs())
            .to_vec();
        assert_eq!(secs, [1, 2, 4, 32, 60, 60]);
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
