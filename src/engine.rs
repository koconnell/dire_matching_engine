//! Single-entry matching engine facade.
//!
//! Holds the order book and ID counters so Phase 2 (protocol layer) can submit orders
//! without managing `OrderBook` and `match_order` directly. All protocol adapters (REST,
//! WebSocket, FIX) use the same entry point: [`Engine`] or [`MultiEngine`] behind shared state ([`crate::api::AppState`]).

use crate::execution::{ExecutionReport, Trade};
use crate::matching::match_order;
use crate::order_book::OrderBook;
use crate::types::{InstrumentId, Order, OrderId, RestingOrder};
use log::info;
use rust_decimal::Decimal;
use std::collections::HashMap;

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
/// call these operations on the same engine instance (see [`crate::api::AppState`]).
pub trait MatchingEngine {
    /// Submit an order; returns trades and execution reports.
    fn submit_order(&mut self, order: Order) -> Result<(Vec<Trade>, Vec<ExecutionReport>), String>;

    /// Cancel a resting order by id. Returns `Some(instrument_id)` if found and removed (for broadcasting that instrument's update), `None` if not found.
    fn cancel_order(&mut self, order_id: OrderId) -> Option<InstrumentId>;

    /// Modify: cancel by `order_id`, then match the replacement. Returns trades and reports.
    fn modify_order(
        &mut self,
        order_id: OrderId,
        replacement: &Order,
    ) -> Result<(Vec<Trade>, Vec<ExecutionReport>), String>;

    /// Instrument(s) this engine handles. Single-instrument returns one element; multi-instrument returns all.
    fn instruments(&self) -> Vec<InstrumentId>;

    /// Top-of-book snapshot for a given instrument. Returns `None` if instrument not found.
    fn book_snapshot_for(&self, id: InstrumentId) -> Option<BookSnapshot>;

    /// First instrument (for backward compat). Default: first of `instruments()`.
    fn instrument_id(&self) -> InstrumentId {
        self.instruments().into_iter().next().unwrap_or(InstrumentId(0))
    }

    /// Best bid for the first instrument (backward compat).
    fn best_bid(&self) -> Option<Decimal> {
        self.book_snapshot_for(self.instrument_id()).and_then(|s| s.best_bid)
    }

    /// Best ask for the first instrument (backward compat).
    fn best_ask(&self) -> Option<Decimal> {
        self.book_snapshot_for(self.instrument_id()).and_then(|s| s.best_ask)
    }

    /// Current top-of-book snapshot for the first instrument (backward compat).
    fn book_snapshot(&self) -> BookSnapshot {
        self.book_snapshot_for(self.instrument_id()).unwrap_or(BookSnapshot {
            instrument_id: self.instrument_id(),
            best_bid: None,
            best_ask: None,
        })
    }
}

impl MatchingEngine for Engine {
    fn submit_order(&mut self, order: Order) -> Result<(Vec<Trade>, Vec<ExecutionReport>), String> {
        Engine::submit_order(self, order)
    }

    fn cancel_order(&mut self, order_id: OrderId) -> Option<InstrumentId> {
        if Engine::cancel_order(self, order_id) {
            Some(self.instrument_id)
        } else {
            None
        }
    }

    fn modify_order(
        &mut self,
        order_id: OrderId,
        replacement: &Order,
    ) -> Result<(Vec<Trade>, Vec<ExecutionReport>), String> {
        Engine::modify_order(self, order_id, replacement)
    }

    fn instruments(&self) -> Vec<InstrumentId> {
        vec![self.instrument_id]
    }

    fn book_snapshot_for(&self, id: InstrumentId) -> Option<BookSnapshot> {
        if id == self.instrument_id {
            Some(BookSnapshot {
                instrument_id: self.instrument_id,
                best_bid: self.book.best_bid(),
                best_ask: self.book.best_ask(),
            })
        } else {
            None
        }
    }

    fn instrument_id(&self) -> InstrumentId {
        self.instrument_id
    }

    fn best_bid(&self) -> Option<Decimal> {
        self.book.best_bid()
    }

