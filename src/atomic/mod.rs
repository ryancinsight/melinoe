//! [`BrandedAtomic`] — conditional atomics: plain access when exclusivity is
//! proven, atomic access only when sharing.

/// Branded atomic wrapper implementing conditional synchronization.
pub mod branded;
/// ZST ordering policies for atomic operations.
pub mod order;
/// Abstract atomic traits wrapping standard library primitives.
pub mod traits;

pub use branded::BrandedAtomic;
pub use order::{AcqRel, AtomicOrder, Relaxed, SeqCst};
pub use traits::{Atomic, AtomicInt};
