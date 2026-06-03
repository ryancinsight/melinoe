//! Sealed capability traits shared by every Melinoe token family.
//!
//! A *permit* is evidence, materialised in the borrow checker, that the holder
//! may access branded data in a particular mode. Permits are produced only by
//! the token types in this crate; the traits are [sealed] so downstream crates
//! cannot forge a permit by implementing the trait on a foreign type.
//!
//! The capability lattice is intentionally tiny:
//!
//! ```text
//!   WritePermit<'brand>  ⊑  ReadPermit<'brand>
//! ```
//!
//! Every write permit is also a read permit; the reverse does not hold.
//!
//! [sealed]: https://rust-lang.github.io/api-guidelines/future-proofing.html

/// Crate-private supertrait used to seal the public capability traits.
///
/// The module is `pub(crate)`, so the trait is nameable in bounds inside this
/// crate yet unreachable—and therefore unimplementable—from any other crate.
pub(crate) mod private {
    /// Sealing marker. Implemented only for the in-crate permit carriers.
    pub trait Sealed {}
}

/// Evidence that the bearer may obtain a shared (`&T`) view of any
/// [`MelinoeCell`](crate::MelinoeCell) carrying the matching `'brand`.
///
/// # Safety
///
/// This trait is `unsafe` because [`MelinoeCell`](crate::MelinoeCell) relies on
/// implementors to uphold the brand's exclusion invariant: while *any*
/// `ReadPermit<'brand>` value is borrowed, no `&mut` token for the same
/// `'brand` may simultaneously exist. Every in-crate implementor discharges
/// this obligation through the borrow checker (the permit either *is* a borrow
/// of the unique token, or carries one in a `PhantomData`). External crates
/// cannot implement this trait because of the private `Sealed` supertrait.
pub unsafe trait ReadPermit<'brand>: private::Sealed {}

/// Evidence that the bearer may obtain an exclusive (`&mut T`) view of any
/// [`MelinoeCell`](crate::MelinoeCell) carrying the matching `'brand`.
///
/// # Safety
///
/// Implementors must additionally guarantee that holding a `WritePermit<'brand>`
/// excludes every other read *and* write permit of the same brand for the
/// duration of the borrow. In practice the only implementors are exclusive
/// `&mut` borrows of a brand's unique owning token, which the borrow checker
/// proves disjoint from all other token borrows.
pub unsafe trait WritePermit<'brand>: ReadPermit<'brand> {}
