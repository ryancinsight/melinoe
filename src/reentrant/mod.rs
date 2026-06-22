//! Re-entrancy-guarded access to *ambient*, thread-confined exclusive state.
//!
//! Two primitives, both gating with a single `!Sync` flag: [`ReentrancyCell`]
//! yields a fresh-brand [`ExclusiveToken`](crate::token::ExclusiveToken) (for ephemeral branded sub-state),
//! and [`GuardedCell`] owns a value and yields `&mut T` directly (for persistent
//! state like a thread's allocator cache). Both refuse re-entry rather than
//! aliasing and clear their flag on panic.
//!
//! Some exclusive state is *ambient* rather than lexically scoped: a thread's
//! allocator slot is touched on every `malloc` across the thread's whole
//! lifetime, so it cannot live inside a single [`brand_scope`](crate::token::brand_scope)
//! closure. The classic guard for such state is a hand-checked re-entrancy
//! boolean (`is_allocating`) wrapping a raw `UnsafeCell` — correct only by
//! audit.
//!
//! `ReentrancyCell` turns that boolean into a typed capability. [`enter`] checks
//! the flag once (the unavoidable runtime gate at the ambient boundary) and, on
//! success, hands the closure a fresh-brand [`ExclusiveToken`](crate::token::ExclusiveToken). Every access
//! *inside* the closure is then compile-time-proven via that token; a re-entrant
//! [`enter`] returns [`Reentered`] instead of aliasing. The runtime cost is one
//! predictable branch at entry; the proof covers the entire body.
//!
//! [`enter`]: ReentrancyCell::enter

mod error;
mod gate;
mod guarded;
mod reset;

pub use error::Reentered;
pub use gate::ReentrancyCell;
pub use guarded::GuardedCell;
