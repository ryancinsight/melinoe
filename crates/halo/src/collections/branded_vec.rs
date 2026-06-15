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
