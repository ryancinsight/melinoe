//! # Melinoe — branded, multi-token phantom capabilities
//!
//! `melinoe` provides zero-sized, brand-parameterised **capability tokens** that
//! encode data-access permissions and thread-synchronisation invariants in the
//! type system. It is a generalised evolution of the `GhostCell` pattern: where
//! `GhostCell` has one token per brand, Melinoe offers a *family* of tokens that
//! share a unified permit interface yet differ in cardinality and thread-safety
//! posture. The crate ships **no allocator, arena, or `GlobalAlloc`**—only the
//! compile-time machinery on which such systems can be built.
//!
//! It is designed for the **Mnemosyne** memory ecosystem, complementing its
//! ZST `AllocPolicy`, heap branding, and `Branded*` evidence types with a more
//! expressive, multi-token proof system for cross-thread handoff, branded heap
//! access, and lock-free interior mutability on validated hot paths.
//!
//! ## The model in one paragraph
//!
//! A [`brand_scope`] mints a unique [`ExclusiveToken<'brand>`](ExclusiveToken)
//! over a fresh, invariant lifetime. [`MelinoeCell<'brand, T>`](MelinoeCell)s
//! created under that brand reveal their contents only to a matching
//! [`ReadPermit`] (`&token`) or [`WritePermit`] (`&mut token`). Because the
//! whole brand is policed by the borrow checker's aliasing rules on that single
//! token, exclusive and shared access are mutually excluded *across every cell
//! in the region* with **zero runtime cost**—no flags, no atomics, no locks.
//!
//! ## Token families
//!
//! | Token | Cardinality | `Send`/`Sync` | Role |
//! |-------|-------------|---------------|------|
//! | [`ExclusiveToken`] | one per brand | both | sole owner; read + write |
//! | [`SharedReadToken`] | many (`Copy`) | both | fan-out read capability |
//! | [`ThreadLocalToken`](sync::ThreadLocalToken) | one per brand | neither | thread-confined owner |
//! | [`SyncRegionToken`](sync::SyncRegionToken) | one per brand | both | thread-portable owner |
//!
//! ## Multi-token composition
//!
//! Brands compose. Because distinct brands are non-unifiable invariant
//! lifetimes, melinoe expresses several independent exclusion domains at once
//! (*multi-XOR*), capability transfer, and joint requirements:
//!
//! * [`brand_scope2`] / [`brand_scope3`] — open several brands together and hold
//!   a `&mut` into each disjoint region *simultaneously*, disjointness proven at
//!   compile time.
//! * [`MelinoeCell2`] — a cell unlocked only by presenting a capability for
//!   *both* of two brands: a compile-time *multi-lock-held* invariant.
//! * [`region::WriterShard`] — split one brand into disjoint sub-regions for
//!   concurrent writers; [`SyncRegionToken`](sync::SyncRegionToken) moves a whole
//!   brand's write capability across threads.
//! * [`reentrant::ReentrancyCell`] — gate *ambient* (thread-lifetime) exclusive
//!   state: one runtime check at the boundary yields a fresh-brand token whose
//!   access is then compile-time-proven, with re-entry refused rather than aliased.
//!
//! ## Quick start
//!
//! ```
//! use melinoe::{brand_scope, MelinoeCell};
//!
//! brand_scope(|mut token| {
//!     let a = MelinoeCell::new(1_i32);
//!     let b = MelinoeCell::new(2_i32);
//!
//!     // Exclusive write: needs `&mut token`.
//!     *a.borrow_mut(&mut token) += 10;
//!
//!     // Shared reads: `&token` (or a copied `SharedReadToken`) reads any cell.
//!     let snap = token.share();
//!     assert_eq!(*a.borrow(snap) + *b.borrow(snap), 13);
//! });
//! ```
//!
//! The borrow checker rejects the unsound interleavings at compile time:
//!
//! ```compile_fail
//! use melinoe::{brand_scope, MelinoeCell};
//! brand_scope(|mut token| {
//!     let cell = MelinoeCell::new(0_i32);
//!     let w = cell.borrow_mut(&mut token); // exclusive borrow of `token`
//!     let r = cell.borrow(&token);         // ERROR: `token` already mutably borrowed
//!     let _ = (w, r);
//! });
//! ```
//!
//! Tokens of different brands never mix. A cell's brand is *inferred from use*,
//! so the first access pins it; a later access with a foreign token is rejected:
//!
//! ```compile_fail
//! use melinoe::{brand_scope, MelinoeCell};
//! brand_scope(|t1| {
//!     let cell = MelinoeCell::new(0_i32);
//!     let _ = cell.borrow(&t1); // pins the cell's brand to `t1`'s region
//!     brand_scope(|t2| {
//!         let _ = cell.borrow(&t2); // ERROR: `t2`'s brand ≠ the cell's brand
//!     });
//! });
//! ```
//!
//! ## Cargo features
//!
//! * `std` *(default)* — superset of `alloc`; enables [`sync::scope_exclusive`]
//!   and other [`std::thread::scope`]-based demonstrations.
//! * `alloc` — links the `alloc` crate for heap-payload examples/tests.
//! * `nightly` — enables `doc_cfg` for precise feature-gated docs (requires a
//!   nightly toolchain); reserved for future `generic_const_exprs` capability
//!   sets.
//!
//! The crate is `#![no_std]` by default and uses no global allocator of its own.
#![no_std]
#![cfg_attr(any(docsrs, feature = "nightly"), feature(doc_cfg))]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod cell;
pub mod reentrant;
pub mod region;
pub mod sync;
pub mod token;

#[cfg(all(doctest, feature = "std"))]
mod readme_doctests {
    //! Compiles the `README.md` code blocks as doctests so the documented
    //! examples cannot drift from the implementation. Gated on `std` because one
    //! example uses [`sync::scope_exclusive`].
    #![doc = include_str!("../README.md")]
}

mod static_assertions;

#[doc(inline)]
pub use cell::{CellSliceExt, MelinoeCell, MelinoeCell2, MelinoeMut, MelinoeRef};
#[doc(inline)]
pub use reentrant::ReentrancyCell;
#[doc(inline)]
pub use region::WriterShard;
#[doc(inline)]
pub use token::{
    brand_scope, brand_scope2, brand_scope3, ExclusiveToken, InvariantLifetime, ReadPermit,
    SharedReadToken, WritePermit,
};
