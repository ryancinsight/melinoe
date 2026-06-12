//! Comparative access-cost benchmark: Melinoe branded tokens vs the standard
//! runtime synchronization primitives (`AtomicU64`, `Mutex`, `RwLock`).
//!
//! # What this measures, and what it does *not*
//!
//! Melinoe discharges the `T xor &mut T xor &T` exclusion at **compile time**:
//! once a brand's token is in scope, access is a bare reference with no atomic,
//! no lock, and no contention possible. The other primitives discharge the same
//! exclusion at **run time**. These benchmarks therefore measure the *cost of
//! one access when exclusion is already guaranteed by each mechanism*:
//!
//! * `increment_*` — single-threaded read-modify-write latency.
//! * `read_*`      — single-threaded read latency.
//! * `interior_mut_*` — single-threaded interior mutability vs `RefCell`/`Cell`,
//!   the std analogues (Melinoe has no runtime borrow flag).
//! * `concurrent_reads_*` — throughput of N threads issuing shared reads; here
//!   Melinoe's lack of an atomic/lock on the read path is the decisive factor.
//!
//! Melinoe is **not** a drop-in for runtime-contended shared *mutation*: it
//! forbids two simultaneous writers at compile time rather than serializing them
//! at run time. The contended-write case is intentionally absent because the two
//! models are not substitutable there. The differential test in
//! `tests/differential.rs` confirms every primitive computes the same result, so
//! the comparison is over identical work.

// The `criterion_group!` macro expands to an undocumented `benches` function;
// the crate's `missing_docs = "deny"` lint does not apply to generated harness code.
#![allow(missing_docs)]

use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};
use std::thread;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use melinoe::sync::{
    partition_for_each, partition_for_each_available, partition_for_each_with, partition_map,
    partition_map_available, partition_map_with, PartitionPlan,
};
use melinoe::{brand_scope, MelinoeCell};

/// Inner-loop length per sampled iteration (amortises criterion's per-sample
/// overhead so the per-access cost dominates the measurement).
const ITERS: u64 = 1024;

fn bench_increment(c: &mut Criterion) {
    let mut g = c.benchmark_group("increment_1024x");
    g.throughput(Throughput::Elements(ITERS));

    g.bench_function("atomic_relaxed", |b| {
        let a = AtomicU64::new(black_box(0));
        b.iter(|| {
            for _ in 0..ITERS {
                a.fetch_add(black_box(1), Ordering::Relaxed);
            }
            black_box(a.load(Ordering::Relaxed))
        });
    });

    g.bench_function("mutex", |b| {
        let m = Mutex::new(black_box(0u64));
        b.iter(|| {
            for _ in 0..ITERS {
                *m.lock().unwrap() += black_box(1);
            }
            black_box(*m.lock().unwrap())
        });
    });

    g.bench_function("rwlock", |b| {
        let l = RwLock::new(black_box(0u64));
        b.iter(|| {
            for _ in 0..ITERS {
                *l.write().unwrap() += black_box(1);
            }
            black_box(*l.read().unwrap())
        });
    });

    g.bench_function("melinoe", |b| {
        brand_scope(|mut token| {
            let cell = MelinoeCell::new(black_box(0u64));
            b.iter(|| {
                for _ in 0..ITERS {
                    *cell.borrow_mut(&mut token) += black_box(1);
                }
                black_box(*cell.borrow(&token))
            });
        });
    });

    g.finish();
}

fn bench_read(c: &mut Criterion) {
    let mut g = c.benchmark_group("read_1024x");
    g.throughput(Throughput::Elements(ITERS));

    g.bench_function("atomic_relaxed", |b| {
        let a = AtomicU64::new(black_box(7));
        b.iter(|| {
            let mut acc = 0u64;
            for _ in 0..ITERS {
                acc = acc.wrapping_add(a.load(Ordering::Relaxed));
            }
            black_box(acc)
        });
    });

    g.bench_function("mutex", |b| {
        let m = Mutex::new(black_box(7u64));
        b.iter(|| {
            let mut acc = 0u64;
            for _ in 0..ITERS {
                acc = acc.wrapping_add(*m.lock().unwrap());
            }
            black_box(acc)
        });
    });

    g.bench_function("rwlock", |b| {
        let l = RwLock::new(black_box(7u64));
        b.iter(|| {
            let mut acc = 0u64;
            for _ in 0..ITERS {
                acc = acc.wrapping_add(*l.read().unwrap());
            }
            black_box(acc)
        });
    });

    g.bench_function("melinoe", |b| {
        brand_scope(|token| {
            let cell = MelinoeCell::new(black_box(7u64));
            b.iter(|| {
                let mut acc = 0u64;
                for _ in 0..ITERS {
                    acc = acc.wrapping_add(*cell.borrow(&token));
                }
                black_box(acc)
            });
        });
    });

    g.finish();
}

