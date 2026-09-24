//! [`ThreadLocalToken`] - a brand confined to its originating thread.

use crate::token::brand::{define_brand_owner_token, ThreadLocalMarker};

define_brand_owner_token!(ThreadLocalToken, ThreadLocalFamily, ThreadLocalMarker, "ThreadLocalToken<'brand>");

/// Open a thread-confined branding scope.
///
/// The token handed to `f` is `!Send`, so neither it nor any cell it governs
/// can be moved to another thread - confinement is proven at compile time.
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
/// The token is `!Send`, so the compiler forbids moving the capability - or any
/// cell governed by it - onto another thread:
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
    crate::token::with_fresh_token::<ThreadLocalFamily, _, _>(f)
}
