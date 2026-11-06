//! Partition a large zero-sized region without allocating element storage.

#![cfg(feature = "std")]

use melinoe::sync::{partition_map_with, partition_read_map_with, PartitionPlan};
use melinoe::MelinoeCell;

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
