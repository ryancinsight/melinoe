use alloc::vec::Vec;
use core::iter::FromIterator;

use crate::MelinoeCell;

/// A branded vector backed by `Vec<MelinoeCell<'brand, T>>`.
///
/// The vector owns allocation and length management; Melinoe owns the access
/// proof. Element and slice borrows require a brand-matching permit and compile
/// to ordinary references over the contiguous vector allocation.
#[derive(Debug, Default)]
pub struct BrandedVec<'brand, T> {
    pub(crate) cells: Vec<MelinoeCell<'brand, T>>,
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

    /// Generate `len` values in index order inside this brand.
    ///
    /// This is the collection-level constructor for callers that already own a
    /// surrounding brand. For an end-to-end generated computation that should mint
    /// and consume a fresh brand in one expression, use
    /// [`with_generated`](super::with_generated). The
    /// generator runs before the vector is handed to any permit-gated operation,
    /// while generated storage can only be accessed with the matching token.
    #[inline]
    #[must_use]
    pub fn from_fn<F>(len: usize, mut generate: F) -> Self
    where
        F: FnMut(usize) -> T,
    {
        let mut values = Self::with_capacity(len);
        values.extend((0..len).map(&mut generate));
        values
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

    /// Return the Melinoe cell storage.
    ///
    /// This is the lowest-level collection/Melinoe boundary: callers that need
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
        // SAFETY: `MelinoeCell` is `#[repr(transparent)]` over `UnsafeCell<T>`;
        // the cell wrapper adds no size, alignment, or validity requirements to
        // the vector allocation, and `ManuallyDrop` transfers its ownership once.
        unsafe {
            let mut values = core::mem::ManuallyDrop::new(values);
            let ptr = values.as_mut_ptr().cast::<MelinoeCell<'brand, T>>();
            Self {
                cells: Vec::from_raw_parts(ptr, values.len(), values.capacity()),
            }
        }
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
