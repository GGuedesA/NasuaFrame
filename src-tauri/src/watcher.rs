//! keeps a best orders list per filter, from the stream and the rest api

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use nasua_core::matcher::matches;
use nasua_core::model::{Filter, Order, OrderType, Platform};
use nasua_core::whisper::build_whisper;
use nasua_market::{client, rest};
use serde::Serialize;
use tauri::async_runtime::{self, JoinHandle};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::commands::Db;
use crate::storage::filters::{StoredFilter, platform_text, status_text};
use crate::storage::items;

/// the stream is subscribed per platform + crossplay
type Feed = (Platform, bool);

const FIRST_RETRY: Duration = Duration::from_secs(1);
const MAX_RETRY: Duration = Duration::from_secs(60);
const STABLE: Duration = Duration::from_secs(60);
/// limitation stated by warframe.market api
const REST_GAP: Duration = Duration::from_millis(350);
/// drops sold orders and sellers that went offline
const REFRESH: Duration = Duration::from_secs(120);
const BEST_KEPT: usize = 5;

/// problem None means connected
#[derive(Debug, Clone, Serialize)]
pub struct FeedStatus {
    pub feed: String,
    pub problem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BestOrder {
    pub id: String,
    pub platinum: u32,
    pub trader: String,
    pub status: String,
    pub rank: Option<u8>,
    pub whisper: String,
}

/// orders None means the list is gone (filter off, deleted or being reloaded)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BestOrders {
    pub filter_id: i64,
    pub orders: Option<Vec<BestOrder>>,
}

pub type BestLists = HashMap<i64, Vec<BestOrder>>;

/// one stream task per feed that an enabled filter needs
pub struct Watcher {
    app: AppHandle,
    filters: Arc<RwLock<Vec<StoredFilter>>>,
    feeds: Mutex<HashMap<Feed, JoinHandle<()>>>,
    snapshots: UnboundedSender<StoredFilter>,
    best: Arc<Mutex<BestLists>>,
}

impl Watcher {
    pub fn new(app: AppHandle) -> Self {
        let filters = Arc::default();
        let best = Arc::default();
        let (snapshots, queue) = mpsc::unbounded_channel();
        async_runtime::spawn(run_snapshots(
            queue,
            Arc::clone(&filters),
            Arc::clone(&best),
            app.clone(),
        ));
        Watcher {
            app,
            filters,
            feeds: Mutex::default(),
            snapshots,
            best,
        }
    }

    pub fn best_orders(&self) -> BestLists {
        self.best.lock().unwrap().clone()
    }

    /// call with every saved filter after any change
    pub fn reload(&self, all: Vec<StoredFilter>) {
        let wanted: HashSet<Feed> = all.iter().filter(|s| s.enabled).map(feed_of).collect();
        let (started, dropped) = {
            let mut current = self.filters.write().unwrap();
            let started = newly_started(&current, &all);
            let still_valid: HashSet<i64> = all
                .iter()
                .filter(|s| s.enabled && !started.contains(s))
                .map(|s| s.id)
                .collect();
            let mut best = self.best.lock().unwrap();
            let dropped: Vec<i64> = best
                .keys()
                .filter(|id| !still_valid.contains(id))
                .copied()
                .collect();
            for id in &dropped {
                best.remove(id);
            }
            *current = all;
            (started, dropped)
        };
        for id in dropped {
            emit_list(&self.app, id, None);
        }
        for stored in started {
            if self.snapshots.send(stored).is_err() {
                log::warn!("snapshot queue is gone, no current orders for new filters");
            }
        }

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
                async_runtime::spawn(run_feed(
                    feed,
                    self.filters.clone(),
                    self.best.clone(),
                    self.app.clone(),
                ))
            });
        }
    }
}

fn feed_of(stored: &StoredFilter) -> Feed {
    (stored.filter.platform, stored.filter.crossplay)
}

fn newly_started(old: &[StoredFilter], new: &[StoredFilter]) -> Vec<StoredFilter> {
    new.iter()
        .filter(|s| s.enabled && !old.contains(s))
        .cloned()
        .collect()
}

fn ranks_to_ask(filter: &Filter) -> Vec<Option<u8>> {
    match &filter.rank {
        Some(range) => range.clone().map(Some).collect(),
        None => vec![None],
    }
}

