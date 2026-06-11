use core::fmt;
use core::marker::PhantomData;
use core::sync::atomic::Ordering;

use crate::token::{InvariantLifetime, ReadPermit, WritePermit};
use super::traits::{Atomic, AtomicInt};
use super::order::AtomicOrder;

/// A branded atomic whose access cost is conditional on the capability presented:
/// plain in the exclusive phase, atomic in the shared phase.
///
/// `#[repr(transparent)]` over the underlying atomic `A` (the brand marker is a
/// ZST), so it has the same size, alignment, and bit-validity as `A`. It is
/// `Send`/`Sync` exactly when `A` is (the standard atomics are both).
#[repr(transparent)]
pub struct BrandedAtomic<'brand, A: Atomic> {
    inner: A,
    _brand: InvariantLifetime<'brand>,
}

impl<'brand, A: Atomic> BrandedAtomic<'brand, A> {
    /// Create a branded atomic holding `value`, branded with the ambient `'brand`.
    #[inline]
    pub fn new(value: A::Value) -> Self {
        Self {
            inner: A::new_atomic(value),
            _brand: PhantomData,
        }
    }

    /// Reborrow an existing atomic as a branded atomic, in place — zero-copy.
    ///
    /// `#[repr(transparent)]` makes this a no-op cast: the same atomic, now gated
    /// by `'brand`'s phase discipline. Lets an allocator brand a counter it
    /// already owns (e.g. a field of a larger struct) without moving it.
    #[inline]
    #[must_use]
    pub fn from_mut(atomic: &mut A) -> &mut Self {
        // SAFETY: `Self` is `#[repr(transparent)]` over `A`; the unique `&mut A`
        // becomes a unique `&mut Self`, introducing no aliasing.
        unsafe { &mut *(atomic as *mut A as *mut Self) }
    }

    /// View the underlying atomic in the shared phase, gated by a read permit.
    ///
    /// This is a zero-copy interop boundary for code that already expects a
    /// standard-library atomic. The returned reference is tied to the permit
    /// borrow, so a plain exclusive phase cannot overlap while it is live.
    #[inline]
    #[must_use]
    pub fn as_atomic<'a, P>(&'a self, _permit: P) -> &'a A
    where
        P: ReadPermit<'brand> + 'a,
    {
        &self.inner
    }

    /// View the underlying atomic through unique ownership of the wrapper.
    #[inline]
    #[must_use]
    pub fn as_atomic_mut(&mut self) -> &mut A {
        &mut self.inner
    }

    /// Consume the wrapper, returning the underlying atomic without extracting
    /// the value.
    #[inline]
    #[must_use]
    pub fn into_atomic(self) -> A {
        self.inner
    }

    // ───────────────────────── exclusive phase (plain) ─────────────────────────

    /// Run `f` with plain, non-atomic `&mut` access, under a proof of exclusivity.
    ///
    /// Requires a [`WritePermit`] for `'brand`. No atomic op is issued: this is a
    /// bare borrow of the underlying value, sound because the write permit proves
    /// no other access to this brand can exist for the call.
    #[inline]
    pub fn with_exclusive<P, R>(&self, _permit: P, f: impl FnOnce(&mut A::Value) -> R) -> R
    where
        P: WritePermit<'brand>,
    {
        // SAFETY: a live `WritePermit<'brand>` is an exclusive borrow of the
        // brand's unique token; while held, no `ReadPermit` of this brand exists,
        // so no atomic op can touch this cell concurrently. The plain `&mut` is
        // therefore unaliased. `value_ptr` carries interior-mutable provenance.
        f(unsafe { &mut *self.inner.value_ptr() })
    }

    /// Plain, non-atomic load under a proof of exclusivity.
    #[inline]
    pub fn load_exclusive<P>(&self, permit: P) -> A::Value
    where
        P: WritePermit<'brand>,
    {
        self.with_exclusive(permit, |v| *v)
    }

    /// Plain, non-atomic store under a proof of exclusivity.
    #[inline]
    pub fn store_exclusive<P>(&self, value: A::Value, permit: P)
    where
        P: WritePermit<'brand>,
    {
        self.with_exclusive(permit, |v| *v = value);
    }

    /// Plain `&mut` access from unique ownership of the cell — no permit needed.
    #[inline]
    pub fn get_mut(&mut self) -> &mut A::Value {
        self.inner.atomic_get_mut()
    }

    /// Consume the cell, returning the contained value.
    #[inline]
    pub fn into_inner(self) -> A::Value {
        self.inner.atomic_into_inner()
    }

    // ────────────────────────── shared phase (atomic) ──────────────────────────

    /// Atomic load. Requires a [`ReadPermit`] for `'brand` (the shared phase).
    #[inline]
    pub fn load<P>(&self, _permit: P, order: Ordering) -> A::Value
    where
        P: ReadPermit<'brand>,
    {
        self.inner.atomic_load(order)
    }

    /// Atomic load using a compile-time ZST ordering policy.
    #[inline]
    pub fn load_with<P, O>(&self, _permit: P, _order: O) -> A::Value
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.inner.atomic_load(O::LOAD)
    }

    /// Atomic store. Requires a [`ReadPermit`] for `'brand`.
    #[inline]
    pub fn store<P>(&self, value: A::Value, _permit: P, order: Ordering)
    where
        P: ReadPermit<'brand>,
    {
        self.inner.atomic_store(value, order);
    }

    /// Atomic store using a compile-time ZST ordering policy.
    #[inline]
    pub fn store_with<P, O>(&self, value: A::Value, _permit: P, _order: O)
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.inner.atomic_store(value, O::STORE);
    }

    /// Atomic swap. Requires a [`ReadPermit`] for `'brand`.
    #[inline]
    pub fn swap<P>(&self, value: A::Value, _permit: P, order: Ordering) -> A::Value
    where
        P: ReadPermit<'brand>,
    {
        self.inner.atomic_swap(value, order)
    }

    /// Atomic swap using a compile-time ZST ordering policy.
    #[inline]
    pub fn swap_with<P, O>(&self, value: A::Value, _permit: P, _order: O) -> A::Value
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.inner.atomic_swap(value, O::RMW)
    }

    /// Atomic compare-and-exchange. Requires a [`ReadPermit`] for `'brand`.
    ///
    /// # Errors
    ///
    /// Returns `Err(current)` if the stored value did not equal `current`.
    #[inline]
    pub fn compare_exchange<P>(
        &self,
        current: A::Value,
        new: A::Value,
        success: Ordering,
        failure: Ordering,
        _permit: P,
    ) -> Result<A::Value, A::Value>
    where
        P: ReadPermit<'brand>,
    {
        self.inner
            .atomic_compare_exchange(current, new, success, failure)
    }

    /// Atomic compare-and-exchange using a compile-time ZST ordering policy.
    ///
    /// # Errors
    ///
    /// Returns `Err(current)` if the stored value did not equal `current`.
    #[inline]
    pub fn compare_exchange_with<P, O>(
        &self,
        current: A::Value,
        new: A::Value,
        _permit: P,
        _order: O,
    ) -> Result<A::Value, A::Value>
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.inner
            .atomic_compare_exchange(current, new, O::RMW, O::FAILURE)
    }
}

