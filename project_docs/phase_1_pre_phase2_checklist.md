# Phase 1: Pre–Phase 2 Checklist

Things to do (or confirm) before starting Phase 2. Order is a suggestion.

---

## 1. Modify order

- [ ] **Implement modify**  
  Charter requires “Modify requests.” Typical approach: **cancel by `OrderId` then add** the new order (same or new id). Preserve price-time semantics (modified order goes to the back of its price level).
- [ ] **API**  
  e.g. `modify_order(&mut self, order_id: OrderId, new_price: Decimal, new_quantity: Decimal) -> Result<(), String>`, or accept a replacement `Order` and treat as cancel+add.
- [ ] **Test**  
  Add order → modify price or size → verify book state and (if you run matching) execution reports.

---

## 2. Unit tests (aim for >90% coverage on book + matching)

- [ ] **IOC**  
  Order that cannot be fully filled is canceled; no remainder on book; one Canceled report.
- [ ] **FOK**  
  Order that cannot be fully filled gets no fills and one Canceled report.
- [ ] **Self-trade**  
  Two orders, same `TraderId`, crossing: they do **not** match each other; resting order stays on book.
- [ ] **Price-time priority**  
  Two resting orders at same price; aggressor matches against the **first** (earlier) resting order.
- [ ] **Cancel resting**  
  Add order → cancel by `OrderId` → book no longer has that order (you may already cover this in `add_and_cancel_order`; if so, just confirm).
- [ ] **Market order**  
  Buy/sell with no price; takes liquidity at best ask/bid and produces expected trades/reports.
- [ ] **Modify**  
  Once implemented: add → modify → verify book; optionally run matching and check reports.

---

## 3. Edge cases and invariants

- [ ] **Invalid order**  
  Limit order with `price: None` is rejected (you already return `Err` from `add_order`; add a test that expects `Err`).
- [ ] **No crossed book**  
  After any match, `best_bid < best_ask` (or one side empty). Add a test that runs a few matches and asserts this.
- [ ] **No negative quantities**  
  All `ExecutionReport` and `Trade` quantities ≥ 0. Can be a short test or property.

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

- [ ] All core types and IDs defined and used consistently.
- [ ] Order book supports add, cancel, **modify** with correct price-time ordering.
- [ ] Matching implements price-time priority and produces trades and execution reports.
- [ ] GTC / IOC / FOK behave correctly.
- [ ] Unit test coverage for order book and matching is **> 90%**.
- [ ] No crossed book after matching; no negative quantities; execution reports match charter schema.

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
