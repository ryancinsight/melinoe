//! [`SyncRegionToken`] — a brand whose access right may cross threads.

use crate::token::{brand_owner_token, with_fresh_token};

brand_owner_token! {
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
    ///
    /// # Device-buffer ownership transfer
    ///
    /// A device-buffer owner can store the backend's real buffer handle in a
    /// [`MelinoeCell`](crate::MelinoeCell) and require `SyncRegionToken<'brand>` by
    /// value on the host/device boundary. Moving the token into that boundary
    /// transfers the sole write capability to the code that records the stream or
    /// queue operation. Returning the token after submission or synchronization
    /// restores host-side exclusive capability; borrowing it immutably, or calling
    /// [`share`](Self::share), switches to shared readback/observer capability.
    name: SyncRegionToken;
    /// Token-family selector for cross-thread region scopes.
    family: SyncRegionFamily;
    marker_extra: ();
    marker_init: ();
    debug: "SyncRegionToken<'brand>";
    share: yes;
}

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
    with_fresh_token::<SyncRegionFamily, _, _>(f)
}
