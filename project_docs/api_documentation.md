# API documentation

This document describes the Dire Matching Engine’s public APIs: REST, WebSocket market data, and FIX 4.4. For authentication and admin behavior, see the linked docs below.

---

## Authentication (summary)

- **REST & WebSocket:** When auth is enabled (`API_KEYS` set, `DISABLE_AUTH` not true), send an API key via **`Authorization: Bearer <key>`** or **`X-API-Key: <key>`**.  
  `/health` is always public. Order and WebSocket routes require a valid key (401 if missing/invalid).  
  Admin routes require role **admin** or **operator** (403 for **trader**).
- **FIX:** Session-level only (SenderCompID/TargetCompID). No API-key auth on the FIX acceptor in this release.
- Full details: [auth_config.md](auth_config.md). Admin endpoints and RBAC: [admin_api.md](admin_api.md).

---

## REST API

Base URL is the engine host and port (e.g. `http://localhost:8080`). All order and admin endpoints accept **JSON** request bodies and return **JSON** where applicable.

### Public

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| GET | `/health` | Liveness. Returns `200` with body `ok`. | None |

### Orders (trader or anonymous when auth disabled)

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| POST | `/orders` | Submit a new order. | Key with role `trader` (or anonymous if auth disabled) |
| POST | `/orders/cancel` | Cancel an order by ID. | Same |
| POST | `/orders/modify` | Replace an order (cancel + submit replacement). | Same |

When **market state** is not **Open**, `POST /orders` and `POST /orders/modify` return **503** with `{ "error": "market not open" }`. Cancel is still accepted. See [admin_api.md](admin_api.md).

### Admin (admin or operator only)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/admin/status` | Status check; returns `{ "status": "ok" }`. |
| GET | `/admin/instruments` | List instruments. Returns array of `{ "instrument_id": number, "symbol": string \| null }`. |
| POST | `/admin/instruments` | Add instrument. Body: `{ "instrument_id": number, "symbol": optional string }`. Returns 201; 409 if already exists. |
| DELETE | `/admin/instruments/:id` | Remove instrument. Returns 204 (no body); 404 if not found; 409 if instrument has resting orders. |
| GET | `/admin/config` | Get config (JSON object). |
| PATCH | `/admin/config` | Merge config (body: JSON object). |
| GET | `/admin/market-state` | Get market state: `Open`, `Halted`, `Closed`. |
| POST | `/admin/market-state` | Set state. Body: `{ "state": "Open" \| "Halted" \| "Closed" }`. |
| POST | `/admin/emergency-halt` | Set state to **Halted** (no body). |

Full admin behavior: [admin_api.md](admin_api.md).

---

### Request / response shapes

#### POST /orders

**Request body (Order):**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `order_id` | number | Yes | Unique order ID (client-assigned). |
| `client_order_id` | string | Yes | Client reference. |
| `instrument_id` | number | Yes | Instrument (e.g. `1`). |
| `side` | string | Yes | `"Buy"` or `"Sell"`. |
| `order_type` | string | Yes | `"Limit"` or `"Market"`. |
| `quantity` | string or number | Yes | Order quantity. |
| `price` | string, number, or null | For Limit only | Limit price; required for `"Limit"`, omit/null for `"Market"`. |
| `time_in_force` | string | Yes | `"GTC"`, `"IOC"`, or `"FOK"`. |
| `timestamp` | number | Yes | Client timestamp. |
| `trader_id` | number | Yes | Trader identifier. **Must be stable per trader:** the exchange must use the same `trader_id` for every order from the same trader so that self-trade prevention and execution reports are correct. |

**Response (200):**

```json
{
  "trades": [ /* see Trade below */ ],
  "reports": [ /* see ExecutionReport below */ ]
}
```

**Error (400):** `{ "error": "<message>" }` (e.g. invalid limit order, validation failure).  
**Error (503):** `{ "error": "market not open" }` when market is not Open.

---

#### POST /orders/cancel

**Request body:**

```json
{ "order_id": 123 }
```

**Response (200):**

```json
{ "canceled": true }
```

or `{ "canceled": false }` if the order was not found or already canceled.

---

#### POST /orders/modify

**Request body:**

| Field | Type | Description |
|-------|------|-------------|
| `order_id` | number | ID of the order to replace. |
| `replacement` | object | Full **Order** (same shape as POST /orders). The replacement’s `order_id` can be the same or a new ID depending on engine behavior. |

**Response (200):** Same as POST /orders: `{ "trades": [ ... ], "reports": [ ... ] }`.  
**Error (400):** `{ "error": "<message>" }` (e.g. order not found).  
**Error (503):** `{ "error": "market not open" }` when market is not Open.

