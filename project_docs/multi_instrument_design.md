# Multi-instrument redesign

This document outlines how to evolve the matching engine from **single-instrument** to **multi-instrument**: one process with multiple order books (one per instrument), optional symbol per instrument, and admin add/remove of instruments.

---

## Current state (multi-instrument implemented)

- **Engine:** `MultiEngine` with `HashMap<InstrumentId, OrderBook>`, instrument registry (optional symbol), and `order_to_instrument` map. Global `next_trade_id` / `next_exec_id`.
- **Orders:** Routed by `order.instrument_id`; unknown instrument returns error.
- **Admin instruments:** GET returns `[{"instrument_id": 1, "symbol": "AAPL"}, ...]`; POST adds instrument (201/409); DELETE removes (204/404/409 if resting orders).
- **WebSocket:** On connect sends one snapshot per instrument; book updates broadcast per instrument (`BookUpdate.instrument_id`).
- **FIX:** Orders carry `instrument_id` from message; engine routes by it. No single-instrument parameter; acceptor uses `MultiEngine`.
- **Startup:** `INSTRUMENT_ID` (default 1) for one instrument, or `INSTRUMENT_IDS=1,2,3` or `INSTRUMENT_IDS=1:AAPL,2:GOOG` for multiple with optional symbols.

---

## Target state (multi-instrument)

- **Engine:** Holds multiple order books keyed by `InstrumentId`. Submits route to the book for `order.instrument_id`. Cancel/modify resolve `order_id` to an instrument (reverse index), then operate on that book.
- **Instruments:** Admin can **add** (create new book) and **delete** (remove book; define behavior when book has resting orders). Optional **symbol** (e.g. `"AAPL"`) per instrument for display/reference.
- **Order ID space:** Keep **global** uniqueness: an `order_id` can appear on only one instrument. Cancel/modify use a map `order_id → instrument_id` to find the book.
- **WebSocket:** Snapshot and updates **per instrument** (existing `BookUpdate.instrument_id`); on connect send one snapshot per instrument, or a combined payload.
- **FIX / REST:** No change to message shapes; `instrument_id` (or Symbol) already on the order; engine routes by it.
- **Startup:** Optional `INSTRUMENT_IDS` or config to pre-create multiple books; or start with empty set and add via admin.

---

## Core design choices

| Topic | Choice | Notes |
|-------|--------|--------|
| **Book storage** | `HashMap<InstrumentId, OrderBook>` | One book per instrument; create on add, remove on delete. |
| **Order → instrument** | `HashMap<OrderId, InstrumentId>` | Updated on submit (resting), cancel, modify. Needed so cancel/modify find the right book without scanning all books. |
| **Trade/Exec IDs** | Global counters | Same `next_trade_id` / `next_exec_id` across all books (simplest). |
| **Symbol** | Optional per instrument | Store `symbol: Option<String>` in instrument registry; GET /admin/instruments returns `[{"instrument_id": 1, "symbol": "AAPL"}]`. Not required for matching. |
| **Delete instrument** | Define policy | Options: (a) reject if book has resting orders; (b) cancel all resting then delete; (c) soft-delete (reject new orders, allow cancel only). Recommend (a) or (b). |
| **Empty engine** | Allow zero instruments | Submit returns "unknown instrument"; admin adds first instrument(s). Or require at least one at startup via env/config. |

---

## Implementation plan

### Phase A: Engine and matching (core)

1. **Instrument registry (minimal)**  
   - Type: `HashMap<InstrumentId, InstrumentMeta>` where `InstrumentMeta` is `{ symbol: Option<String> }` (or just a set of `InstrumentId` at first).  
   - Engine holds: `books: HashMap<InstrumentId, OrderBook>`, `order_to_instrument: HashMap<OrderId, InstrumentId>`, `next_trade_id`, `next_exec_id`, and the registry.

2. **Multi-engine struct**  
   - Replace single `Engine` with `MultiEngine` (or keep name `Engine` and change internals).  
   - `submit_order(order)`: look up `order.instrument_id` in `books`; if missing return `Err("unknown instrument")`. Run matching on that book. When order rests, insert `order_id → instrument_id` into `order_to_instrument`.  
   - `cancel_order(order_id)`: look up `order_id` in `order_to_instrument`; if missing return false. Get book for that instrument, cancel in book, remove from `order_to_instrument`.  
   - `modify_order(order_id, replacement)`: same lookup; replacement must be for same instrument (or allow move? usually same). Call book’s modify; update `order_to_instrument` if replacement has new order_id.

3. **MatchingEngine trait**  
   - `instrument_id()` no longer fits; replace with `instruments() -> Vec<InstrumentId>` or keep for “default” and add `instruments()`.  
   - `best_bid()` / `best_ask()` / `book_snapshot()`: either take `InstrumentId` or return “primary” / first; prefer taking `instrument_id` so WebSocket can ask per instrument.

4. **Tests**  
   - Keep existing single-instrument tests by starting `MultiEngine` with one instrument (e.g. from a `MultiEngine::new_with_instruments([(1, None)])` or similar).  
   - Add tests: two instruments, submit/cancel/modify per instrument, unknown instrument rejected, cancel from correct book.

