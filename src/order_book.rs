//! Single-instrument order book: bids and asks, price-time priority.
//!
//! Supports add, cancel, modify, and taking liquidity (used by [`crate::matching`]).
//! Each price level is FIFO; best bid is highest price, best ask is lowest.

use crate::types::{Order, OrderId, OrderType, RestingOrder, Side, TimeInForce, TraderId};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashMap};

/// One order at a price level: (OrderId, remaining_qty, TraderId) for price-time and self-trade.
type BookEntry = (OrderId, Decimal, TraderId);
/// Price level -> FIFO queue of orders.
type PriceLevel = BTreeMap<Decimal, Vec<BookEntry>>;

/// Result of taking liquidity from the book (one per resting order filled).
#[derive(Clone, Debug)]
pub struct Fill {
    pub resting_order_id: OrderId,
    pub resting_trader_id: TraderId,
    pub price: Decimal,
    pub quantity: Decimal,
    /// True if the resting order was fully filled (removed from book).
    pub resting_fully_filled: bool,
}

/// Single-instrument order book.
#[derive(Debug)]
pub struct OrderBook {
    instrument_id: crate::types::InstrumentId,
    bids: PriceLevel,
    asks: PriceLevel,
    /// Orders by id for cancel/modify: (side, price, remaining_qty).
    orders: HashMap<OrderId, (Side, Decimal, Decimal)>,
}

impl OrderBook {
    pub fn new(instrument_id: crate::types::InstrumentId) -> Self {
        Self {
            instrument_id,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            orders: std::collections::HashMap::new(),
        }
    }

