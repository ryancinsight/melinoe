# 2. Token Families

Every melinoe token is a zero-sized type parameterised by a `'brand`. The
families differ in **how many may exist** and **where they may travel**; they
share one unified permit interface ([`ReadPermit`]/[`WritePermit`]) so that
cells and access surfaces are written once against the permit traits, not once
per token type.

| Token | Cardinality | `Send`/`Sync` | Permits |
|-------|-------------|---------------|---------|
| [`ExclusiveToken`] | exactly one per brand | both | read + write |
| [`SharedReadToken`] | many (`Copy`) | both | read |
| [`ThreadLocalToken`] | one per brand | neither | read + write |
| [`SyncRegionToken`] | one per brand | both | read + write |

## ExclusiveToken — the unique owner

[`ExclusiveToken<'brand>`] is the token handed out by [`brand_scope`]. It is
**move-only** (deliberately neither `Clone` nor `Copy`), so exactly one exists
per brand. Because the type is move-only, the borrow checker's aliasing rules on
this *one value* police all cells of the brand:

- a shared borrow `&ExclusiveToken` is a [`ReadPermit`], and
- an exclusive borrow `&mut ExclusiveToken` is a [`WritePermit`].

You cannot hold `&mut` and `&` to the same token at once, so you cannot hold a
write permit and any read permit of the same brand at once. The XOR discipline
is lifted from a single value to an entire region of branded cells at zero
runtime cost. The token is `Send + Sync`, so its write capability can travel
across a thread boundary.

## SharedReadToken — fan-out read capability

[`SharedReadToken<'a, 'brand>`] is `Copy`, so it can be stored in many places
and handed to many readers (including many threads). Soundness is preserved by
construction: the only ways to obtain one are [`ExclusiveToken::share`] and
[`SyncRegionToken::share`], each of which borrows its owning token *immutably*
for the window `'a`. As long as any copy survives, that immutable borrow keeps
the owning token from being borrowed mutably, so no write permit of the brand
can be formed concurrently.

The two lifetimes carry different meanings:

- `'brand` is the brand identity (invariant);
- `'a` is the **sharing window** (covariant `&'a ()` phantom) — how long the
  owning token's immutable borrow lasts.

Calling `token.share()` on an exclusive owner is the supported way to switch a
brand from single-writer to multi-reader. The reverse transition (getting a
write back) is simply dropping every shared copy, after which `&mut token`
becomes available again.

## ThreadLocalToken — thread-confined owner

[`ThreadLocalToken<'brand>`] implements neither `Send` nor `Sync` (it carries a
`*const ()` phantom). The whole capability — and therefore every cell it
governs — is consequently un-sendable: the compiler rejects any attempt to move
the access right to another thread. Use it for allocator metadata whose
soundness rests on single-thread confinement rather than synchronisation: free
lists, bump cursors, per-task scratch. It provides the same read/write permit
interface as `ExclusiveToken`; the `!Send` posture only narrows *where* the
capability may be used. Open one with [`thread_local_scope`].

## SyncRegionToken — thread-portable owner

[`SyncRegionToken<'brand>`] is `Send + Sync` and carries the same permit
semantics as `ExclusiveToken`, but names the *region* pattern explicitly: a
contiguous branded region whose ownership migrates between worker threads.
Moving the token to a thread transfers the right to mutate every cell of the
region; sharing `&token` across threads grants concurrent read access. Open one
with [`sync_region_scope`].

For device-buffer ownership transfer, store the backend's real buffer handle in
a `MelinoeCell` and require `SyncRegionToken<'brand>` by value at the
host/device boundary. Moving the token in transfers the sole write capability;
returning it (or calling `share`) switches to shared readback.

## ReadPermit / WritePermit — the sealed permit lattice

Cells and atomics are written against the **permit traits**, not the concrete
token types:

```text
  WritePermit<'brand>  ⊑  ReadPermit<'brand>
```

Every write permit is also a read permit; the reverse does not hold. The traits
are `unsafe` and **sealed** (private `Sealed` supertrait), so downstream crates
cannot forge a permit by implementing the trait on a foreign type — permits are
produced only by the in-crate token borrows, each of which discharges the
brand-exclusion obligation through the borrow checker. This is why new token
families can be added without touching `MelinoeCell`, `BrandedAtomic`, or the
slice views: they only need to implement the permit traits.

The [next chapter](multi_token.md) composes these families: nesting scopes for
independent regions and splitting one brand into disjoint write shards.
