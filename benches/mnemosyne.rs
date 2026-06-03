//! Mnemosyne-oriented benchmarks: access patterns a branded allocator actually
//! exercises, comparing per-cell token access against Melinoe's zero-copy slice
//! views, plus a `Cow` borrow-or-spill escape decision.
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

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
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

criterion_group!(benches, bench_slab_fill, bench_slab_scan, bench_cow_escape);
criterion_main!(benches);
