use alloc::borrow::Cow;

use crate::{CowPolicy, MelinoeMut, MelinoeRef, ReadPermit, RetainDecision, WritePermit};

use super::BrandedVecDeque;

impl<'brand, T> BrandedVecDeque<'brand, T> {
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

    /// View the queue as a pair of shared slices.
    #[inline]
    pub fn as_slices<'a, P>(&'a self, _permit: P) -> (&'a [T], &'a [T])
    where
        P: ReadPermit<'brand> + 'a,
    {
        let (s1, s2) = self.cells.as_slices();
        // SAFETY: The presence of `ReadPermit<'brand>` guarantees no concurrent mutation is possible.
        // We cast the MelinoeCell slices to plain slices. Since MelinoeCell is #[repr(transparent)]
        // over UnsafeCell<T>, they share layout and provenance.
        unsafe {
            let s1_ptr = s1.as_ptr() as *const T;
            let s2_ptr = s2.as_ptr() as *const T;
            (
                core::slice::from_raw_parts(s1_ptr, s1.len()),
                core::slice::from_raw_parts(s2_ptr, s2.len()),
            )
        }
    }

    /// View the queue as a pair of mutable slices.
    #[allow(clippy::mut_from_ref)]
    #[inline]
    pub fn as_mut_slices<'a, P>(&'a self, _permit: P) -> (&'a mut [T], &'a mut [T])
    where
        P: WritePermit<'brand> + 'a,
    {
        let (s1, s2) = self.cells.as_slices();
        // SAFETY: The presence of `WritePermit<'brand>` guarantees exclusive access to the branded scope.
        // We cast the MelinoeCell slices to mutable slices using UnsafeCell's raw pointer to preserve provenance.
        unsafe {
            let s1_ptr = s1.as_ptr() as *const core::cell::UnsafeCell<T> as *mut T;
            let s2_ptr = s2.as_ptr() as *const core::cell::UnsafeCell<T> as *mut T;
            (
                core::slice::from_raw_parts_mut(s1_ptr, s1.len()),
                core::slice::from_raw_parts_mut(s2_ptr, s2.len()),
            )
        }
    }

    /// Return a `Cow` view over the queue, borrowing zero-copy if contiguous,
    /// or cloning into an owned vector if it wraps around.
    #[inline]
    pub fn borrow_cow<'a, P>(&'a self, permit: P) -> Cow<'a, [T]>
    where
        T: Clone,
        P: ReadPermit<'brand> + 'a,
    {
        let (s1, s2) = self.as_slices(permit);
        if s2.is_empty() {
            Cow::Borrowed(s1)
        } else {
            let mut v = alloc::vec::Vec::with_capacity(s1.len() + s2.len());
            v.extend_from_slice(s1);
            v.extend_from_slice(s2);
            Cow::Owned(v)
        }
    }

    /// Return an owned `Cow` by cloning all queue elements into a vector.
    #[inline]
    pub fn retain_cow<'a, P>(&'a self, permit: P) -> Cow<'a, [T]>
    where
        T: Clone,
        P: ReadPermit<'brand> + 'a,
    {
        let (s1, s2) = self.as_slices(permit);
        let mut v = alloc::vec::Vec::with_capacity(s1.len() + s2.len());
        v.extend_from_slice(s1);
        v.extend_from_slice(s2);
        Cow::Owned(v)
    }

    /// Return a `Cow` according to a runtime retain decision.
    #[inline]
    pub fn cow_if<'a, P>(&'a self, permit: P, decision: RetainDecision) -> Cow<'a, [T]>
    where
        T: Clone,
        P: ReadPermit<'brand> + 'a,
    {
        match decision {
            RetainDecision::Borrow => self.borrow_cow(permit),
            RetainDecision::Retain => self.retain_cow(permit),
        }
    }

    /// Return a `Cow` according to a compile-time ZST retain policy.
    ///
    /// If the queue is contiguous, the decision is routed to the policy `C`.
    /// If the queue wraps around, it is always cloned into an owned vector.
    #[inline]
    pub fn cow_with<'a, P, C>(&'a self, permit: P, _policy: C) -> Cow<'a, [T]>
    where
        T: Clone,
        P: ReadPermit<'brand> + 'a,
        C: CowPolicy,
    {
        let (s1, s2) = self.as_slices(permit);
        if s2.is_empty() {
            C::cow(s1)
        } else {
            let mut v = alloc::vec::Vec::with_capacity(s1.len() + s2.len());
            v.extend_from_slice(s1);
            v.extend_from_slice(s2);
            Cow::Owned(v)
        }
    }

    /// Clone the branded queue by presenting a read permit.
    #[inline]
    #[must_use]
    pub fn clone_with<'a, P>(&'a self, permit: P) -> Self
    where
        T: Clone,
        P: ReadPermit<'brand> + 'a,
    {
        let (s1, s2) = self.as_slices(permit);
        let mut cloned = Self::with_capacity(self.len());
        cloned.extend(s1.iter().cloned().chain(s2.iter().cloned()));
        cloned
    }
}
