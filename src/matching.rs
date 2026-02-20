//! Price-time priority matching.
//!
//! [`match_order`] runs one order against the book: takes liquidity (respecting
//! self-trade prevention), produces trades and execution reports, and rests remainder for GTC.

use crate::execution::{ExecutionReport, Trade};
use crate::order_book::{Fill, OrderBook};
use crate::types::{ExecType, ExecutionId, Order, OrderStatus, Side, TimeInForce, TradeId};
use rust_decimal::Decimal;

/// Run matching for one order against the book. Price-time priority, partial fills, TIF (GTC/IOC/FOK), self-trade prevention.
/// Returns (trades, execution_reports). Reports include one per fill for resting orders and the aggressor's New/PartialFill/Fill or Canceled.
pub fn match_order(
    book: &mut OrderBook,
    order: &Order,
    next_trade_id: u64,
    next_exec_id: u64,
) -> (Vec<Trade>, Vec<ExecutionReport>) {
    let instrument_id = book.instrument_id();
    let mut trades = Vec::new();
    let mut reports = Vec::new();
    let mut exec_id = next_exec_id;
    let mut trade_id = next_trade_id;

    // Market order: use extreme price so we take all available liquidity
    let price_limit = match (order.side, order.price) {
        (Side::Buy, Some(p)) => p,
        (Side::Buy, None) => Decimal::MAX,
        (Side::Sell, Some(p)) => p,
        (Side::Sell, None) => Decimal::ZERO,
    };

    // FOK: must fill entirely or not at all
    let available = match order.side {
        Side::Buy => book.available_ask_qty_at_or_below(price_limit, order.trader_id),
        Side::Sell => book.available_bid_qty_at_or_above(price_limit, order.trader_id),
    };
    if matches!(order.time_in_force, TimeInForce::FOK) && available < order.quantity {
        reports.push(ExecutionReport {
            order_id: order.order_id,
            exec_id: ExecutionId(exec_id),
            exec_type: ExecType::Canceled,
            order_status: OrderStatus::Canceled,
            filled_quantity: Decimal::ZERO,
            remaining_quantity: order.quantity,
            avg_price: None,
            last_qty: None,
            last_px: None,
            timestamp: order.timestamp,
        });
        return (trades, reports);
    }

    let fills: Vec<Fill> = match order.side {
        Side::Buy => book.take_from_asks(price_limit, order.quantity, order.trader_id),
        Side::Sell => book.take_from_bids(price_limit, order.quantity, order.trader_id),
    };

    let mut filled_qty = Decimal::ZERO;
    let mut avg_px_sum = Decimal::ZERO;
    for f in &fills {
        filled_qty += f.quantity;
        avg_px_sum += f.price * f.quantity;
    }
    let avg_price = if filled_qty > Decimal::ZERO {
        Some(avg_px_sum / filled_qty)
    } else {
        None
    };
    let remaining = order.quantity - filled_qty;

    // Emit trades and execution reports for resting orders
    for f in &fills {
        let (buy_oid, sell_oid) = match order.side {
            Side::Buy => (order.order_id, f.resting_order_id),
            Side::Sell => (f.resting_order_id, order.order_id),
        };
        trades.push(Trade {
            trade_id: TradeId(trade_id),
            instrument_id,
            buy_order_id: buy_oid,
            sell_order_id: sell_oid,
            price: f.price,
            quantity: f.quantity,
            timestamp: order.timestamp,
            aggressor_side: order.side,
        });
        trade_id += 1;
        // Resting order report (PartialFill or Fill)
        reports.push(ExecutionReport {
            order_id: f.resting_order_id,
            exec_id: ExecutionId(exec_id),
            exec_type: if f.resting_fully_filled {
                ExecType::Fill
            } else {
                ExecType::PartialFill
            },
            order_status: if f.resting_fully_filled {
                OrderStatus::Filled
            } else {
                OrderStatus::PartiallyFilled
            },
            filled_quantity: f.quantity,
            remaining_quantity: Decimal::ZERO, // per-fill report; full state would require lookup
            avg_price: Some(f.price),
            last_qty: Some(f.quantity),
            last_px: Some(f.price),
            timestamp: order.timestamp,
        });
        exec_id += 1;
    }

    // Aggressor: New (if we have any fill we can send New first, then Fill or PartialFill)
    // IOC with no fill: emit only Canceled, then return (don't add to book)
    if fills.is_empty() && matches!(order.time_in_force, TimeInForce::IOC) {
        reports.push(ExecutionReport {
            order_id: order.order_id,
            exec_id: ExecutionId(exec_id),
            exec_type: ExecType::Canceled,
            order_status: OrderStatus::Canceled,
            filled_quantity: Decimal::ZERO,
            remaining_quantity: order.quantity,
            avg_price: None,
            last_qty: None,
            last_px: None,
            timestamp: order.timestamp,
        });
        return (trades, reports);
    }

    let aggressor_status = if remaining <= Decimal::ZERO {
        OrderStatus::Filled
    } else if filled_qty > Decimal::ZERO {
        OrderStatus::PartiallyFilled
    } else {
        OrderStatus::New
    };
    let aggressor_exec_type = if remaining <= Decimal::ZERO {
        ExecType::Fill
    } else if filled_qty > Decimal::ZERO {
        ExecType::PartialFill
    } else {
        ExecType::New
    };

    reports.push(ExecutionReport {
        order_id: order.order_id,
        exec_id: ExecutionId(exec_id),
        exec_type: aggressor_exec_type,
        order_status: aggressor_status,
        filled_quantity: filled_qty,
        remaining_quantity: remaining,
        avg_price,
        last_qty: fills.last().map(|f| f.quantity),
        last_px: fills.last().map(|f| f.price),
        timestamp: order.timestamp,
    });

    // GTC: add remainder to book. IOC/FOK: don't add (FOK reject already returned above).
    if remaining > Decimal::ZERO && matches!(order.time_in_force, TimeInForce::GTC) {
        if let Some(limit_price) = order.price {
            let mut rest_order = order.clone();
            rest_order.quantity = remaining;
            rest_order.price = Some(limit_price);
            let _ = book.add_order(&rest_order);
        }
    }

    (trades, reports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ExecType, InstrumentId, OrderId, OrderStatus, OrderType, TraderId};

    fn order(
        id: u64,
        side: Side,
        qty: i64,
        price: Option<i64>,
        tif: TimeInForce,
        trader: u64,
    ) -> Order {
        Order {
            order_id: OrderId(id),
            client_order_id: format!("c{}", id),
            instrument_id: InstrumentId(1),
            side,
            order_type: if price.is_some() {
                OrderType::Limit
            } else {
                OrderType::Market
            },
            quantity: Decimal::from(qty),
            price: price.map(Decimal::from),
            time_in_force: tif,
            timestamp: id,
            trader_id: TraderId(trader),
        }
    }

    #[test]
    fn placeholder_matching_returns_empty() {
        let mut book = OrderBook::new(InstrumentId(1));
        let order = Order {
            order_id: OrderId(1),
            client_order_id: "c1".into(),
            instrument_id: InstrumentId(1),
            side: Side::Buy,
            order_type: OrderType::Limit,
            quantity: Decimal::from(10),
            price: Some(Decimal::from(100)),
            time_in_force: TimeInForce::GTC,
            timestamp: 0,
            trader_id: TraderId(1),
        };
        let (trades, reports) = match_order(&mut book, &order, 1, 1);
        assert!(trades.is_empty());
        assert!(!reports.is_empty()); // we get at least New or one report
    }

    #[test]
    fn two_orders_match_full() {
        let mut book = OrderBook::new(InstrumentId(1));
        let sell = Order {
            order_id: OrderId(1),
            client_order_id: "c1".into(),
            instrument_id: InstrumentId(1),
            side: Side::Sell,
            order_type: OrderType::Limit,
            quantity: Decimal::from(10),
            price: Some(Decimal::from(100)),
            time_in_force: TimeInForce::GTC,
            timestamp: 1,
            trader_id: TraderId(1),
        };
        book.add_order(&sell).unwrap();
        let buy = Order {
            order_id: OrderId(2),
            client_order_id: "c2".into(),
            instrument_id: InstrumentId(1),
            side: Side::Buy,
            order_type: OrderType::Limit,
            quantity: Decimal::from(10),
            price: Some(Decimal::from(100)),
            time_in_force: TimeInForce::GTC,
            timestamp: 2,
            trader_id: TraderId(2),
        };
        let (trades, _reports) = match_order(&mut book, &buy, 1, 1);
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, Decimal::from(10));
        assert_eq!(trades[0].price, Decimal::from(100));
        assert!(book.best_bid().is_none());
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn partial_fill_then_rest_on_book() {
        let mut book = OrderBook::new(InstrumentId(1));
        let sell = Order {
            order_id: OrderId(1),
            client_order_id: "c1".into(),
            instrument_id: InstrumentId(1),
            side: Side::Sell,
            order_type: OrderType::Limit,
            quantity: Decimal::from(5),
            price: Some(Decimal::from(100)),
            time_in_force: TimeInForce::GTC,
            timestamp: 1,
            trader_id: TraderId(1),
        };
        book.add_order(&sell).unwrap();
        let buy = Order {
            order_id: OrderId(2),
            client_order_id: "c2".into(),
            instrument_id: InstrumentId(1),
            side: Side::Buy,
            order_type: OrderType::Limit,
            quantity: Decimal::from(10),
            price: Some(Decimal::from(100)),
            time_in_force: TimeInForce::GTC,
            timestamp: 2,
            trader_id: TraderId(2),
        };
        let (trades, _) = match_order(&mut book, &buy, 1, 1);
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, Decimal::from(5));
        // 5 remaining from buy should be on book
        assert_eq!(book.best_bid(), Some(Decimal::from(100)));
    }

    #[test]
    fn ioc_partial_fill_remainder_not_on_book() {
        let mut book = OrderBook::new(InstrumentId(1));
        book.add_order(&order(1, Side::Sell, 5, Some(100), TimeInForce::GTC, 1))
            .unwrap();
        let buy_ioc = order(2, Side::Buy, 10, Some(100), TimeInForce::IOC, 2);
        let (trades, reports) = match_order(&mut book, &buy_ioc, 1, 1);
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, Decimal::from(5));
        assert_eq!(trades[0].price, Decimal::from(100));
        // IOC: aggressor remainder (5) must not be on book
        assert!(book.best_bid().is_none());
        // Aggressor report: filled 5, remaining 5 (canceled)
        let aggressor = reports
            .iter()
            .find(|r| r.order_id == OrderId(2))
            .expect("aggressor report");
        assert_eq!(aggressor.filled_quantity, Decimal::from(5));
        assert_eq!(aggressor.remaining_quantity, Decimal::from(5));
    }

    #[test]
    fn fok_insufficient_liquidity_no_fill_canceled() {
        let mut book = OrderBook::new(InstrumentId(1));
        book.add_order(&order(1, Side::Sell, 5, Some(100), TimeInForce::GTC, 1))
            .unwrap();
        let buy_fok = order(2, Side::Buy, 10, Some(100), TimeInForce::FOK, 2);
        let (trades, reports) = match_order(&mut book, &buy_fok, 1, 1);
        assert!(trades.is_empty());
        let canceled = reports
            .iter()
            .find(|r| r.exec_type == ExecType::Canceled)
            .expect("Canceled report");
        assert_eq!(canceled.order_id, OrderId(2));
        assert_eq!(canceled.filled_quantity, Decimal::ZERO);
        assert_eq!(canceled.remaining_quantity, Decimal::from(10));
        // Resting sell still on book
        assert_eq!(book.best_ask(), Some(Decimal::from(100)));
    }

    #[test]
    fn self_trade_does_not_match() {
        let mut book = OrderBook::new(InstrumentId(1));
        book.add_order(&order(1, Side::Sell, 10, Some(100), TimeInForce::GTC, 1))
            .unwrap();
        let buy_same_trader = order(2, Side::Buy, 10, Some(100), TimeInForce::GTC, 1);
        let (trades, _) = match_order(&mut book, &buy_same_trader, 1, 1);
        assert!(trades.is_empty(), "self-trade must not match");
        assert_eq!(
            book.best_ask(),
            Some(Decimal::from(100)),
            "resting sell still on book"
        );
        assert_eq!(
            book.best_bid(),
            Some(Decimal::from(100)),
            "aggressor buy rested on book"
        );
    }

    #[test]
    fn price_time_priority_matches_earlier_order_first() {
        let mut book = OrderBook::new(InstrumentId(1));
        book.add_order(&order(1, Side::Sell, 5, Some(100), TimeInForce::GTC, 1))
            .unwrap();
        book.add_order(&order(2, Side::Sell, 5, Some(100), TimeInForce::GTC, 2))
            .unwrap();
        let buy = order(3, Side::Buy, 5, Some(100), TimeInForce::GTC, 3);
        let (trades, _) = match_order(&mut book, &buy, 1, 1);
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, Decimal::from(5));
        assert_eq!(
            trades[0].sell_order_id,
            OrderId(1),
            "must match first resting order (price-time)"
        );
        assert_eq!(trades[0].buy_order_id, OrderId(3));
    }

    #[test]
    fn market_order_takes_liquidity() {
        let mut book = OrderBook::new(InstrumentId(1));
        book.add_order(&order(1, Side::Sell, 10, Some(100), TimeInForce::GTC, 1))
            .unwrap();
        let market_buy = order(2, Side::Buy, 10, None, TimeInForce::GTC, 2);
        let (trades, reports) = match_order(&mut book, &market_buy, 1, 1);
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, Decimal::from(100));
        assert_eq!(trades[0].quantity, Decimal::from(10));
        assert_eq!(trades[0].buy_order_id, OrderId(2));
        assert_eq!(trades[0].sell_order_id, OrderId(1));
        let aggressor = reports
            .iter()
            .find(|r| r.order_id == OrderId(2))
            .expect("aggressor report");
        assert_eq!(aggressor.order_status, OrderStatus::Filled);
        assert_eq!(aggressor.filled_quantity, Decimal::from(10));
        assert!(book.best_ask().is_none());
    }

    /// Invariant: book is never crossed (best_bid < best_ask when both sides exist).
    fn assert_no_crossed_book(book: &OrderBook) {
        match (book.best_bid(), book.best_ask()) {
            (Some(bid), Some(ask)) => assert!(
                bid < ask,
                "invariant violated: best_bid {:?} >= best_ask {:?}",
                bid,
                ask
            ),
            _ => {}
        }
    }

    #[test]
    fn invariant_no_crossed_book_after_matching() {
        let mut book = OrderBook::new(InstrumentId(1));
        // Full fill: sell 10@100, buy 10@100
        book.add_order(&order(1, Side::Sell, 10, Some(100), TimeInForce::GTC, 1))
            .unwrap();
        let (_, _) = match_order(
            &mut book,
            &order(2, Side::Buy, 10, Some(100), TimeInForce::GTC, 2),
            1,
            1,
        );
        assert_no_crossed_book(&book);

        // Partial fill leaves bid: sell 5@100, buy 10@100
        let mut book = OrderBook::new(InstrumentId(1));
        book.add_order(&order(1, Side::Sell, 5, Some(100), TimeInForce::GTC, 1))
            .unwrap();
        let (_, _) = match_order(
            &mut book,
            &order(2, Side::Buy, 10, Some(100), TimeInForce::GTC, 2),
            1,
            1,
        );
        assert_no_crossed_book(&book);

        // Multiple levels: add bid 99 and ask 101, then match at 100
        let mut book = OrderBook::new(InstrumentId(1));
        book.add_order(&order(1, Side::Sell, 10, Some(101), TimeInForce::GTC, 1))
            .unwrap();
        book.add_order(&order(2, Side::Buy, 10, Some(99), TimeInForce::GTC, 2))
            .unwrap();
        book.add_order(&order(3, Side::Sell, 10, Some(100), TimeInForce::GTC, 3))
            .unwrap();
        let (_, _) = match_order(
            &mut book,
            &order(4, Side::Buy, 10, Some(100), TimeInForce::GTC, 4),
            1,
            1,
        );
        assert_no_crossed_book(&book);
    }

    #[test]
    fn invariant_no_negative_quantities_in_trades_and_reports() {
        let mut book = OrderBook::new(InstrumentId(1));
        book.add_order(&order(1, Side::Sell, 10, Some(100), TimeInForce::GTC, 1))
            .unwrap();
        let (trades, reports) = match_order(
            &mut book,
            &order(2, Side::Buy, 10, Some(100), TimeInForce::GTC, 2),
            1,
            1,
        );

        for t in &trades {
            assert!(
                t.quantity > Decimal::ZERO,
                "trade quantity must be positive"
            );
            assert!(t.price >= Decimal::ZERO, "trade price must be non-negative");
        }

        for r in &reports {
            assert!(
                r.filled_quantity >= Decimal::ZERO,
                "report filled_quantity must be non-negative"
            );
            assert!(
                r.remaining_quantity >= Decimal::ZERO,
                "report remaining_quantity must be non-negative"
            );
            if let Some(q) = r.last_qty {
                assert!(q >= Decimal::ZERO, "report last_qty must be non-negative");
            }
            if let Some(p) = r.last_px {
                assert!(p >= Decimal::ZERO, "report last_px must be non-negative");
            }
            if let Some(p) = r.avg_price {
                assert!(p >= Decimal::ZERO, "report avg_price must be non-negative");
            }
        }
    }

    #[test]
    fn partial_fill_resting_bid_leaves_remainder_on_book() {
        let mut book = OrderBook::new(InstrumentId(1));
        book.add_order(&order(1, Side::Buy, 10, Some(100), TimeInForce::GTC, 1))
            .unwrap();
        let sell = order(2, Side::Sell, 5, Some(100), TimeInForce::GTC, 2);
        let (trades, _) = match_order(&mut book, &sell, 1, 1);
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, Decimal::from(5));
        assert_eq!(trades[0].sell_order_id, OrderId(2));
        assert_eq!(trades[0].buy_order_id, OrderId(1));
        assert_eq!(book.best_bid(), Some(Decimal::from(100)));
    }

    #[test]
    fn fok_sell_insufficient_liquidity_no_fill_canceled() {
        let mut book = OrderBook::new(InstrumentId(1));
        book.add_order(&order(1, Side::Buy, 5, Some(100), TimeInForce::GTC, 1))
            .unwrap();
        let sell_fok = order(2, Side::Sell, 10, Some(100), TimeInForce::FOK, 2);
        let (trades, reports) = match_order(&mut book, &sell_fok, 1, 1);
        assert!(trades.is_empty());
        let canceled = reports
            .iter()
            .find(|r| r.exec_type == ExecType::Canceled)
            .expect("Canceled report");
        assert_eq!(canceled.order_id, OrderId(2));
        assert_eq!(book.best_bid(), Some(Decimal::from(100)));
    }
}
