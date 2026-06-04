//! Mnemosyne-oriented benchmarks: access patterns a branded allocator actually
//! exercises — per-cell token access vs Melinoe's zero-copy slice views, a `Cow`
//! borrow-or-spill escape decision, and ambient guarded interior mutability
//! (`GuardedCell` vs `RefCell` vs the raw `is_allocating` idiom).
//!
//! Mnemosyne brands heap blocks with an invariant `'brand` and mediates access
//! through an `AllocatorToken<'brand>` (a single `!Send` token), reading/writing
//! one `BrandedCell` at a time. Melinoe adds zero-copy *region* views: present a
//! permit once and operate on the whole slab as a native `&[T]` / `&mut [T]`
//! (`CellSliceExt`), which an allocator uses to bulk-initialise or scan slabs.
//!
//! `Cow` is deliberately absent from Melinoe's zero-cost access core—borrowing a
//! branded slice is always zero-copy there, so a `Cow` would be a degenerate
//! always-`Borrowed`. It earns its place only at the *ownership boundary*, where
//! a slab buffer is usually consumed transiently (borrow) but occasionally must
//! outlive the brand scope (own); `bench_cow_escape` measures exactly that.

#![allow(missing_docs)]

use std::borrow::Cow;
use std::cell::{Cell, RefCell, UnsafeCell};
use std::sync::atomic::{AtomicU64, Ordering};

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use melinoe::atomic::BrandedAtomic;
use melinoe::reentrant::GuardedCell;
use melinoe::{brand_scope, CellSliceExt, MelinoeCell};

/// Bulk slab initialisation: write every block. The slice view lowers to a
/// vectorised fill; the per-cell path issues a token-mediated store per block.
fn bench_slab_fill(c: &mut Criterion) {
    const N: usize = 1 << 16;
    let mut g = c.benchmark_group("slab_fill_64k");
    g.throughput(Throughput::Elements(N as u64));

    g.bench_function("per_cell_token", |b| {
        brand_scope(|mut token| {
            let cells: Vec<MelinoeCell<'_, u64>> = (0..N).map(|_| MelinoeCell::new(0)).collect();
            b.iter(|| {
                let v = black_box(0x5151_5151_5151_5151u64);
                for c in &cells {
                    *c.borrow_mut(&mut token) = v;
                }
            });
            black_box(cells.len());
        });
    });

    g.bench_function("slice_view_fill", |b| {
        brand_scope(|mut token| {
            let cells: Vec<MelinoeCell<'_, u64>> = (0..N).map(|_| MelinoeCell::new(0)).collect();
            b.iter(|| {
                cells
                    .borrow_slice_mut(&mut token)
                    .fill(black_box(0x5151_5151_5151_5151u64));
            });
            black_box(cells.len());
        });
    });

    g.finish();
}

/// Bulk slab scan: sum every block. The slice view exposes a contiguous `&[u64]`
/// for an autovectorised reduction; the per-cell path reads via a shared token.
fn bench_slab_scan(c: &mut Criterion) {
    const N: usize = 1 << 16;
    let mut g = c.benchmark_group("slab_scan_64k");
    g.throughput(Throughput::Elements(N as u64));

    g.bench_function("per_cell_token", |b| {
        brand_scope(|token| {
            let cells: Vec<MelinoeCell<'_, u64>> = (0..N as u64).map(MelinoeCell::new).collect();
            let snap = token.share();
            b.iter(|| {
                let mut acc = 0u64;
                for c in &cells {
                    acc = acc.wrapping_add(*c.borrow(snap));
                }
                black_box(acc)
            });
        });
    });

    g.bench_function("slice_view_sum", |b| {
        brand_scope(|token| {
            let cells: Vec<MelinoeCell<'_, u64>> = (0..N as u64).map(MelinoeCell::new).collect();
            b.iter(|| black_box(cells.borrow_slice(&token).iter().copied().sum::<u64>()));
        });
    });

    g.finish();
}

