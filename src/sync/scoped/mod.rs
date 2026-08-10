//! `std`-gated demonstration of cross-thread exclusive handoff and partition execution.

mod partition;

pub use partition::{
    clear_parallel_executor, partition_for_each, partition_for_each_available,
    partition_for_each_with, partition_map, partition_map_available, partition_map_with,
    partition_read_for_each, partition_read_for_each_available, partition_read_for_each_with,
    partition_read_map, partition_read_map_available, partition_read_map_with,
    register_parallel_executor, ParallelExecutor, PartitionPlan,
};

use super::tokens::SyncRegionFamily;
use super::SyncRegionToken;
use crate::token::with_fresh_token;

/// Run a branded computation on a freshly spawned worker thread, transferring
/// exclusive write capability across the thread boundary.
///
/// This is the executable proof of the *exclusive handoff* pattern described in
/// the [module documentation](crate::sync): the [`SyncRegionToken`] (and every
/// [`MelinoeCell`](crate::MelinoeCell) `f` mints under its brand) is created and
/// consumed entirely on the worker thread, statically guaranteeing that no
/// other thread holds a competing write capability for the brand.
///
/// `f` is universally quantified over `'brand`, so the brand cannot escape the
/// worker; `R: Send` ensures the result can return to the caller.
///
/// # Panics
///
/// Propagates (re-raises) any panic that unwinds out of `f` on the worker
/// thread, preserving the original panic payload.
///
/// # Examples
///
/// ```
/// use melinoe::{sync::scope_exclusive, MelinoeCell};
///
/// let result = scope_exclusive(|mut token| {
///     let cell = MelinoeCell::new(10_i64);
///     *cell.borrow_mut(&mut token) += 32;
///     *cell.borrow(&token)
/// });
/// assert_eq!(result, 42);
/// ```
#[inline]
pub fn scope_exclusive<R, F>(f: F) -> R
where
    R: Send,
    F: Send + for<'brand> FnOnce(SyncRegionToken<'brand>) -> R,
{
    std::thread::scope(|scope| {
        let handle = scope.spawn(|| with_fresh_token::<SyncRegionFamily, _, _>(f));
        match handle.join() {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    })
}
