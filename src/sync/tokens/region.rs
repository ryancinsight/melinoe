//! [`SyncRegionToken`] - a brand whose access right may cross threads.

use crate::token::brand::{define_brand_owner_token, SyncRegionMarker};

define_brand_owner_token!(SyncRegionToken, SyncRegionFamily, SyncRegionMarker, "SyncRegionToken<'brand>");

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
    crate::token::with_fresh_token::<SyncRegionFamily, _, _>(f)
}
