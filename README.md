# melinoe

**Branded, multi-token phantom capabilities for compile-time data-access and thread-synchronization proofs.**

`melinoe` is a `#![no_std]` foundation crate that encodes *who may touch what,
and from where* entirely in the type system. It is a generalised evolution of
the [`GhostCell`] pattern: where `GhostCell` pairs one brand with one token,
Melinoe offers a **family** of zero-sized tokens that share a single permit
interface yet differ in cardinality and thread-safety posture.

The crate ships **no allocator, arena, or `GlobalAlloc`** — only the
compile-time machinery. It is built to slot into the **Mnemosyne** memory
ecosystem alongside its ZST `AllocPolicy`, heap branding, and `Branded*` types.

> *In Greek myth, Melinoë leads a restless train of phantoms through the night.
> Here she leads a train of phantom **types** — each a wisp of pure evidence,
> weightless at runtime, that polices access to memory.*

[`GhostCell`]: https://plv.mpi-sws.org/rustbelt/ghostcell/

---

## Why

`RefCell` pays for safety with a runtime borrow flag and a panic path.
`Mutex`/`RwLock` pay with atomics and syscalls. Both verify at *run* time what a
sufficiently expressive type system can verify at *compile* time. `GhostCell`
showed that a single branded token can hoist the `T xor &mut T xor &T` rule out
of one cell and over an entire region, at zero cost. Melinoe extends that idea
along the axis allocators actually care about — **thread topology** — by giving
each region a token whose `Send`/`Sync` posture *is* its cross-thread contract.

## The model

A [`brand_scope`] mints a unique [`ExclusiveToken<'brand>`] over a fresh,
**invariant** lifetime. [`MelinoeCell<'brand, T>`] values created under that
brand reveal their contents only to a matching permit:

| Borrow of the owning token | Permit | Grants |
|----------------------------|--------|--------|
| `&token`                   | [`ReadPermit`]  | `&T` (shared) |
| `&mut token`               | [`WritePermit`] | `&mut T` (exclusive) |

Because the borrow checker already forbids holding `&mut token` and `&token`
simultaneously, it transitively forbids a write and any read **across every cell
of the brand** — no runtime state required.

### Token families

All tokens are ZSTs parameterised by `'brand`; they differ only in how many may
exist and where they may travel.

| Token | Cardinality | `Send` / `Sync` | Role |
|-------|-------------|-----------------|------|
| [`ExclusiveToken`] | exactly one per brand | both | sole owner; read + write |
| [`SharedReadToken`] | many (`Copy`) | both | fan a brand's read capability out to many readers/threads |
| [`ThreadLocalToken`] | one per brand | **neither** | owner pinned to one thread; soundness by confinement |
| [`SyncRegionToken`] | one per brand | both | owner that may migrate between threads (single writer) |

[`brand_scope`]: https://docs.rs/melinoe/latest/melinoe/fn.brand_scope.html
[`ExclusiveToken<'brand>`]: https://docs.rs/melinoe/latest/melinoe/struct.ExclusiveToken.html
[`MelinoeCell<'brand, T>`]: https://docs.rs/melinoe/latest/melinoe/struct.MelinoeCell.html
[`ExclusiveToken`]: https://docs.rs/melinoe/latest/melinoe/struct.ExclusiveToken.html
[`SharedReadToken`]: https://docs.rs/melinoe/latest/melinoe/struct.SharedReadToken.html
[`ThreadLocalToken`]: https://docs.rs/melinoe/latest/melinoe/sync/struct.ThreadLocalToken.html
[`SyncRegionToken`]: https://docs.rs/melinoe/latest/melinoe/sync/struct.SyncRegionToken.html
[`ReadPermit`]: https://docs.rs/melinoe/latest/melinoe/trait.ReadPermit.html
[`WritePermit`]: https://docs.rs/melinoe/latest/melinoe/trait.WritePermit.html

### Multi-token composition

Distinct brands are non-unifiable, so they compose into several independent
exclusion domains. melinoe ships one primitive per axis and composes them — no
arity-specific `brand_scopeN`/`CellN` variants:

* **Multi-XOR** — *nest* `brand_scope`. Each nested scope is a fresh, distinct
  brand, so a `&mut` into one region and a `&mut` into another are held
  simultaneously, proven disjoint at compile time (no runtime checks).
  Composition yields any arity for free:

  ```rust
  use melinoe::{brand_scope, MelinoeCell};
  brand_scope(|mut ta| brand_scope(|mut tb| {
      let a = MelinoeCell::new(10_u64);
      let b = MelinoeCell::new(32_u64);
      let mut ma = a.borrow_mut(&mut ta);
      let mb = b.borrow_mut(&mut tb);   // distinct brand ⇒ second live &mut is legal
      *ma += *mb;
      assert_eq!(*a.borrow(&ta), 42);
  }));
  ```

* **Disjoint concurrent writes** — `region::WriterShard` splits one brand into
  disjoint sub-regions for parallel writers; `SyncRegionToken` moves a whole
  brand's write capability across threads.
