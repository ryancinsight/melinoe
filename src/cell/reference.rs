//! Branded borrow guards returned by cell access methods.
//!
//! [`MelinoeRef`] and [`MelinoeMut`] are zero-overhead smart pointers that wrap
//! the produced reference together with the brand it was proven against. They
//! exist so that a borrow *carries its capability evidence in its type*: a value
//! of type `MelinoeMut<'a, 'brand, T>` is itself proof that exclusive access to
//! `'brand`-branded data was lawfully obtained.

use core::fmt;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use crate::token::InvariantLifetime;

/// Define the shared/exclusive pair of branded borrow guards from one spec.
///
/// The two guards are mirror images: `MelinoeRef` wraps `&'a T` and lends it by
/// `Deref`; `MelinoeMut` wraps `&'a mut T`, adds `DerefMut`, and threads the
/// exclusive `'a` borrow through its projections. Everything else — the
/// `#[repr(transparent)]` layout, the crate-private `new`, the associated
/// `map`/`map_split` projections and their `needless_pass_by_value` rationale,
/// `Deref`, and `Debug` — is written once here and instantiated for both, so the
/// two guards can never drift apart. The `&`/`&mut` distinction is the only
/// structural difference the spec need not restate.
macro_rules! branded_guard {
    (
        $(#[$shared_doc:meta])*
        shared: $shared:ident;
        $(#[$shared_into_doc:meta])*
        shared_into: $shared_into:ident $([ $shared_const:tt ])?;
        shared_map: { $(#[$shared_map_doc:meta])* }
        shared_split: { $(#[$shared_split_doc:meta])* }
        $(#[$excl_doc:meta])*
        exclusive: $excl:ident;
        $(#[$excl_into_doc:meta])*
        exclusive_into: $excl_into:ident $([ $excl_const:tt ])?;
        excl_map: { $(#[$excl_map_doc:meta])* }
        excl_split: { $(#[$excl_split_doc:meta])* }
    ) => {
        branded_guard! {
            @one
            [ $(#[$shared_doc])* ]
            [ $shared ]
            [ & 'a ]
            [ $(#[$shared_into_doc])* ]
            [ $shared_into ]
            [ $($shared_const)? ]
            [ $(#[$shared_map_doc])* ]
            [ $(#[$shared_split_doc])* ]
            [ no ]
        }
        branded_guard! {
            @one
            [ $(#[$excl_doc])* ]
            [ $excl ]
            [ & 'a mut ]
            [ $(#[$excl_into_doc])* ]
            [ $excl_into ]
            [ $($excl_const)? ]
            [ $(#[$excl_map_doc])* ]
            [ $(#[$excl_split_doc])* ]
            [ yes ]
        }
    };

    (
        @one
        [ $(#[$doc:meta])* ]
        [ $name:ident ]
        [ $($refer:tt)* ]
        [ $(#[$into_doc:meta])* ]
        [ $into:ident ]
        [ $($into_const:tt)? ]
        [ $(#[$map_doc:meta])* ]
        [ $(#[$split_doc:meta])* ]
        [ $deref_mut:tt ]
    ) => {
        $(#[$doc])*
        #[repr(transparent)]
        pub struct $name<'a, 'brand, T: ?Sized> {
            value: $($refer)* T,
            _brand: InvariantLifetime<'brand>,
        }

        impl<'a, 'brand, T: ?Sized> $name<'a, 'brand, T> {
            #[inline]
            pub(crate) fn new(value: $($refer)* T) -> Self {
                Self {
                    value,
                    _brand: PhantomData,
                }
            }

            $(#[$into_doc])*
            #[inline]
            #[must_use]
            pub $($into_const)? fn $into(self) -> $($refer)* T {
                self.value
            }

            $(#[$map_doc])*
            // Consuming the guard is the contract (std `Ref::map`/`map_split`
            // parity). `orig` has no `Drop`, so this is a move, not a copy.
            #[inline]
            pub fn map<U: ?Sized, F>(orig: Self, f: F) -> $name<'a, 'brand, U>
            where
                F: FnOnce($($refer)* T) -> $($refer)* U,
            {
                // Zero-cost: the consumed guard's `'a` borrow threads into the
                // projected reference, so the brand's exclusion is carried by the
                // projection's lifetime alone (`orig` has no `Drop`).
                $name::new(f(orig.value))
            }

            $(#[$split_doc])*
            #[inline]
            pub fn map_split<U: ?Sized, V: ?Sized, F>(
                orig: Self,
                f: F,
            ) -> ($name<'a, 'brand, U>, $name<'a, 'brand, V>)
            where
                F: FnOnce($($refer)* T) -> ($($refer)* U, $($refer)* V),
            {
                let (a, b) = f(orig.value);
                ($name::new(a), $name::new(b))
            }
        }

        impl<'a, 'brand, T: ?Sized> Deref for $name<'a, 'brand, T> {
            type Target = T;

            #[inline]
            fn deref(&self) -> &T {
                self.value
            }
        }

        branded_guard!(@deref_mut $deref_mut, $name);

        impl<'a, 'brand, T: ?Sized + fmt::Debug> fmt::Debug for $name<'a, 'brand, T> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(self.value, f)
            }
        }
    };

    (@deref_mut no, $name:ident) => {};

    (@deref_mut yes, $name:ident) => {
        impl<'a, 'brand, T: ?Sized> DerefMut for $name<'a, 'brand, T> {
            #[inline]
            fn deref_mut(&mut self) -> &mut T {
                self.value
            }
        }
    };
}

branded_guard! {
    /// A shared, branded view of a cell's contents (`Deref` to `T`).
    ///
    /// Construction is crate-private; the only sources are
    /// [`MelinoeCell::borrow`](crate::MelinoeCell::borrow) and friends, which
    /// require a [`ReadPermit`](crate::ReadPermit).
    ///
    /// `#[repr(transparent)]` over `&'a T`: the guard is ABI-identical to the bare
    /// reference and preserves its null-pointer niche, so it is a true zero-cost
    /// wrapper (e.g. `Option<MelinoeRef<'_, '_, T>>` stays pointer-sized).
    shared: MelinoeRef;
    /// Consume the guard, returning the underlying shared reference.
    shared_into: into_ref [const];
    shared_map: {
        /// Project the guard onto a borrowed component of its contents, preserving
        /// the brand evidence.
        ///
        /// This is the branded analogue of [`Ref::map`](core::cell::Ref::map): it
        /// narrows a `MelinoeRef<'a, 'brand, T>` to a `MelinoeRef<'a, 'brand, U>`
        /// pointing at some part *of the same allocation* (typically a field), with
        /// **no copy and no re-presentation of the permit**. The original read
        /// capability is threaded through the returned guard's lifetime, so the
        /// brand's read/write exclusion stays in force for the whole projection.
        ///
        /// Provided as an associated function, not a method, so it does not collide
        /// with field/method access on `T` reached through `Deref`. Call it as
        /// `MelinoeRef::map(guard, |t| &t.field)`.
        ///
        /// # Examples
        ///
        /// ```
        /// use melinoe::{brand_scope, MelinoeCell, MelinoeRef};
        ///
        /// struct Header { tag: u32, len: u32 }
        ///
        /// brand_scope(|token| {
        ///     let cell = MelinoeCell::new(Header { tag: 7, len: 42 });
        ///     // Reach `len` through the permit without cloning the `Header`.
        ///     let len: MelinoeRef<'_, '_, u32> = MelinoeRef::map(cell.borrow(&token), |h| &h.len);
        ///     assert_eq!(*len, 42);
        /// });
        /// ```
    }
    shared_split: {
        /// Split the guard into two branded sub-guards over disjoint components.
        ///
        /// The branded analogue of [`Ref::map_split`](core::cell::Ref::map_split):
        /// `f` returns two shared references into distinct parts of the contents, and
        /// each is rewrapped as an independent `MelinoeRef` carrying the brand. Both
        /// sub-guards share the original `'a` read window, so neither can outlive the
        /// permit and no write of the brand can intervene while either is live.
    }
    /// An exclusive, branded view of a cell's contents (`Deref`/`DerefMut` to `T`).
    ///
    /// Construction is crate-private; the only sources are
    /// [`MelinoeCell::borrow_mut`](crate::MelinoeCell::borrow_mut) and friends,
    /// which require a [`WritePermit`](crate::WritePermit).
    ///
    /// `#[repr(transparent)]` over `&'a mut T`: ABI-identical to the bare exclusive
    /// reference with its niche preserved—a true zero-cost wrapper.
    exclusive: MelinoeMut;
    /// Consume the guard, returning the underlying exclusive reference.
    exclusive_into: into_mut;
    excl_map: {
        /// Project the guard onto a borrowed-mutably component of its contents,
        /// preserving the brand evidence.
        ///
        /// The branded analogue of [`RefMut::map`](core::cell::RefMut::map): it
        /// narrows a `MelinoeMut<'a, 'brand, T>` to a `MelinoeMut<'a, 'brand, U>`
        /// pointing at a part *of the same allocation* (typically a field) with **no
        /// copy and no re-presentation of the permit**. Consuming the original guard
        /// moves its exclusive `'a` borrow into the projection, so the brand's
        /// single-writer invariant is preserved by the lifetime system.
        ///
        /// Provided as an associated function so it does not collide with field or
        /// method access reached through `DerefMut`. Call it as
        /// `MelinoeMut::map(guard, |t| &mut t.field)`.
        ///
        /// # Examples
        ///
        /// ```
        /// use melinoe::{brand_scope, MelinoeCell, MelinoeMut};
        ///
        /// struct Header { tag: u32, len: u32 }
        ///
        /// brand_scope(|mut token| {
        ///     let cell = MelinoeCell::new(Header { tag: 7, len: 0 });
        ///     // Mutate `len` in place through the permit; the `Header` is never moved.
        ///     let mut len: MelinoeMut<'_, '_, u32> =
        ///         MelinoeMut::map(cell.borrow_mut(&mut token), |h| &mut h.len);
        ///     *len = 42;
        ///     drop(len);
        ///     assert_eq!(cell.borrow(&token).len, 42);
        /// });
        /// ```
    }
    excl_split: {
        /// Split the guard into two branded sub-guards over disjoint components.
        ///
        /// The branded analogue of
        /// [`RefMut::map_split`](core::cell::RefMut::map_split): `f` returns two
        /// **non-overlapping** exclusive references into distinct parts of the
        /// contents (e.g. via [`slice::split_at_mut`] or splitting a struct's
        /// fields), and each is rewrapped as an independent `MelinoeMut`. Disjointness
        /// is the caller's `f` contract — exactly as in the standard library — and is
        /// what makes the two simultaneous `&mut` projections sound; both inherit the
        /// brand and the original exclusive window.
    }
}
