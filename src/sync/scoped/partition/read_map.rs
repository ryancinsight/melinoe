//! Shared-slice partition drivers: split a plain `&[T]` into disjoint shared
//! sub-slices and run a closure on each concurrently.
//!
//! The read-only counterpart to [`super::map`]. It shares the same engine
//! ([`driver_core::drive`]); the only difference is the per-index task builds a
//! shared `&[T]` sub-slice instead of a mutable [`WriterShard`].

use std::vec::Vec;

use super::driver_core::drive;
use super::plan::PartitionPlan;

/// Split `slice` into `parts` disjoint shared shards and run `f` on each concurrently,
/// returning the per-shard results in partition order.
pub fn partition_read_map<T, R, F>(slice: &[T], parts: usize, f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(usize, &[T]) -> R + Sync,
{
    partition_read_map_with(slice, PartitionPlan::parts(parts), f)
}

/// Split `slice` according to `plan` and run `f` on each disjoint shared shard
/// concurrently, returning per-shard results in partition order.
///
/// If a custom parallel executor is registered, it will be used instead of
/// spawning raw OS threads.
#[inline]
pub fn partition_read_map_with<T, R, F>(slice: &[T], plan: PartitionPlan, f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(usize, &[T]) -> R + Sync,
{
    let chunk = plan.resolve(slice.len());
    let chunk_size = chunk.max(1);
    let slice_len = slice.len();
    let num_chunks = if slice_len == 0 {
        0
    } else {
        1 + (slice_len - 1) / chunk_size
    };
    if num_chunks == 0 {
        return Vec::new();
    }

    drive(num_chunks, |index| {
        let start = index * chunk_size;
        let end = (start + chunk_size).min(slice_len);
        // `drive` runs each `index` in `0..num_chunks` at most once, and
        // `num_chunks = ceil(slice_len / chunk_size)`, so `start < slice_len` and
        // the ranges tile `0..slice_len` disjointly. A shared `&[T]` sub-slice
        // needs no unsafe: ordinary slicing yields the disjoint view directly.
        f(start, &slice[start..end])
    })
}

/// Split `slice` using the process's reported hardware parallelism and run `f`
/// on each disjoint shared shard concurrently, returning the per-shard results.
#[inline]
pub fn partition_read_map_available<T, R, F>(slice: &[T], f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(usize, &[T]) -> R + Sync,
{
    partition_read_map_with(slice, PartitionPlan::available_parallelism(), f)
}

/// Split `slice` into `parts` disjoint shared shards and run `f` on each concurrently,
/// discarding results.
#[inline]
pub fn partition_read_for_each<T, F>(slice: &[T], parts: usize, f: F)
where
    T: Sync,
    F: Fn(usize, &[T]) + Sync,
{
    partition_read_map(slice, parts, f);
}

/// Split `slice` according to `plan` and run `f` on each disjoint shared shard
/// concurrently, discarding results.
#[inline]
pub fn partition_read_for_each_with<T, F>(slice: &[T], plan: PartitionPlan, f: F)
where
    T: Sync,
    F: Fn(usize, &[T]) + Sync,
{
    partition_read_map_with(slice, plan, f);
}

/// Split `slice` using the process's reported hardware parallelism and run `f`
/// on each disjoint shared shard concurrently, discarding results.
#[inline]
pub fn partition_read_for_each_available<T, F>(slice: &[T], f: F)
where
    T: Sync,
    F: Fn(usize, &[T]) + Sync,
{
    partition_read_map_available(slice, f);
}
