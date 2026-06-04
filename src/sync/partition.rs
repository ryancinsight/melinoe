//! `std`-gated drivers that run one [`WriterShard`] per thread concurrently.

use core::num::NonZeroUsize;
use std::vec::Vec;

use crate::cell::MelinoeCell;
use crate::region::WriterShard;

/// Shard sizing policy for partitioned scoped-thread execution.
///
/// The plan controls only how a region is tiled into non-empty
/// [`WriterShard`]s. It does not introduce locks, atomics, queues, or worker
/// pools; each shard is still moved into one `std::thread::scope` worker and
/// joined before the call returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartitionPlan {
    /// Split into at most this many non-empty shards.
    Parts(NonZeroUsize),
    /// Split into at most `std::thread::available_parallelism()` non-empty
    /// shards, falling back to one shard if the platform cannot report it.
    AvailableParallelism,
    /// Split into non-empty shards containing at most this many cells.
    ChunkSize(NonZeroUsize),
}

impl PartitionPlan {
    /// Create a fixed-part plan, clamping zero to one.
    #[inline]
    #[must_use]
    pub fn parts(parts: usize) -> Self {
        Self::Parts(nonzero_or_one(parts))
    }

    /// Create a plan based on the process's reported hardware parallelism.
    #[inline]
    #[must_use]
    pub const fn available_parallelism() -> Self {
        Self::AvailableParallelism
    }

    /// Create a fixed-chunk-size plan, clamping zero to one.
    #[inline]
    #[must_use]
    pub fn chunk_size(chunk_size: usize) -> Self {
        Self::ChunkSize(nonzero_or_one(chunk_size))
    }

    #[inline]
    fn resolve(self, len: usize) -> ResolvedPartitionPlan {
        let chunk = match self {
            Self::Parts(parts) => chunk_for_parts(len, parts.get()),
            Self::AvailableParallelism => {
                let parts = std::thread::available_parallelism().map_or(1, NonZeroUsize::get);
                chunk_for_parts(len, parts)
            }
            Self::ChunkSize(chunk_size) => chunk_size.get(),
        };
        let shard_count = shard_count(len, chunk);
        ResolvedPartitionPlan { chunk, shard_count }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ResolvedPartitionPlan {
    chunk: usize,
    shard_count: usize,
}

#[inline]
fn nonzero_or_one(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| NonZeroUsize::new(1).expect("1 is non-zero"))
}

#[inline]
fn chunk_for_parts(len: usize, parts: usize) -> usize {
    if len == 0 {
        1
    } else {
        // ceil(len / parts), clamped to >= 1. Written as
        // `1 + (len - 1) / parts` because `usize::div_ceil` is not in the MSRV
        // and `len + parts - 1` can overflow for adversarial inputs.
        1 + (len - 1) / parts
    }
}

#[inline]
fn shard_count(len: usize, chunk: usize) -> usize {
    if len == 0 {
        0
    } else {
        1 + (len - 1) / chunk
    }
}

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
    let ResolvedPartitionPlan { chunk, shard_count } = plan.resolve(cells.len());
    std::thread::scope(|scope| {
        let f = &f;
        let mut handles = Vec::with_capacity(shard_count);
        let mut start = 0usize;
        for shard in WriterShard::new(cells).chunks(chunk) {
            let shard_start = start;
            start += shard.len();
            handles.push(scope.spawn(move || f(shard_start, shard)));
        }
        handles
            .into_iter()
            .map(|h| match h.join() {
                Ok(value) => value,
                Err(payload) => std::panic::resume_unwind(payload),
            })
            .collect()
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
