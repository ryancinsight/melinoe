//! Thread-synchronisation tokens and the cross-thread access model.
//!
//! The token families here differ from [`ExclusiveToken`](crate::ExclusiveToken)
//! solely in their auto-trait posture, which is exactly what governs *where* a
//! capability may travel:
//!
//! * [`ThreadLocalToken`] is `!Send + !Sync` — its brand is pinned to one
//!   thread, so soundness rests on confinement, not synchronisation.
//! * [`SyncRegionToken`] is `Send + Sync` — its brand may migrate between
//!   threads (single writer) or be shared for concurrent reads.
//!
//! # The cross-thread proof
//!
//! [`MelinoeCell<'brand, T>`](crate::MelinoeCell) is `Send` when `T: Send` and
//! `Sync` when `T: Send + Sync`. Combined with the cardinality guarantees of the
//! tokens, this yields two statically-checked parallelism shapes:
//!
//! * **Exclusive handoff.** Move a `SyncRegionToken<'brand>` to another thread
//!   to relocate the sole write capability for the region. No other thread can
//!   form a write permit because no other token exists.
//! * **Shared fan-out.** Share `&SyncRegionToken<'brand>` (or copies of a
//!   [`SharedReadToken`](crate::SharedReadToken)) across threads for concurrent
//!   reads; the absence of a live `&mut` token statically excludes writers.
//!
//! When the `std` feature is enabled, [`scope_exclusive`] demonstrates the
//! handoff pattern over [`std::thread::scope`], and [`PartitionPlan`] controls
//! lock-free disjoint write scheduling by fixed part count, reported hardware
//! parallelism, or fixed chunk size.

mod region;
mod thread_local;

pub use region::{sync_region_scope, SyncRegionToken};
pub use thread_local::{thread_local_scope, ThreadLocalToken};

#[cfg(feature = "std")]
mod scoped;
#[cfg(feature = "std")]
pub use scoped::scope_exclusive;

#[cfg(feature = "std")]
mod partition;
#[cfg(feature = "std")]
pub use partition::{
    partition_for_each, partition_for_each_available, partition_for_each_with, partition_map,
    partition_map_available, partition_map_with, register_parallel_executor, ParallelExecutorFn,
    PartitionPlan,
};
