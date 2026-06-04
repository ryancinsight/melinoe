//! [`BrandedAtomic`] — conditional atomics: plain access when exclusivity is
//! proven, atomic access only when sharing.
//!
//! Atomics exist to make concurrent access to one location sound, but every
//! atomic op pays for that on *every* access — a locked RMW is ~30× a plain
//! store (see `benches/`). Much allocator state, though, has a *single-writer
//! phase* (a thread building a counter, initialising a slab) followed by a
//! *shared phase* (other threads read or CAS it). Paying atomic cost during the
//! exclusive phase is waste.
//!
//! `BrandedAtomic` removes that waste, the way [`Cow`](std::borrow::Cow) removes
//! a needless clone: the capability you present selects the access mode.
//!
//! * **Exclusive phase** — present a [`WritePermit`] (`&mut` the brand's owning
//!   token). Access is **plain**, non-atomic ([`with_exclusive`], [`store_exclusive`],
//!   [`load_exclusive`]) — a bare load/store, no synchronization.
//! * **Shared phase** — present a [`ReadPermit`] (`&` a shared token). Access is
//!   **atomic** ([`load`], [`store`], [`swap`], [`compare_exchange`],
//!   [`fetch_add`]…), usable concurrently from many threads.
//! * **Interop boundary** — [`as_atomic`] returns the underlying `Atomic*`
//!   reference under a read permit, for zero-copy integration with existing
//!   atomic APIs while the shared phase proof is live.
//!
//! [`with_exclusive`]: BrandedAtomic::with_exclusive
//! [`store_exclusive`]: BrandedAtomic::store_exclusive
//! [`load_exclusive`]: BrandedAtomic::load_exclusive
//! [`load`]: BrandedAtomic::load
//! [`as_atomic`]: BrandedAtomic::as_atomic
//! [`store`]: BrandedAtomic::store
//! [`swap`]: BrandedAtomic::swap
//! [`compare_exchange`]: BrandedAtomic::compare_exchange
//! [`fetch_add`]: BrandedAtomic::fetch_add
//!
//! # Soundness
//!
//! Plain and atomic access to one location must never overlap concurrently —
//! that is a data race regardless of "conditionality". The brand makes them
//! **temporally exclusive**: plain access borrows the token `&mut`, atomic access
//! borrows it `&`, and the borrow checker forbids holding both at once (even when
//! feeding [`std::thread::scope`]). So a plain write can never race an atomic op;
//! within the shared phase only atomics touch the cell, which is sound by
//! definition. The cross-thread *visibility* at the phase boundary is supplied by
//! the mechanism that hands the capability over (a scope join, a channel) plus
//! the atomic ops' own ordering — the brand proves the discipline, not the
//! happens-before. This module is verified data-race-free under Miri.

use core::fmt;
use core::marker::PhantomData;
use core::sync::atomic::{
    AtomicBool, AtomicI32, AtomicI64, AtomicIsize, AtomicU32, AtomicU64, AtomicUsize, Ordering,
};

use crate::token::{InvariantLifetime, ReadPermit, WritePermit};

mod sealed {
    pub trait Sealed {}
}

/// ZST ordering policy for atomic load/store/swap/fetch operations.
///
/// Use this when the ordering contract is fixed by the algorithm. The policy is
/// a zero-sized type; monomorphization substitutes the associated constants at
/// compile time. The trait is sealed so downstream code cannot introduce an
/// ordering combination outside this crate's audited policy set.
pub trait AtomicOrder: sealed::Sealed + Copy {
    /// Ordering for load operations.
    const LOAD: Ordering;
    /// Ordering for store operations.
    const STORE: Ordering;
    /// Ordering for read-modify-write operations.
    const RMW: Ordering;
    /// Failure ordering for compare-exchange operations.
    const FAILURE: Ordering;
}

/// Relaxed atomic ordering policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Relaxed;

/// Acquire load / release store / acquire-release RMW ordering policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcqRel;

/// Sequentially consistent ordering policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeqCst;

impl AtomicOrder for Relaxed {
    const LOAD: Ordering = Ordering::Relaxed;
    const STORE: Ordering = Ordering::Relaxed;
    const RMW: Ordering = Ordering::Relaxed;
    const FAILURE: Ordering = Ordering::Relaxed;
}

