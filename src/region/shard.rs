use crate::cell::MelinoeCell;

use super::ShardChunks;

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
