//! Region partitioning: sound, zero-cost concurrent writes over disjoint slices.
//!
//! Two threads writing the *same* cell is a data race and cannot be made sound
//! by any phantom-type scheme—`&mut T` is exclusive by definition. Concurrent
//! *writes* are therefore expressed as concurrent access to **disjoint
//! partitions** of a branded region, which is exactly what per-thread allocator
//! slabs need.
//!
//! [`WriterShard`] is the unit of that partition: a move-only, [`Send`]
//! capability over a disjoint `&mut [MelinoeCell<'brand, T>]` sub-slice. It is
//! produced by [splitting](WriterShard::split_at) a parent region, whose
//! disjointness is guaranteed by the standard library's
//! [`<[_]>::split_at_mut`](slice::split_at_mut). Each shard can be moved to its
//! own thread for parallel writes with **no atomics and no locks**.
//!
//! # Read depends on write, structurally
//!
//! A shard exposes reads through `&self` ([`get`](WriterShard::get),
//! [`iter`](WriterShard::iter)) and writes through `&mut self`
//! ([`get_mut`](WriterShard::get_mut), [`iter_mut`](WriterShard::iter_mut)).
//! Holding the shard mutably therefore grants both read and write, while a
//! shared `&shard` grants read only: write is the strictly higher capability,
//! and obtaining it presupposes the read capability over the same partition.
//! This mirrors the crate's [`WritePermit`](crate::WritePermit) ⊒
//! [`ReadPermit`](crate::ReadPermit) lattice, here enforced by the borrow
//! checker on the shard value itself.
//!
//! # Lifecycle
//!
//! ```
//! use melinoe::{brand_scope, region::WriterShard, MelinoeCell};
//!
//! brand_scope(|token| {
//!     let mut cells: [MelinoeCell<'_, u32>; 6] =
//!         core::array::from_fn(|_| MelinoeCell::new(0));
//!
//!     // Phase 1 — partition into disjoint shards and write each independently.
//!     let (mut lo, mut hi) = WriterShard::new(&mut cells).split_at(3);
//!     for (j, slot) in lo.iter_mut().enumerate() { *slot = j as u32; }
//!     for (j, slot) in hi.iter_mut().enumerate() { *slot = 100 + j as u32; }
//!
//!     // Phase 2 — shards dropped; read the whole region back via the token.
//!     let snap = token.share();
//!     let seen: [u32; 6] = core::array::from_fn(|k| *cells[k].borrow(snap));
//!     assert_eq!(seen, [0, 1, 2, 100, 101, 102]);
//! });
//! ```

use crate::cell::MelinoeCell;

/// A move-only, [`Send`] write capability over a disjoint partition of a branded
/// region.
///
/// Construct one over a whole region with [`new`](Self::new), then subdivide
/// with [`split_at`](Self::split_at) / [`chunks`](Self::chunks) to obtain
/// disjoint shards for concurrent threads. The shard grants exclusive read+write
/// to *its* cells and—because it is move-only and each sub-slice is
/// non-overlapping—two shards can never reach the same cell.
///
/// `#[repr(transparent)]` over the underlying `&mut [MelinoeCell<'brand, T>]`:
/// the capability is just the slice reference, with no extra footprint. It is
/// `Send`/`Sync` exactly when `MelinoeCell<'brand, T>` is (i.e. `T: Send` /
/// `T: Send + Sync`).
#[repr(transparent)]
pub struct WriterShard<'a, 'brand, T> {
    cells: &'a mut [MelinoeCell<'brand, T>],
}

