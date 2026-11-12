//! Parsing of WebSocket messages into `nasua_core` types
//!
//! The wire types are private: they mirror the JSON, then convert into the core model,
//! so `nasua-core` stays free of serde and API naming

use nasua_core::model::{Order, OrderType, Platform, Status, Trader};
use serde::Deserialize;

/// Route of the event the `newOrders` subscription sends for each new order
pub const NEW_ORDER_EVENT: &str = "@wfm|event/subscriptions/newOrder";

/// Parses one text frame from the socket
///
/// `Ok(None)`: valid message that is not a new order (reports, command replies, ...).
/// `Err`: not JSON, or a new order with a missing field or an unknown platform/status.
/// Callers should log the error and keep reading, never stop the stream
pub fn parse_new_order(text: &str) -> Result<Option<Order>, serde_json::Error> {
    let envelope: Envelope = serde_json::from_str(text)?;
    if envelope.route != NEW_ORDER_EVENT {
        return Ok(None);
    }
    let wire: WireOrder = serde_json::from_value(envelope.payload)?;
    Ok(Some(wire.into()))
}

#[derive(Deserialize)]
struct Envelope {
    route: String,
    #[serde(default)]
    payload: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireOrder {
    id: String,
    item_id: String,
    #[serde(rename = "type")]
    order_type: WireOrderType,
    platinum: u32,
    rank: Option<u8>,
    subtype: Option<String>,
    user: WireUser,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireUser {
    ingame_name: String,
    platform: WirePlatform,
    crossplay: bool,
    status: WireStatus,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum WireOrderType {
    Buy,
    Sell,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum WirePlatform {
    Pc,
    Ps4,
    Xbox,
    Switch,
    Mobile,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum WireStatus {
    #[serde(alias = "invisible")]
    Offline,
    Online,
    Ingame,
}

impl From<WireOrder> for Order {
    fn from(w: WireOrder) -> Self {
        Order {
            id: w.id,
            item_id: w.item_id,
            order_type: match w.order_type {
                WireOrderType::Buy => OrderType::Buy,
                WireOrderType::Sell => OrderType::Sell,
            },
            platinum: w.platinum,
            rank: w.rank,
            subtype: w.subtype,
            user: Trader {
                ingame_name: w.user.ingame_name,
                platform: match w.user.platform {
                    WirePlatform::Pc => Platform::Pc,
                    WirePlatform::Ps4 => Platform::Ps4,
                    WirePlatform::Xbox => Platform::Xbox,
                    WirePlatform::Switch => Platform::Switch,
                    WirePlatform::Mobile => Platform::Mobile,
                },
                crossplay: w.user.crossplay,
                status: match w.user.status {
                    WireStatus::Offline => Status::Offline,
                    WireStatus::Online => Status::Online,
                    WireStatus::Ingame => Status::Ingame,
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real message from probe run, order and user anon
    const REAL: &str = r#"{"route":"@wfm|event/subscriptions/newOrder","payload":{"id":"000000000000000000000001","type":"sell","platinum":75,"quantity":1,"perTrade":1,"visible":true,"createdAt":"2026-11-11T18:42:13Z","updatedAt":"2026-11-11T18:42:13Z","itemId":"62d3494075156700ce450df4","user":{"id":"000000000000000000000002","ingameName":"TestTrader","slug":"TestTrader","reputation":1,"platform":"pc","crossplay":true,"locale":"pt","status":"ingame","activity":null,"lastSeen":"2026-11-11T18:41:52Z"}}}"#;

    #[test]
    fn parses_real_message() {
        let order = parse_new_order(REAL).unwrap().unwrap();
        assert_eq!(
            order,
            Order {
                id: "000000000000000000000001".into(),
                item_id: "62d3494075156700ce450df4".into(),
                order_type: OrderType::Sell,
                platinum: 75,
                rank: None,
                subtype: None,
                user: Trader {
                    ingame_name: "TestTrader".into(),
                    platform: Platform::Pc,
                    crossplay: true,
                    status: Status::Ingame,
                },
            }
        );
    }

    #[test]
    fn reads_rank_subtype_and_invisible() {
        let text = REAL
            .replace(
                r#""platinum":75,"#,
                r#""platinum":75,"rank":3,"subtype":"radiant","#,
            )
            .replace(r#""status":"ingame""#, r#""status":"invisible""#);
        let order = parse_new_order(&text).unwrap().unwrap();
        assert_eq!(order.rank, Some(3));
        assert_eq!(order.subtype.as_deref(), Some("radiant"));
        assert_eq!(order.user.status, Status::Offline);
    }

    #[test]
    fn other_routes_are_ignored() {
        let report = r#"{"route":"@wfm|event/reports/online","payload":{"connections":1,"authorizedUsers":0}}"#;
        assert_eq!(parse_new_order(report).unwrap(), None);
        assert_eq!(
            parse_new_order(r#"{"route":"@wfm|cmd/subscribe/newOrders:ok"}"#).unwrap(),
            None
        );
    }

    #[test]
    fn unknown_values_and_garbage_are_errors() {
        assert!(parse_new_order(&REAL.replace(r#""pc""#, r#""stadia""#)).is_err());
        assert!(parse_new_order(&REAL.replace(r#""ingame""#, r#""away""#)).is_err());
        assert!(parse_new_order("not json").is_err());
    }
}
