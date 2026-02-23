# Phase 4: Market Data & Testing — Checklist

Work items for Phase 4 in suggested order. Tick as you complete.

---

## 1. Synthetic market data generator

- [ ] **Deterministic RNG**  
  Seeded RNG (e.g. `rand::SeedableRng` or `fastrand` with seed) so same seed ⇒ same order stream.
- [ ] **Configurable model**  
  Parameters: e.g. order count, side ratio, price/quantity range, order type mix (limit/market), TIF.
- [ ] **Order stream**  
  Generator yields a stream of `Order` (or equivalent) that can be replayed or fed into the engine.
- [ ] **Optional feed**  
  Optional mode to push orders into the engine at a set rate (demos/load); or document “replay only” for MVP.
- [ ] **Documentation**  
  How to run, config format, and seed usage (e.g. in `project_docs/market_data_generator.md`).

---

## 2. Property-based / deterministic tests

- [ ] **Proptest (or similar)**  
  Add dependency and at least one property-based test that generates random order sequences.
- [ ] **Invariants**  
  At least two of: no crossed book after match; quantity conservation (filled vs trades); no negative quantities; valid order state transitions.
- [ ] **Deterministic replay**  
  At least one test that runs a fixed order sequence (from generator or fixture) and asserts exact trades/reports (same seed ⇒ same outcome).
- [ ] **Documentation**  
  How to run property-based and deterministic tests (e.g. in `project_docs/integration_tests.md` or a dedicated testing doc).

---

## 3. Performance benchmarks

- [ ] **Criterion (or equivalent)**  
  Benchmarks for engine: e.g. submit_order, cancel_order, modify_order, or mixed workload.
- [ ] **Metrics**  
  Report throughput (orders/sec) and/or latency (e.g. p50, p99) where applicable.
- [ ] **Baseline**  
  Document baseline numbers and how to run (e.g. `cargo bench`).
- [ ] **Optional load test**  
  Optional: minimal load test for REST or FIX (concurrent connections, sustained rate); document if done.

---

## 4. Phase 4 definition of done

- [ ] Market data generator with deterministic seed and configurable model; documented.
- [ ] At least one property-based or deterministic invariant test; documented.
- [ ] Engine benchmarks (criterion) with run instructions and baseline; documented.
- [ ] All new code covered by tests or benchmarks as appropriate.

---

When the items above are done, you’re in good shape to start **Phase 5: User Onboarding & Documentation**.
