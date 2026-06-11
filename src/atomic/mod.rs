//! [`BrandedAtomic`] — conditional atomics: plain access when exclusivity is
//! proven, atomic access only when sharing.

/// ZST ordering policies for atomic operations.
pub mod order;
/// Abstract atomic traits wrapping standard library primitives.
pub mod traits;
/// Branded atomic wrapper implementing conditional synchronization.
pub mod branded;

pub use order::{AcqRel, AtomicOrder, Relaxed, SeqCst};
pub use traits::{Atomic, AtomicInt};
pub use branded::BrandedAtomic;
