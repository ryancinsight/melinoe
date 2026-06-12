//! Region partitioning: sound, zero-cost concurrent writes over disjoint slices.
//!
//! Two threads writing the *same* cell is a data race and cannot be made sound
//! by any phantom-type scheme—`&mut T` is exclusive by definition. Concurrent
//! *writes* are therefore expressed as concurrent access to **disjoint
//! partitions** of a branded region, which is exactly what per-thread allocator
//! slabs need.
//!
//! [`WriterShard`] is the unit of that partition: a move-only, [`Send`]
//! capability over a disjoint `&mut [MelinoeCell<'brand, T>]` sub-slice. It is
//! produced by [splitting](WriterShard::split_at) a parent region, whose
//! disjointness is guaranteed by the standard library's
//! [`<[_]>::split_at_mut`](slice::split_at_mut). Each shard can be moved to its
//! own thread for parallel writes with **no atomics and no locks**.
//!
//! # Read depends on write, structurally
//!
//! A shard exposes reads through `&self` ([`get`](WriterShard::get),
//! [`iter`](WriterShard::iter)) and writes through `&mut self`
//! ([`get_mut`](WriterShard::get_mut), [`iter_mut`](WriterShard::iter_mut)).
//! Holding the shard mutably therefore grants both read and write, while a
//! shared `&shard` grants read only: write is the strictly higher capability,
//! and obtaining it presupposes the read capability over the same partition.
//! This mirrors the crate's [`WritePermit`](crate::WritePermit) ⊒
//! [`ReadPermit`](crate::ReadPermit) lattice, here enforced by the borrow
//! checker on the shard value itself.
//!
//! # Lifecycle
//!
//! ```
//! use melinoe::{brand_scope, region::WriterShard, MelinoeCell};
//!
//! brand_scope(|token| {
//!     let mut cells: [MelinoeCell<'_, u32>; 6] =
//!         core::array::from_fn(|_| MelinoeCell::new(0));
//!
//!     // Phase 1 — partition into disjoint shards and write each independently.
//!     let (mut lo, mut hi) = WriterShard::new(&mut cells).split_at(3);
//!     for (j, slot) in lo.iter_mut().enumerate() { *slot = j as u32; }
//!     for (j, slot) in hi.iter_mut().enumerate() { *slot = 100 + j as u32; }
//!
//!     // Phase 2 — shards dropped; read the whole region back via the token.
//!     let snap = token.share();
//!     let seen: [u32; 6] = core::array::from_fn(|k| *cells[k].borrow(snap));
//!     assert_eq!(seen, [0, 1, 2, 100, 101, 102]);
//! });
//! ```

mod chunks;
mod shard;

pub use chunks::ShardChunks;
pub use shard::WriterShard;
