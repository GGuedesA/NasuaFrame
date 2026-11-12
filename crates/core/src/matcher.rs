//! Decides whether an order is worth an alert for a filter.

use crate::model::{Filter, Order, OrderType, Trader};

/// True when `order` satisfies every rule of `filter`
pub fn matches(filter: &Filter, order: &Order) -> bool {
    order.item_id == filter.item_id
        && order.order_type == filter.intent.counterpart()
        && price_ok(filter, order.platinum)
        && can_trade(filter, &order.user)
        && order.user.status >= filter.min_status
        && filter
            .rank
            .as_ref()
            .is_none_or(|range| order.rank.is_some_and(|rank| range.contains(&rank)))
        && filter
            .subtype
            .as_ref()
            .is_none_or(|wanted| order.subtype.as_ref() == Some(wanted))
}

fn price_ok(filter: &Filter, platinum: u32) -> bool {
    match filter.intent {
        OrderType::Buy => platinum <= filter.price_limit,
        OrderType::Sell => platinum >= filter.price_limit,
    }
}

/// Same platform always works, across platforms both sides need crossplay on
fn can_trade(filter: &Filter, trader: &Trader) -> bool {
    trader.platform == filter.platform || (filter.crossplay && trader.crossplay)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Platform, Status};

    const INTENSIFY: &str = "6322918add457f0072aeb8b1";

    fn buy_filter() -> Filter {
        Filter {
            item_id: INTENSIFY.into(),
            intent: OrderType::Buy,
            price_limit: 50,
            platform: Platform::Pc,
            crossplay: true,
            rank: None,
            subtype: None,
            min_status: Status::Online,
        }
    }

    fn sell_order() -> Order {
        Order {
            id: "000000000000000000000001".into(),
            item_id: INTENSIFY.into(),
            order_type: OrderType::Sell,
            platinum: 35,
            rank: Some(7),
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
    fn matches_a_cheap_sell_order() {
        assert!(matches(&buy_filter(), &sell_order()));
    }

    #[test]
    fn rejects_other_items() {
        let order = Order {
            item_id: "other".into(),
            ..sell_order()
        };
        assert!(!matches(&buy_filter(), &order));
    }

    #[test]
    fn buyers_ignore_buy_orders() {
        let order = Order {
            order_type: OrderType::Buy,
            ..sell_order()
        };
        assert!(!matches(&buy_filter(), &order));
    }

    #[test]
    fn buy_limit_is_a_maximum_and_inclusive() {
        let filter = buy_filter();
        assert!(matches(
            &filter,
            &Order {
                platinum: 50,
                ..sell_order()
            }
        ));
        assert!(!matches(
            &filter,
            &Order {
                platinum: 51,
                ..sell_order()
            }
        ));
    }

    #[test]
    fn sell_limit_is_a_minimum_and_inclusive() {
        let filter = Filter {
            intent: OrderType::Sell,
            price_limit: 60,
            ..buy_filter()
        };
        let buy = Order {
            order_type: OrderType::Buy,
            ..sell_order()
        };
        assert!(matches(
            &filter,
            &Order {
                platinum: 60,
                ..buy.clone()
            }
        ));
        assert!(!matches(
            &filter,
            &Order {
                platinum: 59,
                ..buy
            }
        ));
    }

    #[test]
    fn cross_platform_needs_crossplay_on_both_sides() {
        let xbox = |crossplay| Order {
            user: Trader {
                platform: Platform::Xbox,
                crossplay,
                ..sell_order().user
            },
            ..sell_order()
        };
        assert!(matches(&buy_filter(), &xbox(true)));
        assert!(!matches(&buy_filter(), &xbox(false)));
        let no_crossplay = Filter {
            crossplay: false,
            ..buy_filter()
        };
        assert!(!matches(&no_crossplay, &xbox(true)));
    }

    #[test]
    fn status_must_be_at_least_the_minimum() {
        let with_status = |status| Order {
            user: Trader {
                status,
                ..sell_order().user
            },
            ..sell_order()
        };
        assert!(matches(&buy_filter(), &with_status(Status::Online)));
        assert!(!matches(&buy_filter(), &with_status(Status::Offline)));
        let ingame_only = Filter {
            min_status: Status::Ingame,
            ..buy_filter()
        };
        assert!(!matches(&ingame_only, &with_status(Status::Online)));
    }

    #[test]
    fn rank_range_is_inclusive_and_needs_a_rank() {
        let filter = Filter {
            rank: Some(7..=10),
            ..buy_filter()
        };
        assert!(matches(&filter, &sell_order()));
        assert!(matches(
            &filter,
            &Order {
                rank: Some(10),
                ..sell_order()
            }
        ));
        assert!(!matches(
            &filter,
            &Order {
                rank: Some(6),
                ..sell_order()
            }
        ));
        assert!(!matches(
            &filter,
            &Order {
                rank: None,
                ..sell_order()
            }
        ));
    }

    #[test]
    fn subtype_must_match_when_set() {
        let filter = Filter {
            subtype: Some("radiant".into()),
            ..buy_filter()
        };
        let with = |subtype: Option<&str>| Order {
            subtype: subtype.map(Into::into),
            ..sell_order()
        };
        assert!(matches(&filter, &with(Some("radiant"))));
        assert!(!matches(&filter, &with(Some("intact"))));
        assert!(!matches(&filter, &with(None)));
        assert!(matches(&buy_filter(), &with(Some("intact"))));
    }
}
