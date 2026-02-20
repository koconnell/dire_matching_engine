//! Single-entry matching engine facade.
//!
//! Holds the order book and ID counters so Phase 2 (protocol layer) can submit orders
//! without managing `OrderBook` and `match_order` directly.

use crate::execution::{ExecutionReport, Trade};
use crate::matching::match_order;
use crate::order_book::OrderBook;
use crate::types::{InstrumentId, Order};

/// Single-instrument matching engine.
///
/// Use [`Engine::submit_order`] to send orders; the engine runs matching and returns
/// trades and execution reports. Use [`Engine::cancel_order`] and [`Engine::modify_order`]
/// to change resting orders.
#[derive(Debug)]
pub struct Engine {
    instrument_id: InstrumentId,
    book: OrderBook,
    next_trade_id: u64,
    next_exec_id: u64,
}

impl Engine {
    /// Creates an engine for the given instrument.
    pub fn new(instrument_id: InstrumentId) -> Self {
        Self {
            instrument_id,
            book: OrderBook::new(instrument_id),
            next_trade_id: 1,
            next_exec_id: 1,
        }
    }

    /// Submits an order: runs matching and returns trades and execution reports.
    ///
    /// Returns `Err` if the order is for a different instrument.
    pub fn submit_order(&mut self, order: Order) -> Result<(Vec<Trade>, Vec<ExecutionReport>), String> {
        if order.instrument_id != self.instrument_id {
            return Err(format!(
                "Order instrument does not match engine instrument"
            ));
        }
        let (trades, reports) = match_order(
            &mut self.book,
            &order,
            self.next_trade_id,
            self.next_exec_id,
        );
        self.next_trade_id += trades.len() as u64;
        self.next_exec_id += reports.len() as u64;
        Ok((trades, reports))
    }

    /// Cancels a resting order by id. Returns `true` if the order was found and removed.
    pub fn cancel_order(&mut self, order_id: crate::types::OrderId) -> bool {
        self.book.cancel_order(order_id)
    }

    /// Modifies an order: cancel by `order_id`, then add the replacement.
    /// Replacement may use the same or a new order id.
    pub fn modify_order(
        &mut self,
        order_id: crate::types::OrderId,
        replacement: &Order,
    ) -> Result<(), String> {
        if replacement.instrument_id != self.instrument_id {
            return Err("Replacement order must be for the same instrument".into());
        }
        self.book.modify_order(order_id, replacement)
    }

    /// Returns the instrument this engine handles.
    pub fn instrument_id(&self) -> InstrumentId {
        self.instrument_id
    }

    /// Best bid price, if any.
    pub fn best_bid(&self) -> Option<rust_decimal::Decimal> {
        self.book.best_bid()
    }

    /// Best ask price, if any.
    pub fn best_ask(&self) -> Option<rust_decimal::Decimal> {
        self.book.best_ask()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Order, OrderId, OrderType, Side, TimeInForce, TraderId};
    use rust_decimal::Decimal;

    #[test]
    fn engine_submit_order_matches_and_returns_trades() {
        let mut engine = Engine::new(InstrumentId(1));
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
        engine.submit_order(sell).unwrap();
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
        let (trades, reports) = engine.submit_order(buy).unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, Decimal::from(10));
        assert!(!reports.is_empty());
    }

    #[test]
    fn engine_submit_order_wrong_instrument_returns_err() {
        let mut engine = Engine::new(InstrumentId(1));
        let order = Order {
            order_id: OrderId(1),
            client_order_id: "c1".into(),
            instrument_id: InstrumentId(2),
            side: Side::Buy,
            order_type: OrderType::Limit,
            quantity: Decimal::from(10),
            price: Some(Decimal::from(100)),
            time_in_force: TimeInForce::GTC,
            timestamp: 1,
            trader_id: TraderId(1),
        };
        assert!(engine.submit_order(order).is_err());
    }
}
