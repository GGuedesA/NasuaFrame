//! Domain types shared by the matcher, the whisper builder and (later) storage and UI.

use std::ops::RangeInclusive;

/// Which side of a trade an order is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Buy,
    Sell,
}

impl OrderType {
    /// The order type on the other side of the trade: buyers trade with sellers (must not be confused on front)
    pub fn counterpart(self) -> Self {
        match self {
            OrderType::Buy => OrderType::Sell,
            OrderType::Sell => OrderType::Buy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Pc,
    Ps4,
    Xbox,
    Switch,
    Mobile,
}

/// A player's online status, declared from least to most available, so `>=` compares
/// availability (`Ingame >= Online` is true), the API reports `invisible` as `offline`
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Offline,
    Online,
    Ingame,
}

/// player who posted an order
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trader {
    pub ingame_name: String,
    pub platform: Platform,
    pub crossplay: bool,
    pub status: Status,
}

/// order from the warframe.market stream or REST API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub id: String,
    pub item_id: String,
    pub order_type: OrderType,
    pub platinum: u32,
    pub rank: Option<u8>,
    pub subtype: Option<String>,
    pub user: Trader,
}

/// tradable item from the market catalog
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub slug: String,
    pub name: String,
    /// None for items without ranks
    pub max_rank: Option<u8>,
    /// relic refinements and such, empty for most items
    pub subtypes: Vec<String>,
}

/// What the user is looking for, `intent` is what the USER wants to do, so a `Buy`
/// filter matches other players' `Sell` orders
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub item_id: String,
    pub intent: OrderType,
    /// Max price when buying, min price when selling
    pub price_limit: u32,
    pub platform: Platform,
    pub crossplay: bool,
    /// `None` accepts any rank, including unranked items.
    pub rank: Option<RangeInclusive<u8>>,
    /// `None` accepts any subtype.
    pub subtype: Option<String>,
    /// Least available status still worth alerting for (maybe, needs feedback)
    pub min_status: Status,
}
