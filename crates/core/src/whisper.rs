//! Builds the in-game whisper, in the same format as warframe.market's "copy" button.

use crate::model::{Order, OrderType};

/// The `/w` message the user pastes into Warframe chat to answer `order`
/// `item_name` is the item's display name (e.g. "Archon Intensify")
pub fn build_whisper(order: &Order, item_name: &str) -> String {
    // Answering a sell order means the user buys, and the other way round
    let action = match order.order_type {
        OrderType::Sell => "buy",
        OrderType::Buy => "sell",
    };
    let item = match order.rank {
        Some(rank) => format!("{item_name} (rank {rank})"),
        None => item_name.to_owned(),
    };
    format!(
        "/w {} Hi! I want to {action}: \"{item}\" for {} platinum. (warframe.market)",
        order.user.ingame_name, order.platinum
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Platform, Status, Trader};

    fn order(order_type: OrderType, name: &str, platinum: u32, rank: Option<u8>) -> Order {
        Order {
            id: "id".into(),
            item_id: "item".into(),
            order_type,
            platinum,
            rank,
            subtype: None,
            user: Trader {
                ingame_name: name.into(),
                platform: Platform::Pc,
                crossplay: true,
                status: Status::Ingame,
            },
        }
    }

    #[test]
    fn answering_a_sell_order() {
        // name should be anon on tests aaaaaa
        let order = order(OrderType::Sell, "TestTrader", 35, Some(7));
        assert_eq!(
            build_whisper(&order, "Archon Intensify"),
            r#"/w TestTrader Hi! I want to buy: "Archon Intensify (rank 7)" for 35 platinum. (warframe.market)"#
        );
    }

    #[test]
    fn answering_a_buy_order() {
        let order = order(OrderType::Buy, "TestBuyer", 60, Some(10));
        assert_eq!(
            build_whisper(&order, "Archon Intensify"),
            r#"/w TestBuyer Hi! I want to sell: "Archon Intensify (rank 10)" for 60 platinum. (warframe.market)"#
        );
    }

    #[test]
    fn unranked_items_have_no_rank_suffix() {
        let order = order(OrderType::Sell, "someone", 20, None);
        assert_eq!(
            build_whisper(&order, "Neo A1 Relic"),
            r#"/w someone Hi! I want to buy: "Neo A1 Relic" for 20 platinum. (warframe.market)"#
        );
    }
}
