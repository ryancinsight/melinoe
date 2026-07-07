//! `VecDeque` storage whose element access is gated by a Melinoe brand permit.
//!
//! # Safety & Correctness Proofs
//!
//! ### Theorem 1 (Single-Writer/Multiple-Reader Safety)
//! Let $C$ be a `BrandedVecDeque<'brand, T>`. For any lifetime `'a`, the coexistence of
//! an exclusive mutable slice borrow $M: \&'a \text{mut } [T]$ and any other active
//! borrow (shared $R: \&'a [T]$ or mutable) of the same brand `'brand` is statically
//! prohibited by the Rust compiler.
//!
//! **Proof**:
//! 1. To obtain $M$ via [`BrandedVecDeque::as_mut_slices`], the caller must present a type
//!    witness `P` satisfying [`WritePermit<'brand>`](melinoe::WritePermit) with lifetime `'a`.
//! 2. The only in-crate type satisfying `WritePermit<'brand>` is `&'a mut ExclusiveToken<'brand>`
//!    (or type projections derived from it).
//! 3. By the borrow checker rules, while `&'a mut ExclusiveToken<'brand>` is borrowed for `'a`,
//!    no other borrow of `ExclusiveToken<'brand>` can be live.
//! 4. Any shared access via [`BrandedVecDeque::as_slices`] requires a `ReadPermit<'brand>`
//!    satisfied by `&'a ExclusiveToken<'brand>` or `SharedReadToken<'a, 'brand>`.
//! 5. Since both read and write permits borrow the unique `ExclusiveToken<'brand>`, the
//!    exclusivity of the mutable permit prevents the creation of any read permits for `'a`.
//!    Thus, concurrent read/write and write/write aliasing is compile-time impossible. $\blacksquare$
//!
//! ### Theorem 2 (Unaliased Ring-Buffer Split)
//! Let $(S_1, S_2) = \text{cells.as\_slices()}$ be the underlying contiguous segments
//! of the `VecDeque`. The slices $S_1$ and $S_2$ are guaranteed to be disjoint.
//!
//! **Proof**:
//! 1. The standard library [`VecDeque::as_slices`] guarantees that the ring buffer is split
//!    into two disjoint memory segments representing the contiguous logical sections.
//! 2. Safety is preserved during casting inside [`BrandedVecDeque::as_mut_slices`] because
//!    disjointness of $(S_1, S_2)$ guarantees that pointer transmutation does not introduce
//!    overlapping `&mut [T]` regions. $\blacksquare$

use alloc::borrow::Cow;
use alloc::collections::VecDeque;
use core::iter::FromIterator;

use melinoe::{
    CowPolicy, MelinoeCell, MelinoeMut, MelinoeRef, ReadPermit, RetainDecision, WritePermit,
};

/// A branded double-ended queue backed by `VecDeque<MelinoeCell<'brand, T>>`.
///
/// The queue owns allocation and indexing; Melinoe owns the access proof.
/// Element and slice borrows require a brand-matching permit and compile
/// to ordinary references over the underlying ring-buffer slices.
#[derive(Debug, Default)]
pub struct BrandedVecDeque<'brand, T> {
    cells: VecDeque<MelinoeCell<'brand, T>>,
}

#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DequeSegment {
    Front,
    Back,
}

#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DequeShard {
    start: usize,
    segment: DequeSegment,
    offset: usize,
    len: usize,
}

#[cfg(feature = "std")]
struct DequeShardPlan {
    shards: alloc::vec::Vec<DequeShard>,
}

#[cfg(feature = "std")]
impl DequeShardPlan {
    fn from_lengths(front_len: usize, back_len: usize, plan: melinoe::sync::PartitionPlan) -> Self {
        let total_len = front_len + back_len;
        let chunk_len = plan.chunk_len_for(total_len).max(1);
        let mut shards = alloc::vec::Vec::new();
        let mut start = 0;

        while start < total_len {
            let end = (start + chunk_len).min(total_len);
            if start < front_len {
                let front_end = end.min(front_len);
                shards.push(DequeShard {
                    start,
                    segment: DequeSegment::Front,
                    offset: start,
                    len: front_end - start,
                });
            }
            if end > front_len {
                let back_start = start.max(front_len) - front_len;
                shards.push(DequeShard {
                    start: front_len + back_start,
                    segment: DequeSegment::Back,
                    offset: back_start,
                    len: end - start.max(front_len),
                });
            }
            start = end;
        }

        Self { shards }
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.shards.is_empty()
    }

    fn map_read<T, R, F>(&self, front: &[T], back: &[T], f: F) -> alloc::vec::Vec<R>
    where
        T: Sync,
        R: Send,
        F: Fn(usize, &[T]) -> R + Sync,
    {
        if self.is_empty() {
            return alloc::vec::Vec::new();
        }

        melinoe::sync::partition_read_map_with(
            &self.shards,
            melinoe::sync::PartitionPlan::chunk_size(1),
            |_, shard| {
                debug_assert_eq!(shard.len(), 1);
                let shard = shard[0];
                f(shard.start, shard.read_slice(front, back))
            },
        )
    }

