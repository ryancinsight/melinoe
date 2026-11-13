//! Partition a large zero-sized region without allocating element storage.

#![cfg(feature = "std")]

use std::collections::VecDeque;

use melinoe::collections::BrandedVecDeque;
use melinoe::sync::{partition_map_with, partition_read_map_with, PartitionPlan};
use melinoe::{brand_scope, MelinoeCell};

#[test]
fn read_partitions_clamp_the_final_zero_sized_chunk() {
    let values = vec![(); usize::MAX];
    let chunk = usize::MAX / 2 + 1;
    let result =
        partition_read_map_with(&values, PartitionPlan::chunk_size(chunk), |start, shard| {
            (start, shard.len())
        });

    assert_eq!(result, [(0, chunk), (chunk, usize::MAX - chunk)]);
}

#[test]
fn write_partitions_clamp_the_final_zero_sized_chunk() {
    let mut values = vec![(); usize::MAX];
    let cells = MelinoeCell::from_mut_slice(&mut values);
    let chunk = usize::MAX / 2 + 1;
    let result = partition_map_with(cells, PartitionPlan::chunk_size(chunk), |start, shard| {
        (start, shard.len())
    });

    assert_eq!(result, [(0, chunk), (chunk, usize::MAX - chunk)]);
}

/// A `usize::MAX`-long zero-sized deque is held in one storage slice, so the
/// logical plan is the two halves `[0, MAX / 2 + 1)` and `[MAX / 2 + 1, MAX)`.
const EXPECTED_DEQUE_SHARDS: [(usize, usize); 2] = [
    (0, usize::MAX / 2 + 1),
    (usize::MAX / 2 + 1, usize::MAX - (usize::MAX / 2 + 1)),
];

#[test]
fn deque_read_partitions_clamp_the_final_zero_sized_chunk() {
    for plan in [
        PartitionPlan::chunk_size(usize::MAX / 2 + 1),
        PartitionPlan::parts(2),
    ] {
        brand_scope(|token| {
            let deque = BrandedVecDeque::from(VecDeque::from(vec![(); usize::MAX]));
            let result =
                deque.partition_map_with(&token, plan, |start, shard| (start, shard.len()));

            assert_eq!(result, EXPECTED_DEQUE_SHARDS);
        });
    }
}

#[test]
fn deque_write_partitions_clamp_the_final_zero_sized_chunk() {
    for plan in [
        PartitionPlan::chunk_size(usize::MAX / 2 + 1),
        PartitionPlan::parts(2),
    ] {
        let mut deque = BrandedVecDeque::from(VecDeque::from(vec![(); usize::MAX]));
        let result = deque.partition_map_mut_with(plan, |start, shard| (start, shard.len()));

        assert_eq!(result, EXPECTED_DEQUE_SHARDS);
    }
}
