# Synthetic market data generator (Phase 4 §1)

Deterministic, configurable order stream for replay tests, demos, and load tests. **Same seed ⇒ same sequence of orders.**

## Quick start

```rust
use dire_matching_engine::{Engine, Generator, GeneratorConfig, InstrumentId, replay_into_engine};

// Default config: 1000 orders, 50% buy, 90% limit, prices 95–105, etc.
let config = GeneratorConfig {
    seed: 42,
    num_orders: 100,
    ..Default::default()
};
let orders = Generator::new(config).all_orders();

let mut engine = Engine::new(InstrumentId(1));
let (trades, reports) = replay_into_engine(&mut engine, orders).unwrap();
```

## Config

| Field | Description | Default |
|-------|-------------|---------|
| `seed` | RNG seed. Same seed ⇒ same stream. | `0` |
| `instrument_id` | Instrument for all orders. | `InstrumentId(1)` |
| `num_orders` | Length of stream (used by `all_orders()`). | `1000` |
| `buy_ratio` | Probability of Buy (0.0–1.0). | `0.5` |
| `limit_ratio` | Probability of Limit order (0.0–1.0). | `0.9` |
| `price_min`, `price_max` | Price range for limit orders (inclusive). | `95`, `105` |
| `quantity_min`, `quantity_max` | Quantity range in whole units (inclusive). | `1`, `100` |
| `tif_gtc_ratio`, `tif_ioc_ratio` | TIF: GTC, then IOC, remainder FOK (sum to 1.0). | `0.8`, `0.1` |
| `num_traders` | Trader IDs from 1 to this value. | `5` |

## API

- **`Generator::new(config)`** — Builds a generator. Same config (including seed) ⇒ same stream.
- **`generator.next_order()`** — Returns one order and advances state.
- **`generator.take_orders(n)`** — Returns a `Vec` of the next `n` orders.
- **`generator.all_orders()`** — Returns a `Vec` of `config.num_orders` orders.
- **`replay_into_engine(engine, orders)`** — Replays an order sequence into the engine; returns `(total_trades, total_reports)` or the first error.
- **`replay_into_engine_with_delay(engine, orders, duration)`** — Same as above but sleeps `duration` after each order (for rate-limited demos or load).

## Replay vs feed

- **Replay:** Call `all_orders()` or `take_orders(n)` and pass the slice/iterator to `replay_into_engine`. No timing; good for tests and benchmarks.
- **Rate-limited feed:** Use `replay_into_engine_with_delay` with a `Duration`, or loop over `next_order()` and call `engine.submit_order(order)` plus your own delay (e.g. `std::thread::sleep` or a timer in an async runtime).

## Determinism

- The generator uses `rand::rngs::StdRng` seeded with `config.seed`.
- Same `GeneratorConfig` (including `seed`) produces the same sequence.
- Order IDs start at 1 and increment; timestamps start at 1 and increment. Client order IDs are `gen-1`, `gen-2`, …

## Tests

- `same_seed_same_stream` — Two generators with same config yield identical orders.
- `different_seed_different_stream` — Different seeds yield different order content.
- `replay_into_engine_succeeds` — 20 generated orders replay into the engine without error.

Run: `cargo test market_data_gen`
