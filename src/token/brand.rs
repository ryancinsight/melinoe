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

/// Covariant lifetime marker for a shared-borrow window.
pub(crate) type BorrowWindow<'a> = PhantomData<&'a ()>;

/// Shared zero-sized branding witness used by owner and shared-read tokens.
///
/// The `'brand` proof is always present; `Extra` lets each token family thread
/// its own auto-trait posture or borrow window through the same carrier
/// without adding runtime state.
#[derive(Clone, Copy)]
pub(crate) struct BrandMarker<'brand, Extra = ()> {
    _invariant: InvariantLifetime<'brand>,
    _extra: Extra,
}

impl<'brand, Extra> BrandMarker<'brand, Extra> {
    #[inline]
    pub(crate) const fn new(extra: Extra) -> Self {
        Self {
            _invariant: PhantomData,
            _extra: extra,
        }
    }
}

/// The private input to a fresh token-family factory.
///
/// It is intentionally linear: the factory creates one value for one
/// higher-ranked brand, and a token family consumes it while constructing its
/// concrete token. This keeps the fresh-brand proof in one generic boundary
/// while allowing token families to retain distinct auto-trait postures.
pub(crate) struct FreshBrand<'brand> {
    invariant: InvariantLifetime<'brand>,
}

impl<'brand> FreshBrand<'brand> {
    #[inline]
    const fn new() -> Self {
        Self {
            invariant: PhantomData,
        }
    }

    #[inline]
    pub(crate) fn into_marker<Extra>(self, extra: Extra) -> BrandMarker<'brand, Extra> {
        BrandMarker {
            _invariant: self.invariant,
            _extra: extra,
        }
    }
}

/// A zero-sized family that constructs one concrete token per fresh brand.
pub(crate) trait TokenFamily {
    type Token<'brand>
    where
        Self: 'brand;

    fn mint<'brand>(brand: FreshBrand<'brand>) -> Self::Token<'brand>;
}

/// Define a brand-owner token family: the token struct, its [`TokenFamily`]
/// selector, the unsafe constructor, its [`Debug`](core::fmt::Debug) impl, the
/// two `BrandOwner` impls, and — for families that permit sharing — a `share`
/// constructor yielding a [`SharedReadToken`](crate::token::SharedReadToken).
///
/// This is the single authoritative copy of the minting argument. Every owner
/// token is laid out as an invariant brand proof plus an optional
/// confinement-marker field, is minted from one [`FreshBrand`], and is
/// witnessed as the brand's read/write XOR proof by exactly one `unsafe` impl.
/// Folding the families here keeps that `unsafe` reasoning — and its SAFETY
/// comment — from drifting between otherwise-identical copies.
///
/// * `name` / `family` — the public token type and its crate-private family
///   selector, both documented with the supplied attributes.
/// * `debug` — the [`Debug`](core::fmt::Debug) string for the token.
/// * `marker_extra` / `marker_init` — the extra ZST payload threaded through
///   the shared [`BrandMarker`] carrier to preserve a family's auto-trait
///   posture (e.g. `PhantomData<*const ()>` for `!Send`).
/// * `share` — `yes` to emit the `share` constructor, `no` to omit it.
macro_rules! brand_owner_token {
    (
        $(#[$token_doc:meta])*
        name: $token:ident;
        $(#[$family_doc:meta])*
        family: $family:ident;
        marker_extra: $marker_extra:ty;
        marker_init: $marker_init:expr;
        debug: $debug_name:literal;
        share: $share:tt;
    ) => {
        $(#[$token_doc])*
        pub struct $token<'brand> {
            _marker: $crate::token::BrandMarker<'brand, $marker_extra>,
        }

        $(#[$family_doc])*
        pub(crate) struct $family;

        impl $crate::token::TokenFamily for $family {
            type Token<'brand>
                = $token<'brand>
            where
                Self: 'brand;

            #[inline]
            fn mint<'brand>(
                brand: $crate::token::FreshBrand<'brand>,
            ) -> Self::Token<'brand> {
                $token {
                    _marker: brand.into_marker($marker_init),
                }
            }
        }

        impl<'brand> $token<'brand> {
            /// Construct a token without proving brand uniqueness.
            ///
            /// # Safety
            ///
            /// The caller must guarantee that no other token for the same
            /// `'brand` exists for the lifetime of the returned value.
            /// Violating this allows two write permits of one brand to coexist,
            /// which is undefined behaviour.
            #[inline]
            #[must_use]
            pub const unsafe fn new_unchecked() -> Self {
                Self {
                    _marker: $crate::token::BrandMarker::new($marker_init),
                }
            }
        }

        impl<'brand> core::fmt::Debug for $token<'brand> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str($debug_name)
            }
        }

        impl<'brand> $crate::token::capability::private::BrandOwner for $token<'brand> {}

        // SAFETY: every family mints exactly one owner token for each fresh
        // brand, so borrowing this token is the canonical proof of that brand's
        // read/write XOR. A family's auto-trait posture only restricts where the
        // unique proof may travel (e.g. `!Send` pins it to one thread); it never
        // weakens the aliasing guarantees attached to borrowing the token.
        unsafe impl<'brand> $crate::token::capability::BrandOwner<'brand> for $token<'brand> {}

        $crate::token::brand_owner_token_share!($token, $share);
    };
}

