# Example: Conditional Atomics

**Crate**: `melinoe`
**Source**: `examples/book_conditional_atomics.rs`

`BrandedAtomic<'brand, A>` selects its access cost through the token type:
an `ExclusiveToken` gives a **plain** (non-atomic) store; a `SharedReadToken`
gives a **true** atomic RMW.  ZST ordering policies (`Relaxed`, `AcqRel`,
`SeqCst`) embed the ordering in the type rather than passing a runtime
`Ordering` argument.

## Source

```rust
{{#include ../../../examples/book_conditional_atomics.rs}}
```

## Output

```text
after exclusive store: 42
fetch_add(5): prev=100, now=105
fetch_add(10) AcqRel: prev=105, now=115
interop fetch_add: 7
all conditional-atomic assertions passed
```

## What to notice

- `counter.store_exclusive(42, &mut token)` is a plain `movq` on x86-64
  (as the `codegen.rs` probe confirms) — no `lock` prefix, no memory fence.
  This is safe because `&mut token` proves no other reader or writer exists
  for this brand at the same time.

- `counter.fetch_add_with(5, snap, Relaxed)` is a real `lock xaddq`:
  the `SharedReadToken` does not prove exclusivity, so the full atomic
  instruction is required.

- The ZST ordering policies (`Relaxed`, `AcqRel`) are zero-sized types that
  implement the `AtomicOrder` sealed trait.  Passing `Relaxed` vs. `AcqRel`
  selects a different monomorphization of `fetch_add_with`; the ordering
  choice is baked into the instruction at compile time.

- `as_atomic(snap)` returns `&AtomicU64` so existing non-branded APIs that
  accept `&AtomicU64` can be called without `unsafe`.  The cost is identical
  to calling the atomic directly; no indirection is added.
