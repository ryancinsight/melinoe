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

use alloc::collections::VecDeque;
use core::iter::FromIterator;

use melinoe::{MelinoeCell, MelinoeMut, MelinoeRef, ReadPermit, WritePermit};

/// A branded double-ended queue backed by `VecDeque<MelinoeCell<'brand, T>>`.
///
/// The queue owns allocation and indexing; Melinoe owns the access proof.
/// Element and slice borrows require a brand-matching permit and compile
/// to ordinary references over the underlying ring-buffer slices.
#[derive(Debug, Default)]
pub struct BrandedVecDeque<'brand, T> {
    cells: VecDeque<MelinoeCell<'brand, T>>,
}

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
        // We cast &[MelinoeCell] to &UnsafeCell<[T]> to preserve provenance correctly before dereferencing.
        unsafe {
            let u1 = &*(s1 as *const [MelinoeCell<'brand, T>] as *const core::cell::UnsafeCell<[T]>);
            let u2 = &*(s2 as *const [MelinoeCell<'brand, T>] as *const core::cell::UnsafeCell<[T]>);
            (
                &*(u1.get() as *const [T]),
                &*(u2.get() as *const [T]),
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
        // We cast &[MelinoeCell] to &UnsafeCell<[T]> to preserve provenance correctly before dereferencing.
        unsafe {
            let u1 = &*(s1 as *const [MelinoeCell<'brand, T>] as *const core::cell::UnsafeCell<[T]>);
            let u2 = &*(s2 as *const [MelinoeCell<'brand, T>] as *const core::cell::UnsafeCell<[T]>);
            (
                &mut *u1.get(),
                &mut *u2.get(),
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
}

impl<'brand, T> FromIterator<T> for BrandedVecDeque<'brand, T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self {
            cells: iter.into_iter().map(MelinoeCell::new).collect(),
        }
    }
}
