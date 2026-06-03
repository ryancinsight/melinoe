//! Brand identity: the invariant lifetime that fuses a token to its cells.
//!
//! A *brand* is an [invariant] lifetime parameter `'brand`. Invariance is what
//! makes branding sound: two distinct [`brand_scope`] invocations receive
//! lifetimes that the compiler will never unify, so a token minted in one scope
//! can never be passed off as the token of another. This is the same mechanism
//! that underpins `GhostCell`, generalised here across multiple token families.
//!
//! [invariant]: https://doc.rust-lang.org/nomicon/subtyping.html#variance

use core::marker::PhantomData;

use super::ExclusiveToken;

/// A zero-sized marker that is **invariant** in `'brand` and unconditionally
/// `Send + Sync`.
///
/// `fn(&'brand ()) -> &'brand ()` places `'brand` in both argument and return
/// position, forcing invariance, while function pointers are always `Send` and
/// `Sync`, so the marker never perturbs the auto-trait inference of its host.
pub type InvariantLifetime<'brand> = PhantomData<fn(&'brand ()) -> &'brand ()>;

/// Open a fresh branding scope and hand its unique [`ExclusiveToken`] to `f`.
///
/// The higher-ranked bound `for<'brand>` universally quantifies the brand, so
/// `'brand` cannot escape the closure and cannot unify with any other scope's
/// brand. Consequently the token passed to `f` is provably the *only*
/// `ExclusiveToken<'brand>` in existence—the cornerstone of every downstream
/// access proof.
///
/// # Examples
///
/// ```
/// use melinoe::{brand_scope, MelinoeCell};
///
/// let doubled = brand_scope(|mut token| {
///     let cell = MelinoeCell::new(21_u32);
///     *cell.borrow_mut(&mut token) *= 2;
///     *cell.borrow(&token)
/// });
/// assert_eq!(doubled, 42);
/// ```
#[inline]
pub fn brand_scope<R>(f: impl for<'brand> FnOnce(ExclusiveToken<'brand>) -> R) -> R {
    // SAFETY: `for<'brand>` makes `'brand` a fresh, invariant lifetime that the
    // type system cannot unify with any other brand. No other `ExclusiveToken`
    // can name this `'brand`, so the token produced here is unique, satisfying
    // `ExclusiveToken::new_unchecked`'s contract.
    f(unsafe { ExclusiveToken::new_unchecked() })
}

/// Open two independent branding scopes at once, for simultaneous exclusive
/// access to two disjoint regions (*multi-XOR*).
///
/// The two tokens carry **distinct** brands: they are minted by nesting
/// [`brand_scope`], so `'a` and `'b` come from separate higher-ranked
/// quantifications and the type system can never unify them. You may therefore
/// hold a `&mut` into a `'a`-branded cell and a `&mut` into a `'b`-branded cell
/// *simultaneously*, with disjointness proven at compile time and zero runtime
/// checks — a single thread holding two independent exclusion domains.
///
/// # Examples
///
/// ```
/// use melinoe::{brand_scope2, MelinoeCell};
///
/// brand_scope2(|mut ta, mut tb| {
///     let a = MelinoeCell::new(10_u64);
///     let b = MelinoeCell::new(32_u64);
///     // Two live `&mut`, into different brands — accepted by the borrow checker.
///     let mut ma = a.borrow_mut(&mut ta);
///     let mb = b.borrow_mut(&mut tb);
///     *ma += *mb;
///     assert_eq!(*a.borrow(&ta), 42);
/// });
/// ```
///
/// The two brands never unify, so a cell pinned to one brand rejects the other's
/// token:
///
/// ```compile_fail
/// use melinoe::{brand_scope2, MelinoeCell};
/// brand_scope2(|ta, tb| {
///     let a = MelinoeCell::new(0_u64);
///     let _ = a.borrow(&ta);   // pins `a` to brand `'a`
///     let _ = a.borrow(&tb);   // ERROR: `tb`'s brand ≠ `a`'s brand
/// });
/// ```
#[inline]
pub fn brand_scope2<R>(
    f: impl for<'a, 'b> FnOnce(ExclusiveToken<'a>, ExclusiveToken<'b>) -> R,
) -> R {
    // Nesting two `brand_scope`s gives two independently-generative brands, so
    // `'a` and `'b` are structurally distinct; soundness reduces to the
    // single-brand case.
    brand_scope(|ta| brand_scope(|tb| f(ta, tb)))
}

/// Open three independent branding scopes at once (*multi-XOR*, arity 3).
///
/// As [`brand_scope2`], with three mutually-distinct brands.
///
/// # Examples
///
/// ```
/// use melinoe::{brand_scope3, MelinoeCell};
///
/// let sum = brand_scope3(|mut ta, mut tb, mut tc| {
///     let a = MelinoeCell::new(1_u64);
///     let b = MelinoeCell::new(2_u64);
///     let c = MelinoeCell::new(3_u64);
///     *a.borrow_mut(&mut ta) += 10;
///     *b.borrow_mut(&mut tb) += 10;
///     *c.borrow_mut(&mut tc) += 10;
///     *a.borrow(&ta) + *b.borrow(&tb) + *c.borrow(&tc)
/// });
/// assert_eq!(sum, 36);
/// ```
#[inline]
pub fn brand_scope3<R>(
    f: impl for<'a, 'b, 'c> FnOnce(ExclusiveToken<'a>, ExclusiveToken<'b>, ExclusiveToken<'c>) -> R,
) -> R {
    brand_scope(|ta| brand_scope(|tb| brand_scope(|tc| f(ta, tb, tc))))
}
