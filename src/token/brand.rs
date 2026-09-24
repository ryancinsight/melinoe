//! Brand identity: the invariant lifetime that fuses a token to its cells.
//!
//! A *brand* is an [invariant] lifetime parameter `'brand`. Invariance is what
//! makes branding sound: two distinct [`brand_scope`] invocations receive
//! lifetimes that the compiler will never unify, so a token minted in one scope
//! can never be passed off as the token of another. This is the same mechanism
//! that underpins `GhostCell`, generalised here across multiple token families.
//!
//! [invariant]: https://doc.rust-lang.org/nomicon/subtyping.html#variance

use core::fmt;
use core::marker::PhantomData;

use super::SharedReadToken;

/// A zero-sized marker that is **invariant** in `'brand` and unconditionally
/// `Send + Sync`.
///
/// `fn(&'brand ()) -> &'brand ()` places `'brand` in both argument and return
/// position, forcing invariance, while function pointers are always `Send` and
/// `Sync`, so the marker never perturbs the auto-trait inference of its host.
pub type InvariantLifetime<'brand> = PhantomData<fn(&'brand ()) -> &'brand ()>;

/// Marker type that keeps the same invariant branding proof while preserving a
/// token family's auto-trait posture.
pub(crate) trait TokenMarker: 'static {
    const DEBUG_NAME: &'static str;
}

/// The common zero-sized payload shared by every family-owned token.
pub(crate) struct BrandToken<'brand, Marker> {
    _invariant: InvariantLifetime<'brand>,
    _marker: PhantomData<Marker>,
}

impl<'brand, Marker> BrandToken<'brand, Marker> {
    #[inline]
    pub(crate) const fn new_unchecked() -> Self {
        Self {
            _invariant: PhantomData,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub(crate) fn mint(brand: FreshBrand<'brand>) -> Self {
        Self {
            _invariant: brand.into_invariant(),
            _marker: PhantomData,
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn share<'a>(&'a self) -> SharedReadToken<'a, 'brand> {
        // SAFETY: the promoted shared token stays within the lifetime of this
        // borrowed owner token, so the write-capability exclusion proof remains
        // intact for the entire sharing window.
        unsafe { SharedReadToken::new_unchecked() }
    }
}

impl<'brand, Marker: TokenMarker> fmt::Debug for BrandToken<'brand, Marker> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Marker::DEBUG_NAME)
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
    pub(crate) fn into_invariant(self) -> InvariantLifetime<'brand> {
        self.invariant
    }
}

/// A zero-sized family that constructs one concrete token per fresh brand.
pub(crate) trait TokenFamily {
    type Token<'brand>
    where
        Self: 'brand;

    fn mint<'brand>(brand: FreshBrand<'brand>) -> Self::Token<'brand>;
}

/// Marker for a unique, move-only token family whose writes are thread-portable.
pub(crate) struct ExclusiveMarker;

impl TokenMarker for ExclusiveMarker {
    const DEBUG_NAME: &'static str = "ExclusiveToken<'brand>";
}

/// Marker for a unique, move-only token family confined to its originating
/// thread.
pub(crate) struct ThreadLocalMarker {
    _not_threadsafe: PhantomData<*const ()>,
}

impl TokenMarker for ThreadLocalMarker {
    const DEBUG_NAME: &'static str = "ThreadLocalToken<'brand>";
}

/// Marker for a unique, move-only token family whose region ownership may be
/// transferred across threads.
pub(crate) struct SyncRegionMarker;

impl TokenMarker for SyncRegionMarker {
    const DEBUG_NAME: &'static str = "SyncRegionToken<'brand>";
}

macro_rules! define_brand_owner_token {
    ($token:ident, $family:ident, $marker:ident, $debug_name:literal) => {
        /// The unique, move-only owner of a brand's access rights.
        pub struct $token<'brand> {
            _brand: crate::token::brand::BrandToken<'brand, $marker>,
        }

        pub(crate) struct $family;

        impl crate::token::brand::TokenFamily for $family {
            type Token<'brand>
                = $token<'brand>
            where
                Self: 'brand;

            #[inline]
            fn mint<'brand>(brand: crate::token::brand::FreshBrand<'brand>) -> Self::Token<'brand> {
                $token {
                    _brand: crate::token::brand::BrandToken::mint(brand),
                }
            }
        }

        impl<'brand> $token<'brand> {
            /// Construct a token without proving brand uniqueness.
            ///
            /// # Safety
            ///
            /// The caller must guarantee that no other instance of this token for
            /// the same `'brand` exists for the lifetime of the returned value.
            #[inline]
            #[must_use]
            pub const unsafe fn new_unchecked() -> Self {
                Self {
                    _brand: crate::token::brand::BrandToken::new_unchecked(),
                }
            }

            /// Mint a `Copy`, read-only [`SharedReadToken`] tied to this borrow.
            #[inline]
            #[must_use]
            pub fn share<'a>(&'a self) -> crate::token::SharedReadToken<'a, 'brand> {
                self._brand.share()
            }
        }

        impl<'brand> core::fmt::Debug for $token<'brand> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self._brand.fmt(f)
            }
        }

        impl<'brand> crate::token::capability::private::BrandOwner for $token<'brand> {}

        unsafe impl<'brand> crate::token::capability::BrandOwner<'brand> for $token<'brand> {}
    };
}

pub(crate) use define_brand_owner_token;

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
///         let mb = b.borrow_mut(&mut tb); // distinct brand => second live `&mut` is legal
///         *ma += *mb;
///         assert_eq!(*a.borrow(&ta), 42);
///     })
/// });
/// ```
#[inline]
pub fn brand_scope<R>(f: impl for<'brand> FnOnce(super::ExclusiveToken<'brand>) -> R) -> R {
    with_fresh_token::<super::ExclusiveFamily, _, _>(f)
}
