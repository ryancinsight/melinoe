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
#[cfg(feature = "std")]
use melinoe::sync::PartitionPlan;

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

#[cfg(feature = "std")]
fn branded_partition_fill(c: &mut Criterion) {
    c.bench_function("halo_branded_vec/partition_fill_4096", |b| {
        b.iter(|| {
            brand_scope(|token| {
                let mut values = BrandedVec::from_iter(0_u64..4096);
                values.partition_for_each_mut_with(
                    PartitionPlan::chunk_size(512),
                    |start, shard| {
                        for (offset, value) in shard.iter_mut().enumerate() {
                            *value = (start + offset) as u64;
                        }
                    },
                );
                black_box(values.as_slice(&token)[4095])
            })
        });
    });
}

#[cfg(feature = "std")]
criterion_group!(benches, branded_slice_sum, branded_partition_fill);
#[cfg(not(feature = "std"))]
criterion_group!(benches, branded_slice_sum);
criterion_main!(benches);
