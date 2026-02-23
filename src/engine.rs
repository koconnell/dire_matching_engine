//! Single-entry matching engine facade.
//!
//! Holds the order book and ID counters so Phase 2 (protocol layer) can submit orders
//! without managing `OrderBook` and `match_order` directly. All protocol adapters (REST,
//! WebSocket, FIX) use the same entry point: [`Engine`] behind shared state ([`crate::api::AppState`]).

use crate::execution::{ExecutionReport, Trade};
use crate::matching::match_order;
use crate::order_book::OrderBook;
use crate::types::{InstrumentId, Order, OrderId};
use log::info;
use rust_decimal::Decimal;

// ---------------------------------------------------------------------------
// Protocol abstraction (Phase 2): trait used by REST, WebSocket, FIX adapters
// ---------------------------------------------------------------------------

/// Top-of-book snapshot for market data (e.g. WebSocket snapshot).
#[derive(Clone, Debug)]
pub struct BookSnapshot {
    pub instrument_id: InstrumentId,
    pub best_bid: Option<Decimal>,
    pub best_ask: Option<Decimal>,
}

/// Service interface for the matching engine. All protocol adapters (REST, WebSocket, FIX)
/// call these operations on the same [`Engine`] instance (see [`crate::api::AppState`]).
pub trait MatchingEngine {
    /// Submit an order; returns trades and execution reports.
    fn submit_order(&mut self, order: Order) -> Result<(Vec<Trade>, Vec<ExecutionReport>), String>;

    /// Cancel a resting order by id. Returns `true` if found and removed.
    fn cancel_order(&mut self, order_id: OrderId) -> bool;

    /// Modify: cancel by `order_id`, then match the replacement. Returns trades and reports.
    fn modify_order(
        &mut self,
        order_id: OrderId,
        replacement: &Order,
    ) -> Result<(Vec<Trade>, Vec<ExecutionReport>), String>;

    /// Instrument this engine handles.
    fn instrument_id(&self) -> InstrumentId;

    /// Best bid price, if any.
    fn best_bid(&self) -> Option<Decimal>;

    /// Best ask price, if any.
    fn best_ask(&self) -> Option<Decimal>;

    /// Current top-of-book snapshot (optional; used by WebSocket market data).
    fn book_snapshot(&self) -> BookSnapshot {
        BookSnapshot {
            instrument_id: self.instrument_id(),
            best_bid: self.best_bid(),
            best_ask: self.best_ask(),
        }
    }
}

impl MatchingEngine for Engine {
    fn submit_order(&mut self, order: Order) -> Result<(Vec<Trade>, Vec<ExecutionReport>), String> {
        Engine::submit_order(self, order)
    }

    fn cancel_order(&mut self, order_id: OrderId) -> bool {
        Engine::cancel_order(self, order_id)
    }

    fn modify_order(
        &mut self,
        order_id: OrderId,
        replacement: &Order,
    ) -> Result<(Vec<Trade>, Vec<ExecutionReport>), String> {
        Engine::modify_order(self, order_id, replacement)
    }

    fn instrument_id(&self) -> InstrumentId {
        Engine::instrument_id(self)
    }

    fn best_bid(&self) -> Option<Decimal> {
        Engine::best_bid(self)
    }

    fn best_ask(&self) -> Option<Decimal> {
        Engine::best_ask(self)
    }
}

