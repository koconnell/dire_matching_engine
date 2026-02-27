//! # Dire Matching Engine
//!
//! High-performance, deterministic matching engine (Phase 1): order book,
//! price-time priority matching, and execution reports.
//!
//! ## Entry point
//!
//! Use [`Engine`] as the single entry point: create with [`Engine::new`], then
//! [`Engine::submit_order`], [`Engine::cancel_order`], and [`Engine::modify_order`].
//!
//! ## Example
//!
//! ```rust
//! use dire_matching_engine::{Engine, Order, OrderId, Side, OrderType, TimeInForce, TraderId, InstrumentId};
//! use rust_decimal::Decimal;
//!
//! let mut engine = Engine::new(InstrumentId(1));
//! let order = Order {
//!     order_id: OrderId(1),
//!     client_order_id: "c1".into(),
//!     instrument_id: InstrumentId(1),
//!     side: Side::Buy,
//!     order_type: OrderType::Limit,
//!     quantity: Decimal::from(10),
//!     price: Some(Decimal::from(100)),
//!     time_in_force: TimeInForce::GTC,
//!     timestamp: 1,
//!     trader_id: TraderId(1),
//! };
//! let (trades, reports) = engine.submit_order(order).unwrap();
//! assert!(trades.is_empty());
//! assert!(!reports.is_empty());
//! ```
//!
//! ## Lower-level API
//!
//! You can also use [`OrderBook`] and [`match_order`] directly if you manage
//! trade/execution IDs yourself.

pub mod api;
pub mod audit;
pub mod auth;
pub mod engine;
pub mod market_data_gen;
pub mod execution;
pub mod fix;
pub mod matching;
pub mod order_book;
pub mod persistence;
pub mod types;

pub use engine::{BookSnapshot, Engine, EngineSnapshot, InstrumentMeta, MatchingEngine, MultiEngine};
pub use execution::{ExecutionReport, Trade};
pub use matching::match_order;
pub use order_book::{Fill, OrderBook};
pub use auth::{AuthConfig, AuthUser, Role};
pub use types::{ExecType, InstrumentId, Order, OrderId, OrderStatus, OrderType, RestingOrder, Side, TimeInForce, TraderId};
pub use market_data_gen::{replay_into_engine, replay_into_engine_with_delay, Generator, GeneratorConfig};