    fn best_ask(&self) -> Option<Decimal> {
        self.book.best_ask()
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

// ---------------------------------------------------------------------------
// Multi-instrument engine: one book per instrument, admin can add/remove
// ---------------------------------------------------------------------------

/// Serializable snapshot of MultiEngine state for persistence.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EngineSnapshot {
    pub instruments: Vec<(InstrumentId, Option<String>)>,
    /// Per-instrument resting orders.
    pub books: Vec<(InstrumentId, Vec<RestingOrder>)>,
    pub order_to_instrument: Vec<(OrderId, InstrumentId)>,
    pub next_trade_id: u64,
    pub next_exec_id: u64,
}

/// Metadata for an instrument (optional symbol for display).
#[derive(Clone, Debug)]
pub struct InstrumentMeta {
    pub symbol: Option<String>,
}

/// Multi-instrument matching engine. Holds one order book per instrument; admin can add/remove instruments.
/// Order IDs are globally unique; cancel/modify resolve order_id → instrument via an internal map.
#[derive(Debug)]
pub struct MultiEngine {
    books: HashMap<InstrumentId, OrderBook>,
    registry: HashMap<InstrumentId, InstrumentMeta>,
    order_to_instrument: HashMap<OrderId, InstrumentId>,
    next_trade_id: u64,
    next_exec_id: u64,
}

impl MultiEngine {
    /// Creates a multi-instrument engine with the given initial instruments. Each entry is (instrument_id, optional symbol).
    pub fn new_with_instruments(initial: Vec<(InstrumentId, Option<String>)>) -> Self {
        let mut books = HashMap::new();
        let mut registry = HashMap::new();
        for (id, symbol) in initial {
            books.insert(id, OrderBook::new(id));
            registry.insert(id, InstrumentMeta { symbol });
        }
        Self {
            books,
            registry,
            order_to_instrument: HashMap::new(),
            next_trade_id: 1,
            next_exec_id: 1,
        }
    }

    /// Add an instrument (new order book). Returns error if instrument already exists.
    pub fn add_instrument(&mut self, instrument_id: InstrumentId, symbol: Option<String>) -> Result<(), String> {
        if self.books.contains_key(&instrument_id) {
            return Err(format!("Instrument {} already exists", instrument_id.0));
        }
        self.books.insert(instrument_id, OrderBook::new(instrument_id));
        self.registry.insert(instrument_id, InstrumentMeta { symbol });
        Ok(())
    }

    /// Remove an instrument. Returns error if the book has resting orders.
    pub fn remove_instrument(&mut self, instrument_id: InstrumentId) -> Result<(), String> {
        let book = self.books.get(&instrument_id).ok_or_else(|| format!("Instrument {} not found", instrument_id.0))?;
        if book.has_resting_orders() {
            return Err("Instrument has resting orders; cancel them first".to_string());
        }
        self.books.remove(&instrument_id);
        self.registry.remove(&instrument_id);
        self.order_to_instrument.retain(|_, id| *id != instrument_id);
        Ok(())
    }

    /// Snapshot of engine state for persistence. Serialize to JSON and restore with [`load_from_snapshot`].
    pub fn snapshot(&self) -> EngineSnapshot {
        let instruments: Vec<(InstrumentId, Option<String>)> = self
            .registry
            .iter()
            .map(|(&id, meta)| (id, meta.symbol.clone()))
            .collect();
        let books: Vec<(InstrumentId, Vec<RestingOrder>)> = self
            .books
            .iter()
            .map(|(&id, book)| (id, book.resting_orders_snapshot()))
            .collect();
        let order_to_instrument: Vec<(OrderId, InstrumentId)> = self
            .order_to_instrument
            .iter()
            .map(|(&oid, &iid)| (oid, iid))
            .collect();
        EngineSnapshot {
            instruments,
            books,
            order_to_instrument,
            next_trade_id: self.next_trade_id,
            next_exec_id: self.next_exec_id,
        }
    }

    /// Restore engine from a snapshot (e.g. after loading from persistence). Replaces current state.
    pub fn load_from_snapshot(&mut self, snap: EngineSnapshot) -> Result<(), String> {
        use crate::types::{OrderType, TimeInForce};
        self.books.clear();
        self.registry.clear();
        self.order_to_instrument.clear();
        for (id, symbol) in &snap.instruments {
            self.books.insert(*id, OrderBook::new(*id));
            self.registry.insert(*id, InstrumentMeta { symbol: symbol.clone() });
        }
        for (instrument_id, resting) in &snap.books {
            let book = self.books.get_mut(instrument_id).ok_or_else(|| format!("Instrument {} not in snapshot instruments", instrument_id.0))?;
            book.load_resting_orders(resting, OrderType::Limit, TimeInForce::GTC)?;
            for r in resting {
                self.order_to_instrument.insert(r.order_id, *instrument_id);
            }
        }
        self.next_trade_id = snap.next_trade_id;
        self.next_exec_id = snap.next_exec_id;
        Ok(())
    }