async fn run_snapshots(
    mut queue: UnboundedReceiver<StoredFilter>,
    filters: Arc<RwLock<Vec<StoredFilter>>>,
    best: Arc<Mutex<BestLists>>,
    app: AppHandle,
) {
    let mut next_refresh = tokio::time::Instant::now() + REFRESH;
    loop {
        let (jobs, refresh) = match tokio::time::timeout_at(next_refresh, queue.recv()).await {
            Ok(Some(stored)) => (vec![stored], false),
            Ok(None) => return,
            Err(_) => (enabled(&filters), true),
        };
        for stored in &jobs {
            snapshot(stored, &filters, &best, &app).await;
        }
        if refresh {
            next_refresh = tokio::time::Instant::now() + REFRESH;
        }
    }
}

fn enabled(filters: &RwLock<Vec<StoredFilter>>) -> Vec<StoredFilter> {
    let filters = filters.read().unwrap();
    filters.iter().filter(|s| s.enabled).cloned().collect()
}

/// replaces the filter's list with what's listed now, keeps the old one if a call fails
async fn snapshot(
    stored: &StoredFilter,
    filters: &RwLock<Vec<StoredFilter>>,
    best: &Mutex<BestLists>,
    app: &AppHandle,
) {
    let item = {
        let db = app.state::<Db>();
        let conn = db.lock().unwrap();
        items::get(&conn, &stored.filter.item_id).ok().flatten()
    };
    let Some(item) = item else {
        log::warn!("filter {}: item not in the catalog, no snapshot", stored.id);
        return;
    };
    let mut fresh = Vec::new();
    for rank in ranks_to_ask(&stored.filter) {
        tokio::time::sleep(REST_GAP).await;
        // edited or turned off while it waited
        if !filters.read().unwrap().contains(stored) {
            return;
        }
        let (slug, filter) = (item.slug.clone(), stored.filter.clone());
        let fetched = async_runtime::spawn_blocking(move || {
            rest::fetch_top_orders(
                &slug,
                filter.platform,
                filter.crossplay,
                rank,
                filter.subtype.as_deref(),
            )
        })
        .await;
        let orders = match fetched {
            Ok(Ok(orders)) => orders,
            Ok(Err(e)) => {
                log::warn!("filter {}: top orders failed: {e}", stored.id);
                return;
            }
            Err(e) => {
                log::warn!("filter {}: top orders task failed: {e}", stored.id);
                return;
            }
        };
        for order in orders.iter().filter(|o| matches(&stored.filter, o)) {
            insert_best(
                &mut fresh,
                best_order(order, &item.name),
                stored.filter.intent,
            );
        }
    }
    let current = filters.read().unwrap();
    if current.contains(stored) {
        best.lock().unwrap().insert(stored.id, fresh.clone());
        emit_list(app, stored.id, Some(fresh));
    }
}

/// best price first, at most BEST_KEPT
fn insert_best(list: &mut Vec<BestOrder>, order: BestOrder, intent: OrderType) {
    let key = |o: &BestOrder| match intent {
        OrderType::Buy => i64::from(o.platinum),
        OrderType::Sell => -i64::from(o.platinum),
    };
    // the same order can come from the stream and a refresh
    if list.iter().any(|o| o.id == order.id) {
        return;
    }
    list.push(order);
    list.sort_by_key(key);
    list.truncate(BEST_KEPT);
}

fn best_order(order: &Order, item_name: &str) -> BestOrder {
    BestOrder {
        id: order.id.clone(),
        platinum: order.platinum,
        trader: order.user.ingame_name.clone(),
        status: status_text(order.user.status).into(),
        rank: order.rank,
        whisper: build_whisper(order, item_name),
    }
}

fn emit_list(app: &AppHandle, filter_id: i64, orders: Option<Vec<BestOrder>>) {
    if let Err(e) = app.emit("best-orders", BestOrders { filter_id, orders }) {
        log::warn!("best orders not sent to the ui: {e}");
    }
}

