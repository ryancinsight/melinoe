//! [`MelinoeCell2`] — interior mutability gated by *two* brands at once.
//!
//! Where [`MelinoeCell`](crate::MelinoeCell) is unlocked by one brand's token,
//! `MelinoeCell2<'a, 'b, T>` is unlocked only when the caller presents a
//! capability for **both** `'a` *and* `'b`. This encodes a *multi-lock-held*
//! invariant at compile time: data co-owned by two subsystems that may be
//! touched only while both subsystems' exclusion domains are held — the static
//! analogue of acquiring two locks before touching shared state, with the lock
//! ordering and held-set proven by the type system rather than convention.

use core::cell::UnsafeCell;
use core::fmt;
use core::marker::PhantomData;

use crate::token::{InvariantLifetime, ReadPermit, WritePermit};

/// A cell whose access requires a capability for each of two distinct brands.
///
/// `#[repr(transparent)]` over `UnsafeCell<T>` (the two brand markers are ZSTs),
/// so it adds no footprint over `T`. Invariant in both `'a` and `'b`.
///
/// Holding only one brand's token is insufficient — both are demanded:
///
/// ```compile_fail
/// use melinoe::{brand_scope2, MelinoeCell2};
/// brand_scope2(|mut ta, _tb| {
///     let cell: MelinoeCell2<'_, '_, u64> = MelinoeCell2::new(0);
///     // ERROR: `borrow_mut` needs a WritePermit for *both* brands.
///     let _ = cell.borrow_mut(&mut ta);
/// });
/// ```
#[repr(transparent)]
pub struct MelinoeCell2<'a, 'b, T: ?Sized> {
    _a: InvariantLifetime<'a>,
    _b: InvariantLifetime<'b>,
    value: UnsafeCell<T>,
}

impl<'a, 'b, T> MelinoeCell2<'a, 'b, T> {
    /// Wrap `value` in a cell branded by the ambient `'a` and `'b`.
    #[inline]
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self {
            _a: PhantomData,
            _b: PhantomData,
            value: UnsafeCell::new(value),
        }
    }

    /// Consume the cell, returning the contained value.
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<'a, 'b, T: ?Sized> MelinoeCell2<'a, 'b, T> {
    /// Shared access, proven by a [`ReadPermit`] for *each* brand.
    #[inline]
    pub fn borrow<'s, PA, PB>(&'s self, _a: PA, _b: PB) -> &'s T
    where
        PA: ReadPermit<'a> + 's,
        PB: ReadPermit<'b> + 's,
    {
        // SAFETY: a live `ReadPermit<'a>` and `ReadPermit<'b>` jointly prove that
        // no `&mut` view of *either* brand exists for `'s`; since this cell is
        // identified by the pair `('a, 'b)`, no `&mut T` to it can exist.
        unsafe { &*self.value.get() }
    }

    /// Exclusive access, proven by a [`WritePermit`] for *each* brand.
    ///
    /// `&self -> &mut T` is the intended interior-mutability shape: exclusivity
    /// is supplied by the two write permits, not the cell reference.
    #[allow(clippy::mut_from_ref)]
    #[inline]
    pub fn borrow_mut<'s, PA, PB>(&'s self, _a: PA, _b: PB) -> &'s mut T
    where
        PA: WritePermit<'a> + 's,
        PB: WritePermit<'b> + 's,
    {
        // SAFETY: live `WritePermit<'a>` and `WritePermit<'b>` are exclusive
        // borrows of both brands' unique tokens; while both are held no other
        // permit of either brand can exist, so this `&mut T` is unaliased.
        unsafe { &mut *self.value.get() }
    }

    /// Acquire `&mut T` from unique ownership of the cell — no tokens required.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }
}

// SAFETY / Sync: identical reasoning to `MelinoeCell` — moving needs `T: Send`,
// and `&`-sharing across threads (under tokens) needs `T: Send + Sync`.
unsafe impl<'a, 'b, T: ?Sized + Send> Send for MelinoeCell2<'a, 'b, T> {}
unsafe impl<'a, 'b, T: ?Sized + Send + Sync> Sync for MelinoeCell2<'a, 'b, T> {}

impl<'a, 'b, T: Default> Default for MelinoeCell2<'a, 'b, T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<'a, 'b, T: ?Sized> fmt::Debug for MelinoeCell2<'a, 'b, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MelinoeCell2").finish_non_exhaustive()
    }
}
