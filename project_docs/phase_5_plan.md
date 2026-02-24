# Phase 5: User Onboarding and Documentation — Plan

**Scope:** Weeks 14–16 per charter  
**Goal:** Clear onboarding for new users, a certification test suite so integrations can be validated before go-live, complete API documentation, and a deployed sandbox environment.

---

## Charter deliverables (Phase 5)

- Onboarding workflow implementation
- Certification test suite
- API documentation
- Deployment procedures
- Sandbox environment setup

**Deliverables:**

- Automated user onboarding (or documented manual workflow)
- Certification suite that runs a defined set of operations (REST/FIX) and passes/fails
- Complete API documentation (REST, WebSocket, FIX; auth and admin)
- Sandbox deployed (e.g. Docker image, runbook, or hosted instance)

---

## User stories in scope (Phase 5)

| Story ID | Summary |
|----------|--------|
| US-014 | As a user, I want a clear onboarding process with certification workflows so I can easily start using the platform |

---

## Suggested order of work

### 1. Onboarding workflow

- **Purpose:** New users (or integrating systems) know how to get access and what to do first.
- **Scope (MVP):** Document or implement steps: request API keys and roles (trader/admin/operator), get sandbox URL and ports (REST, FIX, WebSocket). Optional: simple request-access form; or keep it doc-only (e.g. Contact ops for API keys and sandbox credentials).
- **Output:** Onboarding doc (e.g. `project_docs/onboarding.md`) that links to API docs, auth config, and certification.

### 2. Certification test suite

- **Purpose:** Users run a defined sequence of operations; if it passes, they are certified to go live (or to use production credentials).
- **Scope:** Script or test binary that connects to a running engine (configurable URL/ports), performs a fixed set of actions (e.g. health check, submit order, cancel, modify; optional FIX logon + NewOrderSingle + cancel), and exits 0 on success, non-zero on failure. Can reuse existing integration tests run against a deployed sandbox, or a dedicated cert script (e.g. Rust binary or shell + curl).
- **Documentation:** How to run the certification suite against sandbox and what pass means.

### 3. API documentation

- **REST:** OpenAPI (Swagger) spec for health, orders, cancel, modify, admin endpoints, auth headers. Generate from code or maintain by hand; publish as HTML or in repo.
- **WebSocket:** Document endpoint, auth, message format (snapshot + updates).
- **FIX:** Document session params (SenderCompID/TargetCompID), supported messages (Logon, NewOrderSingle, Cancel, CancelReplace, ExecutionReport), and how to obtain credentials if applicable.
- **Auth:** Link to or repeat `auth_config.md` (API keys, roles, dev bypass).
- **Admin:** Link to or repeat `admin_api.md` (market state, instruments, config, emergency halt).

### 4. Deployment and sandbox

- **Deployment procedures:** How to build and run the engine (e.g. Docker, env vars, ports). Document Dockerfile, PORT, FIX_PORT, API_KEYS, DISABLE_AUTH, etc.
- **Sandbox:** A deployed instance (or one-click run) that users can point the certification suite and their clients at. Options: Docker Compose for local sandbox; or a hosted sandbox (e.g. Cloud Run, GKE) with a stable URL. Document URL(s) and any rate limits or reset policy.

### 5. Definition of done

- Onboarding doc points to API docs, auth, and certification.
- Certification suite exists and is documented; can be run against sandbox.
- API documentation covers REST (and optionally OpenAPI), WebSocket, FIX, auth, and admin.
- Deployment/sandbox is documented (and sandbox is deployed if in scope).

---

## Out of scope for Phase 5 (or later)

- Full identity provider (OAuth2/OIDC) or self-service API key portal.
- Multi-tenant or per-customer sandbox isolation (single shared sandbox is enough for MVP).
- Automated request-access ticketing or approval flows (can be manual/email).

---

## Definition of done (Phase 5)

- [ ] Onboarding workflow documented (or implemented); users know how to get keys and access sandbox.
- [ ] Certification test suite: runnable against sandbox, documented; pass/fail clear.
- [ ] API documentation: REST (and optionally OpenAPI), WebSocket, FIX, auth, admin.
- [ ] Deployment and sandbox: build/run documented; sandbox available (local or hosted).

---

## After Phase 5

With Phase 5 complete, the engine has core matching, protocol adapters, security/governance, testing/benchmarks, and user onboarding/documentation. Next steps could be Phase 6 (if defined), production hardening, or feature work (e.g. more instruments, additional protocols).
