# Phase 1: Pre–Phase 2 Checklist

Things to do (or confirm) before starting Phase 2. Order is a suggestion.

---

## 1. Modify order

- [x] **Implement modify**  
  Charter requires “Modify requests.” Typical approach: **cancel by `OrderId` then add** the new order (same or new id). Preserve price-time semantics (modified order goes to the back of its price level).
- [x] **API**  
  `Engine::modify_order(order_id, replacement) -> Result<(Vec<Trade>, Vec<ExecutionReport>), String>`: cancel then run matching on replacement; remainder rests at back of price level.
- [x] **Test**  
  Add order → modify price or size → verify book state and execution reports (`engine_modify_order_*` tests).

---

## 2. Unit tests (aim for >90% coverage on book + matching)

- [x] **IOC**  
  Order that cannot be fully filled is canceled; no remainder on book; one Canceled report. (`ioc_no_liquidity_canceled_one_report`, `ioc_partial_fill_remainder_not_on_book`)
- [x] **FOK**  
  Order that cannot be fully filled gets no fills and one Canceled report. (`fok_insufficient_liquidity_no_fill_canceled`, `fok_sell_insufficient_liquidity_no_fill_canceled`)
- [x] **Self-trade**  
  Two orders, same `TraderId`, crossing: they do **not** match each other; resting order stays on book. (`self_trade_does_not_match`)
- [x] **Price-time priority**  
  Two resting orders at same price; aggressor matches against the **first** (earlier) resting order. (`price_time_priority_matches_earlier_order_first`)
- [x] **Cancel resting**  
  Add order → cancel by `OrderId` → book no longer has that order. (`add_and_cancel_order`, `engine_order_flow_submit_then_cancel` with `best_ask().is_none()`)
- [x] **Market order**  
  Buy/sell with no price; takes liquidity at best ask/bid and produces expected trades/reports. (`market_order_takes_liquidity`)
- [x] **Modify**  
  Add → modify → verify book and execution reports; modify then incoming matches. (`engine_modify_order_*`, `engine_modify_then_incoming_matches`, order_book `modify_order_*`)

---

## 3. Edge cases and invariants

- [x] **Invalid order**  
  Limit order with `price: None` is rejected. Engine returns `Err` in `submit_order`; order_book `add_order` returns `Err`. (`engine_submit_order_limit_without_price_rejected`, `add_order_limit_without_price_returns_err`)
- [x] **No crossed book**  
  After any match, `best_bid < best_ask` (or one side empty). (`invariant_no_crossed_book_after_matching`)
- [x] **No negative quantities**  
  All `ExecutionReport` and `Trade` quantities ≥ 0. (`invariant_no_negative_quantities_in_trades_and_reports`)

---

## 4. Coverage and quality

- [x] **Measure coverage**  
  `cargo llvm-cov --lib`: **order_book 91%**, **matching 96%**, total **93%** (all >90%).
- [x] **Lints**  
  `cargo clippy` — clean (removed dead `exec_id` assign, use `_reports` where unused).
- [x] **Format**  
  `cargo fmt`.

---

## 5. API and docs (helps Phase 2)

- [x] **Single entry point**  
  `Engine` in `src/engine.rs`: holds the book and trade/execution ID counters; `submit_order`, `cancel_order`, `modify_order`; Phase 2 can use this instead of `OrderBook` + `match_order` directly.
- [x] **Rustdoc**  
  Module and type docs in `lib.rs`, `engine`, `types`, `order_book`, `matching`, `execution`; `cargo doc --no-deps` builds the API docs; crate-level example in `lib.rs` runs as a doc test.

---

## 6. Phase 1 definition of done

From `phase_1_plan.md`:

- [x] All core types and IDs defined and used consistently. (`types.rs`: OrderId, ExecutionId, TradeId, InstrumentId, TraderId, Side, OrderType, TimeInForce, OrderStatus, ExecType, Order; used across engine, order_book, matching, execution.)
- [x] Order book supports add, cancel, **modify** with correct price-time ordering. (§1, order_book tests.)
- [x] Matching implements price-time priority and produces trades and execution reports. (`matching.rs`, `execution.rs`, §2 tests.)
- [x] GTC / IOC / FOK behave correctly. (IOC/FOK/GTC covered in §2 unit tests.)
- [x] Unit test coverage for order book and matching is **> 90%**. (§4: order_book 91%, matching 96%, total 93%.)
- [x] No crossed book after matching; no negative quantities; execution reports match charter schema. (§3: `invariant_no_crossed_book_after_matching`, `invariant_no_negative_quantities_in_trades_and_reports`; `ExecutionReport`/`Trade` fields align with charter.)

**Phase 1 sign-off:** All definition-of-done items are satisfied. Ready to start **Phase 2: Protocol Layer** (FIX, REST, WebSocket).

---

## 7. Deploy to GKE

- [ ] **Run deploy script**  
  `./deploy/deploy-gcp.sh GCP_PROJECT [GCP_REGION] [GKE_CLUSTER]` (or set `GCP_PROJECT`, `GCP_REGION`, `GKE_CLUSTER`). Builds image, pushes to Artifact Registry, applies manifests, waits for rollout (300s).  
  To run tests in GCP and see them in the console: `USE_CLOUD_BUILD=1 ./deploy/deploy-gcp.sh ...`; then open **Cloud Build → History** and click a build to see the **test** step log.  
  **Prerequisite for Cloud Build:** enable the API: `gcloud services enable cloudbuild.googleapis.com --project=YOUR_PROJECT`.  
  **Run from GCP only:** Push code to a repo connected to GCP (GitHub or Cloud Source Repositories), then **Cloud Build → Submit build** → choose repo + branch, set config to `cloudbuild.yaml`. Or create a **Trigger** so each push runs the build automatically.
- [ ] **Service is LoadBalancer**  
  `deploy/kubernetes/service.yaml` uses `type: LoadBalancer`. Get external IP: `kubectl get svc dire-matching-engine` (or `kubectl describe svc dire-matching-engine` for LoadBalancer Ingress if EXTERNAL-IP is pending).
- [ ] **Health check**  
  `curl http://<EXTERNAL-IP>/health` returns `ok`.
- [ ] **Workloads in GCP Console**  
  Kubernetes Engine → Clusters → select cluster → **Workloads**; confirm deployment and pods for `dire-matching-engine`.

---

## 8. Optional (if you have time)

- **Property-based test**  
  Use `proptest`: generate random order streams, run through engine, assert invariants (e.g. no crossed book, quantity conservation). Charter mentions this in Phase 4; a single invariant test in Phase 1 is a nice head start.
- **`matching_logic_diagram.md`**  
  Keep it in sync with the code (e.g. “modify = cancel + add”) so it stays the single place that describes behavior.

---

When the items above are done (and you’re happy with the definition of done), you’re in good shape to start **Phase 2: Protocol Layer** (FIX, REST, WebSocket).
