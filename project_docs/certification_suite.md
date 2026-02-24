# Certification test suite

The certification suite is a **runnable script** that hits the engine’s REST critical path. Use it to verify a local or sandbox instance before going to production.

---

## What it runs

| Step | Request | Pass condition |
|------|---------|-----------------|
| 1 | `GET /health` | HTTP 200, body `ok` |
| 2 | `POST /orders` (submit order 1001) | HTTP 200, response has `reports` |
| 3 | `POST /orders/cancel` (cancel 1001) | HTTP 200, `canceled: true` |
| 4 | `POST /orders` (submit order 1002) | HTTP 200 |
| 5 | `POST /orders/modify` (modify 1002) | HTTP 200, response has `reports` |

The script uses **order IDs 1001 and 1002** so it doesn’t clash with typical test data. Use a fresh engine or ensure those IDs are free.

**FIX** (logon + order + cancel) is not included in this script; it is optional and can be added later or run separately (e.g. via FIX integration tests or a dedicated FIX cert script).

---

## How to run

### Prerequisites

- **curl** (with HTTP/JSON support).
- Engine running and reachable (e.g. local or sandbox).

### Config (environment)

| Variable | Meaning | Default |
|----------|---------|---------|
| `CERT_BASE_URL` | REST API base URL (no trailing slash) | `http://127.0.0.1:8080` |
| `CERT_API_KEY` | Optional. Bearer token when the server uses `API_KEYS`. | (none) |

### Examples

**Local engine, auth disabled:**

```bash
# Terminal 1: start engine
DISABLE_AUTH=true PORT=8080 cargo run

# Terminal 2: run cert
./scripts/cert_suite.sh
```

**Local engine with API keys:**

```bash
# Terminal 1
API_KEYS="certkey:trader" cargo run

# Terminal 2
CERT_API_KEY=certkey ./scripts/cert_suite.sh
```

**Sandbox (or any remote host):**

```bash
CERT_BASE_URL=http://sandbox.example.com:8080 CERT_API_KEY=your-sandbox-key ./scripts/cert_suite.sh
```

**Docker:**

```bash
docker run -p 8080:8080 -e DISABLE_AUTH=true dire-matching-engine &
sleep 2
./scripts/cert_suite.sh
```

---

## What “pass” means

- **Exit code 0** — All steps returned the expected HTTP status and response shape. The instance is **cert-ready** for the REST path covered.
- **Exit code non-zero** — At least one step failed. The script prints which step failed and why.

Pass means: health is up, orders can be submitted, canceled, and modified with the given (or no) auth. It does **not** assert on business logic (e.g. exact fill amounts) or on FIX/WebSocket.

---

## How to interpret failures

| Symptom | Likely cause | What to do |
|--------|----------------|------------|
| `GET /health failed (HTTP 000)` or connection timeout | Engine not running or wrong host/port | Start engine; set `CERT_BASE_URL` to the correct base URL. |
| `POST /orders 401 Unauthorized` | Server requires auth, no key sent | Set `CERT_API_KEY` to a key that has **trader** role (e.g. from sandbox or `API_KEYS`). |
| `POST /orders 503` | Market not Open | Set market state to Open via admin API (admin/operator key). |
| `POST /orders/cancel expected canceled:true` | Order 1001 was already canceled or not present | Run against a fresh engine or avoid reusing order 1001. |
| `POST /orders/modify failed` | Order 1002 not resting or already modified | Use a fresh engine or ensure 1002 exists and is restable. |

Other HTTP codes (4xx/5xx) are printed in the failure line; check the response body in the message and the [Admin API](admin_api.md) / [integration tests](integration_tests.md) for expected behavior.

---

## Runbook: “Start sandbox and run certification”

1. Start the engine (e.g. `docker run -p 8080:8080 -e DISABLE_AUTH=true dire-matching-engine` or `cargo run` with `DISABLE_AUTH=true`).
2. Wait until it is listening (e.g. 2–5 seconds).
3. Run:  
   `CERT_BASE_URL=http://127.0.0.1:8080 ./scripts/cert_suite.sh`  
   (add `CERT_API_KEY=...` if auth is enabled).
4. If exit code is 0, certification passed. If not, fix the reported step and re-run.

---

## See also

- [Deployment and runbook](deployment.md) — Build, env vars, sandbox options, start/stop and run cert.
- [Onboarding](onboarding.md) — Keys, sandbox URL/ports, first calls.
- [Integration tests](integration_tests.md) — Full test inventory (REST, WebSocket, FIX).
- [Admin API](admin_api.md) — Market state, config, emergency halt.
