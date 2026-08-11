# 3. Multi-Token Composition

One brand gives one exclusion domain. Real systems need several simultaneous
domains and, within a domain, disjoint write parallelism. Melinoe exposes one
primitive per axis and composes them rather than shipping arity-specific
variants.

## Nested scopes: several independent exclusion domains

Each nested `brand_scope` mints a fresh, non-unifiable brand. A `&mut` into one
region and a `&mut` into another can therefore be held simultaneously, with
disjointness proven at compile time:

```rust
extern crate melinoe;
use melinoe::{brand_scope, MelinoeCell};

brand_scope(|mut ta| {
    brand_scope(|mut tb| {
        let a = MelinoeCell::new(10_u64);
        let b = MelinoeCell::new(32_u64);
        let mut ma = a.borrow_mut(&mut ta);
        let mb = b.borrow_mut(&mut tb); // distinct brand ⇒ second live `&mut` is legal
        *ma += *mb;
        assert_eq!(*a.borrow(&ta), 42);
    })
});
```

Because `ta` and `tb` are different types, the borrow checker never conflates
the two borrows. Composition gives any arity for free — there is no
`brand_scopeN` family.

## WriterShard: split one brand into disjoint write regions

Two threads writing the *same* cell is a data race and cannot be made sound by
any phantom-type scheme — `&mut T` is exclusive by definition. Concurrent
*writes* are therefore expressed as concurrent access to **disjoint
partitions** of a branded region, which is exactly what per-thread allocator
slabs need.

[`WriterShard`] is the unit of that partition: a move-only, `Send` capability
over a disjoint `&mut [MelinoeCell<'brand, T>]` sub-slice. Construct one over a
whole region, then subdivide:

```rust
extern crate melinoe;
use melinoe::{brand_scope, region::WriterShard, MelinoeCell};

brand_scope(|token| {
    let mut cells: [MelinoeCell<'_, u32>; 6] =
        core::array::from_fn(|_| MelinoeCell::new(0));

    // Phase 1 — partition into disjoint shards and write each independently.
    let (mut lo, mut hi) = WriterShard::new(&mut cells).split_at(3);
    for (j, slot) in lo.iter_mut().enumerate() { *slot = j as u32; }
    for (j, slot) in hi.iter_mut().enumerate() { *slot = 100 + j as u32; }

    // Phase 2 — shards dropped; read the whole region back via the token.
    let snap = token.share();
    let seen: [u32; 6] = core::array::from_fn(|k| *cells[k].borrow(snap));
    assert_eq!(seen, [0, 1, 2, 100, 101, 102]);
});
```

Disjointness is guaranteed by [`slice::split_at_mut`]; each shard covers a
non-overlapping slice of the cell array, so two shards can never reach the same
cell. Shards are `#[repr(transparent)]` over the underlying `&mut` slice — the
capability is just the reference, with no extra footprint — and a shard is
`Send`/`Sync` exactly when its cells are.

The **key invariant**: the type system rejects any attempt to alias across
shard boundaries, because producing two overlapping shards from one region
would require a `&mut` borrow that `split_at_mut` already proved disjoint.

### Chunked and indexed subdivision

For distributing across a thread pool, [`WriterShard::chunks`] yields an
exact-size iterator of disjoint shards of at most `chunk_size` cells each
(`ShardChunks`). The random-access counterpart, [`WriterShard::par_chunks`]
(`ParChunks`), exposes `len` and indexed chunk access so a work-stealing pool
can request partition `c` on demand without threading `&mut` state through a
sequence.

## partition_for_each_with: lock-free parallel writes

With the `std` feature, [`partition_for_each_with`] drives scoped, disjoint
multithreaded writes with **no runtime lock**. The callback receives the
shard's starting index and its own `WriterShard`:

```rust
extern crate melinoe;
use melinoe::sync::{partition_for_each_with, PartitionPlan};
use melinoe::{brand_scope, MelinoeCell};

brand_scope(|token| {
    let mut cells: Vec<MelinoeCell<'_, usize>> =
        (0..8).map(|_| MelinoeCell::new(0)).collect();

    partition_for_each_with(&mut cells, PartitionPlan::chunk_size(2), |start, mut shard| {
        for (j, slot) in shard.iter_mut().enumerate() {
            *slot = start + j;
        }
    });

    let snap = token.share();
    for (index, cell) in cells.iter().enumerate() {
        assert_eq!(*cell.borrow(snap), index);
    }
});
```

[`PartitionPlan`] controls only how the region is tiled into non-empty shards.
The three constructors:

- `PartitionPlan::parts(n)` — split into at most `n` shards;
- `PartitionPlan::available_parallelism()` — use the reported hardware
  parallelism, falling back to one shard when the platform cannot report it;
- `PartitionPlan::chunk_size(k)` — at most `k` cells per shard.

The plan introduces no locks, atomics, queues, or worker pools: each shard is
moved into one scoped worker and joined before the call returns. Consumers with
a richer topology provider can pass their validated processor count to
`PartitionPlan::parts` — topology stays above Melinoe, which remains dependency-
free of any topology crate. The read-only analogues `partition_read_for_each`
and `partition_read_map` fan shared reads out across workers.

## SyncRegionToken: move a whole brand across a thread boundary

When an entire region crosses a thread boundary — a spawned task that takes over
a slab — [`SyncRegionToken`] carries the write capability with it:

```rust
extern crate melinoe;
use melinoe::{sync::sync_region_scope, MelinoeCell};

let sum = sync_region_scope(|token| {
    let cells = [MelinoeCell::new(1), MelinoeCell::new(2), MelinoeCell::new(3)];
    cells.iter().map(|c| *c.borrow(&token)).sum::<i32>()
});
assert_eq!(sum, 6);
```

Moving the token into a thread relocates the sole write right; `&token` (or
copies of `token.share()`) fans concurrent reads across threads. The absence of
a live `&mut` token statically excludes writers. The `thread_cached` module and
`scope_exclusive` demonstrate the handoff pattern over `std::thread::scope`.

With two placement axes — split within a brand, move a brand across threads —
the [next chapter](melinoe_cell.md) turns to the access surfaces those tokens
unlock.