impl AtomicOrder for AcqRel {
    const LOAD: Ordering = Ordering::Acquire;
    const STORE: Ordering = Ordering::Release;
    const RMW: Ordering = Ordering::AcqRel;
    const FAILURE: Ordering = Ordering::Acquire;
}

impl AtomicOrder for SeqCst {
    const LOAD: Ordering = Ordering::SeqCst;
    const STORE: Ordering = Ordering::SeqCst;
    const RMW: Ordering = Ordering::SeqCst;
    const FAILURE: Ordering = Ordering::SeqCst;
}

/// An atomic primitive abstracted over its value type, so [`BrandedAtomic`] is
/// one generic implementation rather than a type-numbered family.
///
/// Sealed: implemented only for the standard-library atomics. The methods are
/// the crate-internal mediation surface; users call [`BrandedAtomic`].
pub trait Atomic: sealed::Sealed {
    /// The plain value carried by this atomic (e.g. `u64` for `AtomicU64`).
    type Value: Copy;

    #[doc(hidden)]
    fn new_atomic(value: Self::Value) -> Self;
    #[doc(hidden)]
    fn atomic_load(&self, order: Ordering) -> Self::Value;
    #[doc(hidden)]
    fn atomic_store(&self, value: Self::Value, order: Ordering);
    #[doc(hidden)]
    fn atomic_swap(&self, value: Self::Value, order: Ordering) -> Self::Value;
    #[doc(hidden)]
    fn atomic_compare_exchange(
        &self,
        current: Self::Value,
        new: Self::Value,
        success: Ordering,
        failure: Ordering,
    ) -> Result<Self::Value, Self::Value>;
    #[doc(hidden)]
    fn atomic_get_mut(&mut self) -> &mut Self::Value;
    #[doc(hidden)]
    fn atomic_into_inner(self) -> Self::Value;
    /// Pointer to the underlying value for plain (non-atomic) access.
    ///
    /// Sound to dereference only under a proof of exclusivity. An atomic has the
    /// same size/bit-validity as its value and interior mutability, so the cast
    /// is layout-valid and the pointer carries interior-mutable provenance.
    #[doc(hidden)]
    fn value_ptr(&self) -> *mut Self::Value;
}

impl sealed::Sealed for Relaxed {}
impl sealed::Sealed for AcqRel {}
impl sealed::Sealed for SeqCst {}

/// Integer atomics, which additionally support arithmetic/bitwise RMW.
pub trait AtomicInt: Atomic {
    #[doc(hidden)]
    fn atomic_fetch_add(&self, value: Self::Value, order: Ordering) -> Self::Value;
    #[doc(hidden)]
    fn atomic_fetch_sub(&self, value: Self::Value, order: Ordering) -> Self::Value;
    #[doc(hidden)]
    fn atomic_fetch_and(&self, value: Self::Value, order: Ordering) -> Self::Value;
    #[doc(hidden)]
    fn atomic_fetch_or(&self, value: Self::Value, order: Ordering) -> Self::Value;
}

// A contained macro generates the trait forwards for the integer atomics. This
// is the textbook unavoidable per-type atomic boilerplate (each std atomic is a
// distinct type with identical method shapes); a single local macro keeps the
// ~10 trivial forwards per type in one place and prevents copy-paste divergence,
// which is preferable here to four hand-duplicated impl blocks.
macro_rules! impl_atomic_int {
    ($atomic:ty, $value:ty) => {
        impl sealed::Sealed for $atomic {}

        impl Atomic for $atomic {
            type Value = $value;

            #[inline]
            fn new_atomic(value: $value) -> Self {
                <$atomic>::new(value)
            }
            #[inline]
            fn atomic_load(&self, order: Ordering) -> $value {
                self.load(order)
            }
            #[inline]
            fn atomic_store(&self, value: $value, order: Ordering) {
                self.store(value, order);
            }
            #[inline]
            fn atomic_swap(&self, value: $value, order: Ordering) -> $value {
                self.swap(value, order)
            }
            #[inline]
            fn atomic_compare_exchange(
                &self,
                current: $value,
                new: $value,
                success: Ordering,
                failure: Ordering,
            ) -> Result<$value, $value> {
                self.compare_exchange(current, new, success, failure)
            }
            #[inline]
            fn atomic_get_mut(&mut self) -> &mut $value {
                self.get_mut()
            }
            #[inline]
            fn atomic_into_inner(self) -> $value {
                self.into_inner()
            }
            #[inline]
            fn value_ptr(&self) -> *mut $value {
                // SAFETY of later deref: same layout as `$value`, interior-mutable.
                self as *const Self as *mut $value
            }
        }

        impl AtomicInt for $atomic {
            #[inline]
            fn atomic_fetch_add(&self, value: $value, order: Ordering) -> $value {
                self.fetch_add(value, order)
            }
            #[inline]
            fn atomic_fetch_sub(&self, value: $value, order: Ordering) -> $value {
                self.fetch_sub(value, order)
            }
            #[inline]
            fn atomic_fetch_and(&self, value: $value, order: Ordering) -> $value {
                self.fetch_and(value, order)
            }
            #[inline]
            fn atomic_fetch_or(&self, value: $value, order: Ordering) -> $value {
                self.fetch_or(value, order)
            }
        }
    };
}