* **Ambient state** — `reentrant::GuardedCell` / `reentrant::ReentrancyCell` brand
  thread-lifetime exclusive state (e.g. an allocator's per-thread slot): one
  boundary check yields a borrow-checked `&mut T` (or a fresh-brand token),
  re-entry is refused rather than aliased, panic-safe by construction. This is the
  sound bridge for state that outlives any single `brand_scope` closure.
* **Conditional atomics** — `atomic::BrandedAtomic` is the write-side analogue of
  `Cow`: the capability you present selects the *cost*. A `WritePermit` (proven
  single-writer phase) gives **plain, non-atomic** access; a `ReadPermit` (shared
  phase) gives **atomic** `load`/`store`/`fetch_add`/`compare_exchange`. You pay
  for synchronization only while sharing — ~32× cheaper in the exclusive phase
  (0.19 ns vs 6.1 ns/op). The brand makes plain and atomic access *temporally
  exclusive*, so they can never race; verified data-race-free under Miri.

## Quick start

```rust
use melinoe::{brand_scope, MelinoeCell};

brand_scope(|mut token| {
    let a = MelinoeCell::new(1_i32);
    let b = MelinoeCell::new(2_i32);

    // Exclusive write: needs `&mut token`.
    *a.borrow_mut(&mut token) += 10;

    // Shared reads: one `Copy` read token serves every cell in the brand.
    let snap = token.share();
    assert_eq!(*a.borrow(snap) + *b.borrow(snap), 13);
});
```

The unsound interleavings simply do not compile:

```rust,compile_fail
use melinoe::{brand_scope, MelinoeCell};
brand_scope(|mut token| {
    let cell = MelinoeCell::new(0_i32);
    let w = cell.borrow_mut(&mut token); // exclusive borrow of `token`
    let r = cell.borrow(&token);         // ERROR: `token` already mutably borrowed
    let _ = (w, r);
});
```

## Thread safety without atomics

[`MelinoeCell<'brand, T>`] is `Send` when `T: Send` and `Sync` when
`T: Send + Sync` — the bound proven sound for `GhostCell` by RustBelt. Combined
with the token cardinality guarantees, this yields two statically-checked
parallelism shapes:

* **Exclusive handoff** — move a [`SyncRegionToken`] to another thread to
  relocate the sole write capability. No competing writer can exist because no
  competing token exists. [`sync::scope_exclusive`] demonstrates this over
  `std::thread::scope`.
* **Shared fan-out** — share `&SyncRegionToken` (or copies of a
  [`SharedReadToken`]) across threads for concurrent reads; the absence of a
  live `&mut` token statically excludes writers.

```rust
use melinoe::sync::scope_exclusive;
use melinoe::MelinoeCell;

// The whole branded computation runs on a worker thread; the write
// capability is transferred across the boundary and proven unique.
let sum = scope_exclusive(|mut token| {
    let cell = MelinoeCell::new(0_i64);
    for i in 1..=10 { *cell.borrow_mut(&mut token) += i; }
    *cell.borrow(&token)
});
assert_eq!(sum, 55);
```

A [`ThreadLocalToken`] is `!Send`, so the compiler rejects any attempt to move a
thread-confined capability — or a cell it governs — onto another thread.

## Benchmarks

Token access is a bare reference: zero synchronization instructions, fully
transparent to the optimizer. Measured against the runtime primitives that
discharge the same `T xor &mut T xor &T` exclusion at run time (Criterion
harness in [`benches/access.rs`](benches/access.rs); equivalence of the measured
work pinned by [`tests/differential.rs`](tests/differential.rs)):

| Single-threaded RMW | Per op | vs Melinoe |
|---------------------|--------|-----------:|
| **Melinoe** (`borrow_mut`) | ~0.21 ns | 1.0× |
| `AtomicU64` (Relaxed `fetch_add`) | ~6.1 ns | ~29× |
| `RwLock` (write) | ~8.6 ns | ~40× |
| `Mutex` | ~11.6 ns | ~54× |

For concurrent reads, Melinoe's `SharedReadToken` scales **near-linearly** with
cores (a branded read is a plain load with zero shared mutable state), reaching
**~10× `RwLock`** and **~15× `Mutex`** at 16 threads — where `RwLock` stops
scaling entirely, its reader-count atomic bouncing between cores. (Full
thread-scaling table in [`BENCHMARKS.md`](BENCHMARKS.md).)

For **concurrent writes**, disjoint [`WriterShard`](src/region/mod.rs) partitions
(below) scale near-linearly using plain stores, matching lock-free atomics while
a `Mutex<Vec>` — which cannot express disjoint `&mut` — serializes and loses to
the single-threaded baseline. Full methodology, all tables, and the honest
caveats are in [`BENCHMARKS.md`](BENCHMARKS.md). Ratios are the signal; absolute
figures are hardware-dependent. Reproduce with `cargo bench --bench access`.

## Concurrent writes via disjoint shards

Two threads writing the *same* cell is a data race no phantom type can excuse.
Sound concurrent *writes* mean concurrent access to **disjoint partitions** — the
per-thread allocator-slab pattern. A [`WriterShard`](src/region/mod.rs) is a
move-only, `Send` write capability over a disjoint `&mut [MelinoeCell<'brand, T>]`
sub-slice; disjointness comes from the standard library's `split_at_mut`. Read is
gated behind `&shard` and write behind `&mut shard`, so **write strictly subsumes
read** — the dependency realized structurally, at zero runtime cost.

```rust
use melinoe::sync::partition_for_each;
use melinoe::{brand_scope, MelinoeCell};

brand_scope(|token| {
    let mut cells: Vec<MelinoeCell<'_, usize>> =
        (0..10_000).map(|_| MelinoeCell::new(0)).collect();

    // Four threads each fill a disjoint partition concurrently — no locks, no atomics.
    partition_for_each(&mut cells, 4, |start, mut shard| {
        for (j, slot) in shard.iter_mut().enumerate() {
            *slot = start + j;
        }
    });

    // Then read the whole region back through the token.
    let snap = token.share();
    for (k, c) in cells.iter().enumerate() {
        assert_eq!(*c.borrow(snap), k);
    }
});
```

Verified data-race-free under Miri (Stacked Borrows + data-race detection).

## Integration with Mnemosyne

Melinoe is intentionally orthogonal to allocation:

* **Brand a heap in place.** `MelinoeCell::from_mut` reborrows an existing
  `&mut T` as `&mut MelinoeCell<'brand, T>` at zero cost (the cell is
  `#[repr(transparent)]`), so Mnemosyne's `BrandedHeap` storage can be governed
  by a Melinoe token without copying or wrapping.
* **Bulk slab access, zero-copy.** [`CellSliceExt`](src/cell/slice.rs) views a
  whole `[MelinoeCell<'brand, T>]` slab as a native `&[T]` / `&mut [T]` once a
  permit is presented — `slab.borrow_slice_mut(&mut token).fill(0)` for
  vectorised initialisation, `slab.borrow_slice(&token)` for SIMD scans — instead
  of `BrandedCell`-at-a-time access. Benchmarks confirm this matches the
  already-zero-cost per-cell path while adding slice ergonomics.
* **Upgrade the token model.** Mnemosyne's `AllocatorToken` is a single `!Send`
  token with runtime `assert_ne!` distinctness checks in `borrow_mut_2/3`.
  Melinoe replaces those with the compile-time-disjoint [`WriterShard`](src/region/mod.rs)
  and adds `Send`/`Sync` token families for cross-thread slabs.
* **Compose with `branded_scope`.** A Mnemosyne brand and a Melinoe brand are
  both invariant lifetimes; nest the scopes to require *both* an allocation
  witness and an access permit at a call site.
* **Replace lock-based interior mutability** on validated hot paths with
  [`MelinoeCell`] + a [`SyncRegionToken`], moving the synchronization proof from
  runtime atomics to compile-time evidence.
* **Encode `BrandedCell` capabilities** as distinct tokens (e.g. a read-only
  view handed to observers via [`SharedReadToken`] while a single subsystem
  retains the [`ExclusiveToken`]).

Every token is a ZST; carrying one through an API costs zero bytes and zero
instructions after monomorphization.

## Cargo features

| Feature | Default | Effect |
|---------|:-------:|--------|
| `std`     | ✅ | Superset of `alloc`; enables `std::thread::scope`-based helpers such as [`sync::scope_exclusive`]. |
| `alloc`   |    | Links the `alloc` crate for heap-payload examples/tests. |
| `nightly` |    | Enables `doc_cfg` for precise feature-gated docs (requires a nightly toolchain); reserved for future `generic_const_exprs` capability sets. |

The crate is `#![no_std]` by default and brings no global allocator of its own.

[`sync::scope_exclusive`]: https://docs.rs/melinoe/latest/melinoe/sync/fn.scope_exclusive.html

## Safety

All `unsafe` is confined to four points, each preceded by a `// SAFETY:` comment
discharging its obligation:

1. **Token minting** (`*_scope` functions) — the `for<'brand>` higher-ranked
   bound makes each brand fresh and invariant, so a minted owning token is
   provably unique.
2. **Cell access** (`borrow`/`borrow_mut`) — a live `ReadPermit`/`WritePermit`
   *is* a borrow of the brand's unique token, so the produced `&T`/`&mut T`
   cannot alias.
3. **`from_mut`** — justified by the `#[repr(transparent)]` layout chain
   `MelinoeCell → UnsafeCell<T> → T`.
4. **`Send`/`Sync` impls for `MelinoeCell`** — the `GhostCell` bound, with
   reasoning recorded inline.

The capability traits are **sealed**: downstream crates cannot forge a permit.
Soundness boundaries are pinned by `compile_fail` doctests (brand mixing,
read/write overlap, sending a thread-local token).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Unless you explicitly state
otherwise, any contribution intentionally submitted for inclusion in this crate
by you, as defined in the Apache-2.0 license, shall be dual licensed as above,
without any additional terms or conditions.
