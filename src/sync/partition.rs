//! `std`-gated drivers that run one [`WriterShard`] per thread concurrently.

use std::vec::Vec;

use crate::cell::MelinoeCell;
use crate::region::WriterShard;

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
    let parts = parts.max(1);
    let len = cells.len();
    // ceil(len / parts), clamped to >= 1; `div_ceil` is not in our MSRV.
    let chunk = if len == 0 {
        1
    } else {
        (len + parts - 1) / parts
    };

    std::thread::scope(|scope| {
        let f = &f;
        let mut handles = Vec::with_capacity(parts);
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
