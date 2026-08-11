# 8. Position in the Stack

`melinoe` is a **foundation** crate in the Atlas stack: it owns a single,
narrow law — branded capability evidence for memory access and
synchronization — and nothing else. Everything above it consumes that law;
everything beside it owns a different law. The stack map in the Atlas meta
repository is the authoritative topology.

```text
        Foundation (law)                          Providers
  ┌──────────────────────────────┐        ┌──────────────────────────┐
  │ aequitas   physical units    │        │ mnemosyne   allocation   │
  │ eunomia    datatype laws     │        │ moirai      execution    │
  │ melinoe    capability proof  │───────>│                          │
  │ themis     placement law     │        └──────────────────────────┘
  └──────────────────────────────┘
```

`melinoe` sits parallel to `themis` in the foundation tier: themis owns *where*
data may live (NUMA placement), melinoe owns *how* data may be accessed
(branded permits). The two compose — placement tokens are themselves branded.

## Consumers in the stack

- **mnemosyne — allocation.** The branded heap is built directly on melinoe's
  vocabulary. `InvariantLifetime`, `ThreadLocalToken`, and
  `thread_local_scope` underpin the allocator, and `BrandedBlock`,
  `BrandedBox`, and `BrandedVec` tie each block to a scope-local invariant
  lifetime. The type system — not a runtime check — prevents a block from
  outliving the heap or scope that owns it, which is what enables lock-free
  pool patterns. mnemosyne states the relationship explicitly: it implements
  allocation, not branding policy, and consumes melinoe's law.
- **moirai — execution.** melinoe provides the branded thread-local state
  behind moirai's task-local ambient state. A `ThreadLocalToken` gates that
  state so re-entrant task execution cannot alias its own storage — the same
  confinement guarantee [chapter 2](token_families.md) derives from the
  token's `!Send + !Sync` posture, applied where tasks can interleave.
- **themis — placement.** With the optional `melinoe` feature, themis's
  placement facts (`ConstNumaPinnedCell`, `ConstNumaPinnedSlice`) are branded
  with the placement proof: a NUMA-pinned allocation cannot be moved to a
  different NUMA node without the compiler catching it. themis depends on
  `melinoe/alloc` and keeps the integration optional.

## The halo: melinoe applied to itself

The placeholder for this chapter once promised a "halo module" — a public
module where melinoe applies its own capabilities to itself. That framing is
obsolete; the reality is more interesting and lives in two places:

1. **The token bootstrap.** The fresh-brand proof is manufactured by a
   crate-private family factory — `TokenFamily`, `FreshBrand`, and
   `with_fresh_token` — and melinoe uses it to mint the tokens its own
   subsystems need: `brand_scope`, `scope_exclusive`, and the reentrant gate
   all call the same bootstrap. The proof is minted once, in one generic
   boundary, and reused everywhere; there is no public escape hatch that could
   weaken it.
2. **The protective collections.** `BrandedVec`, `BrandedVecDeque`,
   `BrandedDrain`, and `BrandedVecDequeDrain` began life as the `halo`
   sub-crate, melinoe's own capability layer over standard containers, and
   were consolidated into `melinoe::collections` (root re-exported under the
   `alloc` feature) in 0.7.0. A `BrandedVec` is literally a
   `Vec<MelinoeCell<'brand, T>>` accessed through permits and the zero-copy
   slice and conditional-`Cow` traits — melinoe's own toolkit applied to the
   standard library.

## What melinoe does not own

The stack map assigns each concern to exactly one crate:

| Concern | Owning crate | melinoe's contribution |
| --- | --- | --- |
| Allocation and memory policy | mnemosyne | none — consumes branding |
| Execution and scheduling | moirai | branded task-local state |
| NUMA placement and locality | themis | none — branded by consumers |
| Physical quantities | aequitas | none |
| Datatype and scalar laws | eunomia | none |
| Capability evidence | **melinoe** | the token families, cells, and permits |

melinoe provides branded access evidence; it does not allocate, schedule,
place, or quantify. That single-owner separation is what keeps the foundation
tier small enough to be law, and what lets each provider above build on it
without re-implementing it.
