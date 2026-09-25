//! [`ThreadLocalToken`] — a brand confined to its originating thread.

use core::marker::PhantomData;

use crate::token::{brand_owner_token, with_fresh_token};

brand_owner_token! {
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
    name: ThreadLocalToken;
    /// Token-family selector for thread-confined scopes.
    family: ThreadLocalFamily;
    marker_extra: PhantomData<*const ()>;
    marker_init: PhantomData;
    debug: "ThreadLocalToken<'brand>";
    share: no;
}

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
    with_fresh_token::<ThreadLocalFamily, _, _>(f)
}
