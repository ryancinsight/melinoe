use alloc::borrow::Cow;

use crate::{
    CellCowExt, CellSliceExt, CowPolicy, MelinoeMut, MelinoeRef, ReadPermit, RetainDecision,
    WritePermit,
};

use super::BrandedVec;

impl<'brand, T> BrandedVec<'brand, T> {
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

    /// Clone the branded vector by presenting a read permit.
    #[inline]
    #[must_use]
    pub fn clone_with<'a, P>(&'a self, permit: P) -> Self
    where
        T: Clone,
        P: ReadPermit<'brand> + 'a,
    {
        Self::from_iter(self.as_slice(permit).iter().cloned())
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
}
