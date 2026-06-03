//! [`SharedReadToken`] — a `Copy`, read-only capability derived from a brand.

use core::fmt;
use core::marker::PhantomData;

use super::brand::InvariantLifetime;
use super::capability::private::Sealed;
use super::capability::ReadPermit;

/// A freely-copyable, read-only permit for a brand.
///
/// Unlike [`ExclusiveToken`](crate::ExclusiveToken), a `SharedReadToken` is
/// [`Copy`], so it can be stored in many places and handed to many readers
/// (including many threads). Soundness is preserved by construction: the only
/// ways to obtain one are [`ExclusiveToken::share`](crate::ExclusiveToken::share)
/// and [`SyncRegionToken::share`](crate::sync::SyncRegionToken::share), each of
/// which borrows its owning token immutably for `'a`. As long as any copy
/// survives, that immutable borrow keeps the owning token from being borrowed
/// mutably, so no [`WritePermit`](crate::WritePermit) of the same brand can be
/// formed concurrently.
///
/// The `'a` lifetime is the *sharing window*; the `'brand` lifetime is the brand
/// identity. The window phantom is a covariant `&'a ()`—the marker need only
/// carry the immutable-borrow window, not the concrete owning token's type—so
/// the same token type serves every owner family. It is `Send + Sync`, enabling
/// concurrent reads of branded cells across threads.
pub struct SharedReadToken<'a, 'brand> {
    _invariant: InvariantLifetime<'brand>,
    _window: PhantomData<&'a ()>,
}

impl<'a, 'brand> SharedReadToken<'a, 'brand> {
    /// Construct a shared read token without binding it to a sharing window.
    ///
    /// # Safety
    ///
    /// The caller must ensure that for the entire lifetime `'a` of the returned
    /// token, no `WritePermit<'brand>` is or will be formed.
    /// [`ExclusiveToken::share`](crate::ExclusiveToken::share) and
    /// [`SyncRegionToken::share`](crate::sync::SyncRegionToken::share) are the
    /// safe constructors and the only intended callers.
    #[inline]
    #[must_use]
    pub(crate) unsafe fn new_unchecked() -> Self {
        Self {
            _invariant: PhantomData,
            _window: PhantomData,
        }
    }
}

impl<'a, 'brand> Clone for SharedReadToken<'a, 'brand> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, 'brand> Copy for SharedReadToken<'a, 'brand> {}

impl<'a, 'brand> fmt::Debug for SharedReadToken<'a, 'brand> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SharedReadToken<'a, 'brand>")
    }
}

impl<'a, 'brand> Sealed for SharedReadToken<'a, 'brand> {}

// SAFETY: the token's `'a` window is a live immutable borrow of the unique
// owning `ExclusiveToken`; no `&mut` of that token, and hence no write permit,
// can exist while any copy of this token lives.
unsafe impl<'a, 'brand> ReadPermit<'brand> for SharedReadToken<'a, 'brand> {}
