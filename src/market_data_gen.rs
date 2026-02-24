//! Phase 4 §1: Synthetic market data generator.
//!
//! Deterministic, configurable order stream for replay tests, demos, and load tests.
//! Same seed ⇒ same sequence of orders.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rust_decimal::Decimal;

use crate::types::{InstrumentId, Order, OrderId, OrderType, Side, TimeInForce, TraderId};

/// Configuration for the synthetic order generator.
/// All ranges are inclusive. Same config + seed produces the same stream.
#[derive(Clone, Debug)]
pub struct GeneratorConfig {
    /// RNG seed. Same seed ⇒ same order stream.
    pub seed: u64,
    /// Instrument for all generated orders.
    pub instrument_id: InstrumentId,
    /// Number of orders to generate (used by [`Generator::take`] or when collecting).
    pub num_orders: usize,
    /// Probability of Buy (0.0..=1.0). Sell otherwise.
    pub buy_ratio: f64,
    /// Probability of Limit order (0.0..=1.0). Market otherwise.
    pub limit_ratio: f64,
    /// Price range (inclusive) for limit orders. Ignored for market.
    pub price_min: i64,
    pub price_max: i64,
    /// Quantity range (inclusive), whole units.
    pub quantity_min: u64,
    pub quantity_max: u64,
    /// Time-in-force: probability of GTC, then IOC, then FOK (should sum to 1.0).
    pub tif_gtc_ratio: f64,
    pub tif_ioc_ratio: f64,
    /// Number of distinct trader IDs (1..=num_traders).
    pub num_traders: u64,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            instrument_id: InstrumentId(1),
            num_orders: 1000,
            buy_ratio: 0.5,
            limit_ratio: 0.9,
            price_min: 95,
            price_max: 105,
            quantity_min: 1,
            quantity_max: 100,
            tif_gtc_ratio: 0.8,
            tif_ioc_ratio: 0.1,
            num_traders: 5,
        }
    }
}

/// Deterministic order stream. Create with [`Generator::new`]; iterate to get orders.
pub struct Generator {
    rng: StdRng,
    config: GeneratorConfig,
    next_order_id: u64,
    next_timestamp: u64,
}

impl Generator {
    /// Builds a generator with the given config. Same config (including seed) ⇒ same stream.
    pub fn new(config: GeneratorConfig) -> Self {
        let rng = StdRng::seed_from_u64(config.seed);
        Self {
            rng,
            config: config,
            next_order_id: 1,
            next_timestamp: 1,
        }
    }

    /// Generates the next order. Advances internal state (order id, timestamp, RNG).
    pub fn next_order(&mut self) -> Order {
        let order_id = OrderId(self.next_order_id);
        self.next_order_id += 1;
        let client_order_id = format!("gen-{}", order_id.0);
        let side = if self.rng.gen::<f64>() < self.config.buy_ratio {
            Side::Buy
        } else {
            Side::Sell
        };
        let is_limit = self.rng.gen::<f64>() < self.config.limit_ratio;
        let order_type = if is_limit {
            OrderType::Limit
        } else {
            OrderType::Market
        };
        let quantity = Decimal::from(
            self.rng.gen_range(self.config.quantity_min..=self.config.quantity_max),
        );
        let price = if is_limit {
            let p = self
                .rng
                .gen_range(self.config.price_min..=self.config.price_max);
            Some(Decimal::from(p))
        } else {
            None
        };
        let r = self.rng.gen::<f64>();
        let time_in_force = if r < self.config.tif_gtc_ratio {
            TimeInForce::GTC
        } else if r < self.config.tif_gtc_ratio + self.config.tif_ioc_ratio {
            TimeInForce::IOC
        } else {
            TimeInForce::FOK
        };
        let timestamp = self.next_timestamp;
        self.next_timestamp += 1;
        let trader_id = TraderId(
            self.rng.gen_range(1..=self.config.num_traders.max(1)),
        );
        Order {
            order_id,
            client_order_id,
            instrument_id: self.config.instrument_id,
            side,
            order_type,
            quantity,
            price,
            time_in_force,
            timestamp,
            trader_id,
        }
    }