/// Ownership boundary: a slab buffer handed to a consumer that *usually* uses it
/// transiently (borrow, zero-copy) but occasionally must retain it past the brand
/// scope (own, must clone). `Cow` pays the clone only on the retain path.
fn bench_cow_escape(c: &mut Criterion) {
    const N: usize = 1 << 12;
    let mut g = c.benchmark_group("cow_escape_4k");
    g.throughput(Throughput::Elements(N as u64));

    // Baseline: no `Cow` — always materialise an owned copy for the consumer.
    g.bench_function("always_owned", |b| {
        brand_scope(|token| {
            let cells: Vec<MelinoeCell<'_, u8>> =
                (0..N).map(|i| MelinoeCell::new(i as u8)).collect();
            b.iter(|| {
                let owned: Vec<u8> = cells.borrow_slice(&token).to_vec();
                black_box(owned.iter().fold(0u8, |a, x| a.wrapping_add(*x)))
            });
        });
    });

    // `Cow`: borrow on the common transient path, clone only when the buffer
    // must escape (here 1 in 8 calls, decided at runtime).
    g.bench_function("cow_borrow_mostly", |b| {
        brand_scope(|token| {
            let cells: Vec<MelinoeCell<'_, u8>> =
                (0..N).map(|i| MelinoeCell::new(i as u8)).collect();
            let mut tick = 0u32;
            b.iter(|| {
                tick = tick.wrapping_add(1);
                let must_escape = black_box(tick) % 8 == 0;
                let buf: Cow<'_, [u8]> = if must_escape {
                    Cow::Owned(cells.borrow_slice(&token).to_vec())
                } else {
                    Cow::Borrowed(cells.borrow_slice(&token))
                };
                black_box(buf.iter().fold(0u8, |a, x| a.wrapping_add(*x)))
            });
        });
    });

    g.finish();
}

/// The hand-rolled `UnsafeCell<T>` + `is_allocating: bool` idiom that
/// [`GuardedCell`] replaces — re-entrancy-checked, but *not* panic-safe (no drop
/// guard, so a panicking `f` would leave the flag stuck).
struct RawSlot<T> {
    value: UnsafeCell<T>,
    active: Cell<bool>,
}

impl<T> RawSlot<T> {
    fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
            active: Cell::new(false),
        }
    }

    #[inline]
    fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        if self.active.get() {
            return None;
        }
        self.active.set(true);
        // SAFETY: flag rejects re-entry; single-threaded bench.
        let r = f(unsafe { &mut *self.value.get() });
        self.active.set(false);
        Some(r)
    }
}

/// Ambient guarded interior mutability (the per-thread allocator-slot access
/// pattern): cost of one re-entrancy-checked `&mut` borrow + mutation.
/// `GuardedCell` vs `RefCell` vs the raw idiom it supersedes.
fn bench_guarded_access(c: &mut Criterion) {
    const ITERS: u64 = 4096;
    let mut g = c.benchmark_group("guarded_access_4096x");
    g.throughput(Throughput::Elements(ITERS));

    g.bench_function("guardedcell", |b| {
        let cell = GuardedCell::new(0u64);
        b.iter(|| {
            for _ in 0..ITERS {
                cell.enter(|v| *v = v.wrapping_add(black_box(1))).unwrap();
            }
            black_box(cell.enter(|v| *v).unwrap())
        });
    });

    g.bench_function("refcell", |b| {
        let cell = RefCell::new(0u64);
        b.iter(|| {
            for _ in 0..ITERS {
                let mut guard = cell.borrow_mut();
                *guard = guard.wrapping_add(black_box(1));
            }
            black_box(*cell.borrow())
        });
    });

    g.bench_function("raw_unsafecell_bool", |b| {
        let cell = RawSlot::new(0u64);
        b.iter(|| {
            for _ in 0..ITERS {
                cell.with(|v| *v = v.wrapping_add(black_box(1))).unwrap();
            }
            black_box(cell.with(|v| *v).unwrap())
        });
    });

    g.finish();
}

/// Conditional atomics: a counter bumped in a single-writer (exclusive) phase.
/// `BrandedAtomic` does this with plain stores; a plain `AtomicU64` pays a locked
/// RMW on every bump even though no other thread touches it yet.
fn bench_conditional_atomic(c: &mut Criterion) {
    const ITERS: u64 = 4096;
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

criterion_group!(
    benches,
    bench_slab_fill,
    bench_slab_scan,
    bench_cow_escape,
    bench_guarded_access,
    bench_conditional_atomic
);
criterion_main!(benches);
