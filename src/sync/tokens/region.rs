//! [`SyncRegionToken`] — a brand whose access right may cross threads.

use core::fmt;
use core::marker::PhantomData;

use crate::token::capability::private::Sealed;
use crate::token::{InvariantLifetime, ReadPermit, SharedReadToken, WritePermit};

/// A brand owner that is `Send + Sync` and may be handed across thread
/// boundaries to relocate exclusive write capability.
///
/// `SyncRegionToken` carries the same permit semantics as
/// [`ExclusiveToken`](crate::ExclusiveToken) but names the *region* pattern
/// explicitly: a contiguous branded region (e.g. an allocator's slab) whose
/// ownership migrates between worker threads. Moving the token to a thread
/// transfers the right to mutate every cell of the region; sharing `&token`
/// across threads (via [`crate::MelinoeCell`]'s `Sync` impl) grants concurrent
/// read access.
///
/// Because the token is move-only for writes yet freely borrowable for reads,
/// the borrow checker enforces single-writer / multi-reader discipline over the
/// whole region without a single atomic instruction or lock.
pub struct SyncRegionToken<'brand> {
    _invariant: InvariantLifetime<'brand>,
}

impl<'brand> SyncRegionToken<'brand> {
    /// Construct a region token without proving brand uniqueness.
    ///
    /// # Safety
    ///
    /// The caller must guarantee no other `SyncRegionToken<'brand>` for the same
    /// `'brand` exists. Prefer [`sync_region_scope`].
    #[inline]
    #[must_use]
    pub const unsafe fn new_unchecked() -> Self {
        Self {
            _invariant: PhantomData,
        }
    }

    /// Mint a `Copy`, `Send + Sync` [`SharedReadToken`] for concurrent reads.
    ///
    /// The returned token borrows `self` immutably for `'a`, so while any copy
    /// is live the region token cannot be borrowed mutably and no write permit
    /// can be formed. This is the supported way to fan a region's read
    /// capability out across worker threads.
    #[inline]
    #[must_use]
    pub fn share<'a>(&'a self) -> SharedReadToken<'a, 'brand> {
        // SAFETY: `self` is borrowed immutably for `'a`; the produced token
        // carries that window, preserving read/write exclusion for the brand.
        unsafe { SharedReadToken::new_unchecked() }
    }
}

impl<'brand> fmt::Debug for SyncRegionToken<'brand> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SyncRegionToken<'brand>")
    }
}

impl<'brand> Sealed for &SyncRegionToken<'brand> {}
impl<'brand> Sealed for &mut SyncRegionToken<'brand> {}

// SAFETY: the unique owning token mediates brand-wide XOR through the borrow
// checker exactly as `ExclusiveToken` does; `Send + Sync` merely permits the
// capability to travel across threads.
unsafe impl<'brand> ReadPermit<'brand> for &SyncRegionToken<'brand> {}
unsafe impl<'brand> ReadPermit<'brand> for &mut SyncRegionToken<'brand> {}
unsafe impl<'brand> WritePermit<'brand> for &mut SyncRegionToken<'brand> {}

/// Open a thread-portable branding scope.
///
/// The token handed to `f` is `Send + Sync`; together with
/// [`MelinoeCell`](crate::MelinoeCell)'s thread-safety impls this enables the
/// "send the token, share the cells" parallelism pattern used by region-based
/// allocators.
///
/// # Examples
///
/// ```
/// use melinoe::{sync::sync_region_scope, MelinoeCell};
///
/// let sum = sync_region_scope(|token| {
///     let cells = [MelinoeCell::new(1), MelinoeCell::new(2), MelinoeCell::new(3)];
///     // A single read permit fans out to every cell in the region.
///     cells.iter().map(|c| *c.borrow(&token)).sum::<i32>()
/// });
/// assert_eq!(sum, 6);
/// ```
#[inline]
pub fn sync_region_scope<R>(f: impl for<'brand> FnOnce(SyncRegionToken<'brand>) -> R) -> R {
    // SAFETY: `for<'brand>` yields a fresh invariant brand unique to this call,
    // so the token is the only `SyncRegionToken<'brand>` in existence.
    f(unsafe { SyncRegionToken::new_unchecked() })
}
