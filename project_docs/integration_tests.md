# Integration and property-based tests

Integration tests spawn the real server (or FIX acceptor) and call endpoints over the wire. They use explicit auth config and in-memory audit sinks so they do not depend on environment variables and can run in parallel. Phase 4 §2 adds property-based and deterministic tests.

## How to run

```bash
# All integration tests
cargo test --tests

# REST API only
cargo test --test rest_api

# WebSocket market-data only
cargo test --test ws_market_data

# FIX adapter only
cargo test --test fix_adapter

# Phase 4 §2: Property-based and deterministic invariants
cargo test --test proptest_invariants

# Phase 4 §3: Engine performance benchmarks
cargo bench --bench engine
```

## Test inventory

### REST API (`tests/rest_api.rs`)

| Test | Coverage |
|------|----------|
| **Health** | |
| `health_returns_ok` | GET /health → 200, body "ok". |
| **Orders** | |
| `submit_order_accepts_limit_order_returns_200` | POST /orders with valid limit order → 200, reports. |
| `submit_order_then_cancel_returns_canceled_true` | Submit then cancel → canceled: true. |
| `cancel_nonexistent_order_returns_canceled_false` | Cancel unknown order → 200, canceled: false. |
| `modify_order_returns_trades_and_reports` | Submit, modify, assert trades/reports. |
| `submit_order_invalid_limit_no_price_returns_400` | Limit order without price → 400. |
| **Auth (§5)** | |
| `auth_required_returns_401_without_key` | With auth enabled, no key → 401. |
| `auth_with_valid_key_returns_200` | Bearer key → 200 on /orders. |
| `auth_accepts_x_api_key_header` | X-API-Key → 200 on /orders. |
| **RBAC (§5)** | |
| `rbac_trader_to_admin_returns_403` | Trader key → GET /admin/status → 403. |
| `rbac_admin_to_admin_returns_200` | Admin key → GET /admin/status → 200. |
| `rbac_operator_to_admin_returns_200` | Operator key → GET /admin/status → 200. |
| `integration_trader_cannot_set_market_state` | Trader key → POST /admin/market-state → 403. |
| **Audit (§5)** | |
| `audit_order_submit_emits_event` | In-memory sink; submit order → one event order_submit, success. |
| **Market state (§5)** | |
| `admin_market_state_halted_rejects_order_then_open_accepts` | Set Halted → POST /orders → 503; set Open → POST /orders → 200. |
| `admin_emergency_halt_sets_halted` | POST /admin/emergency-halt → GET market-state Halted → POST /orders → 503. |
| **Admin API** | |
| `admin_instruments_list_returns_current` | GET /admin/instruments → 200, one instrument. |
| `admin_config_get_and_patch` | GET config empty; PATCH config; GET shows value. |

### WebSocket (`tests/ws_market_data.rs`)

| Test | Coverage |
|------|----------|
| `ws_market_data_sends_snapshot_on_connect` | Connect → one snapshot message. |
| `ws_market_data_snapshot_reflects_book_after_order` | Submit order, connect → snapshot has updated book. |
| `ws_market_data_broadcasts_update_after_order` | Two clients; order → both receive update. |

### FIX adapter (`tests/fix_adapter.rs`)

| Test | Coverage |
|------|----------|
| `fix_logon_returns_logon` | Send Logon (A) → receive Logon. |
| `fix_new_order_single_returns_execution_report` | Logon, NewOrderSingle (D) → ExecutionReport (8), OrdStatus New. |
| `fix_new_order_single_rejected_when_market_halted` | Market state Halted; NewOrderSingle → ExecutionReport with 39=8 (Rejected), 58 contains "market not open". |

### Property-based / deterministic (Phase 4 §2) (`tests/proptest_invariants.rs`)

| Test | Coverage |
|------|----------|
| `prop_invariants_hold_after_replay` | Proptest: for any (seed, num_orders) in range, replay GTC-only synthetic stream into engine; assert no negative quantities in trades and reports. (No crossed book checked here; see `matching::tests::invariant_no_crossed_book_after_matching`.) |
| `deterministic_replay_same_seed_same_outcome` | Same generator config (seed 999, 80 orders) run twice; assert same trade count, report count, and total traded quantity. |

Run: `cargo test --test proptest_invariants`. Default 50 proptest cases; use `PROPTEST_CASES=100` to increase.

## §5 checklist mapping

- **Auth:** No key → 401; valid key → 200 → `auth_required_returns_401_without_key`, `auth_with_valid_key_returns_200` (and X-API-Key).
- **RBAC:** Trader to admin → 403; admin/operator to admin → 200 → `rbac_*`, `integration_trader_cannot_set_market_state`.
- **Market state:** Halted → order rejected; Open → accepted → `admin_market_state_halted_rejects_order_then_open_accepts`, `admin_emergency_halt_sets_halted`, `fix_new_order_single_rejected_when_market_halted`.
- **Audit:** Audit entry after action → `audit_order_submit_emits_event`.
