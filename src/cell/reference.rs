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

/// A shared, branded view of a cell's contents (`Deref` to `T`).
///
/// Construction is crate-private; the only sources are
/// [`MelinoeCell::borrow`](crate::MelinoeCell::borrow) and friends, which
/// require a [`ReadPermit`](crate::ReadPermit).
///
/// `#[repr(transparent)]` over `&'a T`: the guard is ABI-identical to the bare
/// reference and preserves its null-pointer niche, so it is a true zero-cost
/// wrapper (e.g. `Option<MelinoeRef<'_, '_, T>>` stays pointer-sized).
#[repr(transparent)]
pub struct MelinoeRef<'a, 'brand, T: ?Sized> {
    value: &'a T,
    _brand: InvariantLifetime<'brand>,
}

impl<'a, 'brand, T: ?Sized> MelinoeRef<'a, 'brand, T> {
    #[inline]
    pub(crate) fn new(value: &'a T) -> Self {
        Self {
            value,
            _brand: PhantomData,
        }
    }

    /// Consume the guard, returning the underlying shared reference.
    #[inline]
    #[must_use]
    pub const fn into_ref(self) -> &'a T {
        self.value
    }

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
    #[inline]
    pub fn map<U: ?Sized, F>(orig: Self, f: F) -> MelinoeRef<'a, 'brand, U>
    where
        F: FnOnce(&'a T) -> &'a U,
    {
        // Zero-cost: rewraps the projected reference; the consumed `orig` carries
        // the same `'a` read borrow, so exclusion is preserved by lifetime alone.
        MelinoeRef::new(f(orig.value))
    }

    /// Split the guard into two branded sub-guards over disjoint components.
    ///
    /// The branded analogue of [`Ref::map_split`](core::cell::Ref::map_split):
    /// `f` returns two shared references into distinct parts of the contents, and
    /// each is rewrapped as an independent `MelinoeRef` carrying the brand. Both
    /// sub-guards share the original `'a` read window, so neither can outlive the
    /// permit and no write of the brand can intervene while either is live.
    #[inline]
    pub fn map_split<U: ?Sized, V: ?Sized, F>(
        orig: Self,
        f: F,
    ) -> (MelinoeRef<'a, 'brand, U>, MelinoeRef<'a, 'brand, V>)
    where
        F: FnOnce(&'a T) -> (&'a U, &'a V),
    {
        let (a, b) = f(orig.value);
        (MelinoeRef::new(a), MelinoeRef::new(b))
    }
}

impl<'a, 'brand, T: ?Sized> Deref for MelinoeRef<'a, 'brand, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.value
    }
}

impl<'a, 'brand, T: ?Sized + fmt::Debug> fmt::Debug for MelinoeRef<'a, 'brand, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.value, f)
    }
}

/// An exclusive, branded view of a cell's contents (`Deref`/`DerefMut` to `T`).
///
/// Construction is crate-private; the only sources are
/// [`MelinoeCell::borrow_mut`](crate::MelinoeCell::borrow_mut) and friends,
/// which require a [`WritePermit`](crate::WritePermit).
///
/// `#[repr(transparent)]` over `&'a mut T`: ABI-identical to the bare exclusive
/// reference with its niche preserved—a true zero-cost wrapper.
#[repr(transparent)]
pub struct MelinoeMut<'a, 'brand, T: ?Sized> {
    value: &'a mut T,
    _brand: InvariantLifetime<'brand>,
}

impl<'a, 'brand, T: ?Sized> MelinoeMut<'a, 'brand, T> {
    #[inline]
    pub(crate) fn new(value: &'a mut T) -> Self {
        Self {
            value,
            _brand: PhantomData,
        }
    }

    /// Consume the guard, returning the underlying exclusive reference.
    #[inline]
    #[must_use]
    pub fn into_mut(self) -> &'a mut T {
        self.value
    }

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
    #[inline]
    pub fn map<U: ?Sized, F>(orig: Self, f: F) -> MelinoeMut<'a, 'brand, U>
    where
        F: FnOnce(&'a mut T) -> &'a mut U,
    {
        // Zero-cost: moves the exclusive `'a` borrow into the projected reference;
        // `orig` has no `Drop`, so the partial move of `value` is sound.
        MelinoeMut::new(f(orig.value))
    }

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
    #[inline]
    pub fn map_split<U: ?Sized, V: ?Sized, F>(
        orig: Self,
        f: F,
    ) -> (MelinoeMut<'a, 'brand, U>, MelinoeMut<'a, 'brand, V>)
    where
        F: FnOnce(&'a mut T) -> (&'a mut U, &'a mut V),
    {
        let (a, b) = f(orig.value);
        (MelinoeMut::new(a), MelinoeMut::new(b))
    }
}

impl<'a, 'brand, T: ?Sized> Deref for MelinoeMut<'a, 'brand, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.value
    }
}

impl<'a, 'brand, T: ?Sized> DerefMut for MelinoeMut<'a, 'brand, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.value
    }
}

impl<'a, 'brand, T: ?Sized + fmt::Debug> fmt::Debug for MelinoeMut<'a, 'brand, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.value, f)
    }
}
