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
