# Phase 5: User Onboarding & Documentation — Checklist

Work items for Phase 5 in suggested order. Tick as you complete.

---

## 1. Onboarding workflow

- [x] **Steps documented**  
  How to get API keys and roles (trader/admin/operator); where to find sandbox URL and ports (REST, FIX, WebSocket).
- [x] **Onboarding doc**  
  Single doc (e.g. `project_docs/onboarding.md`) that links to API docs, auth config, and certification; optional “request access” process.
- [x] **Review**  
  Someone unfamiliar can follow the doc and get to “I have keys and can call the sandbox.”

---

## 2. Certification test suite

- [x] **Cert script or test**  
  Runnable sequence: connect to engine (configurable URL/ports), run critical path (e.g. health, submit, cancel, modify; optional FIX logon + order + cancel); exit 0 on success.
- [x] **Configurable target**  
  Cert run can target localhost or a sandbox URL (env or CLI).
- [x] **Documentation**  
  How to run the certification suite against sandbox; what “pass” means and how to interpret failures.

---

## 3. API documentation

- [x] **REST**  
  Document all public endpoints (health, orders, cancel, modify, admin); request/response shapes; auth (Bearer / X-API-Key).
- [x] **OpenAPI (optional)**  
  OpenAPI/Swagger spec for REST; generate or maintain by hand; publish (e.g. HTML or in repo).
- [x] **WebSocket**  
  Document endpoint, auth at upgrade, message format (snapshot + updates).
- [x] **FIX**  
  Document session (SenderCompID/TargetCompID), supported messages (Logon, NewOrderSingle, Cancel, CancelReplace, ExecutionReport), port, and any credential notes.
- [x] **Auth and admin**  
  Link to or embed `auth_config.md` and `admin_api.md` (or equivalent content).

---

## 4. Deployment and sandbox

- [x] **Deployment doc**  
  How to build and run: Docker (and Dockerfile), env vars (PORT, FIX_PORT, API_KEYS, DISABLE_AUTH, etc.), and any production considerations.
- [x] **Sandbox**  
  Sandbox environment available: local (e.g. Docker Compose) or hosted; document URL(s), ports, and any reset/rate-limit policy.
- [x] **Runbook**  
  Short runbook for “start sandbox” and “run certification against sandbox.”

---

## 5. Phase 5 definition of done

- [x] Onboarding workflow documented; users can get keys and access sandbox.
- [x] Certification suite runnable against sandbox and documented.
- [x] API documentation covers REST, WebSocket, FIX, auth, and admin (OpenAPI optional).
- [x] Deployment and sandbox documented; sandbox available for cert and integration testing.

---

When the items above are done, the engine is in good shape for user onboarding and certification. Next: production hardening, Phase 6 (if defined), or further feature work.
