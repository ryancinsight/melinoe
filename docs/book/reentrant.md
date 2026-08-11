# 6. Guarded and Reentrant Cells

Some exclusive state is **ambient** rather than lexically scoped: a thread's
allocator slot is touched on every allocation across the thread's whole
lifetime, so it cannot live inside a single [`brand_scope`] closure. The
classic guard for such state is a hand-checked re-entrancy boolean
(`is_allocating`) wrapping a raw `UnsafeCell` — correct only by audit. Melinoe
offers two typed replacements, both gating with a single `!Sync` flag:
[`GuardedCell`] (yields `&mut T` directly) and [`ReentrancyCell`] (yields a
fresh-brand token). Both refuse re-entry rather than aliasing and clear their
flag on panic.

## GuardedCell — owned ambient state

[`GuardedCell<T>`] owns a value and hands out one exclusive `&mut T` at a time
from `&self`:

```rust
extern crate melinoe;
use melinoe::reentrant::GuardedCell;

let cache = GuardedCell::new(0_u64);
assert_eq!(cache.enter(|n| { *n += 41; *n }), Ok(41));
// Re-entrant access is refused, not aliased:
assert_eq!(
    cache.enter(|_| cache.enter(|_| ())).unwrap(),
    Err(melinoe::reentrant::Reentered)
);
```

`enter` checks a single `bool` flag once (the unavoidable runtime gate at the
ambient boundary) and yields a borrow-checked `&mut T`; the `&mut` *is* the
compile-time exclusivity proof, and a nested `enter` returns
[`Reentered`] instead of aliasing. The flag is cleared by a drop guard even if
the closure unwinds, so a panic cannot poison the cell.

The runtime cost is one predictable branch at entry — the `RefCell` difference
matters here: `RefCell` tracks a full borrow *count* at runtime, while
`GuardedCell` tracks only "locked / not locked". For exclusive ambient state the
count is always 0 or 1, so the flag is the honest model and the codegen is
correspondingly smaller. `GuardedCell` is `!Sync` by construction (it holds a
`Cell` and an `UnsafeCell`); the single centralised `unsafe` deref is the
audited replacement for the hand-rolled idiom.

`enter_unguarded` (an `unsafe` fast path) skips the flag writes for code
statically known not to re-enter; `get_mut(&mut self)` needs no check at all;
`as_ptr` exposes the raw contents for use as a stable owner token.

## ReentrancyCell — ambient state re-branded

[`ReentrancyCell`] is the same gate but yields a **fresh-brand
[`ExclusiveToken`]** instead of a `&mut T`:

```rust
extern crate melinoe;
use melinoe::reentrant::ReentrancyCell;
use melinoe::MelinoeCell;

let gate = ReentrancyCell::new();

let out = gate.enter(|mut token| {
    let slot = MelinoeCell::new(0_u64);
    *slot.borrow_mut(&mut token) = 7;
    assert_eq!(gate.enter(|_| ()), Err(melinoe::reentrant::Reentered));
    *slot.borrow(&token)
});
assert_eq!(out, Ok(7));
```

This is the shape for **ephemeral branded sub-state**: the ambient boundary is
checked once at `enter`, and every access *inside* the closure is then
compile-time-proven via the fresh brand — the proof covers the entire body, not
just the entry point. Re-entrant `enter` returns `Reentered` and the caller
takes a fallback path.

## vs. RefCell, recap

| | `RefCell` | `GuardedCell` | `ReentrancyCell` |
|---|---|---|---|
| Runtime gate | borrow count | one `bool` | one `bool` |
| Re-entry | panic | `Err(Reentered)` | `Err(Reentered)` |
| Proof inside | borrow check at each `.borrow()` | `&mut T` | fresh-brand token |
| Panic safety | yes | yes | yes |

Choose `GuardedCell` when the closure wants a plain `&mut T` (persistent state
like a thread's allocator cache); choose `ReentrancyCell` when the closure
needs a brand to govern nested branded cells (ephemeral sub-state). Both are
the panic-safe, audited replacement for `UnsafeCell<T>` + `is_allocating: bool`.

From ambient state, the [next chapter](cow_boundary.md) moves to the heap
boundary where owned buffers leave the brand.
