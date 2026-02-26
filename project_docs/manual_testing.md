# Manual testing guide

Use this to manually test the engine after deployment. Run the engine in one terminal, then run the commands below in another (from the repo root, or adjust URLs if the engine is remote).

---

## 1. Start the engine

**Option A — Cargo (no auth):**

```bash
cd /path/to/dire_matching_engine
export DISABLE_AUTH=true
export PORT=8080
export FIX_PORT=9876
cargo run
```

**Option B — Docker:**

```bash
docker build -t dire-matching-engine .
docker run -p 8080:8080 -p 9876:9876 -e DISABLE_AUTH=true dire-matching-engine
```

**Option C — Docker Compose:**

```bash
docker compose up -d
```

Wait until you see logs like `listening on http://0.0.0.0:8080` and `FIX acceptor on 0.0.0.0:9876`. Use a **second terminal** for the steps below.

**If you see "Address already in use" (FIX bind or PORT):** Another process is using port 8080 or 9876 (e.g. a previous engine run or a Docker container). Either stop it (e.g. `lsof -i :9876` then `kill <PID>`, or `docker stop <container>`) or use different ports (e.g. `PORT=8081 FIX_PORT=9877 cargo run`, then use `http://localhost:8081` in the steps below).

---

## 2. Health check

```bash
curl -s http://localhost:8080/health
```

**Expected:** `ok` (and HTTP 200).

---

## 3. Submit an order

```bash
curl -s -X POST http://localhost:8080/orders \
  -H "Content-Type: application/json" \
  -d '{
    "order_id": 1,
    "client_order_id": "manual-1",
    "instrument_id": 1,
    "side": "Sell",
    "order_type": "Limit",
    "quantity": "10",
    "price": "100",
    "time_in_force": "GTC",
    "timestamp": 1,
    "trader_id": 1
  }'
```

**Expected:** JSON with `"trades"` and `"reports"` arrays. At least one report with `"exec_type": "New"` (order resting on book).

---

## 4. Cancel the order

```bash
curl -s -X POST http://localhost:8080/orders/cancel \
  -H "Content-Type: application/json" \
  -d '{"order_id": 1}'
```

**Expected:** `{"canceled":true}`.

---

## 5. Submit and modify (second order)

Submit:

```bash
curl -s -X POST http://localhost:8080/orders \
  -H "Content-Type: application/json" \
  -d '{
    "order_id": 2,
    "client_order_id": "manual-2",
    "instrument_id": 1,
    "side": "Buy",
    "order_type": "Limit",
    "quantity": "5",
    "price": "99",
    "time_in_force": "GTC",
    "timestamp": 2,
    "trader_id": 1
  }'
```

Modify (reduce quantity to 3):

```bash
curl -s -X POST http://localhost:8080/orders/modify \
  -H "Content-Type: application/json" \
  -d '{
    "order_id": 2,
    "replacement": {
      "order_id": 2,
      "client_order_id": "manual-2",
      "instrument_id": 1,
      "side": "Buy",
      "order_type": "Limit",
      "quantity": "3",
      "price": "99",
      "time_in_force": "GTC",
      "timestamp": 3,
      "trader_id": 1
    }
  }'
```

**Expected:** JSON with `"trades"` and `"reports"`. Reports may show cancel + new for the replacement.

**To see non-null prices** (`avg_price`, `last_qty`, `last_px`): you must get a **fill**. The engine uses **self-trade prevention** — an order does not match against another order from the same `trader_id`. So use **different** `trader_id` for the two sides, e.g. sell with `trader_id: 1`, buy with `trader_id: 2`:

```bash
# Resting sell (trader 1)
curl -s -X POST http://localhost:8080/orders -H "Content-Type: application/json" \
  -d '{"order_id":20,"client_order_id":"sell-20","instrument_id":1,"side":"Sell","order_type":"Limit","quantity":"10","price":"100","time_in_force":"GTC","timestamp":1,"trader_id":1}'

# Matching buy (trader 2) — response will have trades and reports with prices
curl -s -X POST http://localhost:8080/orders -H "Content-Type: application/json" \
  -d '{"order_id":21,"client_order_id":"buy-21","instrument_id":1,"side":"Buy","order_type":"Limit","quantity":"5","price":"100","time_in_force":"GTC","timestamp":2,"trader_id":2}' | python3 -m json.tool
```

