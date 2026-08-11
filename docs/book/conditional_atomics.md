# 5. Conditional Atomics

[`BrandedAtomic<'brand, A>`] wraps any standard-library atomic `A` (e.g.
`AtomicU64`) and selects its **access cost** through the capability presented:

- under a [`WritePermit`] — a proven-exclusive phase — access is **plain**:
  a bare non-atomic load or store with no `lock` prefix and no memory fence;
- under a [`ReadPermit`] — a shared phase — access is the **true atomic
  operation**: `fetch_add`, `compare_exchange`, and friends.

The capability selects the cost, so you pay for synchronization only while
sharing.

## Why plain access is sound under a write permit

A live `WritePermit<'brand>` is an exclusive borrow of the brand's unique owning
token. While it is held, no `ReadPermit` of that brand exists, so no atomic
operation can touch the value concurrently. The plain `&mut` is therefore
unaliased — exclusivity is the proof that no concurrent reader exists.

```rust
extern crate melinoe;
use core::sync::atomic::AtomicU64;
use melinoe::{brand_scope, BrandedAtomic, Relaxed};

brand_scope(|mut token| {
    let counter: BrandedAtomic<'_, AtomicU64> = BrandedAtomic::new(0);
    counter.store_exclusive(10, &mut token); // plain store: movq, no lock prefix
    let snap = token.share();
    assert_eq!(counter.fetch_add_with(5, snap, Relaxed), 10); // real lock xaddq
    assert_eq!(counter.load_with(snap, Relaxed), 15);
});
```

## The exclusive phase (plain)

- `store_exclusive(value, &mut token)` — plain store under a `WritePermit`.
- `load_exclusive(&token)` — plain load under a `WritePermit`.
- `with_exclusive(permit, f)` — run `f` with a plain `&mut A::Value` for any
  bulk operation.
- `get_mut()` / `into_inner()` — plain access from unique ownership, no permit.

The device-buffer pattern uses this phase for fence, generation, and completion
counters written by the exclusive owner before or after stream submission.

## The shared phase (atomic)

Every standard operation exists in two forms:

- a runtime-`Ordering` form: `load`, `store`, `swap`, `compare_exchange`,
  `fetch_update`, `fetch_add`, `fetch_sub`, `fetch_and`, `fetch_or`,
  `fetch_xor`, `fetch_nand`, `fetch_max`, `fetch_min` — each taking a
  `ReadPermit` and a `core::sync::atomic::Ordering`;
- a compile-time-policy form with a `_with` suffix: `load_with`,
  `store_with`, `compare_exchange_with`, `fetch_add_with`, etc.

## ZST ordering policies

When the ordering contract is fixed by the algorithm, pass a **zero-sized
policy** instead of a runtime `Ordering`:

```rust
extern crate melinoe;
use melinoe::{AcqRel, SeqCst};

// `AcqRel`: LOAD=Acquire, STORE=Release, RMW=AcqRel, FAILURE=Acquire
// `SeqCst`: LOAD=STORE=RMW=FAILURE=SeqCst
// `Relaxed`: everything Relaxed
```

The policies implement the sealed [`AtomicOrder`] trait via associated
constants, so monomorphization substitutes the ordering at compile time: the
`Borrowed`-style policy selects the constant, the inactive branches erase, and
the call site has no runtime ordering argument at all. The trait is sealed so
downstream code cannot introduce an ordering combination outside the crate's
audited policy set. See the [conditional atomics example](examples/conditional_atomics.md)
for the `codegen.rs` probe evidence of the plain `movq` vs. `lock xaddq`
difference.

## Branded → raw interop

[`as_atomic(snap)`](BrandedAtomic::as_atomic) returns a plain `&A` gated by the
read permit, so existing non-branded APIs that accept `&AtomicU64` can be
called **without `unsafe`** — the cost is identical to calling the atomic
directly. `from_mut` goes the other way: reborrow an atomic the caller already
owns (e.g. a field of a larger struct) as a branded atomic in place, zero-copy.

```rust
extern crate melinoe;
use core::sync::atomic::AtomicU64;
use melinoe::{brand_scope, BrandedAtomic};

fn interop(atomic: &core::sync::atomic::AtomicU64) -> u64 {
    atomic.fetch_add(1, core::sync::atomic::Ordering::SeqCst) + 1
}

brand_scope(|token| {
    let counter: BrandedAtomic<'_, AtomicU64> = BrandedAtomic::new(0);
    let snap = token.share();
    assert_eq!(interop(counter.as_atomic(snap)), 1);
});
```

## When not to use it

Use a `BrandedAtomic` when the phase (exclusive-then-shared, or exclusive
handoff) is real: a counter written by one owner then read by many observers. If
the value is shared from the start with no exclusive phase, a plain
`AtomicU64` is simpler; the brand adds nothing there. The benchmarks
(`conditional_atomics`) compare the exclusive/shared/mixed phases against a
bare `AtomicU64` baseline.

The same "condition the cost on the capability" idea applies to heap data at
the ownership boundary — the subject of [chapter 7](cow_boundary.md). First,
[chapter 6](reentrant.md) covers ambient, thread-confined exclusive state.
