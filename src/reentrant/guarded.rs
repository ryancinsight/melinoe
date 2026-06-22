use crate::reentrant::error::Reentered;
use crate::reentrant::reset::Reset;
use core::cell::{Cell, UnsafeCell};

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
/// assert_eq!(
///     cache.enter(|_| cache.enter(|_| ())).unwrap(),
///     Err(melinoe::reentrant::Reentered)
/// );
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
