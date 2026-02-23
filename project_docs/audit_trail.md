# Audit trail (Phase 3 §3)

Structured audit events are emitted for material actions so operators and compliance can trace who did what and with what outcome.

## Events

| Action | When | Resource fields (typical) |
|--------|------|---------------------------|
| `order_submit` | REST or FIX order accepted or rejected | `order_id`, `instrument_id` |
| `order_cancel` | Cancel request processed | `order_id` |
| `order_modify` | Replace request processed | `order_id`, `replacement_order_id` |
| `config_change` | Admin config updated (when implemented) | config key / scope |
| `market_state_change` | Market state set (Open / Halted / Closed) (when implemented) | `state` |
| `emergency_halt` | Emergency halt triggered (when implemented) | — |

## Format

One JSON object per event, one line per event (NDJSON). Fields:

- **timestamp_secs** — Unix seconds since epoch. Log aggregators can convert to ISO8601.
- **actor** — Who performed the action: API key id (when auth enabled), `"anonymous"` (when auth disabled), or `"fix"` for FIX-originated actions.
- **action** — One of the action names above.
- **resource** — Optional object with action-specific ids (e.g. `order_id`, `instrument_id`).
- **outcome** — `"success"`, `"rejected"`, or `"not_found"` (e.g. cancel on unknown order).

Example:

```json
{"timestamp_secs":1734567890,"actor":"key1","action":"order_submit","resource":{"order_id":42,"instrument_id":1},"outcome":"success"}
```

## Sink

- **Default:** stdout. Each event is printed as a single JSON line. In production, redirect stdout to a log pipeline (e.g. file, Fluentd, Datadog) for retention and querying.
- **Pluggable:** The server accepts a custom sink via [`create_app_state_with_sink`]. Tests use [`InMemoryAuditSink`] to capture events and assert on them.

Implement the [`AuditSink`] trait to send events elsewhere (e.g. HTTP, Kafka). The trait is called from the request path; keep work minimal (e.g. enqueue to a channel) to avoid adding latency.

## FIX

FIX order/cancel/replace flows can emit the same action types with `actor: "fix"` (and optional session id in resource). Emitting from the FIX acceptor is optional in this slice; when added, use the same [`AuditEvent`] format and the same sink as REST.
