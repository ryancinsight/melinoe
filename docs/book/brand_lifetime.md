# 1. The Brand Lifetime

*Chapter prose deferred — DoR item.*

<!-- Key points to cover:
  - `'brand` is an invariant lifetime minted fresh inside each `brand_scope`
    closure; the compiler assigns it a unique, non-unifiable type so no two
    scopes share a brand even when nested
  - Invariance: `ExclusiveToken<'brand>` is invariant in `'brand` because it
    is held behind `&mut ExclusiveToken<'brand>`; variance would allow a longer-
    lived token to impersonate a shorter-lived brand
  - InvariantLifetime<'brand>: the zero-sized marker that carries the brand
    into `MelinoeCell`; its size is always 0 regardless of `T`
  - Why the brand prevents cross-scope aliasing: the cell's brand is pinned
    on first use; a token from a different scope has a different brand, so the
    type system rejects the access
-->
