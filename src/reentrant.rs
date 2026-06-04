//! Re-entrancy-guarded access to *ambient*, thread-confined exclusive state.
//!
//! Two primitives, both gating with a single `!Sync` flag: [`ReentrancyCell`]
//! yields a fresh-brand [`ExclusiveToken`] (for ephemeral branded sub-state),
//! and [`GuardedCell`] owns a value and yields `&mut T` directly (for persistent
//! state like a thread's allocator cache). Both refuse re-entry rather than
//! aliasing and clear their flag on panic.
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

use core::cell::{Cell, UnsafeCell};
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
        let _reset = Reset::acquire(&self.active)?;
        // SAFETY: the flag (set by `acquire`, re-checked by any nested `enter`)
        // guarantees no other token minted by this cell is live, and `for<'brand>`
        // makes the brand fresh and non-escaping — so this is the unique
        // `ExclusiveToken` for its brand, satisfying `new_unchecked`.
        let token = unsafe { ExclusiveToken::new_unchecked() };
        Ok(f(token))
    }
}

/// A thread-confined cell that hands out one exclusive `&mut T` at a time.
///
/// This is the value-owning counterpart to [`ReentrancyCell`]: it brands
/// *ambient, persistent* exclusive state — a thread's allocator cache, a
/// per-thread arena cursor — that is touched across a whole thread's lifetime
/// and therefore cannot live inside a [`brand_scope`](crate::brand_scope)
/// closure. [`enter`](Self::enter) checks the re-entrancy flag once and yields a
/// borrow-checked `&mut T`; the `&mut` *is* the compile-time exclusivity proof,
/// and re-entry is refused rather than aliased.
///
/// It is the panic-safe, audited replacement for the hand-rolled
/// `UnsafeCell<T>` + `is_allocating: bool` idiom: the flag is cleared by a drop
/// guard even if `f` unwinds, so a panic cannot poison the cell. `!Sync` by
/// construction; the single `unsafe` deref is centralised and discharged here.
///
/// # Examples
///
/// ```
/// use melinoe::reentrant::GuardedCell;
///
/// let cache = GuardedCell::new(0_u64);
/// assert_eq!(cache.enter(|n| { *n += 41; *n }), Ok(41));
/// // Re-entrant access is refused, not aliased:
/// assert!(cache.enter(|_| cache.enter(|_| ())).unwrap().is_err());
/// ```
#[derive(Debug, Default)]
pub struct GuardedCell<T: ?Sized> {
    active: Cell<bool>,
    value: UnsafeCell<T>,
}

impl<T> GuardedCell<T> {
    /// Wrap `value` in an idle guarded cell.
    #[inline]
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self {
            active: Cell::new(false),
            value: UnsafeCell::new(value),
        }
    }

    /// Consume the cell, returning the contained value.
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<T: ?Sized> GuardedCell<T> {
    /// Whether a borrow is currently in progress on this thread.
    #[inline]
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active.get()
    }

    /// Run `f` with exclusive `&mut T`.
    ///
    /// The flag is cleared when `f` returns, including across a panic.
    ///
    /// # Errors
    ///
    /// Returns [`Reentered`] without running `f` if a borrow is already in
    /// progress on this thread.
    #[inline]
    pub fn enter<R>(&self, f: impl FnOnce(&mut T) -> R) -> Result<R, Reentered> {
        let _reset = Reset::acquire(&self.active)?;
        // SAFETY: the flag (set by `acquire`, re-checked by any nested `enter`)
        // plus the cell's `!Sync` thread confinement guarantee no other `&mut T`
        // to this value is live, so this borrow is unaliased for the call.
        let value = unsafe { &mut *self.value.get() };
        Ok(f(value))
    }

    /// Run `f` with `&mut T` **without** arming the guard.
    ///
    /// Skips the flag writes that bracket [`enter`](Self::enter), for a hot path
    /// where `f` is statically known not to re-enter.
    ///
    /// # Errors
    ///
    /// Returns [`Reentered`] without running `f` if a guarded borrow is already
    /// in progress on this thread.
    ///
    /// # Safety
    ///
    /// `f` must not, directly or transitively, call [`enter`](Self::enter) or
    /// `enter_unguarded` on this cell (which would create an aliasing `&mut T`).
    #[inline]
    pub unsafe fn enter_unguarded<R>(&self, f: impl FnOnce(&mut T) -> R) -> Result<R, Reentered> {
        if self.active.get() {
            return Err(Reentered);
        }
        // SAFETY: the flag is clear, so no guarded `&mut T` is live; the cell is
        // thread-confined (`!Sync`); and the caller's contract forbids re-entry,
        // so no nested `&mut T` can be created during this borrow.
        let value = unsafe { &mut *self.value.get() };
        Ok(f(value))
    }

    /// Raw pointer to the contents (e.g. for use as a stable owner token).
    ///
    /// Dereferencing it is subject to the same exclusivity contract as
    /// [`enter`](Self::enter); prefer the safe methods.
    #[inline]
    #[must_use]
    pub const fn as_ptr(&self) -> *mut T {
        self.value.get()
    }

    /// Acquire `&mut T` from unique ownership — no flag, no check needed.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }
}

// SAFETY: moving the cell moves `T`; sound exactly when `T: Send`. The cell is
// deliberately not `Sync` (it holds `Cell`/`UnsafeCell`): it is a per-thread gate.
unsafe impl<T: ?Sized + Send> Send for GuardedCell<T> {}

/// Holds a gate flag `true` for its lifetime and clears it on scope exit,
/// including a panic unwind. The single point where the re-entrancy flag is
/// acquired and released, shared by both gate types.
struct Reset<'a>(&'a Cell<bool>);

impl<'a> Reset<'a> {
    /// Arm the gate, returning the clearing guard, or [`Reentered`] if it is
    /// already held.
    #[inline]
    fn acquire(active: &'a Cell<bool>) -> Result<Self, Reentered> {
        if active.get() {
            return Err(Reentered);
        }
        active.set(true);
        Ok(Self(active))
    }
}

impl Drop for Reset<'_> {
    #[inline]
    fn drop(&mut self) {
        self.0.set(false);
    }
}
