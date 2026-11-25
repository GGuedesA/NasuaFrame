use std::ops::RangeInclusive;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Buy,
    Sell,
}

impl OrderType {
    /// buyers trade with sellers
    pub fn counterpart(self) -> Self {
        match self {
            OrderType::Buy => OrderType::Sell,
            OrderType::Sell => OrderType::Buy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Pc,
    Ps4,
    Xbox,
    Switch,
    Mobile,
}

/// least to most available so `>=` works, the api reports invisible as offline
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Offline,
    Online,
    Ingame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trader {
    pub ingame_name: String,
    pub platform: Platform,
    pub crossplay: bool,
    pub status: Status,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub name_en: String,
    /// none for unranked items
    pub max_rank: Option<u8>,
    /// relic refinements and such, empty for most items
    pub subtypes: Vec<String>,
    pub ducats: Option<u16>,
    pub game_ref: String,
}

/// `intent` is what the user wants, so a buy filter matches sell orders
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub item_id: String,
    pub intent: OrderType,
    /// max when buying, min when selling
    pub price_limit: u32,
    pub platform: Platform,
    pub crossplay: bool,
    /// none accepts any rank
    pub rank: Option<RangeInclusive<u8>>,
    pub subtype: Option<String>,
    pub min_status: Status,
}
