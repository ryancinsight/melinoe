use alloc::borrow::Cow;

use crate::cell::cow_from_segments;
use crate::{
    Borrowed, CowPolicy, MelinoeCell, MelinoeMut, MelinoeRef, ReadPermit, RetainDecision, Retained,
    WritePermit,
};

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
        // SAFETY: The presence of `ReadPermit<'brand>` guarantees no concurrent
        // mutation is possible. `slice_as_unsafe_cell` preserves the
        // interior-mutability provenance carried by `UnsafeCell` over each ring
        // segment, and its `get()` yields a `*mut [T]` that shares the segment's
        // layout (transparent `MelinoeCell` chain).
        unsafe {
            let s1_ptr = MelinoeCell::slice_as_unsafe_cell(s1).get();
            let s2_ptr = MelinoeCell::slice_as_unsafe_cell(s2).get();
            (&*s1_ptr, &*s2_ptr)
        }
    }

    /// View the queue as a pair of mutable slices.
    #[expect(
        clippy::mut_from_ref,
        reason = "exclusivity is supplied by the WritePermit (a &mut borrow of the brand's unique token), not by the &self slice reference — the GhostCell interior-mutability pattern"
    )]
    #[inline]
    pub fn as_mut_slices<'a, P>(&'a self, _permit: P) -> (&'a mut [T], &'a mut [T])
    where
        P: WritePermit<'brand> + 'a,
    {
        let (s1, s2) = self.cells.as_slices();
        // SAFETY: The presence of `WritePermit<'brand>` guarantees exclusive
        // access to the branded scope. `slice_as_unsafe_cell` preserves the
        // interior-mutability provenance of each ring segment; the two segments
        // are disjoint by the `VecDeque::as_slices` contract, so the two
        // `&mut [T]` regions do not overlap.
        unsafe {
            let s1_ptr = MelinoeCell::slice_as_unsafe_cell(s1).get();
            let s2_ptr = MelinoeCell::slice_as_unsafe_cell(s2).get();
            (&mut *s1_ptr, &mut *s2_ptr)
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
        cow_from_segments(s1, s2, Borrowed)
    }

    /// Return an owned `Cow` by cloning all queue elements into a vector.
    #[inline]
    pub fn retain_cow<'a, P>(&'a self, permit: P) -> Cow<'a, [T]>
    where
        T: Clone,
        P: ReadPermit<'brand> + 'a,
    {
        let (s1, s2) = self.as_slices(permit);
        cow_from_segments(s1, s2, Retained)
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
        cow_from_segments(s1, s2, _policy)
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
