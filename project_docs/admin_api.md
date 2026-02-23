# Admin API (Phase 3 §4)

All admin routes require **Admin** or **Operator** role (403 for Trader). Use `Authorization: Bearer <key>` or `X-API-Key` with a key that has role `admin` or `operator` in `API_KEYS`.

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/admin/status` | Health-style status (ok). |
| GET | `/admin/instruments` | List instruments (single-engine: one entry). |
| POST | `/admin/instruments` | Add instrument (501 single-instrument engine). |
| DELETE | `/admin/instruments/:id` | Remove instrument (501 single-instrument engine). |
| GET | `/admin/config` | Get key-value config (JSON object). |
| PATCH | `/admin/config` | Merge key-value config (body: JSON object). |
| GET | `/admin/market-state` | Get market state: `Open`, `Halted`, or `Closed`. |
| POST | `/admin/market-state` | Set state. Body: `{ "state": "Open" \| "Halted" \| "Closed" }`. Emits audit `market_state_change`. |
| POST | `/admin/emergency-halt` | Set state to **Halted** and emit audit `emergency_halt`. |

## Market state and order rejection

- When state is **Halted** or **Closed**, **new orders** are rejected:
  - **REST:** `POST /orders` and `POST /orders/modify` return **503** with `{ "error": "market not open" }`.
  - **FIX:** NewOrderSingle (D) and OrderCancelReplaceRequest (G) receive a FIX reject with text "market not open".
- **Cancel** (`POST /orders/cancel`, FIX Cancel Request F) is still accepted when Halted/Closed.
- Set state back to **Open** via `POST /admin/market-state` with `{ "state": "Open" }` to accept orders again.

## Config (US-009)

Config is a JSON object; keys and values are arbitrary. The engine does not yet enforce config (e.g. max quantity); it is stored for future use and for operator visibility.

## Audit

- `POST /admin/market-state` emits `market_state_change` with resource `{ "state": "…" }`.
- `POST /admin/emergency-halt` emits `emergency_halt` with resource `{ "state": "Halted" }`.
