# Phase 2: Protocol Layer — Checklist

Work items for Phase 2 in suggested order. Tick as you complete.

---

## 1. Protocol abstraction

- [ ] **Service interface**  
  Document (or trait) the operations adapters use: submit order, cancel, modify, optional book snapshot. Engine already implements these; ensure one clear entry point (e.g. `Engine` or thin wrapper).
- [ ] **Shared state**  
  REST and WebSocket share the same `Engine` (e.g. `Arc<Mutex<Engine>>` in `AppState`). FIX adapter will use same engine (in-process or over local API).

---

## 2. REST API

- [x] **Submit order**  
  `POST /orders` → `Engine::submit_order`; returns trades + reports.
- [x] **Cancel order**  
  `POST /orders/cancel` (body: `{ "order_id": number }`); calls `Engine::cancel_order`; returns `{ "canceled": true|false }`.
- [x] **Modify order**  
  `POST /orders/modify` (body: `{ "order_id": number, "replacement": Order }`); calls `Engine::modify_order`; returns trades + reports.
- [x] **OpenAPI spec**  
  `project_docs/openapi.yaml` with paths and schemas (Order, Trade, ExecutionReport, CancelRequest, ModifyRequest, Error). Serve via `GET /openapi.json` optional (spec in repo for docs/tooling).

---

## 3. WebSocket market data

- [ ] **Endpoint**  
  e.g. `WS /ws/market-data`; Axum WebSocket upgrade.
- [ ] **Snapshot**  
  On connect, send current book (best bid/ask or full depth) as JSON.
- [ ] **Updates (optional)**  
  On trade or book change, broadcast to connected clients (can add in a follow-up slice).

---

## 4. FIX 4.4 adapter

- [ ] **Design**  
  Choose approach: in-process FIX engine (Rust crate) or sidecar that talks to engine via REST/TCP.
- [ ] **NewOrderSingle → Engine::submit_order**  
  Map FIX message to `Order`; call engine; send ExecutionReport(s) as FIX.
- [ ] **Cancel / Replace**  
  Map to `cancel_order` / `modify_order`; send execution reports.
- [ ] **QuickFIX test**  
  Connect with QuickFIX client; submit, cancel, modify; verify execution reports.

---

## 5. Integration tests

- [x] **REST**  
  `tests/rest_api.rs`: spawn server, then test `GET /health`, `POST /orders`, `POST /orders/cancel`, `POST /orders/modify`; assert status and response shape. Run with `cargo test --test rest_api`.
- [ ] **WebSocket**  
  Test connect and receive snapshot (e.g. with `tokio-tungstenite` or similar).
- [ ] **FIX**  
  If CI-friendly, add test with stub FIX client or document manual QuickFIX verification.

---

## 6. Phase 2 definition of done

- [ ] Protocol abstraction used by REST and WebSocket.
- [ ] REST: submit, cancel, modify; OpenAPI spec.
- [ ] WebSocket: market data (snapshot; updates optional).
- [ ] FIX 4.4: orders and execution reports; QuickFIX-tested.
- [ ] Integration tests for REST and WebSocket.

---

When the items above are done, you’re in good shape to start **Phase 3: Security & Governance**.