    /// Returns a vector of exactly `n` orders (or all remaining if generator is finite).
    /// Advances the generator state.
    pub fn take_orders(&mut self, n: usize) -> Vec<Order> {
        (0..n).map(|_| self.next_order()).collect()
    }

    /// Returns the full stream of orders as defined by config.num_orders.
    pub fn all_orders(&mut self) -> Vec<Order> {
        self.take_orders(self.config.num_orders)
    }
}

/// Replays a sequence of orders into the engine. Returns total trades and reports count (or first error).
/// For rate-limited feed, use [`replay_into_engine_with_delay`] or loop with your own delay.
pub fn replay_into_engine<E>(engine: &mut E, orders: impl IntoIterator<Item = Order>) -> Result<(usize, usize), String>
where
    E: crate::MatchingEngine,
{
    let mut total_trades = 0usize;
    let mut total_reports = 0usize;
    for order in orders {
        let (trades, reports) = engine.submit_order(order)?;
        total_trades += trades.len();
        total_reports += reports.len();
    }
    Ok((total_trades, total_reports))
}

/// Replays orders into the engine with a delay between each order (e.g. for demos or rate-limited load).
/// `delay_per_order` is applied after each submission. Returns total trades and reports count (or first error).
pub fn replay_into_engine_with_delay<E>(
    engine: &mut E,
    orders: impl IntoIterator<Item = Order>,
    delay_per_order: std::time::Duration,
) -> Result<(usize, usize), String>
where
    E: crate::MatchingEngine,
{
    let mut total_trades = 0usize;
    let mut total_reports = 0usize;
    for order in orders {
        let (trades, reports) = engine.submit_order(order)?;
        total_trades += trades.len();
        total_reports += reports.len();
        std::thread::sleep(delay_per_order);
    }
    Ok((total_trades, total_reports))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let c = GeneratorConfig {
            seed: 42,
            num_orders: 10,
            ..Default::default()
        };
        let orders1: Vec<Order> = Generator::new(c.clone()).all_orders();
        let orders2: Vec<Order> = Generator::new(c).all_orders();
        assert_eq!(orders1.len(), 10);
        assert_eq!(orders2.len(), 10);
        for (a, b) in orders1.iter().zip(orders2.iter()) {
            assert_eq!(a.order_id, b.order_id);
            assert_eq!(a.side, b.side);
            assert_eq!(a.order_type, b.order_type);
            assert_eq!(a.quantity, b.quantity);
            assert_eq!(a.price, b.price);
        }
    }

    #[test]
    fn different_seed_different_stream() {
        let o1: Vec<Order> = Generator::new(GeneratorConfig {
            seed: 1,
            num_orders: 5,
            ..Default::default()
        })
        .all_orders();
        let o2: Vec<Order> = Generator::new(GeneratorConfig {
            seed: 2,
            num_orders: 5,
            ..Default::default()
        })
        .all_orders();
        // Order IDs are both 1..5; at least one field should differ (side, price, quantity, etc.)
        let identical = o1.iter().zip(o2.iter()).all(|(a, b)| {
            a.side == b.side && a.price == b.price && a.quantity == b.quantity && a.order_type == b.order_type
        });
        assert!(!identical, "different seeds should produce different order content");
    }

    #[test]
    fn replay_into_engine_succeeds() {
        use crate::Engine;
        let mut engine = Engine::new(InstrumentId(1));
        let orders: Vec<Order> = Generator::new(GeneratorConfig {
            seed: 123,
            num_orders: 20,
            ..Default::default()
        })
        .all_orders();
        let (total_trades, total_reports) = replay_into_engine(&mut engine, orders).unwrap();
        assert!(total_reports >= 20);
        assert!(total_trades <= 20 * 20); // at most N^2 possible matches
    }
}
