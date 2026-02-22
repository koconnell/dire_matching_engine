# Phase 2: Protocol Layer — Plan

**Scope:** Weeks 5–7 per charter  
**Goal:** Protocol abstraction, REST API with OpenAPI, WebSocket market data streaming, and FIX 4.4 adapter (QuickFIX-testable). Core engine remains protocol-agnostic; adapters translate to/from `Engine` API.

---

## Charter deliverables (Phase 2)

- Protocol abstraction design
- FIX 4.4 adapter implementation
- REST API implementation
- WebSocket streaming
- Integration tests

**Deliverables:**

- FIX connectivity tested with QuickFIX client
- REST API with OpenAPI specification
- WebSocket market data streaming

---

## User stories in scope (Phase 2)

| Story ID | Summary |
|----------|--------|
| US-013 | Protocol adapters (FIX, REST, WebSocket, gRPC) |
| US-003 (delivery) | Order acknowledgments via protocols |
| US-004 (delivery) | Execution reports via protocols |
| US-007 (streaming) | Real-time market data (WebSocket) |

---

## Suggested order of work

### 1. Protocol abstraction

- Define a **protocol-agnostic service interface** that adapters call (e.g. submit order, cancel, modify, and optionally book snapshot). The existing `Engine` already implements this; formalize it as a trait or documented API so FIX/REST/WebSocket adapters all use the same entry point.
- Keep engine in `src/engine.rs`; add a thin `protocol` or `api` module that wraps `Engine` and exposes the operations adapters need.

### 2. REST API (extend current)

- **Existing:** `GET /health`, `POST /orders` (submit).
- **Add:** `POST /orders/cancel` (or `DELETE /orders/:id`), `POST /orders/modify` (or `PATCH /orders/:id`) so REST supports full order lifecycle (US-003, US-004 delivery).
- **OpenAPI:** Add OpenAPI 3.0 spec (e.g. `openapi.yaml` or generated from code) describing all REST endpoints, request/response schemas. Serve `/openapi.json` or static spec if useful.
- **Structure:** Keep routes in `main.rs` or split into `src/api/rest.rs`; share `AppState` with WebSocket later.

### 3. WebSocket market data

- **Endpoint:** e.g. `WS /ws/market-data` (or `/market-data` with upgrade).
- **Payload:** Book snapshot (bid/ask levels) and/or incremental updates; simple JSON. Data source: current engine book state (no synthetic generator required in Phase 2).
- **Integration:** Same `AppState` (engine); on client connect send snapshot, then optionally broadcast updates on trade/book change (can start with snapshot-only, then add broadcasts).

### 4. FIX 4.4 adapter

- **Design:** Separate process or in-process FIX acceptor; receives NewOrderSingle, Cancel, Replace; translates to `Engine` calls; sends ExecutionReport (and Trade) back as FIX messages.
- **Implementation:** Use a Rust FIX library (e.g. `quickfix-rs` or similar) or run a small FIX engine that bridges to the matching engine via TCP/API.
- **Testing:** Connect with QuickFIX (or equivalent) client; send order, cancel, modify; verify execution reports.

### 5. Integration tests

- REST: Call `POST /orders`, cancel, modify; assert status codes and response bodies.
- WebSocket: Connect, receive snapshot; optionally submit order and assert update.
- FIX: End-to-end test with stub or real QuickFIX client.

---

## Out of scope for Phase 2

- Authentication / RBAC (Phase 3)
- Admin API (Phase 3)
- Synthetic market data generator (Phase 4; use live book for WebSocket in Phase 2)
- gRPC (optional; can defer)

---

## Definition of done (Phase 2)

- [ ] Protocol abstraction documented and used by REST and WebSocket (and FIX when implemented).
- [ ] REST API supports submit, cancel, modify; OpenAPI spec exists and is accurate.
- [ ] WebSocket endpoint delivers market data (snapshot minimum; updates optional).
- [ ] FIX 4.4 adapter accepts orders and returns execution reports; tested with QuickFIX client.
- [ ] Integration tests for REST and WebSocket (and FIX if feasible in CI).

---

## Next phase

After Phase 2 sign-off, proceed to **Phase 3: Security & Governance** (auth, RBAC, Admin API, audit).
