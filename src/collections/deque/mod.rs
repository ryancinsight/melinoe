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
//!    witness `P` satisfying [`WritePermit<'brand>`] with lifetime `'a`.
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

// The module proof above is LaTeX: `$S_1$`/`$S_2$` are mathematical subscripts
// and the surrounding math-mode markup is not Rust. `doc_markdown` cannot tell
// them from unbackticked code paths, and backticking would break the rendered
// math.
#![expect(
    clippy::doc_markdown,
    reason = "LaTeX math in the module-level correctness proof, not code identifiers"
)]

pub mod iter;
pub mod ops;
pub mod partition;
pub mod views;

use alloc::collections::VecDeque;
use core::iter::FromIterator;

use crate::MelinoeCell;

pub use self::iter::BrandedVecDequeDrain;

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

impl<'brand, T> Extend<T> for BrandedVecDeque<'brand, T> {
    #[inline]
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
    {
        self.cells.extend(iter.into_iter().map(MelinoeCell::new));
    }
}

impl<'brand, T> From<VecDeque<T>> for BrandedVecDeque<'brand, T> {
    #[inline]
    fn from(values: VecDeque<T>) -> Self {
        // SAFETY: `MelinoeCell<'brand, T>` is `#[repr(transparent)]` over
        // `UnsafeCell<T>`, preserving the element's size, alignment, and
        // validity. `VecDeque` stores the element allocation and metadata
        // independently of the element value, so reinterpreting its element
        // type preserves the deque representation. `ManuallyDrop` prevents
        // the source deque from being dropped, and `ptr::read` transfers that
        // representation to the branded owner exactly once.
        unsafe {
            let values = core::mem::ManuallyDrop::new(values);
            let cells = core::ptr::read(
                core::ptr::addr_of!(*values).cast::<VecDeque<MelinoeCell<'brand, T>>>(),
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
