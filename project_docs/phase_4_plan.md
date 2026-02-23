# Phase 4: Market Data & Testing — Plan

**Scope:** Weeks 11–13 per charter  
**Goal:** Synthetic market data generator (configurable, deterministic), property-based / deterministic tests for the engine, and performance benchmarks. Supports demos, stress testing, and regression confidence.

---

## Charter deliverables (Phase 4)

- Synthetic market data generator
- Deterministic testing framework
- Property-based tests
- Performance testing harness

**Deliverables:**

- Market data generator with configurable models and deterministic seed
- Deterministic test suite (e.g. proptest) with invariants (no crossed book, quantity conservation)
- Performance benchmarks documented (e.g. criterion for engine; optional load harness for REST/FIX)

---

## User stories in scope (Phase 4)

| Story ID | Summary |
|----------|--------|
| US-007 (data source) | Real-time market data — synthetic generator as optional feed for WebSocket/demos |
| *(implicit)* | Deterministic testing — property-based and replay tests for engine correctness |
| *(implicit)* | Performance — benchmarks and load characterization |

---

## Suggested order of work

### 1. Synthetic market data generator

- **Purpose:** Generate deterministic, configurable order flow and/or price processes for demos, load tests, and replay.
- **Scope (MVP):**
  - Deterministic RNG (e.g. seeded rand or fastrand) so the same seed reproduces the same stream.
  - Configurable models: e.g. number of orders, side distribution (buy/sell), price range, quantity range, order type mix (limit vs market), time-in-force.
  - Output: stream of Order (or equivalent) that can be fed into the engine or replayed.
- **Integration:** Optional feed mode: generator pushes orders into the engine at a configurable rate (for demos or load); or tests consume the stream directly (replay).
- **Charter reference:** Configurable price process simulation, order flow simulation, deterministic random number generation.

### 2. Property-based / deterministic tests

- **Tool:** e.g. proptest for Rust (or similar) to generate random order streams.
- **Invariants (charter):**
  - **Conservation of quantity:** Sum of filled quantity across reports = sum of trade quantities.
  - **No negative quantities:** All quantities >= 0 in trades and execution reports.
  - **No crossed book:** Best bid < best ask (or one side empty) after each match.
  - **Price-time priority:** At same price, earlier orders match first (already tested in unit tests; can be rechecked under random streams).
  - **Order state consistency:** State transitions (New to PartialFill/Fill/Canceled/Rejected) are valid.
- **Deterministic replay:** Run a fixed sequence of orders (from generator or saved fixture) and assert on trades/reports; same seed gives same outcome.
- **Scope:** Start with one or two invariants (e.g. no crossed book, quantity conservation); add more as needed.

### 3. Performance benchmarks

- **Engine-only (criterion):**
  - Throughput: orders/sec for submit-only, submit+cancel, submit+modify, or mixed workload.
  - Latency: time per submit_order / cancel_order / modify_order (e.g. p50, p99).
- **Targets (charter):** 10,000+ orders/sec; p99 order ack < 1 ms; p99 match execution < 5 ms (adjust to engine-only where applicable).
- **Optional:** Lightweight load test for REST or FIX (e.g. many concurrent connections, sustained order rate) to find bottlenecks in the protocol layer.
- **Documentation:** How to run benchmarks, where results are logged, and how to interpret them.

### 4. Documentation and definition of done

- Document the generator (config, seed, how to run).
- Document how to run property-based and deterministic tests.
- Document how to run performance benchmarks and record baseline.

---

## Out of scope for Phase 4 (or later slice)

- Full market data service with multiple instruments and external feeds (Phase 4 generator is synthetic and engine-driven or standalone).
- Long-duration soak testing (e.g. 24 h) unless time permits; charter mentions it in performance strategy.
- Formal verification or model checking.

---

## Definition of done (Phase 4)

- [ ] Synthetic market data generator: deterministic seed, configurable model, produces order stream (and optionally feeds engine).
- [ ] At least one property-based or deterministic invariant test (e.g. no crossed book, quantity conservation) using generated or fixed order streams.
- [ ] Performance benchmarks for the engine (e.g. criterion) with documented run instructions and baseline.
- [ ] Documentation for generator, deterministic tests, and benchmarks.

---

## Next phase

After Phase 4 sign-off, proceed to **Phase 5: User Onboarding & Documentation** (onboarding workflow, certification suite, API docs, sandbox).