In the second response you should see `trades` with one trade and reports with `avg_price`, `last_qty`, `last_px` as strings (e.g. `"100"`, `"5"`), not null.

---

## 5a. Placing trades (admin OK, script for all instruments)

You can place orders with an **admin** (or trader) key; admin keys are allowed to call `POST /orders` as well.

**Single matching trade (two curls):** sell then buy at same price (use different `trader_id` so they match):

```bash
# With auth (replace 'a' with your admin key):
curl -s -X POST http://localhost:8080/orders -H "Authorization: Bearer a" -H "Content-Type: application/json" \
  -d '{"order_id":100,"client_order_id":"c100","instrument_id":1,"side":"Sell","order_type":"Limit","quantity":"10","price":100,"time_in_force":"GTC","timestamp":1,"trader_id":1}'

curl -s -X POST http://localhost:8080/orders -H "Authorization: Bearer a" -H "Content-Type: application/json" \
  -d '{"order_id":101,"client_order_id":"c101","instrument_id":1,"side":"Buy","order_type":"Limit","quantity":"10","price":100,"time_in_force":"GTC","timestamp":2,"trader_id":2}'
```

**Script to place one matching trade per instrument:** for each instrument (from `GET /admin/instruments` or 1..13), the script submits a sell then a buy at 100 so they match. Ensure market is **Open** (script tries to set it).

```bash
# Default: BASE_URL=http://localhost:8080, API_KEY=a, instruments from GET /admin/instruments or 1..13
./scripts/place_trades_all_instruments.sh

# Custom key and URL
API_KEY=myadmin BASE_URL=http://localhost:8080 ./scripts/place_trades_all_instruments.sh

# If you don't use GET /admin/instruments, script uses instrument ids 1..MAX_INSTR (default 13)
MAX_INSTR=5 ./scripts/place_trades_all_instruments.sh
```

---

## 6. Admin (if using API keys)

If you started **with** auth (e.g. `API_KEYS="mykey:trader,adm:admin"`), use a key with admin/operator role:

```bash
# Admin status
curl -s http://localhost:8080/admin/status -H "Authorization: Bearer adm"

# Market state
curl -s http://localhost:8080/admin/market-state -H "Authorization: Bearer adm"
```

**Expected:** `{"status":"ok"}` and `{"state":"Open"}` (or Halted/Closed).

With auth disabled, these endpoints still require a key; without one you get 401. So for a quick manual test without keys, stick to **no auth** (steps 1–5).

---

## 6a. Testing admin features (full sequence)

Start the engine **with auth** so admin endpoints accept a key. Use one terminal for the engine, another for the commands below.

**1. Start engine with trader + admin keys:**

```bash
API_KEYS="trader-key:trader,admin-key:admin,ops-key:operator" cargo run
```

Or with Docker:

```bash
docker run -p 8080:8080 -p 9876:9876 \
  -e API_KEYS="trader-key:trader,admin-key:admin,ops-key:operator" \
  dire-matching-engine
```

**2. Admin status** (admin or operator key):

```bash
curl -s http://localhost:8080/admin/status -H "Authorization: Bearer admin-key"
```

**Expected:** `{"status":"ok"}`

**3. Get market state:**

```bash
curl -s http://localhost:8080/admin/market-state -H "Authorization: Bearer admin-key"
```

**Expected:** `{"state":"Open"}` (or Halted/Closed)

**4. Set market to Halted:**

```bash
curl -s -X POST http://localhost:8080/admin/market-state \
  -H "Authorization: Bearer admin-key" \
  -H "Content-Type: application/json" \
  -d '{"state":"Halted"}'
```

**Expected:** `{"state":"Halted"}`

**5. Try to submit an order (trader key) — should get 503:**

```bash
curl -s -w "\nHTTP_CODE:%{http_code}" -X POST http://localhost:8080/orders \
  -H "Authorization: Bearer trader-key" \
  -H "Content-Type: application/json" \
  -d '{"order_id":30,"client_order_id":"c30","instrument_id":1,"side":"Buy","order_type":"Limit","quantity":"1","price":"100","time_in_force":"GTC","timestamp":1,"trader_id":1}'
```

**Expected:** JSON with `"error":"market not open"` and `HTTP_CODE:503`

