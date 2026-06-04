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
//! Brands compose; melinoe exposes one primitive per axis and composes them
//! rather than shipping arity-specific variants:
//!
//! * **Multi-XOR** — *nest* [`brand_scope`] for several independent exclusion
//!   domains at once. Each nested scope is a fresh, non-unifiable brand, so a
//!   `&mut` into one region and a `&mut` into another are held simultaneously,
//!   disjointness proven at compile time. Composition gives any arity for free —
//!   no `brand_scopeN`.
//! * [`region::WriterShard`] — split one brand into disjoint sub-regions for
//!   concurrent writers; [`SyncRegionToken`](sync::SyncRegionToken) moves a whole
//!   brand's write capability across threads.
//! * [`reentrant::GuardedCell`] / [`reentrant::ReentrancyCell`] — gate *ambient*
//!   (thread-lifetime) exclusive state: one runtime check at the boundary yields
//!   a borrow-checked `&mut T` (or a fresh-brand token), with re-entry refused
//!   rather than aliased.
//! * [`atomic::BrandedAtomic`] — *conditional atomics*: plain non-atomic access
//!   under a [`WritePermit`] (proven-exclusive phase), atomic access under a
//!   [`ReadPermit`] (shared phase). The capability selects the cost, so you pay
//!   for synchronization only while sharing. [`Relaxed`], [`AcqRel`], and
//!   [`SeqCst`] are ZST ordering policies for monomorphized atomic call sites.
//! * [`CellCowExt`] — conditional `Cow` at the ownership boundary: [`Borrowed`]
//!   returns a zero-copy borrowed slice, [`Retained`] clones exactly once, and
//!   [`RetainDecision`] covers data-dependent retain decisions.
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
//! A borrow guard can be **projected** onto a component of its payload without
//! copying and without re-presenting the permit—the branded analogue of
//! [`Ref::map`](core::cell::Ref::map). [`MelinoeMut::map_split`] further yields
//! two disjoint `&mut` projections from a single write permit:
//!
//! ```
//! use melinoe::{brand_scope, MelinoeCell, MelinoeMut};
//!
//! brand_scope(|mut token| {
//!     let cell = MelinoeCell::new((0_u32, 0_u32));
//!     // One write permit, two disjoint field writers, live at once.
//!     let (mut a, mut b) =
//!         MelinoeMut::map_split(cell.borrow_mut(&mut token), |t| (&mut t.0, &mut t.1));
//!     *a = 1;
//!     *b = 2;
//!     drop((a, b));
//!     assert_eq!(*cell.borrow(&token), (1, 2));
//! });
//! ```
//!
//! At an ownership boundary, [`CellCowExt`] makes the retain decision explicit.
//! A ZST policy gives compile-time branch elimination when the decision is
//! static:
//!
//! ```
//! #[cfg(feature = "alloc")]
//! {
//! use std::borrow::Cow;
//! use melinoe::{brand_scope, Borrowed, CellCowExt, MelinoeCell, Retained};
//!
//! brand_scope(|token| {
//!     let cells: Vec<MelinoeCell<'_, u8>> = (0..4).map(MelinoeCell::new).collect();
//!     assert!(matches!(cells.borrow_cow_with(&token, Borrowed), Cow::Borrowed(_)));
//!     assert!(matches!(cells.borrow_cow_with(&token, Retained), Cow::Owned(_)));
//! });
//! }
//! ```
//!
//! Conditional atomics use the same idea on the synchronization side: a write
//! permit selects plain access, while a read permit selects atomic access. ZST
//! ordering policies keep common ordering contracts at the type level:
//!
//! ```
//! use core::sync::atomic::AtomicU64;
//! use melinoe::{brand_scope, BrandedAtomic, Relaxed};
//!
//! brand_scope(|mut token| {
//!     let counter: BrandedAtomic<'_, AtomicU64> = BrandedAtomic::new(0);
//!     counter.store_exclusive(10, &mut token);
//!     let snap = token.share();
//!     assert_eq!(counter.fetch_add_with(5, snap, Relaxed), 10);
//!     assert_eq!(counter.load_with(snap, Relaxed), 15);
//! });
//! ```
//!
//! With `std`, [`sync::PartitionPlan`] drives scoped, disjoint multithreaded
//! writes by fixed part count, current hardware parallelism, or fixed chunk
//! size. Each worker receives one [`WriterShard`] and no runtime lock protects
//! the write path:
//!
//! ```
//! #[cfg(feature = "std")]
//! {
//! use melinoe::sync::{partition_for_each_with, PartitionPlan};
//! use melinoe::{brand_scope, MelinoeCell};
//!
//! brand_scope(|token| {
//!     let mut cells: Vec<MelinoeCell<'_, usize>> =
//!         (0..8).map(|_| MelinoeCell::new(0)).collect();
//!
//!     partition_for_each_with(&mut cells, PartitionPlan::chunk_size(2), |start, mut shard| {
//!         for (j, slot) in shard.iter_mut().enumerate() {
//!             *slot = start + j;
//!         }
//!     });
//!
//!     let snap = token.share();
//!     for (index, cell) in cells.iter().enumerate() {
//!         assert_eq!(*cell.borrow(snap), index);
//!     }
//! });
//! }
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

pub mod atomic;
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
pub use atomic::{AcqRel, AtomicOrder, BrandedAtomic, Relaxed, SeqCst};
#[doc(inline)]
#[cfg(feature = "alloc")]
pub use cell::{Borrowed, CellCowExt, CowPolicy, RetainDecision, Retained};
#[doc(inline)]
pub use cell::{CellSliceExt, MelinoeCell, MelinoeMut, MelinoeRef};
#[doc(inline)]
pub use reentrant::{GuardedCell, ReentrancyCell};
#[doc(inline)]
pub use region::WriterShard;
#[doc(inline)]
pub use token::{
    brand_scope, ExclusiveToken, InvariantLifetime, ReadPermit, SharedReadToken, WritePermit,
};
