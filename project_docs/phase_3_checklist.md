# Phase 3: Security & Governance — Checklist

Work items for Phase 3 in suggested order. Tick as you complete.

---

## 1. Authentication

- [x] **API keys**  
  Validate API key (or bearer token) on REST requests; optional header or `Authorization: Bearer <key>`.
- [x] **Config**  
  Keys (and key→role mapping) from env or file; document format.
- [x] **Dev bypass**  
  Env flag (e.g. `DISABLE_AUTH=true`) to skip auth for local/dev and existing tests.
- [x] **FIX / WebSocket**  
  Optional: apply auth to FIX session or WebSocket upgrade (or document “same key as REST”).

---

## 2. RBAC

- [x] **Roles**  
  Define roles (e.g. `trader`, `admin`, `operator`) and attach to authenticated identity.
- [x] **Permissions**  
  Trader: submit/cancel/modify, market data. Admin: Admin API. Operator: market state, emergency halt.
- [x] **Enforcement**  
  Middleware or per-handler checks; return 403 when role lacks permission.
- [x] **Tests**  
  Request with trader key to Admin API → 403; with admin/operator key → success.

---

## 3. Audit trail

- [x] **Events**  
  Log order submit/cancel/modify, config changes, market state changes, emergency halt.
- [x] **Format**  
  Structured (e.g. JSON) with timestamp, actor, action, resource, outcome.
- [x] **Sink**  
  Stdout, file, or pluggable; document for production (e.g. log aggregator).
- [x] **Test**  
  Assert audit entry (or mock) after a state-changing action.

---

## 4. Admin API

- [x] **Instruments (US-008)**  
  GET/POST/DELETE admin instruments (or single-instrument config); protected by admin role.
- [x] **System config (US-009)**  
  GET/PATCH admin config (small set of parameters); protected by admin role.
- [x] **Market state (US-011)**  
  GET/POST market state: Open | Halted | Closed; operator (or admin) only.
- [x] **Emergency halt (US-012)**  
  POST emergency halt → set Halted, audit; operator (or admin) only.
- [x] **Order rejection when not Open**  
  When state is Halted or Closed, REST and FIX reject new orders (e.g. 503 or FIX reject).

---

## 5. Integration tests

- [x] **Auth**  
  No key → 401; valid key → 200.
- [x] **RBAC**  
  Trader to admin endpoint → 403; admin to admin endpoint → 200.
- [x] **Market state**  
  Halted → order rejected; Open → order accepted.
- [x] **Audit**  
  At least one test that verifies an audit entry is produced (or mock).

---

## 6. Phase 3 definition of done

- [x] Authentication (API key) with dev bypass.
- [x] RBAC with at least trader and admin/operator roles.
- [x] Audit trail for material actions.
- [x] Admin API: instruments/config, market state, emergency halt.
- [x] Integration tests for auth, RBAC, and market state.

---

When the items above are done, you’re in good shape to start **Phase 4: Market Data & Testing**.
