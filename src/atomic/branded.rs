use core::fmt;
use core::marker::PhantomData;
use core::sync::atomic::Ordering;

use super::order::{AtomicOrder, OrderingSource};
use super::traits::Atomic;
use crate::token::{InvariantLifetime, ReadPermit, WritePermit};

/// A branded atomic whose access cost is conditional on the capability presented:
/// plain in the exclusive phase, atomic in the shared phase.
///
/// `#[repr(transparent)]` over the underlying atomic `A` (the brand marker is a
/// ZST), so it has the same size, alignment, and bit-validity as `A`. It is
/// `Send`/`Sync` exactly when `A` is (the standard atomics are both).
///
/// For device-buffer ownership transfers, use `BrandedAtomic` for fence,
/// generation, and completion counters that are written by the exclusive owner
/// before or after stream submission and read by shared observers. The exclusive
/// phase uses plain access under a [`WritePermit`]; the shared phase uses the
/// real atomic operations under a [`ReadPermit`].
#[repr(transparent)]
pub struct BrandedAtomic<'brand, A: Atomic> {
    inner: A,
    _brand: InvariantLifetime<'brand>,
}

/// Emit a runtime-[`Ordering`] method and its compile-time-ZST twin from one
/// body.
///
/// Each pair of entry points differs only in how its ordering is supplied: the
/// base method takes a runtime [`Ordering`], while the `_with` twin takes a
/// sealed [`AtomicOrder`] policy. Both funnel their ordering through the
/// [`OrderingSource`] helpers, so the operation — and its meaning — is written
/// once in the invocation and forwarded into both method bodies. The public
/// two-entry surface, both signatures and both doc comments, is taken verbatim.
///
/// The two arms cover the two shapes in this module: a single ordering
/// parameter shared by both entries, and the compare-exchange/fetch-update shape
/// where the runtime form takes two `Ordering`s that the policy form collapses
/// to one.
macro_rules! order_pair {
    // ── One ordering parameter, shared by both entries ──
    (
        $(#[$doc:meta])*
        fn $name:ident;
        $(#[$doc_with:meta])*
        fn $name_with:ident;
        before: ( $( $arg:ident : $argty:ty ),* $(,)? );
        after: ( $( $tail:ident : $tailty:ty ),* $(,)? );
        generics: ( $( $extra:ident ),* $(,)? );
        bounds: ( $( [ $($bound:tt)* ] ),* $(,)? );
        order: $order_name:ident;
        ret: $ret:ty;
        body: $($body:tt)*
    ) => {
        $(#[$doc])*
        #[inline]
        pub fn $name<P $(, $extra)*>(
            &self,
            $( $arg: $argty, )*
            _permit: P,
            $order_name: Ordering,
            $( $tail: $tailty, )*
        ) -> $ret
        where
            P: ReadPermit<'brand>
            $(, $($bound)*)*
        {
            self.$($body)*
        }

        $(#[$doc_with])*
        #[inline]
        pub fn $name_with<P, O $(, $extra)*>(
            &self,
            $( $arg: $argty, )*
            _permit: P,
            $order_name: O,
            $( $tail: $tailty, )*
        ) -> $ret
        where
            P: ReadPermit<'brand>,
            O: AtomicOrder
            $(, $($bound)*)*
        {
            self.$($body)*
        }
    };

    // ── Two runtime orderings, collapsed to one policy by the `_with` twin ──
    (
        $(#[$doc:meta])*
        fn $name:ident;
        $(#[$doc_with:meta])*
        fn $name_with:ident;
        before: ( $( $arg:ident : $argty:ty ),* $(,)? );
        orders: ( $order_a:ident , $order_b:ident );
        after: ( $( $tail:ident : $tailty:ty ),* $(,)? );
        generics: ( $( $extra:ident ),* $(,)? );
        bounds: ( $( [ $($bound:tt)* ] ),* $(,)? );
        ret: $ret:ty;
        body: $($body:tt)*
    ) => {
        $(#[$doc])*
        #[inline]
        pub fn $name<P $(, $extra)*>(
            &self,
            $( $arg: $argty, )*
            $order_a: Ordering,
            $order_b: Ordering,
            _permit: P,
            $( $tail: $tailty, )*
        ) -> $ret
        where
            P: ReadPermit<'brand>
            $(, $($bound)*)*
        {
            self.$($body)*
        }

        $(#[$doc_with])*
        #[inline]
        pub fn $name_with<P, O $(, $extra)*>(
            &self,
            $( $arg: $argty, )*
            _permit: P,
            order: O,
            $( $tail: $tailty, )*
        ) -> $ret
        where
            P: ReadPermit<'brand>,
            O: AtomicOrder
            $(, $($bound)*)*
        {
            let $order_a = order;
            let $order_b = order;
            self.$($body)*
        }
    };
}

mod integer;

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
        unsafe { &mut *core::ptr::from_mut(atomic).cast::<Self>() }
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

    #[inline]
    fn load_ordered<O: OrderingSource>(&self, order: O) -> A::Value {
        self.inner.atomic_load(order.load_order())
    }

    #[inline]
    fn store_ordered<O: OrderingSource>(&self, value: A::Value, order: O) {
        self.inner.atomic_store(value, order.store_order());
    }

    #[inline]
    fn swap_ordered<O: OrderingSource>(&self, value: A::Value, order: O) -> A::Value {
        self.inner.atomic_swap(value, order.rmw_order())
    }

    #[inline]
    fn compare_exchange_ordered<Success, Failure>(
        &self,
        current: A::Value,
        new: A::Value,
        success: Success,
        failure: Failure,
    ) -> Result<A::Value, A::Value>
    where
        Success: OrderingSource,
        Failure: OrderingSource,
    {
        self.inner.atomic_compare_exchange(
            current,
            new,
            success.rmw_order(),
            failure.failure_order(),
        )
    }

    #[inline]
    fn fetch_update_ordered<SetOrder, FetchOrder, F>(
        &self,
        set_order: SetOrder,
        fetch_order: FetchOrder,
        f: F,
    ) -> Result<A::Value, A::Value>
    where
        SetOrder: OrderingSource,
        FetchOrder: OrderingSource,
        F: FnMut(A::Value) -> Option<A::Value>,
    {
        self.inner
            .atomic_fetch_update(set_order.rmw_order(), fetch_order.failure_order(), f)
    }

    order_pair! {
        /// Atomic load. Requires a [`ReadPermit`] for `'brand` (the shared phase).
        fn load;
        /// Atomic load using a compile-time ZST ordering policy.
        fn load_with;
        before: ();
        after: ();
        generics: ();
        bounds: ();
        order: order;
        ret: A::Value;
        body: load_ordered(order)
    }

    order_pair! {
        /// Atomic store. Requires a [`ReadPermit`] for `'brand`.
        fn store;
        /// Atomic store using a compile-time ZST ordering policy.
        fn store_with;
        before: (value: A::Value);
        after: ();
        generics: ();
        bounds: ();
        order: order;
        ret: ();
        body: store_ordered(value, order)
    }

    order_pair! {
        /// Atomic swap. Requires a [`ReadPermit`] for `'brand`.
        fn swap;
        /// Atomic swap using a compile-time ZST ordering policy.
        fn swap_with;
        before: (value: A::Value);
        after: ();
        generics: ();
        bounds: ();
        order: order;
        ret: A::Value;
        body: swap_ordered(value, order)
    }

    order_pair! {
        /// Atomic compare-and-exchange. Requires a [`ReadPermit`] for `'brand`.
        ///
        /// # Errors
        ///
        /// Returns `Err(current)` if the stored value did not equal `current`.
        fn compare_exchange;
        /// Atomic compare-and-exchange using a compile-time ZST ordering policy.
        ///
        /// # Errors
        ///
        /// Returns `Err(current)` if the stored value did not equal `current`.
        fn compare_exchange_with;
        before: (current: A::Value, new: A::Value);
        orders: (success, failure);
        after: ();
        generics: ();
        bounds: ();
        ret: Result<A::Value, A::Value>;
        body: compare_exchange_ordered(current, new, success, failure)
    }

    order_pair! {
        /// Atomic fetch-update. Requires a [`ReadPermit`] for `'brand`.
        ///
        /// # Errors
        ///
        /// Returns `Err` with the last read value when `f` returns `None`, matching
        /// the `core::sync::atomic` `fetch_update` contract.
        fn fetch_update;
        /// Atomic fetch-update using a compile-time ZST ordering policy.
        ///
        /// # Errors
        ///
        /// Returns `Err` with the last read value when `f` returns `None`, matching
        /// the `core::sync::atomic` `fetch_update` contract.
        fn fetch_update_with;
        before: ();
        orders: (set_order, fetch_order);
        after: (f: F);
        generics: (F);
        bounds: ([F: FnMut(A::Value) -> Option<A::Value>]);
        ret: Result<A::Value, A::Value>;
        body: fetch_update_ordered(set_order, fetch_order, f)
    }
}

impl<'brand, A: Atomic + fmt::Debug> fmt::Debug for BrandedAtomic<'brand, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BrandedAtomic").field(&self.inner).finish()
    }
}
