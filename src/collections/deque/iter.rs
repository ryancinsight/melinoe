use crate::MelinoeCell;

/// A draining iterator for `BrandedVecDeque`.
pub struct BrandedVecDequeDrain<'a, 'brand, T> {
    pub(super) inner: alloc::collections::vec_deque::Drain<'a, MelinoeCell<'brand, T>>,
}

impl<'a, 'brand, T> Iterator for BrandedVecDequeDrain<'a, 'brand, T> {
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

impl<'a, 'brand, T> DoubleEndedIterator for BrandedVecDequeDrain<'a, 'brand, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(MelinoeCell::into_inner)
    }
}

impl<'a, 'brand, T> ExactSizeIterator for BrandedVecDequeDrain<'a, 'brand, T> {}
