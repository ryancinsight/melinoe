//! `Vec` storage whose element access is gated by a Melinoe brand permit.

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::iter::FromIterator;

use melinoe::{
    CellCowExt, CellSliceExt, CowPolicy, MelinoeCell, MelinoeMut, MelinoeRef, ReadPermit,
    RetainDecision, WritePermit,
};

/// A branded vector backed by `Vec<MelinoeCell<'brand, T>>`.
///
/// The vector owns allocation and length management; Melinoe owns the access
/// proof. Element and slice borrows require a brand-matching permit and compile
/// to ordinary references over the contiguous vector allocation.
#[derive(Debug, Default)]
pub struct BrandedVec<'brand, T> {
    cells: Vec<MelinoeCell<'brand, T>>,
}

impl<'brand, T> BrandedVec<'brand, T> {
    /// Create an empty branded vector.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self { cells: Vec::new() }
    }

    /// Create an empty branded vector with space for at least `capacity` values.
    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            cells: Vec::with_capacity(capacity),
        }
    }

    /// Return the number of values in the vector.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Return `true` when the vector contains no values.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Return the current allocation capacity.
    #[inline]
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.cells.capacity()
    }

    /// Append `value` to the vector.
    #[inline]
    pub fn push(&mut self, value: T) {
        self.cells.push(MelinoeCell::new(value));
    }

    /// Remove the last value and return it, if present.
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        self.cells.pop().map(MelinoeCell::into_inner)
    }

    /// Insert `value` at `index`, shifting all following values to the right.
    ///
    /// # Panics
    ///
    /// Panics if `index > len`, matching [`Vec::insert`].
    #[inline]
    pub fn insert(&mut self, index: usize, value: T) {
        self.cells.insert(index, MelinoeCell::new(value));
    }

    /// Remove and return the value at `index`, shifting following values left.
    ///
    /// # Panics
    ///
    /// Panics if `index >= len`, matching [`Vec::remove`].
    #[inline]
    pub fn remove(&mut self, index: usize) -> T {
        self.cells.remove(index).into_inner()
    }

    /// Remove and return the value at `index`, replacing it with the last value.
    ///
    /// # Panics
    ///
    /// Panics if `index >= len`, matching [`Vec::swap_remove`].
    #[inline]
    pub fn swap_remove(&mut self, index: usize) -> T {
        self.cells.swap_remove(index).into_inner()
    }

    /// Swap the values at indices `a` and `b`.
    ///
    /// # Panics
    ///
    /// Panics if either index is out of bounds, matching slice `swap`.
    #[inline]
    pub fn swap(&mut self, a: usize, b: usize) {
        self.cells.swap(a, b);
    }

    /// Reserve capacity for at least `additional` more values.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.cells.reserve(additional);
    }

    /// Remove all values from the vector.
    #[inline]
    pub fn clear(&mut self) {
        self.cells.clear();
    }

    /// Shorten the vector to `len`, dropping trailing values.
    #[inline]
    pub fn truncate(&mut self, len: usize) {
        self.cells.truncate(len);
    }

    /// Shrink capacity as close to length as the allocator permits.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.cells.shrink_to_fit();
    }

    /// Resize the vector by calling `f` for each inserted value.
    #[inline]
    pub fn resize_with<F>(&mut self, new_len: usize, mut f: F)
    where
        F: FnMut() -> T,
    {
        self.cells.resize_with(new_len, || MelinoeCell::new(f()));
    }

    /// Retain only values for which `f` returns `true`.
    ///
    /// The predicate receives `&mut T` through unique vector ownership, so no
    /// token is needed and no value is copied out of the branded storage.
    #[inline]
    pub fn retain_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        self.cells.retain_mut(|cell| f(cell.get_mut()));
    }

    /// Return the Melinoe cell storage.
    ///
    /// This is the lowest-level Halo/Melinoe boundary: callers that need
    /// Melinoe's native cell APIs can use the same cells instead of a copied
    /// adapter representation.
    #[inline]
    #[must_use]
    pub fn as_cells(&self) -> &[MelinoeCell<'brand, T>] {
        &self.cells
    }

    /// Return the Melinoe cell storage with unique vector ownership.
    #[inline]
    #[must_use]
    pub fn as_mut_cells(&mut self) -> &mut [MelinoeCell<'brand, T>] {
        &mut self.cells
    }

    /// Return a permit-gated shared reference to one value.
    #[inline]
    pub fn get<'a, P>(&'a self, index: usize, permit: P) -> Option<MelinoeRef<'a, 'brand, T>>
    where
        P: ReadPermit<'brand> + 'a,
    {
        self.cells.get(index).map(|cell| cell.borrow(permit))
    }

    /// Return a permit-gated mutable reference to one value.
    #[inline]
    pub fn get_mut<'a, P>(&'a self, index: usize, permit: P) -> Option<MelinoeMut<'a, 'brand, T>>
    where
        P: WritePermit<'brand> + 'a,
    {
        self.cells.get(index).map(|cell| cell.borrow_mut(permit))
    }

    /// View the whole vector as a shared slice without copying.
    #[inline]
    pub fn as_slice<'a, P>(&'a self, permit: P) -> &'a [T]
    where
        P: ReadPermit<'brand> + 'a,
    {
        self.cells.borrow_slice(permit)
    }

    /// View the whole vector as a mutable slice without copying.
    #[inline]
    pub fn as_mut_slice<'a, P>(&'a self, permit: P) -> &'a mut [T]
    where
        P: WritePermit<'brand> + 'a,
    {
        self.cells.borrow_slice_mut(permit)
    }

    /// Return a zero-copy borrowed `Cow` over the branded slice.
    #[inline]
    pub fn borrow_cow<'a, P>(&'a self, permit: P) -> Cow<'a, [T]>
    where
        T: Clone,
        P: ReadPermit<'brand> + 'a,
    {
        self.cells.borrow_cow(permit)
    }

    /// Return an owned `Cow` by cloning the branded slice exactly once.
    #[inline]
    pub fn retain_cow<'a, P>(&'a self, permit: P) -> Cow<'a, [T]>
    where
        T: Clone,
        P: ReadPermit<'brand> + 'a,
    {
        self.cells.retain_cow(permit)
    }

    /// Return a `Cow` according to a compile-time ZST retain policy.
    #[inline]
    pub fn cow_with<'a, P, C>(&'a self, permit: P, policy: C) -> Cow<'a, [T]>
    where
        T: Clone,
        P: ReadPermit<'brand> + 'a,
        C: CowPolicy,
    {
        self.cells.borrow_cow_with(permit, policy)
    }

    /// Return a `Cow` according to a runtime retain decision.
    #[inline]
    pub fn cow_if<'a, P>(&'a self, permit: P, decision: RetainDecision) -> Cow<'a, [T]>
    where
        T: Clone,
        P: ReadPermit<'brand> + 'a,
    {
        self.cells.borrow_cow_if(permit, decision)
    }

    /// Consume the branded vector and return the owned values.
    #[inline]
    #[must_use]
    pub fn into_vec(self) -> Vec<T> {
        self.cells
            .into_iter()
            .map(MelinoeCell::into_inner)
            .collect()
    }

    /// Split the vector into disjoint writer shards and mutate them
    /// concurrently according to `plan`.
    ///
    /// This method delegates to Melinoe's partition driver. No token is needed:
    /// `&mut self` already proves unique ownership of the vector storage, and
    /// Melinoe's [`WriterShard`](melinoe::WriterShard) proves every worker sees
    /// a non-overlapping subslice.
    #[cfg(feature = "std")]
    #[inline]
    pub fn partition_for_each_mut_with<F>(&mut self, plan: melinoe::sync::PartitionPlan, f: F)
    where
        T: Send,
        F: Fn(usize, &mut [T]) + Sync,
    {
        melinoe::sync::partition_for_each_with(&mut self.cells, plan, |start, mut shard| {
            f(start, shard.as_mut_slice());
        });
    }

    /// Split the vector into disjoint writer shards and return one result per
    /// shard in partition order.
    #[cfg(feature = "std")]
    #[inline]
    pub fn partition_map_mut_with<R, F>(
        &mut self,
        plan: melinoe::sync::PartitionPlan,
        f: F,
    ) -> Vec<R>
    where
        T: Send,
        R: Send,
        F: Fn(usize, &mut [T]) -> R + Sync,
    {
        melinoe::sync::partition_map_with(&mut self.cells, plan, |start, mut shard| {
            f(start, shard.as_mut_slice())
        })
    }
}

impl<'brand, T> Extend<T> for BrandedVec<'brand, T> {
    #[inline]
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
    {
        self.cells.extend(iter.into_iter().map(MelinoeCell::new));
    }
}

impl<'brand, T> FromIterator<T> for BrandedVec<'brand, T> {
    #[inline]
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        let mut values = Self::new();
        values.extend(iter);
        values
    }
}

impl<'brand, T> From<Vec<T>> for BrandedVec<'brand, T> {
    #[inline]
    fn from(values: Vec<T>) -> Self {
        values.into_iter().collect()
    }
}

impl<'brand, T> IntoIterator for BrandedVec<'brand, T> {
    type Item = T;
    type IntoIter = alloc::vec::IntoIter<T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.into_vec().into_iter()
    }
}
