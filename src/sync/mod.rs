//! Thread-synchronisation tokens and the cross-thread access model.
//!
//! The token families here differ from [`ExclusiveToken`](crate::token::ExclusiveToken)
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
//!   [`SharedReadToken`](crate::token::SharedReadToken)) across threads for concurrent
//!   reads; the absence of a live `&mut` token statically excludes writers.
//!
//! When the `std` feature is enabled, [`scope_exclusive`] demonstrates the
//! handoff pattern over [`std::thread::scope`], and [`PartitionPlan`] controls
//! lock-free disjoint write scheduling by fixed part count, reported hardware
//! parallelism, or fixed chunk size.
//!
//! # Device-buffer ownership transfer
//!
//! Accelerator buffers follow the same pattern as host slabs: store the real
//! backend buffer handle in a [`MelinoeCell`](crate::MelinoeCell), move the
//! [`SyncRegionToken`] into the subsystem that records the device write, and
//! return the token only after the stream is submitted or synchronized. The
//! token move is the ownership-transfer proof; Melinoe does not wrap or mock
//! the backend buffer.
//!
//! After submission, [`SyncRegionToken::share`] mints a
//! [`SharedReadToken`](crate::SharedReadToken) for concurrent readback,
//! validation, or observer streams. Fence and completion counters that are
//! touched from both phases use [`BrandedAtomic`](crate::BrandedAtomic): plain
//! access under the returned write token, atomic access under shared read
//! tokens.

mod tokens;

pub use tokens::{sync_region_scope, thread_local_scope, SyncRegionToken, ThreadLocalToken};

#[cfg(feature = "std")]
mod scoped;

#[cfg(feature = "std")]
pub use scoped::{
    clear_parallel_executor, partition_for_each, partition_for_each_available,
    partition_for_each_with, partition_map, partition_map_available, partition_map_with,
    partition_read_for_each, partition_read_for_each_available, partition_read_for_each_with,
    partition_read_map, partition_read_map_available, partition_read_map_with,
    register_parallel_executor, scope_exclusive, ParallelExecutor, PartitionPlan,
};
