//! [`ThreadLocalToken`] — a brand confined to its originating thread.

use core::fmt;
use core::marker::PhantomData;

use crate::token::capability::private::Sealed;
use crate::token::{InvariantLifetime, ReadPermit, WritePermit};

/// A brand owner that is statically pinned to one thread.
///
/// `ThreadLocalToken` provides the same read/write permit interface as
/// [`ExclusiveToken`](crate::ExclusiveToken)—a `&` borrow is a [`ReadPermit`]
/// and a `&mut` borrow is a [`WritePermit`]—but it deliberately implements
/// neither [`Send`] nor [`Sync`] (it carries a `*const ()` phantom). The whole
/// capability, and therefore every cell it governs, is consequently un-sendable:
/// the compiler rejects any attempt to move the access right to another thread.
///
/// Use this brand for allocator metadata that must never leave its owning
/// thread—free lists, bump cursors, and other structures whose soundness rests
/// on single-thread confinement rather than synchronisation.
pub struct ThreadLocalToken<'brand> {
    _invariant: InvariantLifetime<'brand>,
    /// `*const ()` is `!Send + !Sync`, propagating thread-confinement to the token.
    _not_threadsafe: PhantomData<*const ()>,
}

impl<'brand> ThreadLocalToken<'brand> {
    /// Construct a thread-local token without proving brand uniqueness.
    ///
    /// # Safety
    ///
    /// The caller must guarantee no other `ThreadLocalToken<'brand>` for the
    /// same `'brand` exists. Prefer [`thread_local_scope`].
    #[inline]
    #[must_use]
    pub const unsafe fn new_unchecked() -> Self {
        Self {
            _invariant: PhantomData,
            _not_threadsafe: PhantomData,
        }
    }
}

impl<'brand> fmt::Debug for ThreadLocalToken<'brand> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ThreadLocalToken<'brand>")
    }
}

impl<'brand> Sealed for &ThreadLocalToken<'brand> {}
impl<'brand> Sealed for &mut ThreadLocalToken<'brand> {}

// SAFETY: identical reasoning to `&ExclusiveToken` / `&mut ExclusiveToken`—the
// unique owning token mediates brand-wide XOR through the borrow checker. The
// extra `!Send` posture only narrows where the capability may be used.
unsafe impl<'brand> ReadPermit<'brand> for &ThreadLocalToken<'brand> {}
unsafe impl<'brand> ReadPermit<'brand> for &mut ThreadLocalToken<'brand> {}
unsafe impl<'brand> WritePermit<'brand> for &mut ThreadLocalToken<'brand> {}

/// Open a thread-confined branding scope.
///
/// The token handed to `f` is `!Send`, so neither it nor any cell it governs
/// can be moved to another thread—confinement is proven at compile time.
///
/// # Examples
///
/// ```
/// use melinoe::{sync::thread_local_scope, MelinoeCell};
///
/// let total = thread_local_scope(|mut token| {
///     let counter = MelinoeCell::new(0_usize);
///     for _ in 0..5 {
///         *counter.borrow_mut(&mut token) += 1;
///     }
///     *counter.borrow(&token)
/// });
/// assert_eq!(total, 5);
/// ```
///
/// The token is `!Send`, so the compiler forbids moving the capability—or any
/// cell governed by it—onto another thread:
///
/// ```compile_fail
/// use melinoe::sync::thread_local_scope;
/// fn require_send<T: Send>(_: &T) {}
/// thread_local_scope(|token| {
///     require_send(&token); // ERROR: `ThreadLocalToken` is not `Send`
/// });
/// ```
#[inline]
pub fn thread_local_scope<R>(f: impl for<'brand> FnOnce(ThreadLocalToken<'brand>) -> R) -> R {
    // SAFETY: `for<'brand>` yields a fresh invariant brand unique to this call,
    // so the token is the only `ThreadLocalToken<'brand>` in existence.
    f(unsafe { ThreadLocalToken::new_unchecked() })
}
