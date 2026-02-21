# Phase 1: Core Engine — Plan

**Scope:** Weeks 1–4 per charter  
**Goal:** Working order book with add/cancel/modify, price-time priority matching, and basic execution reports, with unit test coverage > 90%.

---

## Charter deliverables (Phase 1)

- Order book implementation
- Basic matching logic (price-time priority)
- Order state management
- Unit tests

**Deliverables:**

- Working order book with add/cancel/modify
- Matching logic with unit test coverage > 90%
- Basic execution report generation

---

## Suggested order of work

### 1. Types and IDs

- Define `OrderId`, `ExecutionId`, `TradeId`, `InstrumentId`, `TraderId` (e.g. newtype wrappers or type aliases).
- Define `Side` (Buy/Sell), `OrderType` (Limit/Market), `TimeInForce` (GTC, IOC, FOK), `OrderStatus`, `ExecType`.
- Introduce a decimal type for price/quantity (e.g. `rust_decimal::Decimal` or fixed-precision type).
- Define `Order`, `ExecutionReport`, and `Trade` structs aligned with the charter data models.

### 2. Order book (single instrument)

- Implement a single-instrument order book: bids and asks, sorted by price-time.
- **Add:** insert limit order into the correct side/level; maintain price-time ordering.
- **Cancel:** remove by `OrderId`; return success/not-found.
- **Modify:** cancel + add, or in-place update with same price-time semantics as charter (e.g. re-queue at end of same price level if behavior is “modify = cancel/replace”).
- No matching yet; focus on correct book shape and add/cancel/modify behavior.
- Unit tests: add at various price levels; cancel; modify; verify depth and ordering.

### 3. Matching logic (price-time priority)

- Run matching when a new order is added (or when a modify re-enters an order).
- **Price-time priority:** best bid vs best ask; at same price, earlier order (e.g. by `OrderId` or timestamp) first.
- **Partial fills:** reduce resting order and incoming order; leave remainder on book or return for further handling (e.g. IOC/FOK).
- **Outputs:** generate `Trade` and `ExecutionReport` events (or equivalent internal events).
- Handle **GTC / IOC / FOK** (and any other TIF in scope) so that behavior is testable.
- Unit tests: two orders match fully; partial fill; price-time ordering; TIF (e.g. IOC that doesn’t fill fully is cancelled).

### 4. Execution reports and trades

- For every match and every order state change, produce an `ExecutionReport` (and `Trade` where applicable).
- Ensure fields align with charter: `exec_type`, `order_status`, `filled_quantity`, `remaining_quantity`, `avg_price`, `last_qty`, `last_px`, etc.
- Unit tests: assert correct reports and trades for a set of hand-written scenarios.

### 5. Coverage and edge cases

- Aim for **> 90% unit test coverage** on order book and matching.
- Add tests for: empty book, single side, self-trade (if in Phase 1 scope), cancel/modify of partially filled orders, and any edge cases called out in the charter (e.g. no negative quantities, no crossed book post-match).

---

## Out of scope for Phase 1

- Protocol layer (FIX, REST, WebSocket, gRPC)
- Auth, RBAC, risk limits
- Multi-instrument book or routing
- Synthetic market data generator
- Persistence / event log (can be stubbed or minimal)

---

## Definition of done (Phase 1)

- [x] All core types and IDs defined and used consistently.
- [x] Order book supports add, cancel, modify with correct price-time ordering.
- [x] Matching implements price-time priority and produces trades and execution reports.
- [x] GTC/IOC/FOK (and any other in-scope TIF) behave correctly.
- [x] Unit test coverage for order book and matching is > 90%.
- [x] No crossed book after matching; no negative quantities; execution reports match charter schema.

---

## Deploy to GKE

- **Prerequisites:** For Cloud Build, enable the API: `gcloud services enable cloudbuild.googleapis.com --project=YOUR_PROJECT`.
- **Build and push:** Run `./deploy/deploy-gcp.sh GCP_PROJECT [GCP_REGION] [GKE_CLUSTER]` (or set env vars). Script builds the Docker image, pushes to Artifact Registry, and applies Kubernetes manifests.
- **Run from GCP only:** Push your code to a repo connected to GCP, then **Cloud Build → Submit build** or create a **Trigger**. The pipeline runs tests, builds, pushes, and (if you set trigger substitutions) deploys to GKE. In the trigger, set **Substitution variables**: `_GKE_CLUSTER` = your cluster name, `_GKE_REGION` = cluster region; leave `_GKE_CLUSTER` empty to skip deploy.
- **Manifests:** Deployment, Service (LoadBalancer), and HPA under `deploy/kubernetes/`. Rollout wait uses `--timeout=300s`.
- **External access:** After deploy, get the external IP with `kubectl get svc dire-matching-engine`. LoadBalancer ingress may take 1–2 minutes to show an IP; check `kubectl describe svc dire-matching-engine` for **LoadBalancer Ingress** if EXTERNAL-IP is pending.
- **Verify:** `curl http://<EXTERNAL-IP>/health` should return `ok`; POST orders to `http://<EXTERNAL-IP>/order`.
- **Console:** In GCP Console, select **Kubernetes Engine → Clusters**, click the cluster, then **Workloads** to see the deployment and pods.

---

## Next phase

After Phase 1 sign-off, proceed to **Phase 2: Protocol Layer** (or, if you prefer, the **synthetic market data** story as the first slice of Phase 4 for deterministic testing and demos).