    fn for_each_read<T, F>(&self, front: &[T], back: &[T], f: F)
    where
        T: Sync,
        F: Fn(usize, &[T]) + Sync,
    {
        self.map_read(front, back, |start, shard| f(start, shard));
    }

    fn map_mut<'brand, T, R, F>(
        &self,
        front: &mut [MelinoeCell<'brand, T>],
        back: &mut [MelinoeCell<'brand, T>],
        f: F,
    ) -> alloc::vec::Vec<R>
    where
        T: Send,
        R: Send,
        F: Fn(usize, &mut [T]) -> R + Sync,
    {
        if self.is_empty() {
            return alloc::vec::Vec::new();
        }

        let slices = DequeMutSlices::new(front, back);
        melinoe::sync::partition_read_map_with(
            &self.shards,
            melinoe::sync::PartitionPlan::chunk_size(1),
            |_, shard| {
                debug_assert_eq!(shard.len(), 1);
                // SAFETY: this driver partitions `self.shards` with chunk size 1,
                // so each descriptor is requested once. `from_lengths` emits
                // non-overlapping descriptors in ascending logical order, and
                // `DequeMutSlices` points at the two disjoint VecDeque storage
                // slices borrowed for this call.
                unsafe { slices.with_shard_mut(shard[0], |start, values| f(start, values)) }
            },
        )
    }

    fn for_each_mut<'brand, T, F>(
        &self,
        front: &mut [MelinoeCell<'brand, T>],
        back: &mut [MelinoeCell<'brand, T>],
        f: F,
    ) where
        T: Send,
        F: Fn(usize, &mut [T]) + Sync,
    {
        self.map_mut(front, back, |start, shard| f(start, shard));
    }
}

#[cfg(feature = "std")]
impl DequeShard {
    #[inline]
    fn read_slice<'a, T>(self, front: &'a [T], back: &'a [T]) -> &'a [T] {
        match self.segment {
            DequeSegment::Front => &front[self.offset..self.offset + self.len],
            DequeSegment::Back => &back[self.offset..self.offset + self.len],
        }
    }
}

#[cfg(feature = "std")]
struct DequeMutSlices<'a, 'brand, T> {
    front: *mut MelinoeCell<'brand, T>,
    front_len: usize,
    back: *mut MelinoeCell<'brand, T>,
    back_len: usize,
    _borrow: core::marker::PhantomData<&'a mut [MelinoeCell<'brand, T>]>,
}

#[cfg(feature = "std")]
impl<'a, 'brand, T> DequeMutSlices<'a, 'brand, T> {
    fn new(
        front: &'a mut [MelinoeCell<'brand, T>],
        back: &'a mut [MelinoeCell<'brand, T>],
    ) -> Self {
        Self {
            front: front.as_mut_ptr(),
            front_len: front.len(),
            back: back.as_mut_ptr(),
            back_len: back.len(),
            _borrow: core::marker::PhantomData,
        }
    }

    unsafe fn with_shard_mut<R>(
        &self,
        shard: DequeShard,
        f: impl FnOnce(usize, &mut [T]) -> R,
    ) -> R {
        let (base, segment_len) = match shard.segment {
            DequeSegment::Front => (self.front, self.front_len),
            DequeSegment::Back => (self.back, self.back_len),
        };
        debug_assert!(shard.offset <= segment_len);
        debug_assert!(shard.len <= segment_len - shard.offset);

        // SAFETY: the caller guarantees each descriptor is used at most once and
        // descriptors do not overlap. Bounds are established by `from_lengths`
        // and checked above in debug builds before forming the sub-slice.
        let cells = unsafe { core::slice::from_raw_parts_mut(base.add(shard.offset), shard.len) };
        let mut writer = melinoe::WriterShard::new(cells);
        f(shard.start, writer.as_mut_slice())
    }
}

#[cfg(feature = "std")]
// SAFETY: `DequeMutSlices` exposes mutation only through disjoint shard
// descriptors generated by `DequeShardPlan`; `T: Send` matches Melinoe's
// `WriterShard` cross-thread write requirement.
unsafe impl<'a, 'brand, T: Send> Sync for DequeMutSlices<'a, 'brand, T> {}

