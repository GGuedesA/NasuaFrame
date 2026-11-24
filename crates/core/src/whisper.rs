use crate::model::{Order, OrderType};

/// same text as the site's copy button
pub fn build_whisper(order: &Order, item_name: &str) -> String {
    let action = match order.order_type {
        OrderType::Sell => "buy",
        OrderType::Buy => "sell",
    };
    let mut item = match &order.subtype {
        Some(subtype) => format!("{} {item_name}", capitalize(subtype)),
        None => item_name.to_owned(),
    };
    if let Some(rank) = order.rank {
        item = format!("{item} (rank {rank})");
    }
    format!(
        "/w {} Hi! I want to {action}: \"{item}\" for {} platinum. (warframe.market)",
        order.user.ingame_name, order.platinum
    )
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
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
    fn rank_zero_is_still_shown() {
        let order = order(OrderType::Sell, "TestTrader", 23, Some(0));
        assert_eq!(
            build_whisper(&order, "Archon Intensify"),
            r#"/w TestTrader Hi! I want to buy: "Archon Intensify (rank 0)" for 23 platinum. (warframe.market)"#
        );
    }

    #[test]
    fn subtype_is_a_capitalized_prefix() {
        let relic = |subtype: &str| Order {
            subtype: Some(subtype.into()),
            ..order(OrderType::Sell, "TestTrader", 30, None)
        };
        assert_eq!(
            build_whisper(&relic("intact"), "Neo A1 Relic"),
            r#"/w TestTrader Hi! I want to buy: "Intact Neo A1 Relic" for 30 platinum. (warframe.market)"#
        );
        assert_eq!(
            build_whisper(&relic("radiant"), "Meso A6 Relic"),
            r#"/w TestTrader Hi! I want to buy: "Radiant Meso A6 Relic" for 30 platinum. (warframe.market)"#
        );
    }
}
