//! FIX 4.4 adapter (Phase 2): tag-value parse/build and mapping to/from Engine.
//!
//! In-process TCP acceptor runs in [`fix::acceptor`]; this module provides message parsing,
//! building, and conversion between FIX and engine types.

mod acceptor;
pub mod message;

pub use acceptor::run_fix_acceptor;
pub use message::{
    execution_report_to_fix, execution_report_to_fix_with_side, order_from_cancel_replace,
    order_from_new_order_single, parse_fix_message, FixMessage, FixWriter,
};
