//! what the ui can call

use std::sync::Mutex;

use nasua_core::model::{Filter, Item, Status};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::catalog;
use crate::storage::filters::{
    self, StoredFilter, intent_text, parse_intent, parse_platform, parse_status, platform_text,
    status_text,
};
use crate::storage::items;

pub type Db = Mutex<Connection>;

const SEARCH_LIMIT: u32 = 20;

/// a filter as the ui sends and gets it
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterFields {
    pub item_id: String,
    pub intent: String,
    pub price_limit: u32,
    pub platform: String,
    pub crossplay: bool,
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
    pub cooldown_secs: u32,
    #[serde(flatten)]
    pub fields: FilterFields,
}

#[derive(Debug, Serialize)]
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
        Ok(Filter {
            intent: parse_intent(&f.intent)
                .ok_or_else(|| format!("unknown intent `{}`", f.intent))?,
            platform: parse_platform(&f.platform)
                .ok_or_else(|| format!("unknown platform `{}`", f.platform))?,
            item_id: f.item_id,
            price_limit: f.price_limit,
            crossplay: f.crossplay,
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
            cooldown_secs: stored.cooldown_secs,
            fields: FilterFields {
                intent: intent_text(f.intent).into(),
                platform: platform_text(f.platform).into(),
                min_status: Some(status_text(f.min_status).into()),
                rank_min: f.rank.as_ref().map(|r| *r.start()),
                rank_max: f.rank.as_ref().map(|r| *r.end()),
                item_id: f.item_id,
                price_limit: f.price_limit,
                crossplay: f.crossplay,
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
    let stored = filters::list(&db.lock().unwrap()).map_err(|e| e.to_string())?;
    Ok(stored.into_iter().map(FilterView::from).collect())
}

/// returns the new filter's id
#[tauri::command]
pub fn add_filter(db: State<Db>, filter: FilterFields) -> Result<i64, String> {
    let filter = Filter::try_from(filter)?;
    filters::insert(&db.lock().unwrap(), &filter).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_filter(db: State<Db>, filter: FilterView) -> Result<(), String> {
    let stored = StoredFilter {
        id: filter.id,
        enabled: filter.enabled,
        cooldown_secs: filter.cooldown_secs,
        filter: Filter::try_from(filter.fields)?,
    };
    filters::update(&db.lock().unwrap(), &stored).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_filter(db: State<Db>, id: i64) -> Result<(), String> {
    filters::delete(&db.lock().unwrap(), id).map_err(|e| e.to_string())
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
        let error = catalog::refresh(&db).err().map(|e| e.to_string());
        let version = items::cached_version(&db.lock().unwrap()).map_err(|e| e.to_string())?;
        Ok(CatalogStatus { version, error })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use nasua_core::model::{OrderType, Platform};

    fn fields() -> FilterFields {
        FilterFields {
            item_id: "6322918add457f0072aeb8b1".into(),
            intent: "buy".into(),
            price_limit: 50,
            platform: "pc".into(),
            crossplay: true,
            rank_min: Some(7),
            rank_max: Some(10),
            subtype: None,
            min_status: None,
        }
    }

    #[test]
    fn reads_what_the_ui_sends() {
        let json = r#"{"itemId":"6322918add457f0072aeb8b1","intent":"buy","priceLimit":50,
            "platform":"pc","crossplay":true,"rankMin":7,"rankMax":10}"#;
        let parsed: FilterFields = serde_json::from_str(json).unwrap();
        assert_eq!(parsed, fields());
    }

    #[test]
    fn missing_status_defaults_to_ingame() {
        let filter = Filter::try_from(fields()).unwrap();
        assert_eq!(filter.min_status, Status::Ingame);
        assert_eq!(filter.intent, OrderType::Buy);
        assert_eq!(filter.platform, Platform::Pc);
        assert_eq!(filter.rank, Some(7..=10));
    }

    #[test]
    fn view_round_trips() {
        let stored = StoredFilter {
            id: 3,
            enabled: false,
            cooldown_secs: 120,
            filter: Filter::try_from(fields()).unwrap(),
        };
        let view = FilterView::from(stored.clone());
        assert_eq!(view.fields.min_status.as_deref(), Some("ingame"));
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
                platform: "stadia".into(),
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
