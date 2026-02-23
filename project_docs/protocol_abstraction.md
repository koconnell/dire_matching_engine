# Phase 2: Protocol abstraction

This document describes the single entry point used by all protocol adapters (REST, WebSocket, FIX) and the shared state model.

---

## 1. Single entry point: `Engine`

All order and book operations go through the **matching engine** type [`Engine`](../src/engine.rs). There is no separate “service” object; the engine is the service.

- **Create:** `Engine::new(instrument_id)` — one engine per instrument per process.
- **Order operations:** `submit_order`, `cancel_order`, `modify_order` (see below).
- **Book snapshot (read-only):** `instrument_id()`, `best_bid()`, `best_ask()`, or the convenience `book_snapshot()`.

Adapters (REST, WebSocket, FIX) do **not** call `OrderBook` or `match_order` directly; they use `Engine` only.

---

## 2. Service interface: `MatchingEngine` trait

The operations adapters use are captured by the [`MatchingEngine`](../src/engine.rs) trait so that:

- The contract is explicit and documented in one place.
- Tests or alternate backends can mock the engine by implementing the trait.

| Operation          | Method            | Description |
|--------------------|-------------------|-------------|
| Submit order       | `submit_order`    | Submit an order; returns trades and execution reports. |
| Cancel order       | `cancel_order`    | Cancel a resting order by id; returns `true` if found and removed. |
| Modify order       | `modify_order`    | Cancel by id, then match the replacement; returns trades and reports. |
| Instrument         | `instrument_id`   | Instrument this engine handles. |
| Best bid / ask     | `best_bid`, `best_ask` | Top of book (read-only). |
| Book snapshot      | `book_snapshot`   | Optional: `BookSnapshot { instrument_id, best_bid, best_ask }` for market data. |

`Engine` implements `MatchingEngine`; the concrete type is the one used in production.

---

## 3. Shared state: `AppState` and `Arc<Mutex<Engine>>`

All protocol layers share **one engine instance** per process via [`AppState`](../src/api.rs):

```text
AppState {
    engine: Arc<Mutex<Engine>>
}
```

- **REST** (Axum handlers) take `Extension(AppState)` and call `state.engine.lock()` to get `&mut Engine`, then invoke `submit_order`, `cancel_order`, `modify_order`, or read `best_bid`/`best_ask` for responses.
- **WebSocket** (e.g. `/ws/market-data`) receives the same `AppState` on upgrade; the connection handler locks the engine, builds a snapshot (instrument_id, best_bid, best_ask), sends it, then subscribes to a **broadcast channel** and forwards every book update to the client. When any handler calls `submit_order`, `cancel_order`, or `modify_order`, it pushes a `BookUpdate` (same shape as the snapshot) on that channel so all connected WebSocket clients receive the new top-of-book.
- **FIX** (when added) will use the same `Arc<Mutex<Engine>>` — either in-process by holding `AppState` and locking the engine for each FIX request, or in a sidecar that talks to this process over REST/TCP and thus still operates on the same logical engine.

So: **one engine, one lock; REST, WebSocket, and FIX all use the same `Arc<Mutex<Engine>>` in `AppState`.**

---

## 4. Summary

| Item                    | Implementation |
|-------------------------|----------------|
| Single entry point      | `Engine` (and optionally the `MatchingEngine` trait). |
| Shared state            | `AppState { engine: Arc<Mutex<Engine>> }` in the API layer. |
| REST                    | Uses `AppState`; all order and cancel/modify flows go through the engine. |
| WebSocket               | Uses same `AppState`; snapshot built from engine’s `book_snapshot()` / best bid–ask. |
| FIX (future)            | Will use the same engine (in-process or via an API that holds `AppState`). |

When the FIX adapter is implemented, it will call the same `Engine` (via the same `AppState` or a process that owns it) and map FIX messages to `submit_order` / `cancel_order` / `modify_order` and execution reports to FIX.
