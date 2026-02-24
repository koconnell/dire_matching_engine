# Performance benchmarks (Phase 4 §3)

Engine-only benchmarks using [Criterion](https://docs.rs/criterion). No REST/FIX/WebSocket in the measured path.

## How to run

```bash
# All engine benchmarks (default: release build)
cargo bench --bench engine

# Single benchmark
cargo bench --bench engine submit_order
cargo bench --bench engine cancel_order
cargo bench --bench engine modify_order

# Save baseline (optional; Criterion stores history in target/criterion/)
cargo bench --bench engine -- --save-baseline main
# Compare to baseline later:
cargo bench --bench engine -- --baseline main
```

Results are printed to the terminal; Criterion also writes detailed reports under `target/criterion/`.

## Benchmarks

| Benchmark | What it measures | Throughput |
|-----------|------------------|------------|
| **engine/submit_order_1000** | Create engine + generate 1000 GTC orders (seed 42) + submit all. | Elements = 1000 orders per iteration. Report as orders/sec. |
| **engine/cancel_order_100_after_500_resting** | Setup: engine with 500 resting orders. Then 100 `cancel_order` calls per iteration. | Elements = 100 cancels per iteration. |
| **engine/modify_order_50_after_200_resting** | Setup: engine with 200 resting orders. Then 50 `modify_order` calls (cancel + replace) per iteration. | Elements = 50 modifies per iteration. |

Throughput (elements/sec) is reported by Criterion when you set `Throughput::Elements(n)`.

## Baseline (example)

Run on your machine and record. Numbers depend on CPU and load.

- **submit_order_1000:** ~X,XXX–XX,XXX orders/sec (single thread, release).
- **cancel_order_100_after_500_resting:** ~XX,XXX–XXX,XXX cancels/sec.
- **modify_order_50_after_200_resting:** ~X,XXX–XX,XXX modifies/sec.

To establish a baseline: run `cargo bench --bench engine` and paste the “time” and “thrpt” columns from the output into this doc or a spreadsheet.

## Optional: load test (REST)

For HTTP load testing (server + network), use a tool outside the repo, e.g.:

- **wrk:** `wrk -t4 -c100 -d30s --latency http://localhost:8080/health`
- **hey:** `hey -n 10000 -c 50 http://localhost:8080/health`

To stress order submission: start the server, then run a script that POSTs to `/orders` with valid JSON (e.g. using the synthetic generator to build bodies and `reqwest` or `wrk` with a Lua script). Not included in this repo; document your chosen method and baseline in this file or in a separate load-test doc.
