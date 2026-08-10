use crate::reentrant::error::Reentered;
use crate::reentrant::reset::Reset;
use crate::token::{with_fresh_token, ExclusiveFamily, ExclusiveToken};
use core::cell::Cell;

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
    /// The flag is cleared when `f` returns, including across a panic unwinding
    /// through `f`.
    ///
    /// # Errors
    ///
    /// Returns [`Reentered`] without running `f` if the gate is already held on
    /// this thread (a re-entrant call) — callers take a fallback path.
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
    ///     assert_eq!(gate.enter(|_| ()), Err(melinoe::reentrant::Reentered));
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
        let _reset = Reset::acquire(&self.active)?;
        Ok(with_fresh_token::<ExclusiveFamily, _, _>(f))
    }
}
