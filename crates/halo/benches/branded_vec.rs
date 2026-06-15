//! Criterion benchmarks for Melinoe-backed Halo vectors.
//!
//! This harness measures the production `BrandedVec::as_slice` read path rather
//! than a benchmark-specific adapter.
// Criterion macro-generated items are not public API; library docs remain
// denied by crate lint configuration.
#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use halo::BrandedVec;
use melinoe::brand_scope;

fn branded_slice_sum(c: &mut Criterion) {
    c.bench_function("halo_branded_vec/slice_sum_4096", |b| {
        b.iter(|| {
            brand_scope(|token| {
                let values = BrandedVec::from_iter(0_u64..4096);
                let sum = values
                    .as_slice(&token)
                    .iter()
                    .copied()
                    .fold(0_u64, u64::wrapping_add);
                black_box(sum)
            })
        });
    });
}

criterion_group!(benches, branded_slice_sum);
criterion_main!(benches);