pub(crate) use brand_owner_token;

/// Emit the `share` constructor for a brand-owner token, or nothing.
///
/// Split out of [`brand_owner_token`] so the `share` body — and its `unsafe`
/// SAFETY argument — is written once, yet can be omitted for families whose
/// confinement posture forbids fanning out read capability (e.g.
/// `ThreadLocalToken`, which must not hand a `Send + Sync` read token across
/// threads).
macro_rules! brand_owner_token_share {
    ($token:ident, yes) => {
        impl<'brand> $token<'brand> {
            /// Mint a `Copy`, read-only
            /// [`SharedReadToken`](crate::token::SharedReadToken) tied to this
            /// borrow.
            ///
            /// The returned token borrows `self` immutably for `'a`, so while
            /// any copy of it is live the owning token cannot be borrowed
            /// mutably and no write permit can be formed.
            #[inline]
            #[must_use]
            pub fn share<'a>(&'a self) -> $crate::token::SharedReadToken<'a, 'brand> {
                // SAFETY: `self` is borrowed immutably for `'a`; the produced
                // token carries that borrow window in its phantom, so no `&mut`
                // of the unique owner — and hence no write permit — can coexist
                // for the same brand.
                unsafe { $crate::token::SharedReadToken::new_unchecked() }
            }
        }
    };
    ($token:ident, no) => {};
}

pub(crate) use brand_owner_token_share;

#[inline]
fn with_fresh_brand<R, F>(f: F) -> R
where
    F: for<'brand> FnOnce(FreshBrand<'brand>) -> R,
{
    f(FreshBrand::new())
}

/// Run a callback with a token from one fresh, higher-ranked brand.
///
/// The family selects the token's auto-trait and capability posture; the
/// callback result may escape, but the family token and its brand cannot.
#[inline]
pub(crate) fn with_fresh_token<Family, R, F>(f: F) -> R
where
    Family: TokenFamily + 'static,
    F: for<'brand> FnOnce(Family::Token<'brand>) -> R,
{
    with_fresh_brand(|brand| f(Family::mint(brand)))
}

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
///
/// # Multi-XOR by composition
///
/// Several independent exclusion domains are obtained by *nesting* `brand_scope`,
/// not by arity-specific variants: each nested scope is a fresh, non-unifiable
/// brand, so a `&mut` into one region and a `&mut` into another may be held
/// simultaneously, disjointness proven at compile time.
///
/// ```
/// use melinoe::{brand_scope, MelinoeCell};
///
/// brand_scope(|mut ta| {
///     brand_scope(|mut tb| {
///         let a = MelinoeCell::new(10_u64);
///         let b = MelinoeCell::new(32_u64);
///         let mut ma = a.borrow_mut(&mut ta);
///         let mb = b.borrow_mut(&mut tb); // distinct brand ⇒ second live `&mut` is legal
///         *ma += *mb;
///         assert_eq!(*a.borrow(&ta), 42);
///     })
/// });
/// ```
#[inline]
pub fn brand_scope<R>(f: impl for<'brand> FnOnce(ExclusiveToken<'brand>) -> R) -> R {
    with_fresh_token::<super::ExclusiveFamily, _, _>(f)
}
