//! Criterion benchmarks for Melinoe-backed branded double-ended queues.
//!
//! This harness measures the production `BrandedVecDeque` read, wrapped-read,
//! and partition paths rather than benchmark-only adapters.
// Criterion macro-generated items are not public API; library docs remain
// denied by crate lint configuration.
#![allow(missing_docs)]

extern crate alloc;

use alloc::collections::VecDeque;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use melinoe::brand_scope;
use melinoe::collections::BrandedVecDeque;
#[cfg(feature = "std")]
use melinoe::sync::PartitionPlan;

fn wrapped_values() -> VecDeque<u64> {
    let mut values = VecDeque::with_capacity(4096);
    values.extend(0_u64..3072);
    for _ in 0..2048 {
        black_box(values.pop_front());
    }
    values.extend(3072_u64..6144);
    values
}

fn branded_contiguous_slice_sum(c: &mut Criterion) {
    c.bench_function("branded_deque/contiguous_slice_sum_4096", |b| {
        b.iter(|| {
            brand_scope(|token| {
                let values = BrandedVecDeque::from_iter(0_u64..4096);
                let (front, back) = values.as_slices(&token);
                let sum = front
                    .iter()
                    .chain(back.iter())
                    .copied()
                    .fold(0_u64, u64::wrapping_add);
                black_box(sum)
            })
        });
    });
}

fn branded_wrapped_slice_sum(c: &mut Criterion) {
    c.bench_function("branded_deque/wrapped_slice_sum_4096", |b| {
        b.iter(|| {
            brand_scope(|token| {
                let values = BrandedVecDeque::from(wrapped_values());
                let (front, back) = values.as_slices(&token);
                let sum = front
                    .iter()
                    .chain(back.iter())
                    .copied()
                    .fold(0_u64, u64::wrapping_add);
                black_box(sum)
            })
        });
    });
}

#[cfg(feature = "std")]
fn branded_partition_fill(c: &mut Criterion) {
    c.bench_function("branded_deque/partition_fill_4096", |b| {
        b.iter(|| {
            brand_scope(|token| {
                let mut values = BrandedVecDeque::from(wrapped_values());
                values.partition_for_each_mut_with(
                    PartitionPlan::chunk_size(512),
                    |start, shard| {
                        for (offset, value) in shard.iter_mut().enumerate() {
                            *value = (start + offset) as u64;
                        }
                    },
                );
                let (front, back) = values.as_slices(&token);
                black_box(front.last().copied().xor(back.last().copied()))
            })
        });
    });
}

#[cfg(feature = "std")]
fn branded_partition_sum(c: &mut Criterion) {
    c.bench_function("branded_deque/partition_sum_4096", |b| {
        b.iter(|| {
            brand_scope(|token| {
                let values = BrandedVecDeque::from(wrapped_values());
                let sums = values.partition_map_with(
                    &token,
                    PartitionPlan::chunk_size(512),
                    |_start, shard| shard.iter().copied().fold(0_u64, u64::wrapping_add),
                );
                black_box(sums.into_iter().fold(0_u64, u64::wrapping_add))
            })
        });
    });
}

#[cfg(feature = "std")]
criterion_group!(
    benches,
    branded_contiguous_slice_sum,
    branded_wrapped_slice_sum,
    branded_partition_fill,
    branded_partition_sum
);
#[cfg(not(feature = "std"))]
criterion_group!(
    benches,
    branded_contiguous_slice_sum,
    branded_wrapped_slice_sum
);
criterion_main!(benches);