impl<'a, 'brand, T> WriterShard<'a, 'brand, T> {
    /// Wrap an exclusive borrow of a contiguous region as a single shard.
    #[inline]
    #[must_use]
    pub fn new(cells: &'a mut [MelinoeCell<'brand, T>]) -> Self {
        Self { cells }
    }

    /// Number of cells in this partition.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether this partition is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// View the partition as a plain shared slice (the lower capability; needs
    /// `&self`).
    ///
    /// Zero-cost: `MelinoeCell<'brand, T>` is `#[repr(transparent)]` over
    /// `UnsafeCell<T>` over `T`, so `[MelinoeCell<'brand, T>]` shares the layout
    /// of `[T]`. Exposing a `&[T]` lets callers use ordinary slice operations
    /// (iteration, search, SIMD) over the region.
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        let ptr = MelinoeCell::slice_as_unsafe_cell(self.cells).get();
        // SAFETY: the shared `&self` borrow excludes `&mut self`—the only source
        // of a `&mut T` here—and the shard's `&mut [MelinoeCell]` ownership
        // excludes all external/token access, so no `&mut T` to these cells exists
        // while the `&[T]` lives. The pointer carries whole-region provenance via
        // `UnsafeCell::get`.
        unsafe { &*(ptr as *const [T]) }
    }

    /// View the partition as a plain exclusive slice (the higher capability;
    /// needs `&mut self`, which also grants read).
    ///
    /// Zero-cost for the same layout reason as [`as_slice`](Self::as_slice).
    #[inline]
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let ptr = MelinoeCell::slice_as_unsafe_cell(self.cells).get();
        // SAFETY: `&mut self` grants exclusive access to the cells, so the
        // `&mut [T]` is unaliased; `UnsafeCell::get` supplies the interior-mutable
        // provenance over the whole region.
        unsafe { &mut *ptr }
    }

    /// Shared read of the `index`-th cell (needs `&self`).
    #[inline]
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.as_slice().get(index)
    }

    /// Iterator of shared reads over the partition (needs `&self`).
    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    /// Exclusive access to the `index`-th cell (needs `&mut self`).
    #[inline]
    #[must_use]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.as_mut_slice().get_mut(index)
    }

    /// Iterator of exclusive references over the partition (needs `&mut self`).
    #[inline]
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        self.as_mut_slice().iter_mut()
    }

    /// Divide this shard into two disjoint shards at `mid`, consuming `self`.
    ///
    /// The two results borrow non-overlapping halves of the same region, so they
    /// may be written concurrently on different threads. This is the primitive
    /// behind recursive divide-and-conquer partitioning.
    ///
    /// # Panics
    ///
    /// Panics if `mid > self.len()` (as [`slice::split_at_mut`]).
    #[inline]
    #[must_use]
    pub fn split_at(self, mid: usize) -> (Self, Self) {
        let (left, right) = self.cells.split_at_mut(mid);
        (Self::new(left), Self::new(right))
    }

    /// Consume the shard into an iterator of disjoint shards of `chunk_size`
    /// cells each (the final shard may be shorter).
    ///
    /// `chunk_size` is clamped to at least `1`. The iterator yields strictly
    /// non-overlapping shards, suitable for distributing across a thread pool.
    #[inline]
    pub fn chunks(self, chunk_size: usize) -> ShardChunks<'a, 'brand, T> {
        ShardChunks {
            rest: Some(self.cells),
            chunk: chunk_size.max(1),
        }
    }

    /// Borrow the underlying cells immutably (e.g. for token-mediated reads).
    #[inline]
    #[must_use]
    pub fn as_cells(&self) -> &[MelinoeCell<'brand, T>] {
        self.cells
    }

    /// Recover the underlying exclusive slice, consuming the shard.
    #[inline]
    #[must_use]
    pub fn into_cells(self) -> &'a mut [MelinoeCell<'brand, T>] {
        self.cells
    }
}

impl<'a, 'brand, T> core::fmt::Debug for WriterShard<'a, 'brand, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("WriterShard")
            .field("len", &self.cells.len())
            .finish_non_exhaustive()
    }
}

impl<'s, 'a, 'brand, T> IntoIterator for &'s WriterShard<'a, 'brand, T> {
    type Item = &'s T;
    type IntoIter = core::slice::Iter<'s, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl<'s, 'a, 'brand, T> IntoIterator for &'s mut WriterShard<'a, 'brand, T> {
    type Item = &'s mut T;
    type IntoIter = core::slice::IterMut<'s, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.as_mut_slice().iter_mut()
    }
}

/// Iterator over disjoint [`WriterShard`]s produced by [`WriterShard::chunks`].
///
/// Each yielded shard owns a non-overlapping sub-slice carved off the front of
/// the remaining region with [`slice::split_at_mut`], so all yielded shards are
/// mutually disjoint and may be written in parallel.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ShardChunks<'a, 'brand, T> {
    rest: Option<&'a mut [MelinoeCell<'brand, T>]>,
    chunk: usize,
}

impl<'a, 'brand, T> Iterator for ShardChunks<'a, 'brand, T> {
    type Item = WriterShard<'a, 'brand, T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let slice = self.rest.take()?;
        if slice.is_empty() {
            return None;
        }
        let mid = self.chunk.min(slice.len());
        let (head, tail) = slice.split_at_mut(mid);
        self.rest = Some(tail);
        Some(WriterShard::new(head))
    }
}

impl<'a, 'brand, T> core::fmt::Debug for ShardChunks<'a, 'brand, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let remaining = self.rest.as_ref().map_or(0, |s| s.len());
        f.debug_struct("ShardChunks")
            .field("remaining", &remaining)
            .field("chunk", &self.chunk)
            .finish()
    }
}
