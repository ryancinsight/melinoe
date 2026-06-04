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
