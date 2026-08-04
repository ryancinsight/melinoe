# melinoe — Branded Capability Tokens for Atlas

`melinoe` provides zero-sized, brand-parameterised **capability tokens** that
encode data-access permissions and thread-synchronization invariants in the
type system.  It is a generalized evolution of the `GhostCell` pattern: where
`GhostCell` has one token per brand, melinoe offers a *family* of tokens that
share a unified permit interface yet differ in cardinality and thread-safety
posture.

## The model in one sentence

A `brand_scope` mints one `ExclusiveToken<'brand>` for a fresh,
compiler-unique lifetime `'brand`.  Every `MelinoeCell<'brand, T>` created
inside that scope reveals its contents only to a matching token.  The borrow
checker enforces aliasing rules across the *entire cell family* at compile time
with **zero runtime cost** — no flags, no atomics, no locks.

## What this book covers

1. The brand lifetime and why invariance is required.
2. The token families: `ExclusiveToken`, `SharedReadToken`,
   `ThreadLocalToken`, `SyncRegionToken`.
3. Multi-token composition for nested and concurrent regions.
4. `MelinoeCell` and borrow guards, including `map_split` for disjoint fields.
5. Conditional atomics: pay for synchronization only while sharing.
6. Guarded and reentrant cells for ambient exclusive state.
7. `CellCowExt` and the retain-decision policy.
8. Where melinoe fits in the Atlas stack.
