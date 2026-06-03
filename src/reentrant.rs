//! [`ReentrancyCell`] — a thread-confined gate that dispenses one exclusive
//! branded token at a time.
//!
//! Some exclusive state is *ambient* rather than lexically scoped: a thread's
//! allocator slot is touched on every `malloc` across the thread's whole
//! lifetime, so it cannot live inside a single [`brand_scope`](crate::brand_scope)
//! closure. The classic guard for such state is a hand-checked re-entrancy
//! boolean (`is_allocating`) wrapping a raw `UnsafeCell` — correct only by
//! audit.
//!
//! `ReentrancyCell` turns that boolean into a typed capability. [`enter`] checks
//! the flag once (the unavoidable runtime gate at the ambient boundary) and, on
//! success, hands the closure a fresh-brand [`ExclusiveToken`]. Every access
//! *inside* the closure is then compile-time-proven via that token; a re-entrant
//! [`enter`] returns [`Reentered`] instead of aliasing. The runtime cost is one
//! predictable branch at entry; the proof covers the entire body.
//!
//! [`enter`]: ReentrancyCell::enter
//!
//! # Soundness
//!
//! The flag guarantees that at most one token minted by a given cell is live at
//! a time (a nested `enter` returns `Err` *before* minting), and the
//! `for<'brand>` bound makes each token's brand fresh and non-escaping. Together
//! these discharge [`ExclusiveToken::new_unchecked`]'s contract. The cell holds a
//! [`Cell`] and is therefore `!Sync`: it is a per-thread gate, never shared
//! across threads, so the flag check needs no atomicity.

use core::cell::Cell;
use core::fmt;

use crate::token::ExclusiveToken;

/// Returned by [`ReentrancyCell::enter`] when the gate is already held — i.e. a
/// re-entrant call on the same thread. Callers take a fallback path rather than
/// aliasing the guarded state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Reentered;

impl fmt::Display for Reentered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("re-entrant ReentrancyCell::enter (gate already held on this thread)")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Reentered {}

/// A thread-confined gate yielding at most one exclusive branded token at a time.
///
/// Place one in thread-local storage to brand a thread's ambient exclusive state
/// (e.g. its allocator slot). `!Sync` by construction (it holds a [`Cell`]).
#[derive(Debug, Default)]
pub struct ReentrancyCell {
    active: Cell<bool>,
}

impl ReentrancyCell {
    /// Create an idle gate.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: Cell::new(false),
        }
    }

    /// Whether the gate is currently held (an `enter` is in progress).
    #[inline]
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active.get()
    }

    /// Acquire the gate and run `f` with a fresh-brand [`ExclusiveToken`].
    ///
    /// Returns `Err(`[`Reentered`]`)` without running `f` if the gate is already
    /// held (a re-entrant call). The flag is cleared when `f` returns, including
    /// across a panic unwinding through `f`.
    ///
    /// # Examples
    ///
    /// ```
    /// use melinoe::reentrant::ReentrancyCell;
    /// use melinoe::MelinoeCell;
    ///
    /// let gate = ReentrancyCell::new();
    ///
    /// let out = gate.enter(|mut token| {
    ///     // Ambient state, now token-gated with a compile-time exclusivity proof.
    ///     let slot = MelinoeCell::new(0_u64);
    ///     *slot.borrow_mut(&mut token) = 7;
    ///
    ///     // A re-entrant acquisition is refused, not aliased.
    ///     assert!(gate.enter(|_| ()).is_err());
    ///
    ///     *slot.borrow(&token)
    /// });
    /// assert_eq!(out, Ok(7));
    /// ```
    #[inline]
    pub fn enter<R>(
        &self,
        f: impl for<'brand> FnOnce(ExclusiveToken<'brand>) -> R,
    ) -> Result<R, Reentered> {
        if self.active.get() {
            return Err(Reentered);
        }
        self.active.set(true);
        let _reset = Reset(&self.active);
        // SAFETY: the flag (just set, and re-checked by any nested `enter`)
        // guarantees no other token minted by this cell is live, and `for<'brand>`
        // makes the brand fresh and non-escaping — so this is the unique
        // `ExclusiveToken` for its brand, satisfying `new_unchecked`.
        let token = unsafe { ExclusiveToken::new_unchecked() };
        Ok(f(token))
    }
}

/// Clears the gate flag on scope exit, including panic unwind.
struct Reset<'a>(&'a Cell<bool>);

impl Drop for Reset<'_> {
    #[inline]
    fn drop(&mut self) {
        self.0.set(false);
    }
}
