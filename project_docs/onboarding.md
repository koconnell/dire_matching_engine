# Onboarding: Dire Matching Engine

This guide gets you from zero to **“I have credentials and can call the sandbox.”** Use it for sandbox integration and before requesting production access.

---

## What you get

- **REST API** — Submit, cancel, and modify orders; market data over WebSocket; admin and market-state endpoints.
- **FIX 4.4** — NewOrderSingle, Cancel, CancelReplace, ExecutionReports over TCP.
- **Auth** — API keys with roles: **trader** (orders + market data), **admin** (admin API), **operator** (market state, emergency halt).

---

## Step 1: Get API keys and roles

Keys are issued by your platform team (or you create them for local/sandbox).

- **Format:** Each key is assigned a **role**: `trader`, `admin`, or `operator`.  
  Example: `mykey:trader`, `opskey:operator`.
- **How to send:** Use `Authorization: Bearer <key>` or `X-API-Key: <key>` on REST and (for WebSocket) at HTTP upgrade.
- **Sandbox / local:** You can run with auth **disabled** (`DISABLE_AUTH=true`) or with test keys you define yourself (see [Running the engine locally](#running-the-engine-locally)).

Full details: [API key authentication](auth_config.md).

---

## Step 2: Sandbox URL and ports

| Access type | REST (HTTP) | WebSocket | FIX (TCP) |
|-------------|-------------|-----------|-----------|
| **Default (local)** | `http://localhost:8080` | `ws://localhost:8080/ws/market-data` | `localhost:9876` |
| **Docker** | `http://<host>:8080` | `ws://<host>:8080/ws/market-data` | `<host>:9876` |
| **Hosted sandbox** | Provided by platform team | Same host, path `/ws/market-data` | Port provided by platform team |

- **REST base URL:** e.g. `http://localhost:8080` — health at `GET /health`, orders at `POST /orders`, etc.
- **WebSocket:** Same host as REST; path `/ws/market-data`. Send API key in `Authorization` or `X-API-Key` when opening the connection.
- **FIX:** Separate TCP port (default **9876**). SenderCompID/TargetCompID and supported messages are documented in the FIX adapter docs (e.g. `fix_adapter_design.md`, `fix_quickfix_test.md`).

---

## Step 3: First calls

1. **Health (no auth):**  
   `GET http://<sandbox>/health` → `200 OK` and body `ok`.

2. **Submit order (with key):**  
   `POST http://<sandbox>/orders` with `Authorization: Bearer <your-key>` and JSON body (order fields: `order_id`, `client_order_id`, `instrument_id`, `side`, `order_type`, `quantity`, `price` for limit, `time_in_force`, `timestamp`, `trader_id`).  
   See [Integration tests](integration_tests.md) for example payloads.

3. **Admin (admin/operator key):**  
   `GET http://<sandbox>/admin/status` with the same header → `200` with `{"status":"ok"}`.  
   See [Admin API](admin_api.md) for market state, config, and emergency halt.

---

## Request access

- **Sandbox:** Use test keys you define, or run the engine locally with `DISABLE_AUTH=true`. No formal request required unless your organization uses a shared hosted sandbox (then contact the team for URL and keys).
- **Production:** Contact your platform or operations team for production API keys, roles, and endpoints. Keys and key–role mapping are configured server-side (e.g. `API_KEYS` env).

---

## Running the engine locally (sandbox on your machine)

**Using Docker:**

```bash
docker build -t dire-matching-engine .
docker run -p 8080:8080 -p 9876:9876 -e DISABLE_AUTH=true dire-matching-engine
```

Then use `http://localhost:8080` and `localhost:9876` as above. To use API keys instead:

```bash
docker run -p 8080:8080 -p 9876:9876 -e API_KEYS="sandbox-key:trader,admin-key:admin" dire-matching-engine
```

**Using Cargo:**

```bash
export DISABLE_AUTH=true   # optional: no keys required
export PORT=8080
export FIX_PORT=9876
cargo run
```

Optional: set `API_KEYS="key1:trader,key2:admin"` and omit `DISABLE_AUTH` to require keys.

---

## Next steps

| Goal | Doc |
|------|-----|
| Full API reference (REST, WebSocket, FIX) | [api_documentation.md](api_documentation.md) |
| Auth details (keys, roles, headers) | [auth_config.md](auth_config.md) |
| Admin and market state | [admin_api.md](admin_api.md) |
| Deploy and run (Docker, sandbox, runbook) | [deployment.md](deployment.md) |
| REST/WebSocket/FIX test inventory | [integration_tests.md](integration_tests.md) |
| Certification suite | [certification_suite.md](certification_suite.md) |

Once you can call health and submit an order (with or without auth), you’re onboarded. Run the [certification suite](certification_suite.md) against the sandbox to validate your integration before production.