/// keeps one feed connected for as long as the app runs
async fn run_feed(
    feed: Feed,
    filters: Arc<RwLock<Vec<StoredFilter>>>,
    best: Arc<Mutex<BestLists>>,
    app: AppHandle,
) {
    let mut failures = 0;
    loop {
        let started = Instant::now();
        let problem = match client::connect(feed.0, feed.1).await {
            Ok(mut stream) => {
                log::info!("feed {} connected", feed_label(feed));
                report(&app, feed, None);
                read_orders(&mut stream, feed, &filters, &best, &app).await
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
    best: &Mutex<BestLists>,
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
            add_live_match(app, best, &due, &order);
        }
    }
    "connection closed".into()
}

fn retry_delay(failures: u32) -> Duration {
    FIRST_RETRY
        .saturating_mul(2u32.saturating_pow(failures))
        .min(MAX_RETRY)
}

/// enabled filters on this feed that match, with what the user wants to do
fn due_filters(filters: &[StoredFilter], feed: Feed, order: &Order) -> Vec<(i64, OrderType)> {
    filters
        .iter()
        .filter(|s| s.enabled && feed_of(s) == feed && matches(&s.filter, order))
        .map(|s| (s.id, s.filter.intent))
        .collect()
}

fn add_live_match(
    app: &AppHandle,
    best: &Mutex<BestLists>,
    due: &[(i64, OrderType)],
    order: &Order,
) {
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
    let entry = best_order(order, &name);
    for &(filter_id, intent) in due {
        log::info!("match for filter {filter_id}: {name} {}p", order.platinum);
        let list = {
            let mut best = best.lock().unwrap();
            let list = best.entry(filter_id).or_default();
            insert_best(list, entry.clone(), intent);
            list.clone()
        };
        emit_list(app, filter_id, Some(list));
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
    use nasua_core::model::{Status, Trader};

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

    fn entry(trader: &str, platinum: u32) -> BestOrder {
        BestOrder {
            id: format!("{trader}-{platinum}"),
            platinum,
            trader: trader.into(),
            status: "ingame".into(),
            rank: None,
            whisper: String::new(),
        }
    }

    fn prices(list: &[BestOrder]) -> Vec<(&str, u32)> {
        list.iter()
            .map(|o| (o.trader.as_str(), o.platinum))
            .collect()
    }

    #[test]
    fn only_new_or_changed_enabled_filters_start() {
        let same = stored(1);
        let mut repriced = stored(2);
        let mut turned_off = stored(4);
        let old = [same.clone(), repriced.clone(), turned_off.clone()];
        repriced.filter.price_limit = 30;
        turned_off.enabled = false;
        let new = [same, repriced, stored(3), turned_off];
        let ids: Vec<i64> = newly_started(&old, &new).iter().map(|s| s.id).collect();
        assert_eq!(ids, [2, 3]);
    }

    #[test]
    fn a_rank_range_asks_each_rank() {
        let mut filter = stored(1).filter;
        assert_eq!(ranks_to_ask(&filter), [None]);
        filter.rank = Some(8..=10);
        assert_eq!(ranks_to_ask(&filter), [Some(8), Some(9), Some(10)]);
    }

    #[test]
    fn buying_keeps_the_cheapest_first_and_only_five() {
        let mut list = Vec::new();
        for (i, p) in [40, 20, 35, 25, 30, 45].into_iter().enumerate() {
            insert_best(&mut list, entry(&format!("Test{i}"), p), OrderType::Buy);
        }
        assert_eq!(
            prices(&list),
            [
                ("Test1", 20),
                ("Test3", 25),
                ("Test4", 30),
                ("Test2", 35),
                ("Test0", 40)
            ]
        );
    }

    #[test]
    fn selling_keeps_the_highest_first() {
        let mut list = Vec::new();
        insert_best(&mut list, entry("TestBuyer", 50), OrderType::Sell);
        insert_best(&mut list, entry("TestOther", 60), OrderType::Sell);
        assert_eq!(prices(&list), [("TestOther", 60), ("TestBuyer", 50)]);
    }

    #[test]
    fn every_order_is_listed_but_only_once() {
        let mut list = vec![entry("TestTrader", 30)];
        insert_best(&mut list, entry("TestTrader", 30), OrderType::Buy);
        insert_best(&mut list, entry("TestTrader", 25), OrderType::Buy);
        assert_eq!(prices(&list), [("TestTrader", 25), ("TestTrader", 30)]);
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
        assert_eq!(due_filters(&filters, PC, &order()), [(1, OrderType::Buy)]);
        assert_eq!(due_filters(&filters, PC, &order()), [(1, OrderType::Buy)]);
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
        assert_eq!(due, [(4, OrderType::Buy)]);
    }
}
