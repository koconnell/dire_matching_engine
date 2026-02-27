//! Core types and IDs for the matching engine (charter data models).
//!
//! All identifiers are newtype wrappers. [`Order`], [`Side`], [`OrderType`], and
//! [`TimeInForce`] define the order message and lifecycle.

use rust_decimal::Decimal;

/// Unique order identifier (internal).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct OrderId(pub u64);

/// Execution report identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExecutionId(pub u64);

/// Trade identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TradeId(pub u64);

/// Instrument identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct InstrumentId(pub u64);

/// Trader identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TraderId(pub u64);

/// Order side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

/// Order type: limit (with price) or market (take best available).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OrderType {
    Limit,
    Market,
}

/// Time-in-force: how long the order stays active.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TimeInForce {
    /// Good-Till-Cancel: rest on book until filled or canceled.
    GTC,
    /// Immediate-or-Cancel: fill what you can immediately; cancel the rest.
    IOC,
    /// Fill-or-Kill: fill entirely immediately or cancel.
    FOK,
}

/// Order lifecycle status in execution reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
}

/// Execution report type (FIX-style).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExecType {
    New,
    PartialFill,
    Fill,
    Canceled,
    Rejected,
}

/// Order message (charter).
///
/// For limit orders, `price` must be `Some(...)`. For market orders, `price` is `None`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Order {
    pub order_id: OrderId,
    pub client_order_id: String,
    pub instrument_id: InstrumentId,
    pub side: Side,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub time_in_force: TimeInForce,
    pub timestamp: u64,
    pub trader_id: TraderId,
}

impl Order {
    pub fn is_limit(&self) -> bool {
        matches!(self.order_type, OrderType::Limit)
    }

    pub fn is_market(&self) -> bool {
        matches!(self.order_type, OrderType::Market)
    }
}

/// Minimal representation of a resting order for persistence/snapshot.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RestingOrder {
    pub order_id: OrderId,
    pub instrument_id: InstrumentId,
    pub side: Side,
    pub price: Decimal,
    pub quantity: Decimal,
    pub trader_id: TraderId,
}
