use alloc::collections::VecDeque;

use crate::MelinoeCell;

use super::BrandedVecDeque;

impl<'brand, T> BrandedVecDeque<'brand, T> {
    /// Append `value` to the back of the queue.
    #[inline]
    pub fn push_back(&mut self, value: T) {
        self.cells.push_back(MelinoeCell::new(value));
    }

    /// Prepend `value` to the front of the queue.
    #[inline]
    pub fn push_front(&mut self, value: T) {
        self.cells.push_front(MelinoeCell::new(value));
    }

    /// Remove the last value and return it, if present.
    #[inline]
    pub fn pop_back(&mut self) -> Option<T> {
        self.cells.pop_back().map(MelinoeCell::into_inner)
    }

    /// Remove the first value and return it, if present.
    #[inline]
    pub fn pop_front(&mut self) -> Option<T> {
        self.cells.pop_front().map(MelinoeCell::into_inner)
    }

    /// Remove all values from the queue.
    #[inline]
    pub fn clear(&mut self) {
        self.cells.clear();
    }

    /// Reserve capacity for at least `additional` more values.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.cells.reserve(additional);
    }

    /// Shrink capacity as close to length as the allocator permits.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.cells.shrink_to_fit();
    }

    /// Shorten the queue to `len`, dropping trailing values.
    #[inline]
    pub fn truncate(&mut self, len: usize) {
        self.cells.truncate(len);
    }

    /// Swap the values at indices `a` and `b`.
    ///
    /// # Panics
    ///
    /// Panics if either index is out of bounds.
    #[inline]
    pub fn swap(&mut self, a: usize, b: usize) {
        self.cells.swap(a, b);
    }

    /// Create a draining iterator that removes the specified range, yielding the removed values.
    #[inline]
    pub fn drain<R>(&mut self, range: R) -> super::iter::BrandedVecDequeDrain<'_, 'brand, T>
    where
        R: core::ops::RangeBounds<usize>,
    {
        super::iter::BrandedVecDequeDrain {
            inner: self.cells.drain(range),
        }
    }

    /// Split the queue into two at the given index, returning the right part.
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

    /// Retain only values for which `f` returns `true`.
    #[inline]
    pub fn retain_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        self.cells.retain_mut(|cell| f(cell.get_mut()));
    }

    /// Consume the branded queue and return the owned values.
    #[inline]
    #[must_use]
    pub fn into_vec_deque(self) -> VecDeque<T> {
        // SAFETY: the reverse conversion of `From<VecDeque<T>>` above is
        // layout-preserving through the transparent `MelinoeCell` chain.
        // `ManuallyDrop` prevents the branded deque from being dropped, and
        // `ptr::read` transfers its allocation and metadata to the returned
        // owner exactly once.
        unsafe {
            let cells = core::mem::ManuallyDrop::new(self.cells);
            core::ptr::read(
                &*cells as *const VecDeque<MelinoeCell<'brand, T>> as *const VecDeque<T>,
            )
        }
    }
}
