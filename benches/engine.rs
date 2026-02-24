//! Phase 4 §3: Engine performance benchmarks (Criterion).
//!
//! Run: `cargo bench` or `cargo bench --bench engine`.

use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use dire_matching_engine::market_data_gen::{Generator, GeneratorConfig};
use dire_matching_engine::{Engine, InstrumentId, OrderId};
use rust_decimal::Decimal;

fn bench_submit_order_throughput(c: &mut Criterion) {
    const N: usize = 1000;
    let mut group = c.benchmark_group("engine");
    group.throughput(Throughput::Elements(N as u64));
    group.bench_function("submit_order_1000", |b| {
        b.iter_batched(
            || {
                let config = GeneratorConfig {
                    seed: 42,
                    instrument_id: InstrumentId(1),
                    num_orders: N,
                    tif_gtc_ratio: 1.0,
                    tif_ioc_ratio: 0.0,
                    ..Default::default()
                };
                let engine = Engine::new(InstrumentId(1));
                let orders = Generator::new(config).all_orders();
                (engine, orders)
            },
            |(mut engine, orders)| {
                for order in orders {
                    let _ = engine.submit_order(order).unwrap();
                }
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

fn bench_cancel_order(c: &mut Criterion) {
    const RESTING: usize = 500;
    const CANCELS_PER_ITER: usize = 100;
    let mut group = c.benchmark_group("engine");
    group.throughput(Throughput::Elements(CANCELS_PER_ITER as u64));
    group.bench_function("cancel_order_100_after_500_resting", |b| {
        b.iter_batched(
            || {
                let config = GeneratorConfig {
                    seed: 123,
                    instrument_id: InstrumentId(1),
                    num_orders: RESTING,
                    tif_gtc_ratio: 1.0,
                    tif_ioc_ratio: 0.0,
                    ..Default::default()
                };
                let mut engine = Engine::new(InstrumentId(1));
                let orders = Generator::new(config).all_orders();
                for order in &orders {
                    engine.submit_order(order.clone()).unwrap();
                }
                let cancel_ids: Vec<OrderId> = orders[..CANCELS_PER_ITER]
                    .iter()
                    .map(|o| o.order_id)
                    .collect();
                (engine, cancel_ids)
            },
            |(mut engine, cancel_ids)| {
                for id in cancel_ids {
                    engine.cancel_order(id);
                }
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

fn bench_modify_order(c: &mut Criterion) {
    const RESTING: usize = 200;
    const MODIFIES: usize = 50;
    let mut group = c.benchmark_group("engine");
    group.throughput(Throughput::Elements(MODIFIES as u64));
    group.bench_function("modify_order_50_after_200_resting", |b| {
        b.iter_batched(
            || {
                let config = GeneratorConfig {
                    seed: 456,
                    instrument_id: InstrumentId(1),
                    num_orders: RESTING,
                    tif_gtc_ratio: 1.0,
                    tif_ioc_ratio: 0.0,
                    ..Default::default()
                };
                let mut engine = Engine::new(InstrumentId(1));
                let orders = Generator::new(config).all_orders();
                for order in &orders {
                    engine.submit_order(order.clone()).unwrap();
                }
                let mut replacements = Vec::with_capacity(MODIFIES);
                for (i, o) in orders[..MODIFIES].iter().enumerate() {
                    let mut r = o.clone();
                    r.order_id = OrderId((RESTING + 1 + i) as u64);
                    r.client_order_id = format!("mod-{}", r.order_id.0);
                    if let Some(p) = r.price.as_mut() {
                        let n: i64 = p.to_string().parse().unwrap_or(100);
                        *p = Decimal::from(n + 1);
                    }
                    replacements.push((o.order_id, r));
                }
                (engine, replacements)
            },
            |(mut engine, replacements)| {
                for (old_id, replacement) in replacements {
                    let _ = engine.modify_order(old_id, &replacement);
                }
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_submit_order_throughput,
    bench_cancel_order,
    bench_modify_order
);
criterion_main!(benches);
