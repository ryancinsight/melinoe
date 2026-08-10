use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ops::RangeBounds;

use crate::MelinoeCell;

use super::{BrandedDrain, BrandedVec};

impl<'brand, T> BrandedVec<'brand, T> {
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

    /// Consume the branded vector and return its owned cell storage.
    ///
    /// This is the branded-storage handoff for consumers that retain Melinoe
    /// cells inside a domain-specific container. The brand remains attached to
    /// every cell, so the recipient can expose only permit-gated access while
    /// taking ownership of the allocation. Converting to a boxed slice may
    /// reallocate when excess vector capacity must be trimmed.
    #[inline]
    #[must_use]
    pub fn into_boxed_cells(self) -> Box<[MelinoeCell<'brand, T>]> {
        self.cells.into_boxed_slice()
    }

    /// Consume the branded vector and return the owned values.
    #[inline]
    #[must_use]
    pub fn into_vec(self) -> Vec<T> {
        // SAFETY: `MelinoeCell<'brand, T>` has the same layout and validity as
        // `T` through its transparent `UnsafeCell` chain; this consumes the
        // allocation exactly once and preserves length and capacity.
        unsafe {
            let mut cells = core::mem::ManuallyDrop::new(self.cells);
            let ptr = cells.as_mut_ptr() as *mut T;
            Vec::from_raw_parts(ptr, cells.len(), cells.capacity())
        }
    }

    /// Create a draining iterator that removes the specified range, yielding the removed values.
    #[inline]
    pub fn drain<R>(&mut self, range: R) -> BrandedDrain<'_, 'brand, T>
    where
        R: RangeBounds<usize>,
    {
        BrandedDrain::new(self.cells.drain(range))
    }

    /// Split the vector into two at the given index, returning the right part.
    #[inline]
    #[must_use]
    pub fn split_off(&mut self, at: usize) -> Self {
        Self {
            cells: self.cells.split_off(at),
        }
    }

    /// Move all elements from `other` into `self`, leaving `other` empty.
    #[inline]
    pub fn append(&mut self, other: &mut Self) {
        self.cells.append(&mut other.cells);
    }
}
