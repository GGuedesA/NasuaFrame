//! user's saved filters

use nasua_core::model::{Filter, OrderType, Platform, Status};
use rusqlite::{Connection, Row, params};

use super::Error;

/// a Filter plus what only the app cares about
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFilter {
    pub id: i64,
    pub enabled: bool,
    pub cooldown_secs: u32,
    pub filter: Filter,
}

/// new filters start enabled with the default cooldown, returns the new id
pub fn insert(conn: &Connection, filter: &Filter) -> Result<i64, Error> {
    let (rank_min, rank_max) = rank_columns(filter);
    conn.execute(
        "INSERT INTO filters
         (item_id, intent, price_limit, platform, crossplay, rank_min, rank_max, subtype, min_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            filter.item_id,
            intent_text(filter.intent),
            filter.price_limit,
            platform_text(filter.platform),
            filter.crossplay,
            rank_min,
            rank_max,
            filter.subtype,
            status_text(filter.min_status),
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list(conn: &Connection) -> Result<Vec<StoredFilter>, Error> {
    let mut query = conn.prepare(
        "SELECT id, enabled, cooldown_secs, item_id, intent, price_limit, platform, crossplay,
                rank_min, rank_max, subtype, min_status
         FROM filters ORDER BY id",
    )?;
    let rows = query.query_map([], read_raw)?;
    rows.map(|row| StoredFilter::try_from(row?)).collect()
}

pub fn update(conn: &Connection, stored: &StoredFilter) -> Result<(), Error> {
    let filter = &stored.filter;
    let (rank_min, rank_max) = rank_columns(filter);
    let changed = conn.execute(
        "UPDATE filters SET enabled = ?2, cooldown_secs = ?3, item_id = ?4, intent = ?5,
             price_limit = ?6, platform = ?7, crossplay = ?8, rank_min = ?9, rank_max = ?10,
             subtype = ?11, min_status = ?12
         WHERE id = ?1",
        params![
            stored.id,
            stored.enabled,
            stored.cooldown_secs,
            filter.item_id,
            intent_text(filter.intent),
            filter.price_limit,
            platform_text(filter.platform),
            filter.crossplay,
            rank_min,
            rank_max,
            filter.subtype,
            status_text(filter.min_status),
        ],
    )?;
    if changed == 0 {
        return Err(Error::NotFound(stored.id));
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> Result<(), Error> {
    if conn.execute("DELETE FROM filters WHERE id = ?1", [id])? == 0 {
        return Err(Error::NotFound(id));
    }
    Ok(())
}

fn rank_columns(filter: &Filter) -> (Option<u8>, Option<u8>) {
    match &filter.rank {
        Some(range) => (Some(*range.start()), Some(*range.end())),
        None => (None, None),
    }
}

/// columns as sqlite gives them, before checking the text ones
struct RawRow {
    id: i64,
    enabled: bool,
    cooldown_secs: u32,
    item_id: String,
    intent: String,
    price_limit: u32,
    platform: String,
    crossplay: bool,
    rank_min: Option<u8>,
    rank_max: Option<u8>,
    subtype: Option<String>,
    min_status: String,
}

fn read_raw(row: &Row) -> rusqlite::Result<RawRow> {
    Ok(RawRow {
        id: row.get(0)?,
        enabled: row.get(1)?,
        cooldown_secs: row.get(2)?,
        item_id: row.get(3)?,
        intent: row.get(4)?,
        price_limit: row.get(5)?,
        platform: row.get(6)?,
        crossplay: row.get(7)?,
        rank_min: row.get(8)?,
        rank_max: row.get(9)?,
        subtype: row.get(10)?,
        min_status: row.get(11)?,
    })
}

impl TryFrom<RawRow> for StoredFilter {
    type Error = Error;

    fn try_from(raw: RawRow) -> Result<Self, Error> {
        let rank = match (raw.rank_min, raw.rank_max) {
            (Some(min), Some(max)) => Some(min..=max),
            (None, None) => None,
            _ => {
                return Err(Error::Corrupt(format!(
                    "filter {} has half a rank range",
                    raw.id
                )));
            }
        };
        Ok(StoredFilter {
            id: raw.id,
            enabled: raw.enabled,
            cooldown_secs: raw.cooldown_secs,
            filter: Filter {
                item_id: raw.item_id,
                intent: parse_intent(&raw.intent)?,
                price_limit: raw.price_limit,
                platform: parse_platform(&raw.platform)?,
                crossplay: raw.crossplay,
                rank,
                subtype: raw.subtype,
                min_status: parse_status(&raw.min_status)?,
            },
        })
    }
}

fn intent_text(intent: OrderType) -> &'static str {
    match intent {
        OrderType::Buy => "buy",
        OrderType::Sell => "sell",
    }
}

fn parse_intent(text: &str) -> Result<OrderType, Error> {
    match text {
        "buy" => Ok(OrderType::Buy),
        "sell" => Ok(OrderType::Sell),
        other => Err(Error::Corrupt(format!("intent `{other}`"))),
    }
}

fn platform_text(platform: Platform) -> &'static str {
    match platform {
        Platform::Pc => "pc",
        Platform::Ps4 => "ps4",
        Platform::Xbox => "xbox",
        Platform::Switch => "switch",
        Platform::Mobile => "mobile",
    }
}

fn parse_platform(text: &str) -> Result<Platform, Error> {
    match text {
        "pc" => Ok(Platform::Pc),
        "ps4" => Ok(Platform::Ps4),
        "xbox" => Ok(Platform::Xbox),
        "switch" => Ok(Platform::Switch),
        "mobile" => Ok(Platform::Mobile),
        other => Err(Error::Corrupt(format!("platform `{other}`"))),
    }
}

fn status_text(status: Status) -> &'static str {
    match status {
        Status::Offline => "offline",
        Status::Online => "online",
        Status::Ingame => "ingame",
    }
}

fn parse_status(text: &str) -> Result<Status, Error> {
    match text {
        "offline" => Ok(Status::Offline),
        "online" => Ok(Status::Online),
        "ingame" => Ok(Status::Ingame),
        other => Err(Error::Corrupt(format!("status `{other}`"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory;

    fn filter() -> Filter {
        Filter {
            item_id: "6322918add457f0072aeb8b1".into(),
            intent: OrderType::Buy,
            price_limit: 50,
            platform: Platform::Pc,
            crossplay: true,
            rank: Some(7..=10),
            subtype: None,
            min_status: Status::Ingame,
        }
    }

    #[test]
    fn insert_then_list() {
        let conn = open_in_memory().unwrap();
        let id = insert(&conn, &filter()).unwrap();
        assert_eq!(
            list(&conn).unwrap(),
            [StoredFilter {
                id,
                enabled: true,
                cooldown_secs: 60,
                filter: filter()
            }]
        );
    }

    #[test]
    fn every_field_round_trips() {
        let conn = open_in_memory().unwrap();
        let relic = Filter {
            intent: OrderType::Sell,
            platform: Platform::Xbox,
            crossplay: false,
            rank: None,
            subtype: Some("radiant".into()),
            min_status: Status::Online,
            ..filter()
        };
        insert(&conn, &relic).unwrap();
        assert_eq!(list(&conn).unwrap()[0].filter, relic);
    }

    #[test]
    fn update_and_delete() {
        let conn = open_in_memory().unwrap();
        let id = insert(&conn, &filter()).unwrap();
        let mut stored = list(&conn).unwrap().remove(0);
        stored.enabled = false;
        stored.cooldown_secs = 300;
        stored.filter.price_limit = 40;
        update(&conn, &stored).unwrap();
        assert_eq!(list(&conn).unwrap(), [stored]);

        delete(&conn, id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
    }

    #[test]
    fn missing_ids_are_not_found() {
        let conn = open_in_memory().unwrap();
        assert!(matches!(delete(&conn, 99), Err(Error::NotFound(99))));
        let ghost = StoredFilter {
            id: 99,
            enabled: true,
            cooldown_secs: 60,
            filter: filter(),
        };
        assert!(matches!(update(&conn, &ghost), Err(Error::NotFound(99))));
    }

    #[test]
    fn unknown_text_is_an_error_not_a_panic() {
        let conn = open_in_memory().unwrap();
        let id = insert(&conn, &filter()).unwrap();
        conn.execute("UPDATE filters SET platform = 'stadia' WHERE id = ?1", [id])
            .unwrap();
        assert!(matches!(list(&conn), Err(Error::Corrupt(_))));
    }
}
