//! Mutable-shard partition drivers: split a branded region into disjoint
//! [`WriterShard`]s and run a closure on each concurrently.
//!
//! Each driver reduces to one call into [`driver_core::drive`]; the per-index
//! shard construction is expressed through [`WriterShard::par_chunks`], the
//! crate's single authoritative home for the disjoint sub-slice range math (so
//! this module never hand-rolls `from_raw_parts_mut`).

use std::vec::Vec;

use crate::cell::MelinoeCell;
use crate::region::WriterShard;

use super::driver_core::drive;
use super::plan::PartitionPlan;

/// Split `cells` into `parts` disjoint shards and run `f` on each concurrently,
/// returning the per-shard results in partition order.
///
/// Each invocation of `f` receives the global start index of its partition (the
/// offset of the shard's first cell within `cells`) and the [`WriterShard`]
/// itself. Because the shards are non-overlapping, the writes proceed in
/// parallel with no atomics and no locks; the only synchronization is the
/// thread join at the end of the scope.
///
/// `parts` is clamped to at least `1`. The number of shards is
/// `min(parts, cells.len())` (no empty shards are produced).
///
/// # Panics
///
/// Propagates (re-raises) any panic that unwinds out of `f` on a worker thread.
///
/// # Examples
///
/// ```
/// use melinoe::sync::partition_map;
/// use melinoe::{brand_scope, MelinoeCell};
///
/// brand_scope(|token| {
///     let mut cells: Vec<MelinoeCell<'_, usize>> =
///         (0..8).map(|_| MelinoeCell::new(0)).collect();
///
///     // Four threads each fill their disjoint partition with global indices.
///     let written: Vec<usize> = partition_map(&mut cells, 4, |start, mut shard| {
///         for (j, slot) in shard.iter_mut().enumerate() {
///             *slot = start + j;
///         }
///         shard.len()
///     });
///     assert_eq!(written.iter().sum::<usize>(), 8);
///
///     // Read the whole region back via the token: every cell holds its index.
///     let snap = token.share();
///     for (k, c) in cells.iter().enumerate() {
///         assert_eq!(*c.borrow(snap), k);
///     }
/// });
/// ```
pub fn partition_map<'brand, T, R, F>(
    cells: &mut [MelinoeCell<'brand, T>],
    parts: usize,
    f: F,
) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(usize, WriterShard<'_, 'brand, T>) -> R + Sync,
{
    partition_map_with(cells, PartitionPlan::parts(parts), f)
}

/// Split `cells` according to `plan` and run `f` on each disjoint shard
/// concurrently, returning per-shard results in partition order.
///
/// Use [`PartitionPlan::available_parallelism`] when the caller wants the
/// current process's reported hardware parallelism, or
/// [`PartitionPlan::chunk_size`] when cache/NUMA tiling is more important than
/// a fixed worker count.
///
/// # Panics
///
/// Propagates (re-raises) any panic that unwinds out of `f` on a worker thread.
#[inline]
pub fn partition_map_with<'brand, T, R, F>(
    cells: &mut [MelinoeCell<'brand, T>],
    plan: PartitionPlan,
    f: F,
) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(usize, WriterShard<'_, 'brand, T>) -> R + Sync,
{
    let chunk = plan.resolve(cells.len());
    let chunk_size = chunk.max(1);
    let par = WriterShard::new(cells).par_chunks(chunk_size);
    let num_chunks = par.len();
    if num_chunks == 0 {
        return Vec::new();
    }

    drive(num_chunks, |index| {
        let start = index * chunk_size;
        // SAFETY: `drive` invokes each `index` in `0..num_chunks` at most once
        // (both the registered-executor and scoped-thread paths uphold this), and
        // `num_chunks == par.len()`, so every `index` is in range and requested
        // exactly once — the disjointness and single-use contract of
        // `get_unchecked_chunk`. The shard's `'a` borrow is the exclusive borrow
        // of `cells` captured by `par`, kept alive across the whole `drive` call.
        let shard = unsafe { par.get_unchecked_chunk(index) };
        f(start, shard)
    })
}

/// Split `cells` using the process's reported hardware parallelism and run `f`
/// on each disjoint shard concurrently.
///
/// Equivalent to `partition_map_with(cells,
/// PartitionPlan::available_parallelism(), f)`.
#[inline]
pub fn partition_map_available<'brand, T, R, F>(
    cells: &mut [MelinoeCell<'brand, T>],
    f: F,
) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(usize, WriterShard<'_, 'brand, T>) -> R + Sync,
{
    partition_map_with(cells, PartitionPlan::available_parallelism(), f)
}

/// Split `cells` into `parts` disjoint shards and run `f` on each concurrently,
/// discarding results.
///
/// Convenience wrapper over [`partition_map`] for the common write-only case.
///
/// # Panics
///
/// Propagates any panic from a worker thread, as [`partition_map`].
#[inline]
pub fn partition_for_each<'brand, T, F>(cells: &mut [MelinoeCell<'brand, T>], parts: usize, f: F)
where
    T: Send,
    F: Fn(usize, WriterShard<'_, 'brand, T>) + Sync,
{
    partition_map(cells, parts, f);
}

/// Split `cells` according to `plan` and run `f` on each disjoint shard
/// concurrently, discarding results.
///
/// # Panics
///
/// Propagates any panic from a worker thread, as [`partition_map_with`].
#[inline]
pub fn partition_for_each_with<'brand, T, F>(
    cells: &mut [MelinoeCell<'brand, T>],
    plan: PartitionPlan,
    f: F,
) where
    T: Send,
    F: Fn(usize, WriterShard<'_, 'brand, T>) + Sync,
{
    partition_map_with(cells, plan, f);
}

/// Split `cells` using the process's reported hardware parallelism and run `f`
/// on each disjoint shard concurrently, discarding results.
#[inline]
pub fn partition_for_each_available<'brand, T, F>(cells: &mut [MelinoeCell<'brand, T>], f: F)
where
    T: Send,
    F: Fn(usize, WriterShard<'_, 'brand, T>) + Sync,
{
    partition_map_available(cells, f);
}