    /// Add a limit order to the book. Does not run matching; caller uses matching module.
    pub fn add_order(&mut self, order: &Order) -> Result<(), String> {
        let price = order.price.ok_or("Limit order must have price")?;
        let side = order.side;
        let order_id = order.order_id;
        let qty = order.quantity;
        let trader_id = order.trader_id;

        let level = match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };
        level
            .entry(price)
            .or_default()
            .push((order_id, qty, trader_id));
        self.orders.insert(order_id, (side, price, qty));
        Ok(())
    }

    /// Remove order by id. Returns true if found and removed.
    pub fn cancel_order(&mut self, order_id: OrderId) -> bool {
        let Some((side, price, _)) = self.orders.remove(&order_id) else {
            return false;
        };
        let level = match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };
        if let Some(queue) = level.get_mut(&price) {
            queue.retain(|(id, _, _)| *id != order_id);
            if queue.is_empty() {
                level.remove(&price);
            }
        }
        true
    }

    /// Modify an order: cancel by `order_id`, then add the replacement order.
    /// Replacement may use the same `order_id` (in-place replace) or a new one.
    /// Returns `Err` if the order to modify is not found, or if the replacement is invalid (e.g. limit with no price).
    pub fn modify_order(&mut self, order_id: OrderId, replacement: &Order) -> Result<(), String> {
        if !self.cancel_order(order_id) {
            return Err(format!("Order {} not found", order_id.0));
        }
        if replacement.instrument_id != self.instrument_id {
            return Err("Replacement order must be for the same instrument".into());
        }
        self.add_order(replacement)
    }

    /// Total ask quantity at or below given price (excluding exclude_trader). For FOK check.
    pub fn available_ask_qty_at_or_below(
        &self,
        price_limit: Decimal,
        exclude_trader: TraderId,
    ) -> Decimal {
        let mut total = Decimal::ZERO;
        for (&price, queue) in self.asks.iter() {
            if price > price_limit {
                break;
            }
            for (_, qty, trader_id) in queue {
                if *trader_id != exclude_trader {
                    total += qty;
                }
            }
        }
        total
    }

    /// Total bid quantity at or above given price (excluding exclude_trader). For FOK check.
    pub fn available_bid_qty_at_or_above(
        &self,
        price_limit: Decimal,
        exclude_trader: TraderId,
    ) -> Decimal {
        let mut total = Decimal::ZERO;
        for (_, queue) in self.bids.range(price_limit..) {
            for (_, qty, trader_id) in queue {
                if *trader_id != exclude_trader {
                    total += qty;
                }
            }
        }
        total
    }

    /// Take liquidity from the ask side (for an incoming buy). Price-time priority, skip exclude_trader.
    /// Returns fills and updates the book.
    pub fn take_from_asks(
        &mut self,
        price_limit: Decimal,
        mut quantity: Decimal,
        exclude_trader: TraderId,
    ) -> Vec<Fill> {
        let mut fills = Vec::new();
        let mut empty_prices = Vec::new();
        let mut orders_remove = Vec::new();
        let mut orders_update: Vec<(OrderId, Decimal)> = Vec::new();
        for (price, queue) in self.asks.iter_mut() {
            if *price > price_limit || quantity <= Decimal::ZERO {
                break;
            }
            let mut i = 0;
            while i < queue.len() && quantity > Decimal::ZERO {
                let (order_id, rest_qty, trader_id) = queue[i];
                if trader_id == exclude_trader {
                    i += 1;
                    continue;
                }
                let fill_qty = quantity.min(rest_qty);
                quantity -= fill_qty;
                fills.push(Fill {
                    resting_order_id: order_id,
                    resting_trader_id: trader_id,
                    price: *price,
                    quantity: fill_qty,
                    resting_fully_filled: fill_qty >= rest_qty,
                });
                if fill_qty >= rest_qty {
                    orders_remove.push(order_id);
                    queue.remove(i);
                } else {
                    let new_rest = rest_qty - fill_qty;
                    orders_update.push((order_id, new_rest));
                    queue[i] = (order_id, new_rest, trader_id);
                    i += 1;
                }
            }
            if queue.is_empty() {
                empty_prices.push(*price);
            }
        }
        for oid in orders_remove {
            self.orders.remove(&oid);
        }
        for (oid, new_qty) in orders_update {
            if let Some((_, ref mut stored_qty, _)) = self.orders.get_mut(&oid) {
                *stored_qty = new_qty;
            }
        }
        for p in empty_prices {
            self.asks.remove(&p);
        }
        fills
    }

    /// Take liquidity from the bid side (for an incoming sell). Price-time priority, skip exclude_trader.
    pub fn take_from_bids(
        &mut self,
        price_limit: Decimal,
        mut quantity: Decimal,
        exclude_trader: TraderId,
    ) -> Vec<Fill> {
        let mut fills = Vec::new();
        let mut empty_prices = Vec::new();
        let mut orders_remove = Vec::new();
        let mut orders_update: Vec<(OrderId, Decimal)> = Vec::new();
        // BTreeMap: iterate bids in descending price (best bid first).
        let bid_prices: Vec<Decimal> = self.bids.keys().copied().rev().collect();
        for price in bid_prices {
            if price < price_limit || quantity <= Decimal::ZERO {
                break;
            }
            let queue = match self.bids.get_mut(&price) {
                Some(q) => q,
                None => continue,
            };
            let mut i = 0;
            while i < queue.len() && quantity > Decimal::ZERO {
                let (order_id, rest_qty, trader_id) = queue[i];
                if trader_id == exclude_trader {
                    i += 1;
                    continue;
                }
                let fill_qty = quantity.min(rest_qty);
                quantity -= fill_qty;
                fills.push(Fill {
                    resting_order_id: order_id,
                    resting_trader_id: trader_id,
                    price,
                    quantity: fill_qty,
                    resting_fully_filled: fill_qty >= rest_qty,
                });
                if fill_qty >= rest_qty {
                    orders_remove.push(order_id);
                    queue.remove(i);
                } else {
                    let new_rest = rest_qty - fill_qty;
                    orders_update.push((order_id, new_rest));
                    queue[i] = (order_id, new_rest, trader_id);
                    i += 1;
                }
            }
            if queue.is_empty() {
                empty_prices.push(price);
            }
        }
        for oid in orders_remove {
            self.orders.remove(&oid);
        }
        for (oid, new_qty) in orders_update {
            if let Some((_, ref mut stored_qty, _)) = self.orders.get_mut(&oid) {
                *stored_qty = new_qty;
            }
        }
        for p in empty_prices {
            self.bids.remove(&p);
        }
        fills
    }

    pub fn instrument_id(&self) -> crate::types::InstrumentId {
        self.instrument_id
    }

    /// Returns true if the book has at least one resting order (for admin delete-instrument checks).
    pub fn has_resting_orders(&self) -> bool {
        !self.orders.is_empty()
    }

    /// Export resting orders for persistence. Caller must set instrument_id on each (use `instrument_id()`).
    pub fn resting_orders_snapshot(&self) -> Vec<RestingOrder> {
        let mut out = Vec::new();
        for (price, queue) in &self.bids {
            for (order_id, qty, trader_id) in queue {
                out.push(RestingOrder {
                    order_id: *order_id,
                    instrument_id: self.instrument_id,
                    side: Side::Buy,
                    price: *price,
                    quantity: *qty,
                    trader_id: *trader_id,
                });
            }
        }
        for (price, queue) in &self.asks {
            for (order_id, qty, trader_id) in queue {
                out.push(RestingOrder {
                    order_id: *order_id,
                    instrument_id: self.instrument_id,
                    side: Side::Sell,
                    price: *price,
                    quantity: *qty,
                    trader_id: *trader_id,
                });
            }
        }
        out
    }

    /// Restore resting orders (e.g. after load from persistence). Clears the book first. Each order must be for this book's instrument.
    pub fn load_resting_orders(
        &mut self,
        orders: &[RestingOrder],
        order_type: OrderType,
        time_in_force: TimeInForce,
    ) -> Result<(), String> {
        self.bids.clear();
        self.asks.clear();
        self.orders.clear();
        for r in orders {
            if r.instrument_id != self.instrument_id {
                return Err(format!("Resting order instrument {} does not match book {}", r.instrument_id.0, self.instrument_id.0));
            }
            let order = Order {
                order_id: r.order_id,
                client_order_id: format!("restore-{}", r.order_id.0),
                instrument_id: r.instrument_id,
                side: r.side,
                order_type,
                quantity: r.quantity,
                price: Some(r.price),
                time_in_force,
                timestamp: 0,
                trader_id: r.trader_id,
            };
            self.add_order(&order)?;
        }
        Ok(())
    }

    /// Best bid price (None if empty).
    pub fn best_bid(&self) -> Option<Decimal> {
        self.bids.keys().next_back().copied()
    }

    /// Best ask price (None if empty).
    pub fn best_ask(&self) -> Option<Decimal> {
        self.asks.keys().next().copied()
    }

    /// Whether the book has a bid.
    pub fn has_bid(&self) -> bool {
        self.best_bid().is_some()
    }

    /// Whether the book has an ask.
    pub fn has_ask(&self) -> bool {
        self.best_ask().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{InstrumentId, Order, OrderId, OrderType, TimeInForce, TraderId};
    use rust_decimal::Decimal;

    fn order(id: u64, side: Side, qty: i64, price: i64, trader: u64) -> Order {
        Order {
            order_id: OrderId(id),
            client_order_id: format!("c{}", id),
            instrument_id: InstrumentId(1),
            side,
            order_type: OrderType::Limit,
            quantity: Decimal::from(qty),
            price: Some(Decimal::from(price)),
            time_in_force: TimeInForce::GTC,
            timestamp: id,
            trader_id: TraderId(trader),
        }
    }

    #[test]
    fn add_and_cancel_order() {
        let mut book = OrderBook::new(InstrumentId(1));
        let order = order(1, Side::Buy, 10, 100, 1);
        book.add_order(&order).unwrap();
        assert_eq!(book.best_bid(), Some(Decimal::from(100)));
        assert!(book.cancel_order(OrderId(1)));
        assert!(book.best_bid().is_none());
    }

    #[test]
    fn modify_order_same_id_replaces_price_and_quantity() {
        let mut book = OrderBook::new(InstrumentId(1));
        book.add_order(&order(1, Side::Buy, 10, 100, 1)).unwrap();
        let replacement = order(1, Side::Buy, 20, 101, 1);
        book.modify_order(OrderId(1), &replacement).unwrap();
        assert_eq!(book.best_bid(), Some(Decimal::from(101)));
        // Only one order at 101 with qty 20 (same order_id, so one entry)
        book.cancel_order(OrderId(1));
        assert!(book.best_bid().is_none());
    }

    #[test]
    fn modify_order_new_id_cancels_old_adds_new() {
        let mut book = OrderBook::new(InstrumentId(1));
        book.add_order(&order(1, Side::Sell, 10, 100, 1)).unwrap();
        let replacement = order(2, Side::Sell, 5, 99, 1);
        book.modify_order(OrderId(1), &replacement).unwrap();
        assert!(book.best_ask() == Some(Decimal::from(99)));
        assert!(book.cancel_order(OrderId(2)));
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn modify_order_not_found_returns_err() {
        let mut book = OrderBook::new(InstrumentId(1));
        let replacement = order(1, Side::Buy, 10, 100, 1);
        let err = book.modify_order(OrderId(999), &replacement).unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn modify_order_wrong_instrument_returns_err() {
        let mut book = OrderBook::new(InstrumentId(1));
        book.add_order(&order(1, Side::Buy, 10, 100, 1)).unwrap();
        let mut replacement = order(1, Side::Buy, 20, 101, 1);
        replacement.instrument_id = InstrumentId(2);
        let err = book.modify_order(OrderId(1), &replacement).unwrap_err();
        assert!(err.contains("instrument"));
    }

    #[test]
    fn add_order_limit_without_price_returns_err() {
        let mut book = OrderBook::new(InstrumentId(1));
        let mut o = order(1, Side::Buy, 10, 100, 1);
        o.price = None;
        let err = book.add_order(&o).unwrap_err();
        assert!(err.to_lowercase().contains("price"));
    }

    #[test]
    fn instrument_id_returns_book_instrument() {
        let book = OrderBook::new(InstrumentId(42));
        assert_eq!(book.instrument_id(), InstrumentId(42));
    }

    #[test]
    fn available_ask_qty_at_or_below_excludes_trader() {
        let mut book = OrderBook::new(InstrumentId(1));
        book.add_order(&order(1, Side::Sell, 10, 100, 1)).unwrap();
        book.add_order(&order(2, Side::Sell, 20, 100, 2)).unwrap();
        assert_eq!(
            book.available_ask_qty_at_or_below(Decimal::from(100), TraderId(1)),
            Decimal::from(20)
        );
        assert_eq!(
            book.available_ask_qty_at_or_below(Decimal::from(100), TraderId(2)),
            Decimal::from(10)
        );
        assert_eq!(
            book.available_ask_qty_at_or_below(Decimal::from(100), TraderId(3)),
            Decimal::from(30)
        );
    }

    #[test]
    fn available_bid_qty_at_or_above_excludes_trader() {
        let mut book = OrderBook::new(InstrumentId(1));
        book.add_order(&order(1, Side::Buy, 10, 100, 1)).unwrap();
        book.add_order(&order(2, Side::Buy, 20, 100, 2)).unwrap();
        assert_eq!(
            book.available_bid_qty_at_or_above(Decimal::from(100), TraderId(1)),
            Decimal::from(20)
        );
        assert_eq!(
            book.available_bid_qty_at_or_above(Decimal::from(100), TraderId(2)),
            Decimal::from(10)
        );
    }
}
