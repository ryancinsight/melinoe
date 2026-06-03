//! False sharing and memory efficiency on the concurrent disjoint-write path.
//!
//! Pattern: `THREADS` threads each accumulate into their *own* counter and the
//! results are read after the join (e.g. per-thread allocator statistics).
//!
//! The finding: because Melinoe proves *single-writer* access at compile time,
//! the compiler keeps each branded counter in a register across the loop and
//! writes back once — the shared cache line is never touched mid-loop, so there
//! is **no false sharing and no need for cache-line padding**. This matches a
//! raw `split_at_mut` baseline exactly (zero-cost) and stays dense (8 B/counter).
//!
//! An `AtomicU64`, by contrast, cannot be register-promoted: every `fetch_add`
//! is a real memory RMW, so adjacent atomic counters bounce their shared line
//! (false sharing) and must be padded to a full cache line (128 B/counter, 16×
//! the memory) to recover throughput. Melinoe gets both speed and density for
//! free from the single-writer guarantee.

#![allow(missing_docs)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use melinoe::sync::partition_for_each;
use melinoe::MelinoeCell;

const THREADS: usize = 8;
const ITERS: u64 = 1 << 20;

/// `AtomicU64` forced onto its own cache line (128 B covers next-line prefetch).
#[repr(align(128))]
struct PadAtomic(AtomicU64);

fn bench(c: &mut Criterion) {
    let mut g = c.benchmark_group("disjoint_counters_8threads");
    g.throughput(Throughput::Elements(THREADS as u64 * ITERS));

    // Raw `&mut [u64]` chunks via `split_at_mut` — the zero-cost reference point.
    g.bench_function("raw_split_mut", |b| {
        let mut data = [0u64; THREADS];
        b.iter(|| {
            thread::scope(|s| {
                let mut rest = &mut data[..];
                for _ in 0..THREADS {
                    let (head, tail) = rest.split_at_mut(1);
                    rest = tail;
                    s.spawn(move || {
                        let slot = &mut head[0];
                        for _ in 0..ITERS {
                            *slot = slot.wrapping_add(black_box(1));
                        }
                    });
                }
            });
        });
        black_box(data.len());
    });

    // Melinoe disjoint shards — should equal the raw baseline (zero-cost), dense.
    g.bench_function("melinoe_shards_8B", |b| {
        let mut cells: Vec<MelinoeCell<'static, u64>> =
            (0..THREADS).map(|_| MelinoeCell::new(0)).collect();
        b.iter(|| {
            partition_for_each(&mut cells, THREADS, |_, mut shard| {
                let slot = &mut shard.as_mut_slice()[0];
                for _ in 0..ITERS {
                    *slot = slot.wrapping_add(black_box(1));
                }
            });
        });
        black_box(cells.len());
    });

    // Adjacent atomics — every `fetch_add` hits the shared line: false sharing.
    g.bench_function("atomic_adjacent_8B", |b| {
        let data: Vec<AtomicU64> = (0..THREADS).map(|_| AtomicU64::new(0)).collect();
        let data = &data;
        b.iter(|| {
            thread::scope(|s| {
                for counter in data.iter() {
                    s.spawn(move || {
                        for _ in 0..ITERS {
                            counter.fetch_add(black_box(1), Ordering::Relaxed);
                        }
                    });
                }
            });
        });
    });

    // Padded atomics — own cache line each: no false sharing, but 16× the memory.
    g.bench_function("atomic_padded_128B", |b| {
        let data: Vec<PadAtomic> = (0..THREADS).map(|_| PadAtomic(AtomicU64::new(0))).collect();
        let data = &data;
        b.iter(|| {
            thread::scope(|s| {
                for counter in data.iter() {
                    s.spawn(move || {
                        for _ in 0..ITERS {
                            counter.0.fetch_add(black_box(1), Ordering::Relaxed);
                        }
                    });
                }
            });
        });
    });

    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