impl<'brand, A: AtomicInt> BrandedAtomic<'brand, A> {
    /// Atomic fetch-add. Requires a [`ReadPermit`] for `'brand` (the shared phase).
    #[inline]
    pub fn fetch_add<P>(&self, value: A::Value, _permit: P, order: Ordering) -> A::Value
    where
        P: ReadPermit<'brand>,
    {
        self.inner.atomic_fetch_add(value, order)
    }

    /// Atomic fetch-add using a compile-time ZST ordering policy.
    #[inline]
    pub fn fetch_add_with<P, O>(&self, value: A::Value, _permit: P, _order: O) -> A::Value
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.inner.atomic_fetch_add(value, O::RMW)
    }

    /// Atomic fetch-sub. Requires a [`ReadPermit`] for `'brand`.
    #[inline]
    pub fn fetch_sub<P>(&self, value: A::Value, _permit: P, order: Ordering) -> A::Value
    where
        P: ReadPermit<'brand>,
    {
        self.inner.atomic_fetch_sub(value, order)
    }

    /// Atomic fetch-sub using a compile-time ZST ordering policy.
    #[inline]
    pub fn fetch_sub_with<P, O>(&self, value: A::Value, _permit: P, _order: O) -> A::Value
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.inner.atomic_fetch_sub(value, O::RMW)
    }

    /// Atomic fetch-and. Requires a [`ReadPermit`] for `'brand`.
    #[inline]
    pub fn fetch_and<P>(&self, value: A::Value, _permit: P, order: Ordering) -> A::Value
    where
        P: ReadPermit<'brand>,
    {
        self.inner.atomic_fetch_and(value, order)
    }

    /// Atomic fetch-and using a compile-time ZST ordering policy.
    #[inline]
    pub fn fetch_and_with<P, O>(&self, value: A::Value, _permit: P, _order: O) -> A::Value
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.inner.atomic_fetch_and(value, O::RMW)
    }

    /// Atomic fetch-or. Requires a [`ReadPermit`] for `'brand`.
    #[inline]
    pub fn fetch_or<P>(&self, value: A::Value, _permit: P, order: Ordering) -> A::Value
    where
        P: ReadPermit<'brand>,
    {
        self.inner.atomic_fetch_or(value, order)
    }

    /// Atomic fetch-or using a compile-time ZST ordering policy.
    #[inline]
    pub fn fetch_or_with<P, O>(&self, value: A::Value, _permit: P, _order: O) -> A::Value
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.inner.atomic_fetch_or(value, O::RMW)
    }
}

impl<'brand, A: Atomic + fmt::Debug> fmt::Debug for BrandedAtomic<'brand, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BrandedAtomic").field(&self.inner).finish()
    }
}
