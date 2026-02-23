# FIX 4.4 adapter — design

## 1. Design choice: **in-process**

We use an **in-process** FIX 4.4 adapter: a Rust module that accepts FIX over TCP and calls the same `Engine` (via `Arc<Mutex<Engine>>`) as REST and WebSocket.

- **Rationale:** Single binary, no extra processes; same process state and latency; FIX adapter shares `AppState` with the HTTP server (or is given the same engine reference when started from `main`).
- **Alternative (sidecar):** A separate process could speak FIX and call our REST API. We rejected it for Phase 2 to avoid deployment complexity and to keep one engine instance.

---

## 2. Architecture

- **FIX acceptor:** A TCP listener (e.g. port 9876). For each connection we run a session loop: read FIX message, parse, dispatch by MsgType, call engine, send FIX responses.
- **Session state:** Per connection we keep `ClOrdID (11) → OrderId` so that OrderCancelRequest / OrderCancelReplaceRequest can resolve `OrigClOrdID (41)` to the internal order id.
- **Engine:** The same `Engine` used by REST/WebSocket. The FIX listener is given `Arc<Mutex<Engine>>` (or an `AppState` that holds it).

---

## 3. Message mapping

### Inbound (client → engine)

| FIX message              | MsgType (35) | Action |
|--------------------------|--------------|--------|
| NewOrderSingle           | D            | Map to `Order`; call `submit_order`; send ExecutionReport(s). |
| OrderCancelRequest       | F            | Resolve order by OrigClOrdID (41) or OrderID (37); call `cancel_order`; send ExecutionReport (Canceled). |
| OrderCancelReplaceRequest| G            | Resolve order by OrigClOrdID (41); call `modify_order` with replacement built from FIX fields; send ExecutionReport(s). |
| Logon                    | A            | Respond with Logon (session established). |
| Logout                   | 5            | Respond with Logout; close connection. |
| Heartbeat                | 0            | Respond with Heartbeat. |

### Outbound (engine → client)

| Our type             | FIX message       | MsgType (35) |
|----------------------|-------------------|--------------|
| ExecutionReport      | Execution Report  | 8            |
| (Trade implied in report) | —            | (per-fill ExecType=Fill/PartialFill) |

### Field mapping (summary)

- **NewOrderSingle → Order:** ClOrdID (11) → client_order_id; we assign OrderID (37) from engine; Symbol (55) or SecurityID (48) → instrument_id; Side (54) 1=Buy 2=Sell; OrderQty (38) → quantity; Price (44) → price (limit); OrdType (40) 1=Market 2=Limit; TimeInForce (59) 0=GTC 3=IOC 4=FOK; we use a default TraderId (e.g. 1) or a tag if present.
- **ExecutionReport (out):** OrderID (37), ClOrdID (11), ExecID (17), OrdStatus (39), ExecType (150), CumQty (14), LeavesQty (151), AvgPx (6), LastPx (31), LastQty (32), etc.

---

## 4. Implementation notes

- **Minimal FIX layer:** Tag-value parser and builder only for the messages we need (no full FIX engine crate). Messages are parsed into a map of tag → value; we build outbound messages by setting tags and computing BodyLength (9) and CheckSum (10).
- **OrderID assignment:** For NewOrderSingle we require a numeric ClOrdID (11) and use it as our internal OrderId so we don’t need a separate mapping for the first order. For replace we use the same ClOrdID→OrderId map; the replacement order gets a new ClOrdID and we assign a new OrderId from the engine.
- **TraderID:** We use a single default (e.g. TraderId(1)) for FIX-originated orders unless we add a custom tag.

---

## 5. Testing

- **Stub/integration test:** In Rust, open TCP to the FIX acceptor, send a raw FIX NewOrderSingle (with valid 8, 9, 10), then read and parse ExecutionReport(s). Assert ExecType and OrdStatus.
- **QuickFIX (or similar):** Document how to run a QuickFIX initiator (or other FIX client) with a config that points to our acceptor host/port. Manual or CI run: submit NewOrderSingle, cancel, replace; verify ExecutionReports. See `project_docs/fix_quickfix_test.md` (or a section in this doc) for steps.
