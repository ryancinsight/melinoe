use alloc::vec::Drain;

use crate::MelinoeCell;

/// A draining iterator for `BrandedVec`.
pub struct BrandedDrain<'a, 'brand, T> {
    inner: Drain<'a, MelinoeCell<'brand, T>>,
}

impl<'a, 'brand, T> BrandedDrain<'a, 'brand, T> {
    pub(super) fn new(inner: Drain<'a, MelinoeCell<'brand, T>>) -> Self {
        Self { inner }
    }
}

impl<'a, 'brand, T> Iterator for BrandedDrain<'a, 'brand, T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(MelinoeCell::into_inner)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, 'brand, T> DoubleEndedIterator for BrandedDrain<'a, 'brand, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(MelinoeCell::into_inner)
    }
}

impl<'a, 'brand, T> ExactSizeIterator for BrandedDrain<'a, 'brand, T> {}
