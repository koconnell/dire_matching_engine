# Phase 4: Market Data & Testing — Checklist

Work items for Phase 4 in suggested order. Tick as you complete.

---

## 1. Synthetic market data generator

- [x] **Deterministic RNG**  
  Seeded RNG (e.g. `rand::SeedableRng` or `fastrand` with seed) so same seed ⇒ same order stream.
- [x] **Configurable model**  
  Parameters: e.g. order count, side ratio, price/quantity range, order type mix (limit/market), TIF.
- [x] **Order stream**  
  Generator yields a stream of `Order` (or equivalent) that can be replayed or fed into the engine.
- [x] **Optional feed**  
  Optional mode to push orders into the engine at a set rate (demos/load); or document “replay only” for MVP.
- [x] **Documentation**  
  How to run, config format, and seed usage (e.g. in `project_docs/market_data_generator.md`).

---

## 2. Property-based / deterministic tests

- [x] **Proptest (or similar)**  
  Add dependency and at least one property-based test that generates random order sequences.
- [x] **Invariants**  
  At least two of: no crossed book after match; quantity conservation (filled vs trades); no negative quantities; valid order state transitions.
- [x] **Deterministic replay**  
  At least one test that runs a fixed order sequence (from generator or fixture) and asserts exact trades/reports (same seed ⇒ same outcome).
- [x] **Documentation**  
  How to run property-based and deterministic tests (e.g. in `project_docs/integration_tests.md` or a dedicated testing doc).

---

## 3. Performance benchmarks

- [x] **Criterion (or equivalent)**  
  Benchmarks for engine: e.g. submit_order, cancel_order, modify_order, or mixed workload.
- [x] **Metrics**  
  Report throughput (orders/sec) and/or latency (e.g. p50, p99) where applicable.
- [x] **Baseline**  
  Document baseline numbers and how to run (e.g. `cargo bench`).
- [x] **Optional load test**  
  Optional: minimal load test for REST or FIX (concurrent connections, sustained rate); document if done.

---

## 4. Phase 4 definition of done

- [x] Market data generator with deterministic seed and configurable model; documented.
- [x] At least one property-based or deterministic invariant test; documented.
- [x] Engine benchmarks (criterion) with run instructions and baseline; documented.
- [x] All new code covered by tests or benchmarks as appropriate.

---

When the items above are done, you’re in good shape to start **Phase 5: User Onboarding & Documentation**.