impl_atomic_int!(AtomicUsize, usize);
impl_atomic_int!(AtomicIsize, isize);
impl_atomic_int!(AtomicU32, u32);
impl_atomic_int!(AtomicI32, i32);
#[cfg(target_has_atomic = "64")]
impl_atomic_int!(AtomicU64, u64);
#[cfg(target_has_atomic = "64")]
impl_atomic_int!(AtomicI64, i64);

impl sealed::Sealed for AtomicBool {}

impl Atomic for AtomicBool {
    type Value = bool;

    #[inline]
    fn new_atomic(value: bool) -> Self {
        Self::new(value)
    }
    #[inline]
    fn atomic_load(&self, order: Ordering) -> bool {
        self.load(order)
    }
    #[inline]
    fn atomic_store(&self, value: bool, order: Ordering) {
        self.store(value, order);
    }
    #[inline]
    fn atomic_swap(&self, value: bool, order: Ordering) -> bool {
        self.swap(value, order)
    }
    #[inline]
    fn atomic_compare_exchange(
        &self,
        current: bool,
        new: bool,
        success: Ordering,
        failure: Ordering,
    ) -> Result<bool, bool> {
        self.compare_exchange(current, new, success, failure)
    }
    #[inline]
    fn atomic_get_mut(&mut self) -> &mut bool {
        self.get_mut()
    }
    #[inline]
    fn atomic_into_inner(self) -> bool {
        self.into_inner()
    }
    #[inline]
    fn value_ptr(&self) -> *mut bool {
        self as *const Self as *mut bool
    }
}

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
    pub fn load_with<P, O>(&self, permit: P, _order: O) -> A::Value
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.load(permit, O::LOAD)
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
    pub fn store_with<P, O>(&self, value: A::Value, permit: P, _order: O)
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.store(value, permit, O::STORE);
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
    pub fn swap_with<P, O>(&self, value: A::Value, permit: P, _order: O) -> A::Value
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.swap(value, permit, O::RMW)
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
        permit: P,
        _order: O,
    ) -> Result<A::Value, A::Value>
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.compare_exchange(current, new, O::RMW, O::FAILURE, permit)
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
    pub fn fetch_add_with<P, O>(&self, value: A::Value, permit: P, _order: O) -> A::Value
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.fetch_add(value, permit, O::RMW)
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
    pub fn fetch_sub_with<P, O>(&self, value: A::Value, permit: P, _order: O) -> A::Value
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.fetch_sub(value, permit, O::RMW)
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
    pub fn fetch_and_with<P, O>(&self, value: A::Value, permit: P, _order: O) -> A::Value
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.fetch_and(value, permit, O::RMW)
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
    pub fn fetch_or_with<P, O>(&self, value: A::Value, permit: P, _order: O) -> A::Value
    where
        P: ReadPermit<'brand>,
        O: AtomicOrder,
    {
        self.fetch_or(value, permit, O::RMW)
    }
}

impl<'brand, A: Atomic + fmt::Debug> fmt::Debug for BrandedAtomic<'brand, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BrandedAtomic").field(&self.inner).finish()
    }
}