// ---------------------------------------------------------------------------

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
        info!(
            "order submitted order_id={} side={:?} quantity={} price={:?}",
            order.order_id.0,
            order.side,
            order.quantity,
            order.price
        );
        if order.instrument_id != self.instrument_id {
            return Err(format!(
                "Order instrument does not match engine instrument"
            ));
        }
        if order.is_limit() && order.price.is_none() {
            return Err("Limit order must have price".into());
        }
        let (trades, reports) = match_order(
            &mut self.book,
            &order,
            self.next_trade_id,
            self.next_exec_id,
        );
        for report in &reports {
            info!(
                "execution_report order_id={} exec_type={:?} order_status={:?} filled={} remaining={}",
                report.order_id.0,
                report.exec_type,
                report.order_status,
                report.filled_quantity,
                report.remaining_quantity
            );
        }
        for trade in &trades {
            info!(
                "trade trade_id={} buy_order={} sell_order={} price={} quantity={}",
                trade.trade_id.0,
                trade.buy_order_id.0,
                trade.sell_order_id.0,
                trade.price,
                trade.quantity
            );
        }
        self.next_trade_id += trades.len() as u64;
        self.next_exec_id += reports.len() as u64;
        Ok((trades, reports))
    }

    /// Cancels a resting order by id. Returns `true` if the order was found and removed.
    pub fn cancel_order(&mut self, order_id: crate::types::OrderId) -> bool {
        let removed = self.book.cancel_order(order_id);
        if removed {
            info!("order canceled order_id={}", order_id.0);
        }
        removed
    }

    /// Modifies an order: cancel by `order_id`, then run matching on the replacement.
    /// Replacement may use the same or a new order id. Price-time is preserved: any
    /// resting quantity from the replacement goes to the back of its price level.
    /// Returns trades and execution reports from matching the replacement.
    pub fn modify_order(
        &mut self,
        order_id: crate::types::OrderId,
        replacement: &Order,
    ) -> Result<(Vec<Trade>, Vec<ExecutionReport>), String> {
        if replacement.instrument_id != self.instrument_id {
            return Err("Replacement order must be for the same instrument".into());
        }
        if !self.book.cancel_order(order_id) {
            return Err(format!("Order {} not found", order_id.0));
        }
        info!(
            "order modified old_order_id={} replacement order_id={} side={:?} quantity={} price={:?}",
            order_id.0,
            replacement.order_id.0,
            replacement.side,
            replacement.quantity,
            replacement.price
        );
        let (trades, reports) = match_order(
            &mut self.book,
            replacement,
            self.next_trade_id,
            self.next_exec_id,
        );
        for report in &reports {
            info!(
                "execution_report order_id={} exec_type={:?} order_status={:?} filled={} remaining={}",
                report.order_id.0,
                report.exec_type,
                report.order_status,
                report.filled_quantity,
                report.remaining_quantity
            );
        }
        for trade in &trades {
            info!(
                "trade trade_id={} buy_order={} sell_order={} price={} quantity={}",
                trade.trade_id.0,
                trade.buy_order_id.0,
                trade.sell_order_id.0,
                trade.price,
                trade.quantity
            );
        }
        self.next_trade_id += trades.len() as u64;
        self.next_exec_id += reports.len() as u64;
        Ok((trades, reports))
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

    fn init_log() {
        let _ = env_logger::try_init();
    }

    #[test]
    fn engine_submit_order_matches_and_returns_trades() {
        init_log();
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
        init_log();
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

    #[test]
    fn engine_submit_order_limit_without_price_rejected() {
        init_log();
        let mut engine = Engine::new(InstrumentId(1));
        let order = Order {
            order_id: OrderId(1),
            client_order_id: "c1".into(),
            instrument_id: InstrumentId(1),
            side: Side::Buy,
            order_type: OrderType::Limit,
            quantity: Decimal::from(10),
            price: None,
            time_in_force: TimeInForce::GTC,
            timestamp: 1,
            trader_id: TraderId(1),
        };
        let err = engine.submit_order(order).unwrap_err();
        assert!(err.to_lowercase().contains("price"));
    }

    #[test]
    fn engine_order_flow_submit_then_cancel() {
        init_log();
        let mut engine = Engine::new(InstrumentId(1));
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
        engine.submit_order(sell).unwrap();
        let canceled = engine.cancel_order(OrderId(1));
        assert!(canceled);
        assert!(engine.best_ask().is_none(), "cancel resting: book no longer has that order");
    }

    #[test]
    fn engine_modify_then_incoming_matches() {
        init_log();
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
        let replacement = Order {
            order_id: OrderId(1),
            client_order_id: "c1".into(),
            instrument_id: InstrumentId(1),
            side: Side::Sell,
            order_type: OrderType::Limit,
            quantity: Decimal::from(5),
            price: Some(Decimal::from(100)),
            time_in_force: TimeInForce::GTC,
            timestamp: 2,
            trader_id: TraderId(1),
        };
        engine.modify_order(OrderId(1), &replacement).unwrap();
        let buy = Order {
            order_id: OrderId(2),
            client_order_id: "c2".into(),
            instrument_id: InstrumentId(1),
            side: Side::Buy,
            order_type: OrderType::Limit,
            quantity: Decimal::from(5),
            price: Some(Decimal::from(100)),
            time_in_force: TimeInForce::GTC,
            timestamp: 3,
            trader_id: TraderId(2),
        };
        let (trades, _) = engine.submit_order(buy).unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, Decimal::from(5));
        assert!(engine.best_ask().is_none());
        assert!(engine.best_bid().is_none());
    }

    #[test]
    fn engine_modify_order_replacement_rests_and_returns_reports() {
        init_log();
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
        let replacement = Order {
            order_id: OrderId(1),
            client_order_id: "c1".into(),
            instrument_id: InstrumentId(1),
            side: Side::Sell,
            order_type: OrderType::Limit,
            quantity: Decimal::from(5),
            price: Some(Decimal::from(100)),
            time_in_force: TimeInForce::GTC,
            timestamp: 2,
            trader_id: TraderId(1),
        };
        let (trades, reports) = engine.modify_order(OrderId(1), &replacement).unwrap();
        assert_eq!(trades.len(), 0);
        assert!(!reports.is_empty());
        assert_eq!(engine.best_ask(), Some(Decimal::from(100)));
    }

    #[test]
    fn engine_modify_order_not_found_returns_err() {
        init_log();
        let mut engine = Engine::new(InstrumentId(1));
        let replacement = Order {
            order_id: OrderId(2),
            client_order_id: "c2".into(),
            instrument_id: InstrumentId(1),
            side: Side::Sell,
            order_type: OrderType::Limit,
            quantity: Decimal::from(5),
            price: Some(Decimal::from(100)),
            time_in_force: TimeInForce::GTC,
            timestamp: 1,
            trader_id: TraderId(1),
        };
        let err = engine.modify_order(OrderId(999), &replacement).unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn engine_modify_order_wrong_instrument_returns_err() {
        init_log();
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
        let replacement = Order {
            order_id: OrderId(1),
            client_order_id: "c1".into(),
            instrument_id: InstrumentId(2),
            side: Side::Sell,
            order_type: OrderType::Limit,
            quantity: Decimal::from(5),
            price: Some(Decimal::from(100)),
            time_in_force: TimeInForce::GTC,
            timestamp: 2,
            trader_id: TraderId(1),
        };
        let err = engine.modify_order(OrderId(1), &replacement).unwrap_err();
        assert!(err.contains("same instrument"));
    }
}
