# Phase 3: Security & Governance — Plan

**Scope:** Weeks 8–10 per charter  
**Goal:** Authentication (e.g. API keys), role-based access control (RBAC), audit trail, Admin API for configuration and instrument management, and market state (Open/Halted/Closed) with emergency halt. REST, WebSocket, and FIX remain the protocol layer; Phase 3 adds security and operational controls on top.

---

## Charter deliverables (Phase 3)

- Authentication (API keys or equivalent)
- RBAC (role-based access control)
- Audit trail
- Admin API for configuration

**Deliverables:**

- API key (or token) authentication for REST and optionally FIX/WebSocket
- Roles and permissions (e.g. trader vs admin vs operator)
- Audit log of material actions (order submit/cancel/modify, config changes, market state)
- Admin API: instrument management, system parameters, market state (Open/Halted/Closed), emergency halt

---

## User stories in scope (Phase 3)

| Story ID | Summary |
|----------|---------|
| US-008 | Admin: add and remove instruments via Admin API |
| US-009 | Admin: configure system parameters |
| US-010 | Admin: role-based access control for user permissions |
| US-011 | Market operator: control market state (Open/Halted/Closed) |
| US-012 | Market operator: emergency halt capability |

---

## Suggested order of work

### 1. Authentication

- **API keys:** Introduce API key (or bearer token) auth for REST: validate on each request (e.g. `Authorization: Bearer <key>` or custom header). Keys can be configured via env or a simple store (file / in-memory for MVP).
- **FIX:** Optionally validate session (e.g. by SenderCompID or a FIX-specific credential) so only known clients connect.
- **WebSocket:** Require auth at upgrade (e.g. query param or first message) or rely on same API key as REST if used from the same client.
- Keep **no auth** as an option (e.g. env `DISABLE_AUTH=true`) for local/dev so existing tests and QuickFIX flows still work.

### 2. RBAC (roles and permissions)

- **Roles:** Define a small set (e.g. `trader`, `admin`, `operator`) and attach to the authenticated identity (e.g. derived from API key or FIX SenderCompID).
- **Permissions:** Map roles to actions: trader = submit/cancel/modify orders, read market data; admin = Admin API (instruments, config); operator = market state, emergency halt.
- **Enforcement:** Middleware or per-handler checks: before calling engine or admin endpoints, verify the request’s identity has the required permission.
- **Storage:** Role–key mapping can be config (env/file) or a minimal store; no database required for MVP.

### 3. Audit trail

- **Events:** Log (to stdout, file, or a dedicated audit sink) material events: order submitted/canceled/modified, config changed, market state changed, emergency halt.
- **Fields:** Timestamp, actor (key/session id), action, resource (e.g. order_id, instrument_id), outcome (success/failure), optional details (e.g. rejection reason).
- **Format:** Structured (e.g. JSON lines) so it can be shipped to a SIEM or log aggregator later.
- **Scope:** REST, FIX, and Admin API actions that change state; read-only (e.g. market data snapshot) can be excluded or sampled.

### 4. Admin API

- **Base path:** e.g. `/admin` (or `/api/admin`), protected by admin/operator role.
- **Instruments (US-008):** `GET /admin/instruments`, `POST /admin/instruments` (add), `DELETE /admin/instruments/:id` (remove). Today the engine is single-instrument; this can mean “register another engine” or “configure allowed instrument list” for a multi-engine setup, or a single “current instrument” config for the existing single-engine process.
- **System parameters (US-009):** `GET /admin/config`, `PATCH /admin/config` for key-value or structured config (e.g. limits, feature flags). Scope to a small set of parameters that the engine or API respects.
- **Market state (US-011, US-012):** `GET /admin/market-state`, `POST /admin/market-state` with body `{ "state": "Open" | "Halted" | "Closed" }`. When state is Halted or Closed, REST/FIX order submission returns 503 or a FIX reject; matching can be paused. **Emergency halt:** same as setting state to Halted (or a dedicated `POST /admin/emergency-halt` that sets Halted and optionally writes a strong audit entry).

### 5. Integration and tests

- **Auth:** Integration test: request without key → 401; with valid key and correct role → 200.
- **RBAC:** Request with trader key to Admin API → 403; with admin key → 200.
- **Audit:** After an action, assert audit log contains the expected event (or mock the sink and assert calls).
- **Market state:** Submit order when state is Halted → rejected; set state to Open, submit again → accepted.

---

## Out of scope for Phase 3

- Full identity provider (OAuth2/OIDC) or user database (can be Phase 5 or later)
- TLS termination (assume load balancer or reverse proxy; document that production should use HTTPS)
- Rate limiting per key (can be added in Phase 3 or later as a small slice)
- Multi-tenancy or per-instrument permissions (single engine, single instrument; Admin API can grow later)

---

## Definition of done (Phase 3)

- [ ] API key (or token) authentication is implemented and can be disabled for dev/tests.
- [ ] RBAC: at least two roles (e.g. trader, admin/operator) with permissions enforced on REST and Admin API.
- [ ] Audit trail: material actions are written to an audit log in a structured format.
- [ ] Admin API: endpoints for instruments (or instrument config), system config, and market state (Open/Halted/Closed); emergency halt sets Halted and is audited.
- [ ] Integration tests cover auth, RBAC, and market state rejection when halted/closed.

---

## Next phase

After Phase 3 sign-off, proceed to **Phase 4: Market Data & Testing** (synthetic market data generator, deterministic tests, performance benchmarks).
