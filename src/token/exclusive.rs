//! [`ExclusiveToken`] — the unique, move-only owner of a brand.

use core::fmt;
use core::marker::PhantomData;

use super::brand::InvariantLifetime;
use super::capability::private::Sealed;
use super::capability::{ReadPermit, WritePermit};
use super::SharedReadToken;

/// The single, un-clonable owner of a brand's access rights.
///
/// At most one `ExclusiveToken<'brand>` exists per brand (guaranteed by
/// [`brand_scope`](crate::brand_scope)). Because the type is **move-only**—it
/// deliberately implements neither [`Clone`] nor [`Copy`]—the borrow checker's
/// aliasing rules on this one value transitively police *all* cells of the
/// brand:
///
/// * a shared borrow `&ExclusiveToken` is a [`ReadPermit`], and
/// * an exclusive borrow `&mut ExclusiveToken` is a [`WritePermit`].
///
/// Since you cannot hold `&mut` and `&` to the same token at once, you cannot
/// hold a write permit and any read permit of the same brand at once. The XOR
/// discipline `T xor &mut T xor &T` is thereby lifted from a single token to an
/// entire region of branded cells at zero runtime cost.
///
/// `ExclusiveToken` is `Send + Sync`: it is a ZST whose only field is a
/// function-pointer phantom, so it may be moved to another thread to transfer
/// write capability across a thread boundary.
///
/// > *In myth Melinoë leads a train of restless phantoms; the exclusive token
/// > is the one shade permitted to disturb the dead.*
pub struct ExclusiveToken<'brand> {
    _invariant: InvariantLifetime<'brand>,
}

impl<'brand> ExclusiveToken<'brand> {
    /// Construct a token without proving brand uniqueness.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that no other `ExclusiveToken<'brand>` for the
    /// same `'brand` exists for the lifetime of the returned value. Violating
    /// this allows two write permits of one brand to coexist, which is
    /// undefined behaviour. Prefer [`brand_scope`](crate::brand_scope), which
    /// discharges this obligation via a higher-ranked lifetime.
    #[inline]
    #[must_use]
    pub const unsafe fn new_unchecked() -> Self {
        Self {
            _invariant: PhantomData,
        }
    }

    /// Mint a `Copy`, read-only [`SharedReadToken`] tied to this borrow.
    ///
    /// The returned token borrows `self` immutably for `'a`, so while any copy
    /// of it is live the exclusive token cannot be borrowed mutably and no
    /// write permit can be formed. This is the supported way to fan a single
    /// brand's read capability out to many call sites or threads.
    #[inline]
    #[must_use]
    pub fn share<'a>(&'a self) -> SharedReadToken<'a, 'brand> {
        // SAFETY: `self` is borrowed for `'a`; the produced token carries that
        // borrow in its phantom, preserving read/write exclusion.
        unsafe { SharedReadToken::new_unchecked() }
    }
}

impl<'brand> fmt::Debug for ExclusiveToken<'brand> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ExclusiveToken<'brand>")
    }
}

impl<'brand> Sealed for &ExclusiveToken<'brand> {}
impl<'brand> Sealed for &mut ExclusiveToken<'brand> {}

// SAFETY: a shared borrow of the unique owning token grants read access; while
// it is held the token cannot be borrowed mutably, so no write permit of the
// same brand can coexist.
unsafe impl<'brand> ReadPermit<'brand> for &ExclusiveToken<'brand> {}

// SAFETY: an exclusive borrow of the unique owning token is itself unique; no
// other borrow (read or write) of the token can exist for its duration, so the
// brand-wide XOR invariant holds.
unsafe impl<'brand> ReadPermit<'brand> for &mut ExclusiveToken<'brand> {}
unsafe impl<'brand> WritePermit<'brand> for &mut ExclusiveToken<'brand> {}
