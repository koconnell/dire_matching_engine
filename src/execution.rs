//! Execution reports and trades (charter data models).
//!
//! [`ExecutionReport`] is emitted for every order state change (New, PartialFill, Fill, Canceled).
//! [`Trade`] is emitted for each match between a buy and a sell.

use crate::types::{ExecType, ExecutionId, OrderId, OrderStatus};
use rust_decimal::Decimal;

/// Execution report (charter).
#[derive(Clone, Debug)]
pub struct ExecutionReport {
    pub order_id: OrderId,
    pub exec_id: ExecutionId,
    pub exec_type: ExecType,
    pub order_status: OrderStatus,
    pub filled_quantity: Decimal,
    pub remaining_quantity: Decimal,
    pub avg_price: Option<Decimal>,
    pub last_qty: Option<Decimal>,
    pub last_px: Option<Decimal>,
    pub timestamp: u64,
}

/// Trade (charter).
#[derive(Clone, Debug)]
pub struct Trade {
    pub trade_id: crate::types::TradeId,
    pub instrument_id: crate::types::InstrumentId,
    pub buy_order_id: OrderId,
    pub sell_order_id: OrderId,
    pub price: Decimal,
    pub quantity: Decimal,
    pub timestamp: u64,
    pub aggressor_side: crate::types::Side,
}
