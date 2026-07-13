use core::marker::PhantomData;

use crate::cell::MelinoeCell;

use super::WriterShard;

/// Indexed, random-access view over the disjoint `chunk_size`-cell partitions of
/// a branded region — the *indexed* counterpart to the sequential
/// [`ShardChunks`](super::ShardChunks) lending iterator.
///
/// [`ShardChunks`](super::ShardChunks) yields shards front-to-back, each `next()` reborrowing the
/// remainder, so a consumer must thread `&mut` state through the sequence. A
/// work-stealing pool instead wants "give me partition `c` on demand" from many
/// worker threads at once. `ParChunks` provides exactly that: it holds the base
/// pointer, length, and chunk size of the region, and
/// [`get_unchecked_chunk`](Self::get_unchecked_chunk) computes the disjoint
/// sub-shard for an index without any sequential threading.
///
/// This is the single authoritative home for the `from_raw_parts_mut` range
/// math that a parallel driver needs. It is consumed by Melinoe's own
/// `std`-gated partition driver and is intended for external work-stealing
/// executors such as `moirai-parallel`, which would otherwise re-derive the same
/// unsafe sub-slicing.
///
/// The number of partitions is [`len`](Self::len) `= ceil(region_len /
/// chunk_size)` (`0` for an empty region). Each returned [`WriterShard`] carries
/// the region's `'a` borrow and `'brand`, so it obeys the same read/write
/// capability discipline as a shard from [`split_at`](WriterShard::split_at) or
/// [`chunks`](WriterShard::chunks).
///
/// # Examples
///
/// Fetch partitions by index and write each disjointly:
///
/// ```
/// use melinoe::{brand_scope, MelinoeCell};
/// use melinoe::region::WriterShard;
///
/// brand_scope(|token| {
///     let mut cells: [MelinoeCell<'_, usize>; 6] =
///         core::array::from_fn(|_| MelinoeCell::new(0));
///
///     let par = WriterShard::new(&mut cells).par_chunks(2);
///     assert_eq!(par.len(), 3); // ceil(6 / 2)
///
///     // Two *distinct* indices yield non-aliasing shards, so both writes land.
///     // SAFETY: indices 0 and 2 are distinct and each requested exactly once.
///     let mut c0 = unsafe { par.get_unchecked_chunk(0) };
///     let mut c2 = unsafe { par.get_unchecked_chunk(2) };
///     for slot in c0.iter_mut() { *slot = 10; }
///     for slot in c2.iter_mut() { *slot = 20; }
///     drop((c0, c2));
///
///     let snap = token.share();
///     let seen: [usize; 6] = core::array::from_fn(|k| *cells[k].borrow(snap));
///     assert_eq!(seen, [10, 10, 0, 0, 20, 20]);
/// });
/// ```
///
/// A shard obtained from a partition view cannot escape the brand scope — the
/// invariant `'brand` and the `'a` region borrow both pin it:
///
/// ```compile_fail
/// use melinoe::{brand_scope, MelinoeCell};
/// use melinoe::region::WriterShard;
///
/// let escaped = brand_scope(|_token| {
///     let mut cells: [MelinoeCell<'_, usize>; 4] =
///         core::array::from_fn(|_| MelinoeCell::new(0));
///     let par = WriterShard::new(&mut cells).par_chunks(2);
///     // SAFETY: index 0 requested exactly once — sound *inside* the scope.
///     unsafe { par.get_unchecked_chunk(0) } // ERROR: shard borrows `cells`/`'brand`
/// });
/// let _ = escaped;
/// ```
#[must_use = "ParChunks is a lazy indexed view and does nothing unless a chunk is requested"]
pub struct ParChunks<'a, 'brand, T> {
    /// Base pointer to the first cell of the region. Valid for `len` cells and
    /// derived from the original `&'a mut [MelinoeCell<'brand, T>]`.
    base: *mut MelinoeCell<'brand, T>,
    /// Number of cells in the region.
    len: usize,
    /// Cells per partition; clamped to `>= 1` so the ceiling division is total.
    chunk: usize,
    /// Ties the view to the exclusive borrow it was built from, so no shard can
    /// outlive the region and `Send`/`Sync` track `&mut [MelinoeCell]`.
    _marker: PhantomData<&'a mut [MelinoeCell<'brand, T>]>,
}