    /// List instruments with optional symbol (for admin GET).
    pub fn list_instruments(&self) -> Vec<(InstrumentId, Option<String>)> {
        self.registry
            .iter()
            .map(|(&id, meta)| (id, meta.symbol.clone()))
            .collect()
    }

    fn update_order_to_instrument_after_submit(&mut self, order: &Order, reports: &[ExecutionReport]) {
        let aggressor_report = reports.iter().find(|r| r.order_id == order.order_id);
        if let Some(r) = aggressor_report {
            if r.remaining_quantity > Decimal::ZERO {
                self.order_to_instrument.insert(order.order_id, order.instrument_id);
            }
        }
    }

    fn update_order_to_instrument_after_modify(&mut self, replacement: &Order, reports: &[ExecutionReport]) {
        let aggressor_report = reports.iter().find(|r| r.order_id == replacement.order_id);
        if let Some(r) = aggressor_report {
            if r.remaining_quantity > Decimal::ZERO {
                self.order_to_instrument.insert(replacement.order_id, replacement.instrument_id);
            }
        }
    }
}

impl MatchingEngine for MultiEngine {
    fn submit_order(&mut self, order: Order) -> Result<(Vec<Trade>, Vec<ExecutionReport>), String> {
        let book = self.books.get_mut(&order.instrument_id).ok_or_else(|| {
            format!("Unknown instrument {}", order.instrument_id.0)
        })?;
        if order.is_limit() && order.price.is_none() {
            return Err("Limit order must have price".into());
        }
        info!(
            "order submitted order_id={} instrument_id={} side={:?} quantity={} price={:?}",
            order.order_id.0,
            order.instrument_id.0,
            order.side,
            order.quantity,
            order.price
        );
        let (trades, reports) = match_order(
            book,
            &order,
            self.next_trade_id,
            self.next_exec_id,
        );
        self.next_trade_id += trades.len() as u64;
        self.next_exec_id += reports.len() as u64;
        self.update_order_to_instrument_after_submit(&order, &reports);
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
        Ok((trades, reports))
    }

    fn cancel_order(&mut self, order_id: OrderId) -> Option<InstrumentId> {
        let instrument_id = self.order_to_instrument.remove(&order_id)?;
        let book = self.books.get_mut(&instrument_id)?;
        let removed = book.cancel_order(order_id);
        if removed {
            info!("order canceled order_id={} instrument_id={}", order_id.0, instrument_id.0);
            Some(instrument_id)
        } else {
            self.order_to_instrument.insert(order_id, instrument_id);
            None
        }
    }

    fn modify_order(
        &mut self,
        order_id: OrderId,
        replacement: &Order,
    ) -> Result<(Vec<Trade>, Vec<ExecutionReport>), String> {
        let instrument_id = self.order_to_instrument.remove(&order_id).ok_or_else(|| format!("Order {} not found", order_id.0))?;
        if replacement.instrument_id != instrument_id {
            self.order_to_instrument.insert(order_id, instrument_id);
            return Err("Replacement order must be for the same instrument".into());
        }
        let book = self.books.get_mut(&instrument_id).ok_or_else(|| format!("Instrument {} not found", instrument_id.0))?;
        if !book.cancel_order(order_id) {
            self.order_to_instrument.insert(order_id, instrument_id);
            return Err(format!("Order {} not found", order_id.0));
        }
        info!(
            "order modified old_order_id={} replacement order_id={} instrument_id={} side={:?} quantity={} price={:?}",
            order_id.0,
            replacement.order_id.0,
            instrument_id.0,
            replacement.side,
            replacement.quantity,
            replacement.price
        );
        let (trades, reports) = match_order(
            book,
            replacement,
            self.next_trade_id,
            self.next_exec_id,
        );
        self.next_trade_id += trades.len() as u64;
        self.next_exec_id += reports.len() as u64;
        self.update_order_to_instrument_after_modify(replacement, &reports);
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
        Ok((trades, reports))
    }

    fn instruments(&self) -> Vec<InstrumentId> {
        self.registry.keys().copied().collect()
    }

    fn book_snapshot_for(&self, id: InstrumentId) -> Option<BookSnapshot> {
        self.books.get(&id).map(|book| BookSnapshot {
            instrument_id: id,
            best_bid: book.best_bid(),
            best_ask: book.best_ask(),
        })
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