/// Single-threaded interior mutability: Melinoe token access vs the std analogues
/// `RefCell` (runtime borrow counter) and `Cell` (plain). Melinoe carries no
/// runtime borrow flag — the exclusion is the compile-time token.
fn bench_interior_mut(c: &mut Criterion) {
    let mut g = c.benchmark_group("interior_mut_1024x");
    g.throughput(Throughput::Elements(ITERS));

    g.bench_function("melinoe", |b| {
        brand_scope(|mut token| {
            let cell = MelinoeCell::new(0u64);
            b.iter(|| {
                for _ in 0..ITERS {
                    *cell.borrow_mut(&mut token) += black_box(1);
                }
                black_box(*cell.borrow(&token))
            });
        });
    });

    g.bench_function("refcell", |b| {
        let cell = RefCell::new(0u64);
        b.iter(|| {
            for _ in 0..ITERS {
                *cell.borrow_mut() += black_box(1);
            }
            black_box(*cell.borrow())
        });
    });

    g.bench_function("cell", |b| {
        let cell = Cell::new(0u64);
        b.iter(|| {
            for _ in 0..ITERS {
                cell.set(cell.get().wrapping_add(black_box(1)));
            }
            black_box(cell.get())
        });
    });

    g.finish();
}

// Concurrent read throughput is measured rigorously in the dedicated
// `concurrent_reads` benchmark (thread-scaling, spawn amortised). A naive
// spawn-per-sample version here would measure thread-spawn overhead, not reads.

/// A deliberately non-trivial per-element kernel so the partitioned benchmark is
/// compute-bound (and thus actually parallelizable) rather than dominated by
/// memory bandwidth or thread-spawn overhead. A simple-store version would be
/// bandwidth-bound and uninformative about the write mechanism.
#[inline]
fn mix(mut x: u64) -> u64 {
    for _ in 0..16 {
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51_afd7_ed55_8ccd_u64.wrapping_add(x & 0xf));
        x ^= x >> 29;
    }
    x
}

