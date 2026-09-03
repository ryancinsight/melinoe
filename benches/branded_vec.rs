//! Criterion benchmarks for Melinoe-backed branded vectors.
//!
//! This harness measures the production `BrandedVec::as_slice` read path rather
//! than a benchmark-specific adapter.
// Criterion macro-generated items are not public API; library docs remain
// denied by crate lint configuration.
#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use melinoe::brand_scope;
use melinoe::collections::BrandedVec;
#[cfg(feature = "std")]
use melinoe::sync::PartitionPlan;

fn branded_slice_sum(c: &mut Criterion) {
    c.bench_function("branded_vec/slice_sum_4096", |b| {
        brand_scope(|token| {
            let start = black_box(0_u64);
            let values = (0_u64..)
                .take(4096)
                .map(|offset| start.wrapping_add(offset))
                .collect::<BrandedVec<_>>();
            b.iter(|| {
                let sum = black_box(values.as_slice(&token))
                    .iter()
                    .copied()
                    .fold(0_u64, u64::wrapping_add);
                black_box(sum)
            });
        });
    });
}

#[cfg(feature = "std")]
fn branded_partition_fill(c: &mut Criterion) {
    c.bench_function("branded_vec/partition_fill_4096", |b| {
        brand_scope(|token| {
            b.iter_batched(
                || {
                    let start = black_box(0_u64);
                    (0_u64..)
                        .take(4096)
                        .map(|offset| start.wrapping_add(offset))
                        .collect::<BrandedVec<_>>()
                },
                |mut values| {
                    values.partition_for_each_mut_with(
                        PartitionPlan::chunk_size(512),
                        |start, shard| {
                            for (offset, value) in shard.iter_mut().enumerate() {
                                let offset =
                                    u64::try_from(offset).expect("benchmark offset fits in u64");
                                *value = u64::try_from(start)
                                    .expect("benchmark shard start fits in u64")
                                    .wrapping_add(offset);
                            }
                        },
                    );
                    let slice = black_box(values.as_slice(&token));
                    black_box(
                        slice
                            .last()
                            .copied()
                            .expect("benchmark vector is non-empty"),
                    )
                },
                BatchSize::SmallInput,
            );
        });
    });
}

#[cfg(feature = "std")]
fn branded_partition_sum(c: &mut Criterion) {
    c.bench_function("branded_vec/partition_sum_4096", |b| {
        brand_scope(|token| {
            b.iter_batched(
                || {
                    let start = black_box(0_u64);
                    (0_u64..)
                        .take(4096)
                        .map(|offset| start.wrapping_add(offset))
                        .collect::<BrandedVec<_>>()
                },
                |values| {
                    let sums = values.partition_map_with(
                        &token,
                        PartitionPlan::chunk_size(512),
                        |_start, shard| shard.iter().copied().fold(0_u64, u64::wrapping_add),
                    );
                    black_box(sums.into_iter().fold(0_u64, u64::wrapping_add))
                },
                BatchSize::SmallInput,
            );
        });
    });
}

#[cfg(feature = "std")]
criterion_group!(
    benches,
    branded_slice_sum,
    branded_partition_fill,
    branded_partition_sum
);
#[cfg(not(feature = "std"))]
criterion_group!(benches, branded_slice_sum);
criterion_main!(benches);