### Phase B: Admin API

5. **GET /admin/instruments**  
   - Return list of all instruments from registry (and books): e.g. `[{"instrument_id": 1, "symbol": "AAPL"}, {"instrument_id": 2, "symbol": null}]`.

6. **POST /admin/instruments**  
   - Body: `{ "instrument_id": u64, "symbol": optional string }`. If instrument already exists return 409 or 400. Otherwise create new `OrderBook`, insert into `books` and registry. Return 201.

7. **DELETE /admin/instruments/:id**  
   - If book has resting orders, either reject (409) or cancel all then remove. Remove from `books`, registry, and clear `order_to_instrument` for that instrument (or remove entries for orders that were in that book). Return 204.

### Phase C: API layer and startup

8. **AppState**  
   - Replace `engine: Arc<Mutex<Engine>>` with `engine: Arc<Mutex<MultiEngine>>`. All call sites that use `instrument_id()`, `best_bid()`, `best_ask()`, `book_snapshot()` need to pass instrument or iterate (see WebSocket).

9. **REST submit/cancel/modify**  
   - No change to request bodies; order already has `instrument_id`. Engine routes internally.

10. **WebSocket /ws/market-data**  
    - **Option A:** One stream; on connect send snapshots for all instruments (multiple messages or one array). On any book change, broadcast `BookUpdate` with that instrument’s `instrument_id` (already so). Clients that care about one instrument filter by `instrument_id`.  
    - **Option B:** Query param or path for instrument, e.g. `/ws/market-data?instrument_id=1`; single-instrument stream. More API surface but simpler client.  
    - Recommend Option A first (minimal change: send N snapshots on connect, keep broadcasting per-book updates).

11. **Broadcast**  
    - Already per-update with `instrument_id`; no change. When a book is updated, broadcast one `BookUpdate` for that instrument.

12. **main.rs / startup**  
    - Replace single `InstrumentId` with either: (a) `INSTRUMENT_IDS=1,2,3` to pre-create books at startup, or (b) start with empty engine and rely on admin to add instruments. If (b), consider seeding instrument 1 from env for backward compatibility.

### Phase D: FIX and docs

13. **FIX acceptor**  
    - Already passes order’s `instrument_id` from message; engine routes by it. Ensure FIX session or message parsing sets `instrument_id` (e.g. from Symbol 55 or SecurityID 48). No structural change if engine has a `submit_order` that takes full `Order`.

14. **Symbol**  
    - Add `symbol: Option<String>` to instrument registry; include in GET /admin/instruments. Optional in POST body. Use for display only; matching still by `instrument_id`.

15. **Docs**  
    - Update admin_api.md, api_documentation.md, deployment.md: instruments are addable/removable; optional symbol; GET returns list with optional symbol.

---

## Order ID → instrument index

- **When to add:** On submit, when the order (or part of it) rests: record `order_id → instrument_id` for the resting order(s).  
- **When to remove:** On full fill or cancel: remove that `order_id`. On modify: old order_id removed when replaced; if replacement rests, add new order_id.  
- **Scope:** Global: one order_id maps to at most one instrument. So clients must use unique order_ids across instruments (typical anyway).

---

## Backward compatibility

- **Single instrument at startup:** If `INSTRUMENT_ID=1` (or `INSTRUMENT_IDS=1`), create one book and registry entry so behavior matches current.  
- **Existing REST/FIX clients:** No change to order shape; only engine internals and admin instruments change.  
- **Cert script / integration tests:** Keep using instrument_id 1; ensure one instrument exists at startup or add via admin in test setup.

---

## Checklist (summary)

- [ ] Add `InstrumentMeta` (optional symbol) and instrument registry.
- [ ] Add `MultiEngine` with `books`, `order_to_instrument`, global IDs.
- [ ] Implement submit_order (route by instrument_id; update order_to_instrument when resting).
- [ ] Implement cancel_order (lookup order_to_instrument, then book).
- [ ] Implement modify_order (lookup, same-instrument replacement).
- [ ] Adapt MatchingEngine trait (e.g. instruments(), book_snapshot(instrument_id)).
- [ ] GET /admin/instruments returns all instruments (+ optional symbol).
- [ ] POST /admin/instruments creates book and registry entry.
- [ ] DELETE /admin/instruments removes book (policy: reject if non-empty or cancel all).
- [ ] AppState and REST handlers use MultiEngine.
- [ ] WebSocket: on connect send snapshot per instrument; keep per-instrument broadcast.
- [ ] main.rs: create MultiEngine with initial instrument(s) from env.
- [ ] FIX: confirm instrument_id from FIX message used for routing.
- [ ] Tests: single-instrument backward compat; multi-instrument flows; unknown instrument; admin add/delete.
- [ ] Docs: update admin API and deployment for multi-instrument and optional symbol.

When this is done, one process can host multiple instruments, admin can add/remove them (and optionally set a symbol), and order_id remains globally unique with cancel/modify routed via the order→instrument map.
