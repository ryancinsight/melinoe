//! Conditional `Cow` boundary helpers for branded cell slices.
//!
//! Inside a brand scope, [`CellSliceExt`](super::CellSliceExt) already gives a
//! zero-copy `&[T]`. `Cow` belongs at the boundary where a consumer may either
//! use that borrow transiently or retain an owned copy past the brand window.

use alloc::borrow::Cow;

use super::{CellSliceExt, MelinoeCell};
use crate::token::ReadPermit;

mod sealed {
    pub trait Sealed {}
}

/// Compile-time retain policy for [`CellCowExt::borrow_cow_with`].
///
/// Implemented by ZSTs only. Each policy owns its implementation body, so the
/// `Borrowed` monomorph contains no clone path and the `Retained` monomorph
/// contains exactly one clone path.
pub trait CowPolicy: sealed::Sealed + Copy {
    /// Build the boundary `Cow` from the branded slice view.
    fn cow<T: Clone>(slice: &[T]) -> Cow<'_, [T]>;
}

/// Borrow the branded slice; no allocation and no element clone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Borrowed;

/// Clone the branded slice into an owned buffer so it may outlive the brand.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Retained;

impl sealed::Sealed for Borrowed {}
impl sealed::Sealed for Retained {}

impl CowPolicy for Borrowed {
    #[inline]
    fn cow<T: Clone>(slice: &[T]) -> Cow<'_, [T]> {
        Cow::Borrowed(slice)
    }
}

impl CowPolicy for Retained {
    #[inline]
    fn cow<T: Clone>(slice: &[T]) -> Cow<'_, [T]> {
        Cow::Owned(slice.to_vec())
    }
}

/// Runtime retain decision for [`CellCowExt::borrow_cow_if`].
///
/// Use this when the escape decision is data-dependent. Use the ZST
/// [`Borrowed`] / [`Retained`] policies when the decision is static so the
/// optimizer can erase the inactive branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetainDecision {
    /// Return `Cow::Borrowed`.
    Borrow,
    /// Return `Cow::Owned`.
    Retain,
}

/// Conditional `Cow` views over `[MelinoeCell<'brand, T>]`, gated by a read
/// permit.
pub trait CellCowExt<'brand, T: Clone> {
    /// Return a `Cow` according to the compile-time ZST policy `C`.
    ///
    /// `Borrowed` performs no allocation and no clone; `Retained` clones exactly
    /// once into the returned owned buffer.
    fn borrow_cow_with<'a, P, C>(&'a self, permit: P, policy: C) -> Cow<'a, [T]>
    where
        P: ReadPermit<'brand> + 'a,
        C: CowPolicy;

    /// Return a `Cow` according to a runtime retain decision.
    fn borrow_cow_if<'a, P>(&'a self, permit: P, decision: RetainDecision) -> Cow<'a, [T]>
    where
        P: ReadPermit<'brand> + 'a;
}

impl<'brand, T: Clone> CellCowExt<'brand, T> for [MelinoeCell<'brand, T>] {
    #[inline]
    fn borrow_cow_with<'a, P, C>(&'a self, permit: P, _policy: C) -> Cow<'a, [T]>
    where
        P: ReadPermit<'brand> + 'a,
        C: CowPolicy,
    {
        let slice = self.borrow_slice(permit);
        C::cow(slice)
    }

    #[inline]
    fn borrow_cow_if<'a, P>(&'a self, permit: P, decision: RetainDecision) -> Cow<'a, [T]>
    where
        P: ReadPermit<'brand> + 'a,
    {
        let slice = self.borrow_slice(permit);
        match decision {
            RetainDecision::Borrow => Cow::Borrowed(slice),
            RetainDecision::Retain => Cow::Owned(slice.to_vec()),
        }
    }
}