impl<'a, 'brand, T> ParChunks<'a, 'brand, T> {
    /// Build an indexed partition view over the shard's cells.
    ///
    /// `chunk_size` is clamped to at least `1`. Consumes the parent shard's
    /// exclusive borrow, which is re-vended one disjoint partition at a time by
    /// [`get_unchecked_chunk`](Self::get_unchecked_chunk).
    #[inline]
    pub(super) fn new(cells: &'a mut [MelinoeCell<'brand, T>], chunk_size: usize) -> Self {
        Self {
            base: cells.as_mut_ptr(),
            len: cells.len(),
            chunk: chunk_size.max(1),
            _marker: PhantomData,
        }
    }

    /// Number of disjoint partitions: `ceil(region_len / chunk_size)`, or `0`
    /// when the region is empty.
    ///
    /// This is the exact count of valid indices for
    /// [`get_unchecked_chunk`](Self::get_unchecked_chunk) and the value a driver
    /// reserves worker capacity from. `chunk` is `>= 1`, so the division is
    /// total; written as `1 + (len - 1) / chunk` to compute the ceiling without
    /// the `len + chunk - 1` form, which can overflow for adversarial `len`.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        if self.len == 0 {
            0
        } else {
            1 + (self.len - 1) / self.chunk
        }
    }

    /// Whether the region is empty (no partitions).
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The `[start, end)` cell range of partition `index`, saturating at the
    /// region length. Not exposed publicly; used internally and in tests via the
    /// crate to check the partition property without materializing a shard.
    #[inline]
    pub(crate) const fn range(&self, index: usize) -> (usize, usize) {
        let start = index * self.chunk;
        let end = start + self.chunk;
        // `min` on the end clamps the final partition; `start` may exceed `len`
        // for out-of-range indices, in which case `start >= end` (empty range).
        let end = if end < self.len { end } else { self.len };
        let start = if start < self.len { start } else { self.len };
        (start, end)
    }

    /// Return the disjoint [`WriterShard`] for partition `index`, sharing the
    /// region's `'a` borrow so it may be handed to a worker thread.
    ///
    /// Unlike a bounds-checked accessor, the returned shard carries the region's
    /// `'a` lifetime rather than borrowing `&self`, so the caller may hold
    /// several partitions live at once — the shape a work-stealing pool needs.
    /// The type system therefore cannot enforce disjointness across calls; that
    /// obligation is the caller's, stated below.
    ///
    /// # Safety
    ///
    /// The caller must uphold, for the lifetime of every returned shard:
    ///
    /// * `index < self.len()` — the index names an existing partition. Indices
    ///   in `0..self.len()` map to `[index * chunk_size, min((index+1) *
    ///   chunk_size, region_len))`, which is in-bounds and non-empty.
    /// * Each `index` is requested **at most once** while any prior shard is
    ///   live. Distinct indices name disjoint half-open ranges
    ///   (`[i*chunk, (i+1)*chunk)` are pairwise non-overlapping for distinct `i`
    ///   because they tile `0..region_len` by construction), so distinct indices
    ///   yield non-aliasing `&mut` shards. Requesting the same `index` twice
    ///   while both shards live would alias, which is undefined behavior.
    ///
    /// These are exactly the guarantees the
    /// [`ParallelExecutor`](crate::sync::ParallelExecutor) contract already
    /// requires of an executor (each index invoked once, none omitted), so a
    /// driver that maps one task to one index satisfies them for free.
    #[inline]
    #[must_use]
    pub unsafe fn get_unchecked_chunk(&self, index: usize) -> WriterShard<'a, 'brand, T> {
        let (start, end) = self.range(index);
        // SAFETY: by the documented contract `index < self.len()`, so `start =
        // index * chunk < region_len` and `end = min(start + chunk, region_len)`,
        // giving an in-bounds, non-empty range `[start, end)` fully within the
        // `len` cells the `base` pointer is valid for (derived from the original
        // `&'a mut [MelinoeCell<'brand, T>]`). Distinct in-range indices tile
        // `0..region_len` into pairwise-disjoint ranges, and the contract forbids
        // requesting an index more than once while a shard is live, so no two
        // returned `&mut` slices overlap. The `'a` lifetime is that of the
        // original exclusive borrow, which `_marker` pins, so the shard cannot
        // outlive the region.
        let cells = unsafe { core::slice::from_raw_parts_mut(self.base.add(start), end - start) };
        WriterShard::new(cells)
    }
}

impl<'a, 'brand, T> core::fmt::Debug for ParChunks<'a, 'brand, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ParChunks")
            .field("len", &self.len)
            .field("chunk", &self.chunk)
            .field("partitions", &self.len())
            .finish()
    }
}

// SAFETY: `ParChunks` is a partition view over `&'a mut [MelinoeCell<'brand,
// T>]`; the raw `base` pointer stands in for that exclusive borrow, whose
// `Send`/`Sync` posture is governed by `MelinoeCell<'brand, T>`. It is `Send`
// exactly when the borrowed slice is (`T: Send`), matching `WriterShard`, so a
// driver can move the view (and the shards it vends) across threads.
unsafe impl<'a, 'brand, T: Send> Send for ParChunks<'a, 'brand, T> {}

// SAFETY: sharing `&ParChunks` only permits calling `len`/`range`/
// `get_unchecked_chunk`; the last vends `&mut` sub-slices, so concurrent shared
// access is sound exactly when concurrent access to the underlying
// `&mut [MelinoeCell<'brand, T>]` is, i.e. `T: Send` — identical to
// `WriterShard`'s `Sync` posture.
unsafe impl<'a, 'brand, T: Send> Sync for ParChunks<'a, 'brand, T> {}

// Cross-check that `ParChunks::len` agrees with the sequential `ShardChunks`
// count for the same region and chunk size, so the two partition primitives can
// never disagree on how many shards tile a region. Uses a fixed-capacity backing
// array to stay `no_std`/alloc-free.
#[cfg(test)]
mod parity {
    use super::*;

    #[test]
    fn par_chunks_len_matches_shard_chunks_len() {
        let mut backing: [MelinoeCell<'_, u8>; 40] = core::array::from_fn(|_| MelinoeCell::new(0));
        for len in 0usize..=40 {
            for chunk in 1usize..12 {
                let cells = &mut backing[..len];
                let seq_count = WriterShard::new(cells).chunks(chunk).len();
                let par = ParChunks::new(&mut backing[..len], chunk);
                assert_eq!(
                    par.len(),
                    seq_count,
                    "ParChunks::len disagreed with ShardChunks for len={len} chunk={chunk}"
                );
            }
        }
    }
}
