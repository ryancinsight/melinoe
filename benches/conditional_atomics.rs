//! Conditional-atomic (`BrandedAtomic`) benchmarks.
//!
//! Three angles on "pay for synchronization only while sharing":
//! * `exclusive_counter` — the plain exclusive-phase path vs a real atomic RMW.
//! * `shared_atomic` — the atomic shared-phase path vs a raw atomic (must be
//!   parity: `BrandedAtomic` is a zero-cost wrapper on the atomic side).
//! * `mixed_phase` — a realistic build-then-publish workload end to end.

#![allow(missing_docs)]

use std::sync::atomic::{AtomicU64, Ordering};

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use melinoe::atomic::{BrandedAtomic, Relaxed};
use melinoe::brand_scope;

const ITERS: u64 = 4096;

/// Exclusive-phase bump: `BrandedAtomic` plain store vs `AtomicU64::fetch_add`.
fn bench_exclusive(c: &mut Criterion) {
    let mut g = c.benchmark_group("exclusive_counter_4096x");
    g.throughput(Throughput::Elements(ITERS));

    g.bench_function("branded_exclusive_plain", |b| {
        brand_scope(|mut token| {
            let counter: BrandedAtomic<'_, AtomicU64> = BrandedAtomic::new(0);
            b.iter(|| {
                for _ in 0..ITERS {
                    counter.with_exclusive(&mut token, |v| *v = v.wrapping_add(black_box(1)));
                }
                black_box(counter.load_exclusive(&mut token))
            });
        });
    });

    g.bench_function("atomic_fetch_add", |b| {
        let counter = AtomicU64::new(0);
        b.iter(|| {
            for _ in 0..ITERS {
                counter.fetch_add(black_box(1), Ordering::Relaxed);
            }
            black_box(counter.load(Ordering::Relaxed))
        });
    });

    g.finish();
}

/// Shared-phase atomic ops: `BrandedAtomic` vs raw `AtomicU64`. Expected parity —
/// the wrapper adds nothing on the atomic path.
fn bench_shared_atomic(c: &mut Criterion) {
    let mut g = c.benchmark_group("shared_atomic_4096x");
    g.throughput(Throughput::Elements(ITERS));

    g.bench_function("branded_fetch_add", |b| {
        brand_scope(|token| {
            let counter: BrandedAtomic<'_, AtomicU64> = BrandedAtomic::new(0);
            let snap = token.share();
            b.iter(|| {
                for _ in 0..ITERS {
                    counter.fetch_add(black_box(1), snap, Ordering::Relaxed);
                }
                black_box(counter.load(snap, Ordering::Relaxed))
            });
        });
    });

    g.bench_function("branded_fetch_add_zst_order", |b| {
        brand_scope(|token| {
            let counter: BrandedAtomic<'_, AtomicU64> = BrandedAtomic::new(0);
            let snap = token.share();
            b.iter(|| {
                for _ in 0..ITERS {
                    counter.fetch_add_with(black_box(1), snap, Relaxed);
                }
                black_box(counter.load_with(snap, Relaxed))
            });
        });
    });

    g.bench_function("raw_fetch_add", |b| {
        let counter = AtomicU64::new(0);
        b.iter(|| {
            for _ in 0..ITERS {
                counter.fetch_add(black_box(1), Ordering::Relaxed);
            }
            black_box(counter.load(Ordering::Relaxed))
        });
    });

    g.bench_function("branded_compare_exchange", |b| {
        brand_scope(|token| {
            let counter: BrandedAtomic<'_, AtomicU64> = BrandedAtomic::new(0);
            let snap = token.share();
            b.iter(|| {
                for _ in 0..ITERS {
                    let cur = counter.load(snap, Ordering::Relaxed);
                    let _ = counter.compare_exchange_with(
                        cur,
                        cur.wrapping_add(black_box(1)),
                        snap,
                        Relaxed,
                    );
                }
                black_box(counter.load(snap, Ordering::Relaxed))
            });
        });
    });

    g.bench_function("raw_compare_exchange", |b| {
        let counter = AtomicU64::new(0);
        b.iter(|| {
            for _ in 0..ITERS {
                let cur = counter.load(Ordering::Relaxed);
                let _ = counter.compare_exchange(
                    cur,
                    cur.wrapping_add(black_box(1)),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                );
            }
            black_box(counter.load(Ordering::Relaxed))
        });
    });

    g.finish();
}

/// Realistic mixed phase: `BUILD` private plain bumps (exclusive) then `PUBLISH`
/// atomic ops (shared), vs doing the whole thing atomically.
fn bench_mixed_phase(c: &mut Criterion) {
    const BUILD: u64 = 2048;
    const PUBLISH: u64 = 2048;

    let mut g = c.benchmark_group("mixed_phase_build2k_publish2k");
    g.throughput(Throughput::Elements(BUILD + PUBLISH));

    g.bench_function("branded_conditional", |b| {
        brand_scope(|mut token| {
            let counter: BrandedAtomic<'_, AtomicU64> = BrandedAtomic::new(0);
            b.iter(|| {
                // Build-up: private, plain (no atomics).
                for _ in 0..BUILD {
                    counter.with_exclusive(&mut token, |v| *v = v.wrapping_add(black_box(1)));
                }
                // Publish: now shared, atomic.
                let snap = token.share();
                for _ in 0..PUBLISH {
                    counter.fetch_add(black_box(1), snap, Ordering::Relaxed);
                }
                black_box(counter.load(snap, Ordering::Relaxed))
            });
        });
    });

    g.bench_function("always_atomic", |b| {
        let counter = AtomicU64::new(0);
        b.iter(|| {
            for _ in 0..(BUILD + PUBLISH) {
                counter.fetch_add(black_box(1), Ordering::Relaxed);
            }
            black_box(counter.load(Ordering::Relaxed))
        });
    });

    g.finish();
}

criterion_group!(
    benches,
    bench_exclusive,
    bench_shared_atomic,
    bench_mixed_phase
);
criterion_main!(benches);