impl<'brand, T> BrandedVecDeque<'brand, T> {
    /// Create an empty branded double-ended queue.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            cells: VecDeque::new(),
        }
    }

    /// Create an empty branded double-ended queue with space for at least `capacity` values.
    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            cells: VecDeque::with_capacity(capacity),
        }
    }

    /// Return the number of values in the queue.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Return `true` when the queue contains no values.
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

    /// Return the Melinoe cell storage.
    #[inline]
    #[must_use]
    pub fn as_cells(&self) -> &VecDeque<MelinoeCell<'brand, T>> {
        &self.cells
    }

    /// Return the Melinoe cell storage with unique ownership.
    #[inline]
    #[must_use]
    pub fn as_mut_cells(&mut self) -> &mut VecDeque<MelinoeCell<'brand, T>> {
        &mut self.cells
    }

    /// Create a draining iterator that removes the specified range, yielding the removed values.
    #[inline]
    pub fn drain<R>(&mut self, range: R) -> BrandedVecDequeDrain<'_, 'brand, T>
    where
        R: core::ops::RangeBounds<usize>,
    {
        BrandedVecDequeDrain {
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
        unsafe {
            let cells = core::mem::ManuallyDrop::new(self.cells);
            core::ptr::read(
                &*cells as *const VecDeque<MelinoeCell<'brand, T>> as *const VecDeque<T>,
            )
        }
    }

    /// Split a permit-gated shared queue into disjoint read shards and run `f`
    /// on each shard concurrently, returning per-shard results.
    ///
    /// The shard plan is derived once from the queue's total logical length.
    /// When a planned logical shard crosses the ring-buffer wrap, it is exposed
    /// as two physical subshards with stable logical start offsets.
    #[cfg(feature = "std")]
    #[inline]
    pub fn partition_map_with<'a, P, R, F>(
        &'a self,
        permit: P,
        plan: melinoe::sync::PartitionPlan,
        f: F,
    ) -> alloc::vec::Vec<R>
    where
        P: ReadPermit<'brand> + 'a,
        T: Sync,
        R: Send,
        F: Fn(usize, &[T]) -> R + Sync,
    {
        let (s1, s2) = self.as_slices(permit);
        DequeShardPlan::from_lengths(s1.len(), s2.len(), plan).map_read(s1, s2, f)
    }

    /// Split a permit-gated shared queue into disjoint read shards and run `f`
    /// on each shard concurrently, discarding results.
    #[cfg(feature = "std")]
    #[inline]
    pub fn partition_for_each_with<'a, P, F>(
        &'a self,
        permit: P,
        plan: melinoe::sync::PartitionPlan,
        f: F,
    ) where
        P: ReadPermit<'brand> + 'a,
        T: Sync,
        F: Fn(usize, &[T]) + Sync,
    {
        let (s1, s2) = self.as_slices(permit);
        DequeShardPlan::from_lengths(s1.len(), s2.len(), plan).for_each_read(s1, s2, f);
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

    /// Split the queue into disjoint writer shards and mutate them
    /// concurrently according to `plan`.
    ///
    /// The shard plan is derived once from the queue's total logical length.
    /// When a planned logical shard crosses the ring-buffer wrap, it is exposed
    /// as two physical subshards with stable logical start offsets.
    #[cfg(feature = "std")]
    #[inline]
    pub fn partition_for_each_mut_with<F>(&mut self, plan: melinoe::sync::PartitionPlan, f: F)
    where
        T: Send,
        F: Fn(usize, &mut [T]) + Sync,
    {
        let (s1, s2) = self.cells.as_mut_slices();
        DequeShardPlan::from_lengths(s1.len(), s2.len(), plan).for_each_mut(s1, s2, f);
    }

    /// Split the queue into disjoint writer shards and return one result per
    /// shard in partition order.
    #[cfg(feature = "std")]
    #[inline]
    pub fn partition_map_mut_with<R, F>(
        &mut self,
        plan: melinoe::sync::PartitionPlan,
        f: F,
    ) -> alloc::vec::Vec<R>
    where
        T: Send,
        R: Send,
        F: Fn(usize, &mut [T]) -> R + Sync,
    {
        let (s1, s2) = self.cells.as_mut_slices();
        DequeShardPlan::from_lengths(s1.len(), s2.len(), plan).map_mut(s1, s2, f)
    }
}

impl<'brand, T> FromIterator<T> for BrandedVecDeque<'brand, T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self {
            cells: iter.into_iter().map(MelinoeCell::new).collect(),
        }
    }
}

impl<'brand, T> Extend<T> for BrandedVecDeque<'brand, T> {
    #[inline]
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
    {
        self.cells.extend(iter.into_iter().map(MelinoeCell::new));
    }
}

/// A draining iterator for `BrandedVecDeque`.
pub struct BrandedVecDequeDrain<'a, 'brand, T> {
    inner: alloc::collections::vec_deque::Drain<'a, MelinoeCell<'brand, T>>,
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

impl<'brand, T> From<VecDeque<T>> for BrandedVecDeque<'brand, T> {
    #[inline]
    fn from(values: VecDeque<T>) -> Self {
        unsafe {
            let values = core::mem::ManuallyDrop::new(values);
            let cells = core::ptr::read(
                &*values as *const VecDeque<T> as *const VecDeque<MelinoeCell<'brand, T>>,
            );
            Self { cells }
        }
    }
}

impl<'brand, T> IntoIterator for BrandedVecDeque<'brand, T> {
    type Item = T;
    type IntoIter = alloc::collections::vec_deque::IntoIter<T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.into_vec_deque().into_iter()
    }
}