**6. Set market back to Open:**

```bash
curl -s -X POST http://localhost:8080/admin/market-state \
  -H "Authorization: Bearer admin-key" \
  -H "Content-Type: application/json" \
  -d '{"state":"Open"}'
```

**Expected:** `{"state":"Open"}`

**7. Emergency halt** (admin or operator):

```bash
curl -s -X POST http://localhost:8080/admin/emergency-halt -H "Authorization: Bearer admin-key"
```

**Expected:** `{"state":"Halted","message":"emergency halt applied"}`. Then get market state again to confirm `{"state":"Halted"}`. Set back to Open with step 6 if you want to accept orders again.

**8. Config get (empty at start) and patch:**

```bash
curl -s http://localhost:8080/admin/config -H "Authorization: Bearer admin-key"
```

**Expected:** `{}` or empty object.

```bash
curl -s -X PATCH http://localhost:8080/admin/config \
  -H "Authorization: Bearer admin-key" \
  -H "Content-Type: application/json" \
  -d '{"max_order_quantity": 500}'
```

**Expected:** `{"ok":true}`

```bash
curl -s http://localhost:8080/admin/config -H "Authorization: Bearer admin-key"
```

**Expected:** `{"max_order_quantity":500}`

**9. Instruments list:**

```bash
curl -s http://localhost:8080/admin/instruments -H "Authorization: Bearer admin-key"
```

**Expected:** `[{"instrument_id":1}]` (single-instrument engine)

**10. RBAC: trader cannot call admin** (should get 403):

```bash
curl -s -w "\nHTTP_CODE:%{http_code}" http://localhost:8080/admin/status -H "Authorization: Bearer trader-key"
```

**Expected:** `HTTP_CODE:403`

---

## 7. Run the certification script

From the repo root:

```bash
./scripts/cert_suite.sh
```

**Expected:** All lines show `OK ...` and final line `cert_suite: all checks passed (target=http://127.0.0.1:8080)` with exit code 0.

If you used **auth**, run instead:

```bash
CERT_API_KEY=your-trader-key ./scripts/cert_suite.sh
```

---

## 8. Optional: WebSocket market data

In a third terminal, connect and receive one snapshot (then you can Ctrl+C):

```bash
# Install websocat if needed: cargo install websocat (or use another WS client)
websocat ws://localhost:8080/ws/market-data
```

**Expected:** One JSON line like `{"type":"snapshot","instrument_id":1,"best_bid":...,"best_ask":...}`. After you submit/cancel orders in the other terminal and the book changes, more snapshot lines may appear.

---

## 9. Optional: Market halted → order rejected

With an admin/operator key, set market to Halted and try to submit:

```bash
# Set halted (use your admin key)
curl -s -X POST http://localhost:8080/admin/market-state \
  -H "Authorization: Bearer adm" \
  -H "Content-Type: application/json" \
  -d '{"state":"Halted"}'

# Submit order → should get 503
curl -s -o /dev/null -w "%{http_code}" -X POST http://localhost:8080/orders \
  -H "Authorization: Bearer mykey" \
  -H "Content-Type: application/json" \
  -d '{"order_id":99,"client_order_id":"c99","instrument_id":1,"side":"Buy","order_type":"Limit","quantity":"1","price":"100","time_in_force":"GTC","timestamp":1,"trader_id":1}'
```

**Expected:** HTTP `503`. Then set state back to Open:

```bash
curl -s -X POST http://localhost:8080/admin/market-state \
  -H "Authorization: Bearer adm" \
  -H "Content-Type: application/json" \
  -d '{"state":"Open"}'
```

---

## Quick checklist

| Step | What to do | Pass? |
|------|------------|-------|
| 1 | Start engine (Cargo / Docker / Compose) | Logs show listening on 8080 and 9876 |
| 2 | `curl .../health` | Body `ok` |
| 3 | POST /orders (sell 10 @ 100) | 200, JSON with reports |
| 4 | POST /orders/cancel (order 1) | `{"canceled":true}` |
| 5 | POST /orders (buy 5 @ 99), then POST /orders/modify (qty 3) | Both 200, reports |
| 7 | `./scripts/cert_suite.sh` | Exit 0, “all checks passed” |

See [deployment.md](deployment.md) and [certification_suite.md](certification_suite.md) for more.