---

#### ExecutionReport (in responses)

| Field | Type | Description |
|-------|------|-------------|
| `order_id` | number | Order ID. |
| `exec_id` | number | Execution report ID. |
| `exec_type` | string | `"New"`, `"PartialFill"`, `"Fill"`, `"Canceled"`, `"Rejected"`. |
| `order_status` | string | `"New"`, `"PartiallyFilled"`, `"Filled"`, `"Canceled"`, `"Rejected"`. |
| `filled_quantity` | string/number | Cumulative filled quantity. |
| `remaining_quantity` | string/number | Remaining quantity. |
| `avg_price` | string/number or null | Average fill price. |
| `last_qty` | string/number or null | Last fill quantity. |
| `last_px` | string/number or null | Last fill price. |
| `timestamp` | number | Timestamp. |

#### Trade (in responses)

| Field | Type | Description |
|-------|------|-------------|
| `trade_id` | number | Trade ID. |
| `instrument_id` | number | Instrument. |
| `buy_order_id` | number | Buy order ID. |
| `sell_order_id` | number | Sell order ID. |
| `price` | string/number | Trade price. |
| `quantity` | string/number | Trade quantity. |
| `timestamp` | number | Timestamp. |
| `aggressor_side` | string | `"Buy"` or `"Sell"`. |

---

## WebSocket: market data

- **Endpoint:** `GET /ws/market-data` (same host as REST; upgrade to WebSocket).
- **Auth:** When auth is enabled, send the API key on the **HTTP upgrade request** (e.g. `Authorization: Bearer <key>` or `X-API-Key: <key>`). Same as REST.

### Message format

All server messages are **JSON** with a `msg_type` field.

**Snapshot (on connect and on each book change):**

```json
{
  "msg_type": "snapshot",
  "instrument_id": 1,
  "best_bid": "100.50",
  "best_ask": "101.00"
}
```

- `best_bid` / `best_ask` are decimal strings (or `null` if no bid/ask).  
- On connect the server sends **one snapshot per instrument** (current book for each). Then it sends a snapshot whenever a book changes (e.g. after order submit/cancel/modify).  
- Client messages are not required; the server may ignore them.

---

## FIX 4.4

- **Transport:** TCP; default port **9876** (configurable via `FIX_PORT`).
- **Session:** **SenderCompID (49)** = client ID; **TargetCompID (56)** = `DIRED`. The acceptor sends 49=DIRED and expects 56=CLIENT (or your client ID) in client messages. Session parameters and field mapping: [fix_adapter_design.md](fix_adapter_design.md), [fix_quickfix_test.md](fix_quickfix_test.md).

### Supported messages

| Direction | FIX message | MsgType (35) | Description |
|-----------|-------------|--------------|--------------|
| Inbound | Logon | A | Session establishment; acceptor responds with Logon. |
| Inbound | NewOrderSingle | D | Submit order; acceptor responds with ExecutionReport(s). |
| Inbound | OrderCancelRequest | F | Cancel by OrigClOrdID (41); ExecutionReport with OrdStatus=4 (Canceled). |
| Inbound | OrderCancelReplaceRequest | G | Replace order; ExecutionReport(s) for replacement. |
| Outbound | Execution Report | 8 | OrdStatus (39), ExecType (150), CumQty (14), LeavesQty (151), etc. |

When **market state** is not **Open**, NewOrderSingle and OrderCancelReplaceRequest are **rejected** (ExecutionReport with OrdStatus=8 Rejected, text “market not open”). Cancel (35=F) is still accepted.

**Credentials:** The FIX acceptor does not validate API keys in this release; identification is by SenderCompID/TargetCompID only.

---

## OpenAPI (optional)

An OpenAPI 3.0 spec for the REST API is available at [openapi.yaml](openapi.yaml) in this directory. You can use it with Swagger UI or other tools to explore or generate clients. It covers health and order endpoints; admin endpoints are summarized and can be extended in the spec as needed.

---

## See also

| Topic | Document |
|-------|----------|
| API keys, roles, headers, RBAC | [auth_config.md](auth_config.md) |
| Admin endpoints, market state, config, emergency halt | [admin_api.md](admin_api.md) |
| Deploy, sandbox, runbook | [deployment.md](deployment.md) |
| FIX design and message mapping | [fix_adapter_design.md](fix_adapter_design.md) |
| QuickFIX verification steps | [fix_quickfix_test.md](fix_quickfix_test.md) |
| Integration test inventory | [integration_tests.md](integration_tests.md) |
| Certification suite | [certification_suite.md](certification_suite.md) |
