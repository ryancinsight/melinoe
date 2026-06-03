//! Concurrent read-throughput scaling: Melinoe shared reads vs `RwLock` /
//! `Mutex` / per-element atomics, measured correctly.
//!
//! # Why a dedicated benchmark
//!
//! The naive form (spawn N threads inside every `b.iter()`, each doing a few
//! reads) measures *thread-spawn overhead*, not read throughput — the spawn cost
//! dwarfs the work and every mechanism looks the same. This benchmark fixes that:
//!
//! * **Amortise spawn** — each spawned thread performs `PASSES` buffer sweeps, so
//!   the one-time `thread::scope` cost is a fraction of a percent of the sample.
//! * **Defeat hoisting** — each pass re-reads the shared buffer behind
//!   `black_box`, so the loads actually happen (a loop-invariant `&T` read would
//!   otherwise be hoisted out, as in the single-threaded read micro-benchmark).
//! * **Realistic working set** — a 1 KiB-element shared buffer swept repeatedly,
//!   not a single hot word.
//! * **Scale threads** — 1→16, reporting elements/s, to expose *scaling* rather
//!   than a single point.
//!
//! The decisive factor is what each mechanism touches on the read path: Melinoe
//! reads are plain loads with **zero shared mutable state**, so they scale with
//! cores; `RwLock::read` performs an atomic RMW on the shared reader count, whose
//! cache line bounces between cores and caps scaling; `Mutex` serialises.

#![allow(missing_docs)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};
use std::thread;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use melinoe::{brand_scope, CellSliceExt, MelinoeCell};

/// Shared read-only working set: 1024 × u64 = 8 KiB (L1/L2 resident).
const BUF: usize = 1024;
/// Buffer sweeps per thread per sample — large enough to amortise spawn.
const PASSES: u64 = 8192;
/// Thread counts to scan (machine has 24 logical cores).
const THREAD_COUNTS: [usize; 5] = [1, 2, 4, 8, 16];

#[inline]
fn sum(s: &[u64]) -> u64 {
    s.iter().copied().fold(0u64, u64::wrapping_add)
}

fn bench_scaling(c: &mut Criterion) {
    let mut g = c.benchmark_group("concurrent_read_scaling");

    for &threads in &THREAD_COUNTS {
        g.throughput(Throughput::Elements(threads as u64 * PASSES * BUF as u64));

        // ── Melinoe, per-cell `borrow` in a loop. ──
        g.bench_with_input(
            BenchmarkId::new("melinoe_per_cell", threads),
            &threads,
            |b, &t| {
                brand_scope(|token| {
                    let cells: Vec<MelinoeCell<'_, u64>> =
                        (0..BUF as u64).map(MelinoeCell::new).collect();
                    let snap = token.share();
                    let cells = &cells;
                    b.iter(|| {
                        thread::scope(|s| {
                            let hs: Vec<_> = (0..t)
                                .map(|_| {
                                    s.spawn(move || {
                                        let mut acc = 0u64;
                                        for _ in 0..PASSES {
                                            let cells = black_box(cells);
                                            for c in cells {
                                                acc = acc.wrapping_add(*c.borrow(snap));
                                            }
                                        }
                                        acc
                                    })
                                })
                                .collect();
                            black_box(
                                hs.into_iter()
                                    .map(|h| h.join().unwrap())
                                    .fold(0u64, u64::wrapping_add),
                            )
                        })
                    });
                });
            },
        );

        // ── Melinoe, zero-copy slice view (vectorisable). ──
        g.bench_with_input(
            BenchmarkId::new("melinoe_slice", threads),
            &threads,
            |b, &t| {
                brand_scope(|token| {
                    let cells: Vec<MelinoeCell<'_, u64>> =
                        (0..BUF as u64).map(MelinoeCell::new).collect();
                    let snap = token.share();
                    let cells = &cells;
                    b.iter(|| {
                        thread::scope(|s| {
                            let hs: Vec<_> = (0..t)
                                .map(|_| {
                                    s.spawn(move || {
                                        let mut acc = 0u64;
                                        for _ in 0..PASSES {
                                            acc = acc.wrapping_add(sum(black_box(
                                                cells.borrow_slice(snap),
                                            )));
                                        }
                                        acc
                                    })
                                })
                                .collect();
                            black_box(
                                hs.into_iter()
                                    .map(|h| h.join().unwrap())
                                    .fold(0u64, u64::wrapping_add),
                            )
                        })
                    });
                });
            },
        );

        // ── RwLock: a read-lock acquire (atomic RMW on reader count) per sweep. ──
        g.bench_with_input(BenchmarkId::new("rwlock", threads), &threads, |b, &t| {
            let data: RwLock<Vec<u64>> = RwLock::new((0..BUF as u64).collect());
            let data = &data;
            b.iter(|| {
                thread::scope(|s| {
                    let hs: Vec<_> = (0..t)
                        .map(|_| {
                            s.spawn(move || {
                                let mut acc = 0u64;
                                for _ in 0..PASSES {
                                    let guard = data.read().unwrap();
                                    acc = acc.wrapping_add(sum(black_box(guard.as_slice())));
                                }
                                acc
                            })
                        })
                        .collect();
                    black_box(
                        hs.into_iter()
                            .map(|h| h.join().unwrap())
                            .fold(0u64, u64::wrapping_add),
                    )
                })
            });
        });

        // ── Mutex: exclusive lock per sweep — readers serialise. ──
        g.bench_with_input(BenchmarkId::new("mutex", threads), &threads, |b, &t| {
            let data: Mutex<Vec<u64>> = Mutex::new((0..BUF as u64).collect());
            let data = &data;
            b.iter(|| {
                thread::scope(|s| {
                    let hs: Vec<_> = (0..t)
                        .map(|_| {
                            s.spawn(move || {
                                let mut acc = 0u64;
                                for _ in 0..PASSES {
                                    let guard = data.lock().unwrap();
                                    acc = acc.wrapping_add(sum(black_box(guard.as_slice())));
                                }
                                acc
                            })
                        })
                        .collect();
                    black_box(
                        hs.into_iter()
                            .map(|h| h.join().unwrap())
                            .fold(0u64, u64::wrapping_add),
                    )
                })
            });
        });

        // ── Per-element relaxed atomics: lock-free, but each load is atomic and
        //    the reduction cannot vectorise. ──
        g.bench_with_input(
            BenchmarkId::new("atomic_per_elem", threads),
            &threads,
            |b, &t| {
                let data: Vec<AtomicU64> = (0..BUF as u64).map(AtomicU64::new).collect();
                let data = &data;
                b.iter(|| {
                    thread::scope(|s| {
                        let hs: Vec<_> = (0..t)
                            .map(|_| {
                                s.spawn(move || {
                                    let mut acc = 0u64;
                                    for _ in 0..PASSES {
                                        let data = black_box(data);
                                        for a in data {
                                            acc = acc.wrapping_add(a.load(Ordering::Relaxed));
                                        }
                                    }
                                    acc
                                })
                            })
                            .collect();
                        black_box(
                            hs.into_iter()
                                .map(|h| h.join().unwrap())
                                .fold(0u64, u64::wrapping_add),
                        )
                    })
                });
            },
        );
    }

    g.finish();
}

criterion_group!(benches, bench_scaling);
criterion_main!(benches);