fn bench_partitioned_writes(c: &mut Criterion) {
    const THREADS: usize = 4;
    const N: usize = 1 << 20; // ~1M elements, compute-bound via `mix`

    let mut g = c.benchmark_group("partitioned_compute_1m");
    g.throughput(Throughput::Elements(N as u64));

    // Single-threaded baseline — the work, with no parallelism and no sync.
    g.bench_function("single_thread", |b| {
        let mut data = vec![0u64; N];
        b.iter(|| {
            for (i, slot) in data.iter_mut().enumerate() {
                *slot = mix(i as u64);
            }
            black_box(data.len())
        });
    });

    // Melinoe: disjoint shards, one per thread, plain stores — no atomics, no locks.
    g.bench_function("melinoe_fixed_parts", |b| {
        brand_scope(|_token| {
            let mut cells: Vec<MelinoeCell<'_, u64>> =
                (0..N).map(|_| MelinoeCell::new(0)).collect();
            b.iter(|| {
                partition_for_each(&mut cells, THREADS, |start, mut shard| {
                    for (j, slot) in shard.iter_mut().enumerate() {
                        *slot = mix((start + j) as u64);
                    }
                });
            });
            black_box(cells.len());
        });
    });

    // Melinoe: use the process's reported hardware parallelism.
    g.bench_function("melinoe_available_parallelism", |b| {
        brand_scope(|_token| {
            let mut cells: Vec<MelinoeCell<'_, u64>> =
                (0..N).map(|_| MelinoeCell::new(0)).collect();
            b.iter(|| {
                partition_for_each_available(&mut cells, |start, mut shard| {
                    for (j, slot) in shard.iter_mut().enumerate() {
                        *slot = mix((start + j) as u64);
                    }
                });
            });
            black_box(cells.len());
        });
    });

    // Melinoe: fixed cache-sized chunks, independent of worker count.
    g.bench_function("melinoe_chunked", |b| {
        brand_scope(|_token| {
            let mut cells: Vec<MelinoeCell<'_, u64>> =
                (0..N).map(|_| MelinoeCell::new(0)).collect();
            b.iter(|| {
                partition_for_each_with(
                    &mut cells,
                    PartitionPlan::chunk_size(N / THREADS),
                    |start, mut shard| {
                        for (j, slot) in shard.iter_mut().enumerate() {
                            *slot = mix((start + j) as u64);
                        }
                    },
                );
            });
            black_box(cells.len());
        });
    });

    // Atomics: disjoint stores, lock-free but each store is an atomic op.
    g.bench_function("atomic_vec", |b| {
        let data: Vec<AtomicU64> = (0..N).map(|_| AtomicU64::new(0)).collect();
        b.iter(|| {
            thread::scope(|s| {
                for t in 0..THREADS {
                    let data = &data;
                    s.spawn(move || {
                        let chunk = N / THREADS;
                        let start = t * chunk;
                        for j in 0..chunk {
                            data[start + j].store(mix((start + j) as u64), Ordering::Relaxed);
                        }
                    });
                }
            });
        });
    });

    // Mutex: disjoint partitions, but the single lock held across the compute
    // serializes the writers — the idiomatic "share a Vec across threads" path,
    // which cannot express disjoint `&mut` without a lock.
    g.bench_function("mutex_vec", |b| {
        let data = Mutex::new(vec![0u64; N]);
        b.iter(|| {
            thread::scope(|s| {
                for t in 0..THREADS {
                    let data = &data;
                    s.spawn(move || {
                        let chunk = N / THREADS;
                        let start = t * chunk;
                        let mut guard = data.lock().unwrap();
                        for j in 0..chunk {
                            guard[start + j] = mix((start + j) as u64);
                        }
                    });
                }
            });
        });
    });

    g.finish();
}

fn bench_partition_driver(c: &mut Criterion) {
    let mut g = c.benchmark_group("partition_driver");

    g.bench_function("empty_region", |b| {
        brand_scope(|_token| {
            let mut cells: Vec<MelinoeCell<'_, u64>> = Vec::new();
            b.iter(|| {
                let results: Vec<usize> = partition_map(
                    black_box(cells.as_mut_slice()),
                    black_box(128),
                    |_start, shard| shard.len(),
                );
                black_box(results.len())
            });
        });
    });

    g.bench_function("overrequested_parts", |b| {
        brand_scope(|_token| {
            let mut cells: Vec<MelinoeCell<'_, u64>> =
                (0..8).map(|_| MelinoeCell::new(0)).collect();
            b.iter(|| {
                let results: Vec<usize> = partition_map(
                    black_box(cells.as_mut_slice()),
                    black_box(128),
                    |_start, shard| shard.len(),
                );
                black_box(results)
            });
        });
    });

    g.bench_function("available_parallelism", |b| {
        brand_scope(|_token| {
            let mut cells: Vec<MelinoeCell<'_, u64>> =
                (0..128).map(|_| MelinoeCell::new(0)).collect();
            b.iter(|| {
                let results: Vec<usize> =
                    partition_map_available(black_box(cells.as_mut_slice()), |_start, shard| {
                        shard.len()
                    });
                black_box(results)
            });
        });
    });

    g.bench_function("chunk_size_16", |b| {
        brand_scope(|_token| {
            let mut cells: Vec<MelinoeCell<'_, u64>> =
                (0..128).map(|_| MelinoeCell::new(0)).collect();
            b.iter(|| {
                let results: Vec<usize> = partition_map_with(
                    black_box(cells.as_mut_slice()),
                    PartitionPlan::chunk_size(16),
                    |_start, shard| shard.len(),
                );
                black_box(results)
            });
        });
    });

    g.finish();
}

criterion_group!(
    benches,
    bench_increment,
    bench_read,
    bench_interior_mut,
    bench_partitioned_writes,
    bench_partition_driver
);
criterion_main!(benches);
