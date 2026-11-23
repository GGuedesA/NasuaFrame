//! what the ui can call

use std::sync::Mutex;

use nasua_core::model::{Filter, Item, Platform, Status};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::catalog;
use crate::storage::filters::{
    self, StoredFilter, intent_text, parse_intent, parse_platform, parse_status, platform_text,
    status_text,
};
use crate::storage::items;
use crate::storage::settings::{self, Settings};
use crate::tray;
use crate::watcher::{BestLists, Watcher};

pub type Db = Mutex<Connection>;

const SEARCH_LIMIT: u32 = 20;

/// a filter as the ui sends and gets it
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterFields {
    pub item_id: String,
    pub intent: String,
    pub price_limit: u32,
    pub platform: Option<String>,
    pub crossplay: Option<bool>,
    pub rank_min: Option<u8>,
    pub rank_max: Option<u8>,
    pub subtype: Option<String>,
    /// left out means ingame
    pub min_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterView {
    pub id: i64,
    pub enabled: bool,
    #[serde(flatten)]
    pub fields: FilterFields,
    /// only sent to the ui, None if the item left the catalog
    #[serde(default, skip_deserializing)]
    pub item: Option<ItemView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemView {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub max_rank: Option<u8>,
    pub subtypes: Vec<String>,
}

/// no version means nothing cached, the ui can't offer any item
#[derive(Debug, Serialize)]
pub struct CatalogStatus {
    pub version: Option<String>,
    pub error: Option<String>,
}

impl TryFrom<FilterFields> for Filter {
    type Error = String;

    fn try_from(f: FilterFields) -> Result<Self, String> {
        let rank = match (f.rank_min, f.rank_max) {
            (Some(min), Some(max)) if min <= max => Some(min..=max),
            (None, None) => None,
            _ => return Err("rank needs both min and max, min not above max".into()),
        };
        let min_status = match f.min_status.as_deref() {
            None => Status::Ingame,
            Some(text) => parse_status(text).ok_or_else(|| format!("unknown status `{text}`"))?,
        };
        let platform = match f.platform.as_deref() {
            None => Platform::Pc,
            Some(text) => {
                parse_platform(text).ok_or_else(|| format!("unknown platform `{text}`"))?
            }
        };
        Ok(Filter {
            intent: parse_intent(&f.intent)
                .ok_or_else(|| format!("unknown intent `{}`", f.intent))?,
            platform,
            item_id: f.item_id,
            price_limit: f.price_limit,
            crossplay: f.crossplay.unwrap_or(true),
            rank,
            subtype: f.subtype,
            min_status,
        })
    }
}

impl From<StoredFilter> for FilterView {
    fn from(stored: StoredFilter) -> Self {
        let f = stored.filter;
        FilterView {
            id: stored.id,
            enabled: stored.enabled,
            item: None,
            fields: FilterFields {
                intent: intent_text(f.intent).into(),
                platform: Some(platform_text(f.platform).into()),
                min_status: Some(status_text(f.min_status).into()),
                rank_min: f.rank.as_ref().map(|r| *r.start()),
                rank_max: f.rank.as_ref().map(|r| *r.end()),
                item_id: f.item_id,
                price_limit: f.price_limit,
                crossplay: Some(f.crossplay),
                subtype: f.subtype,
            },
        }
    }
}

impl From<Item> for ItemView {
    fn from(item: Item) -> Self {
        ItemView {
            id: item.id,
            slug: item.slug,
            name: item.name,
            max_rank: item.max_rank,
            subtypes: item.subtypes,
        }
    }
}

#[tauri::command]
pub fn list_filters(db: State<Db>) -> Result<Vec<FilterView>, String> {
    let conn = db.lock().unwrap();
    let stored = filters::list(&conn).map_err(|e| e.to_string())?;
    stored
        .into_iter()
        .map(|s| {
            let item = items::get(&conn, &s.filter.item_id).map_err(|e| e.to_string())?;
            Ok(FilterView {
                item: item.map(ItemView::from),
                ..FilterView::from(s)
            })
        })
        .collect()
}

/// returns the new filter's id
#[tauri::command]
pub fn add_filter(
    db: State<Db>,
    watcher: State<Watcher>,
    filter: FilterFields,
) -> Result<i64, String> {
    let filter = Filter::try_from(filter)?;
    let id = filters::insert(&db.lock().unwrap(), &filter).map_err(|e| e.to_string())?;
    reload_watcher(&db, &watcher)?;
    Ok(id)
}

#[tauri::command]
pub fn update_filter(
    db: State<Db>,
    watcher: State<Watcher>,
    filter: FilterView,
) -> Result<(), String> {
    let stored = StoredFilter {
        id: filter.id,
        enabled: filter.enabled,
        filter: Filter::try_from(filter.fields)?,
    };
    filters::update(&db.lock().unwrap(), &stored).map_err(|e| e.to_string())?;
    reload_watcher(&db, &watcher)
}

#[tauri::command]
pub fn delete_filter(db: State<Db>, watcher: State<Watcher>, id: i64) -> Result<(), String> {
    filters::delete(&db.lock().unwrap(), id).map_err(|e| e.to_string())?;
    reload_watcher(&db, &watcher)
}

fn reload_watcher(db: &Db, watcher: &Watcher) -> Result<(), String> {
    let all = filters::list(&db.lock().unwrap()).map_err(|e| e.to_string())?;
    watcher.reload(all);
    Ok(())
}

#[tauri::command]
pub fn search_items(db: State<Db>, query: String) -> Result<Vec<ItemView>, String> {
    let found = items::search(&db.lock().unwrap(), query.trim(), SEARCH_LIMIT)
        .map_err(|e| e.to_string())?;
    Ok(found.into_iter().map(ItemView::from).collect())
}

/// downloads the catalog if it changed, a failed download still reports the cached version
#[tauri::command]
pub async fn refresh_catalog(app: AppHandle) -> Result<CatalogStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = app.state::<Db>();
        let error = match catalog::refresh(&db) {
            Ok(outcome) => {
                log::info!("catalog refresh: {outcome:?}");
                None
            }
            Err(e) => {
                log::warn!("catalog refresh failed: {e}");
                Some(e.to_string())
            }
        };
        let version = items::cached_version(&db.lock().unwrap()).map_err(|e| e.to_string())?;
        Ok(CatalogStatus { version, error })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn list_best_orders(watcher: State<Watcher>) -> BestLists {
    watcher.best_orders()
}

#[tauri::command]
pub fn get_settings(db: State<Db>) -> Result<Settings, String> {
    settings::load(&db.lock().unwrap()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_settings(app: AppHandle, db: State<Db>, settings: Settings) -> Result<(), String> {
    if !nasua_market::rest::LANGUAGES.contains(&settings.language.as_str()) {
        return Err(format!("unknown language `{}`", settings.language));
    }
    if !settings::REFRESH_MINUTES.contains(&settings.refresh_minutes) {
        return Err(format!(
            "refresh every {} min is out of range",
            settings.refresh_minutes
        ));
    }
    settings::save(&db.lock().unwrap(), &settings).map_err(|e| e.to_string())?;
    if let Err(e) = tray::update_menu(&app) {
        log::warn!("tray menu not updated: {e}");
    }
    Ok(())
}

#[tauri::command]
pub fn open_log_folder(app: AppHandle) -> Result<(), String> {
    let dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nasua_core::model::OrderType;

    fn fields() -> FilterFields {
        FilterFields {
            item_id: "6322918add457f0072aeb8b1".into(),
            intent: "buy".into(),
            price_limit: 50,
            platform: None,
            crossplay: None,
            rank_min: Some(7),
            rank_max: Some(10),
            subtype: None,
            min_status: None,
        }
    }

    #[test]
    fn reads_what_the_ui_sends() {
        let json = r#"{"itemId":"6322918add457f0072aeb8b1","intent":"buy","priceLimit":50,
            "rankMin":7,"rankMax":10}"#;
        let parsed: FilterFields = serde_json::from_str(json).unwrap();
        assert_eq!(parsed, fields());
    }

    #[test]
    fn left_out_fields_get_defaults() {
        let filter = Filter::try_from(fields()).unwrap();
        assert_eq!(filter.min_status, Status::Ingame);
        assert!(filter.crossplay);
        assert_eq!(filter.intent, OrderType::Buy);
        assert_eq!(filter.platform, Platform::Pc);
        assert_eq!(filter.rank, Some(7..=10));
    }

    #[test]
    fn view_round_trips() {
        let stored = StoredFilter {
            id: 3,
            enabled: false,
            filter: Filter::try_from(fields()).unwrap(),
        };
        let view = FilterView::from(stored.clone());
        assert_eq!(view.fields.min_status.as_deref(), Some("ingame"));
        assert_eq!(view.fields.platform.as_deref(), Some("pc"));
        assert_eq!(view.fields.crossplay, Some(true));
        assert_eq!(Filter::try_from(view.fields).unwrap(), stored.filter);
    }

    #[test]
    fn bad_input_is_an_error() {
        let bad = [
            FilterFields {
                rank_max: None,
                ..fields()
            },
            FilterFields {
                rank_min: Some(11),
                ..fields()
            },
            FilterFields {
                platform: Some("stadia".into()),
                ..fields()
            },
            FilterFields {
                intent: "trade".into(),
                ..fields()
            },
            FilterFields {
                min_status: Some("away".into()),
                ..fields()
            },
        ];
        for f in bad {
            assert!(Filter::try_from(f).is_err());
        }
    }
}
