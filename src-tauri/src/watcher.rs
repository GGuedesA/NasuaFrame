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
use tauri_plugin_notification::NotificationExt;

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
    let mut failures = 0;
    loop {
        let started = Instant::now();
        let problem = match client::connect(feed.0, feed.1).await {
            Ok(mut stream) => {
                log::info!("feed {} connected", feed_label(feed));
                report(&app, feed, None);
                read_orders(&mut stream, feed, &filters, &app).await
            }
            Err(e) => e.to_string(),
        };
        if started.elapsed() >= STABLE {
            failures = 0;
        }
        let wait = retry_delay(failures);
        failures = failures.saturating_add(1);
        let message = format!("{problem}, retrying in {}s", wait.as_secs());
        log::warn!("feed {}: {message}", feed_label(feed));
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
) -> String {
    while let Some(next) = stream.next_order().await {
        let order = match next {
            Ok(order) => order,
            Err(client::Error::Parse(_)) => continue,
            Err(e) => return e.to_string(),
        };
        let due = due_filters(&filters.read().unwrap(), feed, &order);
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

/// enabled filters on this feed that match
fn due_filters(filters: &[StoredFilter], feed: Feed, order: &Order) -> Vec<i64> {
    filters
        .iter()
        .filter(|s| s.enabled && feed_of(s) == feed && matches(&s.filter, order))
        .map(|s| s.id)
        .collect()
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
        log::info!("alert for filter {filter_id}: {name} {}p", order.platinum);
        if let Err(e) = app.emit("alert", alert) {
            log::warn!("alert not sent to the ui: {e}");
        }
    }
    notify(app, &name, order);
}

/// sent from here, a hidden window may not run its js in time
fn notify(app: &AppHandle, name: &str, order: &Order) {
    let focused = app
        .get_webview_window("main")
        .and_then(|window| window.is_focused().ok())
        .unwrap_or(false);
    // the list on screen is enough while the user is looking at it
    if focused {
        return;
    }
    let shown = app
        .notification()
        .builder()
        .title(format!("{name}, {}p", order.platinum))
        .body(format!(
            "{} - open NasuaFrame to copy the whisper",
            order.user.ingame_name
        ))
        .show();
    if let Err(e) = shown {
        log::warn!("notification failed: {e}");
    }
}

fn feed_label(feed: Feed) -> String {
    let crossplay = if feed.1 { " + crossplay" } else { "" };
    format!("{}{crossplay}", platform_text(feed.0))
}

fn report(app: &AppHandle, feed: Feed, problem: Option<String>) {
    let status = FeedStatus {
        feed: feed_label(feed),
        problem,
    };
    if let Err(e) = app.emit("feed-status", status) {
        log::warn!("feed status not sent to the ui: {e}");
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
    fn every_match_alerts() {
        let filters = [stored(1)];
        assert_eq!(due_filters(&filters, PC, &order()), [1]);
        assert_eq!(due_filters(&filters, PC, &order()), [1]);
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
        let due = due_filters(&filters, PC, &order());
        assert_eq!(due, [4]);
    }
}
