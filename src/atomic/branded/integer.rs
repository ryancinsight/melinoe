//! Integer read-modify-write operations on [`BrandedAtomic`].

use core::sync::atomic::Ordering;

use super::BrandedAtomic;
use crate::atomic::order::{AtomicOrder, OrderingSource};
use crate::atomic::traits::AtomicInt;
use crate::token::ReadPermit;

impl<'brand, A: AtomicInt> BrandedAtomic<'brand, A> {
    order_pair! {
        /// Atomic fetch-add. Requires a [`ReadPermit`] for `'brand` (the shared phase).
        fn fetch_add;
        /// Atomic fetch-add using a compile-time ZST ordering policy.
        fn fetch_add_with;
        before: (value: A::Value);
        after: ();
        generics: ();
        bounds: ();
        order: order;
        ret: A::Value;
        body: inner.atomic_fetch_add(value, order.rmw_order())
    }

    order_pair! {
        /// Atomic fetch-sub. Requires a [`ReadPermit`] for `'brand`.
        fn fetch_sub;
        /// Atomic fetch-sub using a compile-time ZST ordering policy.
        fn fetch_sub_with;
        before: (value: A::Value);
        after: ();
        generics: ();
        bounds: ();
        order: order;
        ret: A::Value;
        body: inner.atomic_fetch_sub(value, order.rmw_order())
    }

    order_pair! {
        /// Atomic fetch-and. Requires a [`ReadPermit`] for `'brand`.
        fn fetch_and;
        /// Atomic fetch-and using a compile-time ZST ordering policy.
        fn fetch_and_with;
        before: (value: A::Value);
        after: ();
        generics: ();
        bounds: ();
        order: order;
        ret: A::Value;
        body: inner.atomic_fetch_and(value, order.rmw_order())
    }

    order_pair! {
        /// Atomic fetch-or. Requires a [`ReadPermit`] for `'brand`.
        fn fetch_or;
        /// Atomic fetch-or using a compile-time ZST ordering policy.
        fn fetch_or_with;
        before: (value: A::Value);
        after: ();
        generics: ();
        bounds: ();
        order: order;
        ret: A::Value;
        body: inner.atomic_fetch_or(value, order.rmw_order())
    }

    order_pair! {
        /// Atomic fetch-xor. Requires a [`ReadPermit`] for `'brand`.
        fn fetch_xor;
        /// Atomic fetch-xor using a compile-time ZST ordering policy.
        fn fetch_xor_with;
        before: (value: A::Value);
        after: ();
        generics: ();
        bounds: ();
        order: order;
        ret: A::Value;
        body: inner.atomic_fetch_xor(value, order.rmw_order())
    }

    order_pair! {
        /// Atomic fetch-nand. Requires a [`ReadPermit`] for `'brand`.
        fn fetch_nand;
        /// Atomic fetch-nand using a compile-time ZST ordering policy.
        fn fetch_nand_with;
        before: (value: A::Value);
        after: ();
        generics: ();
        bounds: ();
        order: order;
        ret: A::Value;
        body: inner.atomic_fetch_nand(value, order.rmw_order())
    }

    order_pair! {
        /// Atomic fetch-max. Requires a [`ReadPermit`] for `'brand`.
        fn fetch_max;
        /// Atomic fetch-max using a compile-time ZST ordering policy.
        fn fetch_max_with;
        before: (value: A::Value);
        after: ();
        generics: ();
        bounds: ();
        order: order;
        ret: A::Value;
        body: inner.atomic_fetch_max(value, order.rmw_order())
    }

    order_pair! {
        /// Atomic fetch-min. Requires a [`ReadPermit`] for `'brand`.
        fn fetch_min;
        /// Atomic fetch-min using a compile-time ZST ordering policy.
        fn fetch_min_with;
        before: (value: A::Value);
        after: ();
        generics: ();
        bounds: ();
        order: order;
        ret: A::Value;
        body: inner.atomic_fetch_min(value, order.rmw_order())
    }
}
