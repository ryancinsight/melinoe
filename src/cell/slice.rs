//! Zero-copy native-slice views over a branded cell region.
//!
//! Reading or writing a branded slab one cell at a time costs a bounds check and
//! a permit pass per element and defeats autovectorization. [`CellSliceExt`]
//! exposes the whole `[MelinoeCell<'brand, T>]` as a plain `&[T]` / `&mut [T]`
//! once a permit is presented, so callers get ordinary slice ergonomics—
//! `fill`, `copy_from_slice`, `iter().sum()`, SIMD—at zero copy and zero added
//! cost. This is the primitive an allocator uses to bulk-initialise or scan a
//! slab held as branded cells.

use super::MelinoeCell;
use crate::token::{ReadPermit, WritePermit};

/// Zero-copy slice views over `[MelinoeCell<'brand, T>]`, gated by a capability
/// permit.
///
/// Implemented for the slice type itself, so it applies to arrays, `Vec`s, and
/// sub-slices of branded cells uniformly.
pub trait CellSliceExt<'brand, T> {
    /// View the whole region as a shared `&[T]`.
    ///
    /// Requires any [`ReadPermit`] for `'brand`; the returned slice borrows both
    /// the cells and the permit for `'a`.
    fn borrow_slice<'a, P>(&'a self, permit: P) -> &'a [T]
    where
        P: ReadPermit<'brand> + 'a;

    /// View the whole region as an exclusive `&mut [T]`.
    ///
    /// Requires a [`WritePermit`] for `'brand` (a `&mut` borrow of the brand's
    /// unique owning token). A shared `&self` slice borrow suffices because the
    /// permit—not the slice reference—supplies the exclusivity proof (the same
    /// interior-mutability shape as `GhostCell::borrow_mut`).
    #[allow(clippy::mut_from_ref)]
    fn borrow_slice_mut<'a, P>(&'a self, permit: P) -> &'a mut [T]
    where
        P: WritePermit<'brand> + 'a;
}

impl<'brand, T> CellSliceExt<'brand, T> for [MelinoeCell<'brand, T>] {
    #[inline]
    fn borrow_slice<'a, P>(&'a self, _permit: P) -> &'a [T]
    where
        P: ReadPermit<'brand> + 'a,
    {
        let cell = MelinoeCell::slice_as_unsafe_cell(self);
        // SAFETY: a live `ReadPermit<'brand>` proves (via the borrow checker on
        // the brand's unique token) that no `&mut` view of this brand exists for
        // `'a`, so forming `&[T]` over the region cannot alias a `&mut T`.
        unsafe { &*(cell.get() as *const [T]) }
    }

    // The `&self -> &mut [T]` shape is the intended interior-mutability pattern:
    // exclusivity is supplied by the `WritePermit` (a `&mut` borrow of the brand's
    // unique token), not by the slice reference — identical to `GhostCell::borrow_mut`.
    #[allow(clippy::mut_from_ref)]
    #[inline]
    fn borrow_slice_mut<'a, P>(&'a self, _permit: P) -> &'a mut [T]
    where
        P: WritePermit<'brand> + 'a,
    {
        let cell = MelinoeCell::slice_as_unsafe_cell(self);
        // SAFETY: a live `WritePermit<'brand>` is an exclusive borrow of the
        // brand's unique token; while it is held no other read or write permit of
        // this brand can exist, so this `&mut [T]` is unaliased for `'a`. The
        // pointer carries interior-mutability provenance via `UnsafeCell::get`.
        unsafe { &mut *cell.get() }
    }
}
