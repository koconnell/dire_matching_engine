# User Stories Mapped to Phased Plan

User stories from the Dire Wolf Matching Engine project, aligned with the charter phases (see `matching_engine_charter.md`).

---

## Phase 1: Core Engine (Weeks 1–4)

| Story ID | User Story | Notes |
|----------|------------|--------|
| **US-001** | As a trader, I want to submit limit orders so that I can specify the price at which I want to buy or sell an asset | Core order book; done in Phase 1. |
| **US-002** | As a trader, I want to submit market orders so that I can execute trades at the current market price | Matching supports market (no price); done in Phase 1. |
| **US-003** | As a trader, I want to receive order acknowledgments so that I can confirm that my order was received by the system | Engine produces execution reports (New); protocol delivery in Phase 2. |
| **US-004** | As a trader, I want to receive execution reports so that I can track the status of my orders | Engine generates reports (New, PartialFill, Fill, Canceled); protocol delivery in Phase 2. |
| **US-005** | As a trader, I want to cancel orders so that I can manage my trading positions effectively | Core engine; done in Phase 1. |
| **US-006** | As a trader, I want to modify orders so that I can adjust my trading strategy as needed | Core engine (cancel + replace); done in Phase 1. |

**Phase 1 deliverables (charter):** Working order book with add/cancel/modify, matching logic with >90% test coverage, basic execution report generation.

---

## Phase 2: Protocol Layer (Weeks 5–7)

| Story ID | User Story | Notes |
|----------|------------|--------|
| **US-013** | As a developer, I want to implement protocol adapters for FIX, REST, WebSocket, and gRPC so that the system can communicate with external platforms | Protocol abstraction, FIX 4.4, REST (extend current), WebSocket; gRPC optional in Phase 2. |
| **US-003** (delivery) | Order acknowledgments | Delivered via FIX/REST/WebSocket in Phase 2. |
| **US-004** (delivery) | Execution reports | Delivered via FIX/REST/WebSocket in Phase 2. |
| **US-007** (streaming) | As a trader, I want to have access to real-time market data so that I can make informed trading decisions | WebSocket market data streaming (Phase 2); data source can be synthetic (Phase 4) or book snapshots. |

**Phase 2 deliverables (charter):** FIX connectivity (e.g. QuickFIX), REST API with OpenAPI spec, WebSocket market data streaming, integration tests.

---

## Phase 3: Security & Governance (Weeks 8–10)

| Story ID | User Story | Notes |
|----------|------------|--------|
| **US-008** | As an admin, I want to add and remove instruments via the Admin API so that I can manage the available trading options | Admin API; instrument management. |
| **US-009** | As an admin, I want to configure system parameters so that I can customize the platform to meet specific requirements | Admin API; configuration. |
| **US-010** | As an admin, I want to have role-based access control so that I can control user permissions effectively | RBAC; charter auth + authorization. |
| **US-011** | As a market operator, I want to control the market state (Open/Halted/Closed) so that I can manage trading sessions | Market state management; session controls. |
| **US-012** | As a market operator, I want to have emergency halt capability so that I can respond to critical situations quickly | Part of market state / Admin or operator API. |

**Phase 3 deliverables (charter):** Authentication (API keys), RBAC, audit trail, Admin API for configuration.

---

## Phase 4: Market Data & Testing (Weeks 11–13)

| Story ID | User Story | Notes |
|----------|------------|--------|
| **US-007** (data source) | Real-time market data | Synthetic market data generator; configurable models, deterministic seed. |
| *(implicit)* | Deterministic testing & performance | Property-based tests, performance harness, benchmarks. |

**Phase 4 deliverables (charter):** Market data generator, deterministic test suite, performance benchmarks.

---

## Phase 5: User Onboarding & Documentation (Weeks 14–16)

| Story ID | User Story | Notes |
|----------|------------|--------|
| **US-014** | As a user, I want to have a clear onboarding process with certification workflows so that I can easily start using the platform | Onboarding workflow, certification test suite, sandbox. |

**Phase 5 deliverables (charter):** Automated onboarding, certification suite, API documentation, sandbox deployed.

---

## Summary Table (for backlog ordering)

| Story ID | Phase | Phase name |
|----------|-------|------------|
| US-001 | 1 | Core Engine |
| US-002 | 1 | Core Engine |
| US-003 | 1 (logic) / 2 (delivery) | Core Engine / Protocol Layer |
| US-004 | 1 (logic) / 2 (delivery) | Core Engine / Protocol Layer |
| US-005 | 1 | Core Engine |
| US-006 | 1 | Core Engine |
| US-007 | 2 (streaming) / 4 (data) | Protocol Layer / Market Data & Testing |
| US-008 | 3 | Security & Governance |
| US-009 | 3 | Security & Governance |
| US-010 | 3 | Security & Governance |
| US-011 | 3 | Security & Governance |
| US-012 | 3 | Security & Governance |
| US-013 | 2 | Protocol Layer |
| US-014 | 5 | User Onboarding & Documentation |

---

*Source: User stories from Dire Wolf Matching Engine Project (Excel); phases from `matching_engine_charter.md`.*
