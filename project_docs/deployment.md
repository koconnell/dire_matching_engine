# Deployment and sandbox

This document describes how to build and run the Dire Matching Engine, which environment variables to set, where the sandbox lives, and a short runbook for starting the sandbox and running certification.

---

## Build and run

### Docker (recommended for deployment and local sandbox)

**Build:**

```bash
docker build -t dire-matching-engine .
```

**Run:**

```bash
docker run -p 8080:8080 -p 9876:9876 dire-matching-engine
```

- **REST and WebSocket:** `http://<host>:8080` (e.g. `http://localhost:8080`).
- **FIX:** `<host>:9876`.

Publish both ports so REST and FIX are reachable. The image runs as a non-root user and includes only the release binary and ca-certificates.

**With environment variables:**

```bash
docker run -p 8080:8080 -p 9876:9876 \
  -e PORT=8080 \
  -e FIX_PORT=9876 \
  -e DISABLE_AUTH=true \
  dire-matching-engine
```

Or with API keys:

```bash
docker run -p 8080:8080 -p 9876:9876 \
  -e API_KEYS="trader-key:trader,admin-key:admin,ops-key:operator" \
  dire-matching-engine
```

### Cargo (development)

```bash
cargo build --release
./target/release/dire_matching_engine
```

Or run without installing:

```bash
cargo run --release
```

Set env vars as needed (see below). Defaults: `PORT=8080`, `FIX_PORT=9876`, auth disabled if `API_KEYS` is unset.

### Docker Compose (local sandbox)

A one-command local sandbox is provided:

```bash
docker compose up -d
```

This starts the engine with REST on port 8080 and FIX on 9876. Use `http://localhost:8080` and `localhost:9876`. To run with auth, edit `docker-compose.yml` to set `API_KEYS` and omit `DISABLE_AUTH`. See the file in the repo root.

---

## Environment variables

| Variable | Meaning | Default | Docker |
|----------|---------|---------|--------|
| `PORT` | HTTP (REST + WebSocket) listen port | `8080` | Set in Dockerfile; override with `-e PORT=...` |
| `FIX_PORT` | FIX TCP listen port | `9876` | Not in Dockerfile; pass `-e FIX_PORT=9876` and `-p 9876:9876` |
| `INSTRUMENT_ID` | Instrument ID for the single-instrument engine | `1` | Optional |
| `API_KEYS` | Comma-separated `key:role` (e.g. `k1:trader,k2:admin`). Roles: `trader`, `admin`, `operator`. | (unset = auth disabled) | Set for production-like auth |
| `DISABLE_AUTH` | If `true` or `1`, ignore `API_KEYS` and accept all requests (default role). | (unset) | Use for local/sandbox without keys |
| `RUST_LOG` | Log level (e.g. `info`, `debug`). Optional. | (none) | Optional |

See [auth_config.md](auth_config.md) for auth details.

---

## Production considerations

- **Single binary:** The image contains only the engine binary and ca-certificates; no shell or extra tools in the runtime image.
- **Non-root:** The Docker image runs as user `app` (UID 1000).
- **Ports:** Publish both `8080` (REST/WebSocket) and `9876` (FIX) when deploying.
- **Auth:** In production, set `API_KEYS` and do **not** set `DISABLE_AUTH`. Issue keys and roles per client.
- **State:** The engine is in-memory only; restart clears orders and book. Persistence (if needed) is out of scope for this release.
- **TLS:** The server does not terminate TLS. Run behind a reverse proxy (e.g. nginx, Caddy) or a cloud load balancer for HTTPS.
- **Resource limits:** Use `docker run --memory=...` or orchestrator limits as appropriate for your load.

---

## Sandbox availability

| Environment | REST | FIX | How to use |
|-------------|------|-----|------------|
| **Local (Cargo)** | `http://localhost:8080` | `localhost:9876` | `DISABLE_AUTH=true cargo run` (or set `API_KEYS`). |
| **Local (Docker)** | `http://localhost:8080` | `localhost:9876` | `docker run -p 8080:8080 -p 9876:9876 -e DISABLE_AUTH=true dire-matching-engine`. |
| **Local (Docker Compose)** | `http://localhost:8080` | `localhost:9876` | `docker compose up -d`. See `docker-compose.yml`. |
| **Hosted sandbox** | URL and ports from platform team | Same | Contact platform/ops for base URL, FIX port, and test API keys. |

**Reset / rate limit:**

- **Local:** Restart the process or container to reset all state (empty book, default market state Open). No rate limiting.
- **Hosted:** Reset and rate-limit policy are defined by the platform team (documented or on request).

---

## Runbook

### 1. Start sandbox (local)

**Option A — Docker:**

```bash
docker build -t dire-matching-engine .
docker run -p 8080:8080 -p 9876:9876 -e DISABLE_AUTH=true dire-matching-engine
```

**Option B — Docker Compose:**

```bash
docker compose up -d
```

**Option C — Cargo:**

```bash
export DISABLE_AUTH=true
export PORT=8080
export FIX_PORT=9876
cargo run
```

Wait until the process logs that it is listening (e.g. “listening on http://0.0.0.0:8080”, “FIX acceptor on 0.0.0.0:9876”). Typically 2–5 seconds.

### 2. Run certification against sandbox

From the repo root:

```bash
./scripts/cert_suite.sh
```

If the sandbox uses API keys:

```bash
CERT_API_KEY=your-trader-key ./scripts/cert_suite.sh
```

If the sandbox is not on localhost:

```bash
CERT_BASE_URL=http://<sandbox-host>:8080 CERT_API_KEY=... ./scripts/cert_suite.sh
```

**Pass:** Exit code 0 and “all checks passed”.  
**Fail:** Non-zero exit; the script prints the failing step. See [certification_suite.md](certification_suite.md) for how to interpret failures.

### 3. Stop sandbox (Docker Compose)

```bash
docker compose down
```

For a single `docker run` container, stop it with `docker stop <container_id>` or Ctrl+C if run in the foreground.

---

## See also

| Topic | Document |
|-------|----------|
| API keys and roles | [auth_config.md](auth_config.md) |
| Certification suite (details) | [certification_suite.md](certification_suite.md) |
| Onboarding and first calls | [onboarding.md](onboarding.md) |
| REST, WebSocket, FIX API | [api_documentation.md](api_documentation.md) |
