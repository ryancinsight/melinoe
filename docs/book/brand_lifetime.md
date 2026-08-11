# 1. The Brand Lifetime

Every melinoe capability is parameterised by a **brand**: a lifetime `'brand`
that the compiler mints fresh inside each [`brand_scope`] closure. The brand is
what fuses a token to its cells. A cell created under one brand can never be
unlocked by a token from another, and the compiler enforces that separation at
compile time.

## Where the brand comes from

[`brand_scope`] opens with a higher-ranked bound:

```rust,ignore
pub fn brand_scope<R>(f: impl for<'brand> FnOnce(ExclusiveToken<'brand>) -> R) -> R
```

The `for<'brand>` quantifier makes `'brand` **universally quantified**: the
compiler must prove the closure works for *any* lifetime, which means the brand
cannot escape the closure and cannot unify with any other lifetime in the
program. Two `brand_scope` invocations therefore always receive distinct,
non-unifiable brands — even when one scope is nested inside another. A token
minted in scope A can never be passed off as the token of scope B, because the
type checker has no way to equate the two brands.

This is the same mechanism that underpins `GhostCell`, generalised here across
multiple token families.

## Why invariance is required

If `'brand` were *covariant*, a token of a longer-lived brand could be coerced
into a shorter-lived brand. That would let a brand's write capability outlive
its scope and leak into a region it was never minted for.

Melinoe makes `'brand` **invariant** by storing it in a marker that places the
lifetime in both argument and return position of a function pointer:

```rust,ignore
pub type InvariantLifetime<'brand> = PhantomData<fn(&'brand ()) -> &'brand ()>;
```

A lifetime that appears in both input and output of a function type is
constrained to be exactly what it is — the compiler can neither widen it nor
narrow it. Invariance is what makes branding sound: distinct scopes produce
lifetimes that will never unify, so cross-scope impersonation is a type error.

## The zero-sized marker

`InvariantLifetime<'brand>` is a `PhantomData` of a function pointer, so:

- its size is always `0`, regardless of `T` and regardless of the host struct;
- it is unconditionally `Send + Sync`, because function pointers are, so the
  marker never perturbs the auto-trait inference of the tokens and cells that
  carry it.

`MelinoeCell<'brand, T>` is `#[repr(transparent)]` over `UnsafeCell<T>` with an
`InvariantLifetime<'brand>` alongside, so a cell is exactly as large as its
payload `T` — the brand costs nothing at runtime.

## Why the brand prevents cross-scope aliasing

A cell's brand is **pinned on first use**. When you write
`cell.borrow(&token)`, the cell's `'brand` is inferred to be the token's brand,
and from then on every access must present a permit of that same brand. A token
from a different scope has a different `'brand`, so the access is rejected:

```rust,compile_fail
extern crate melinoe;
use melinoe::{brand_scope, MelinoeCell};

brand_scope(|t1| {
    let cell = MelinoeCell::new(0_i32);
    let _ = cell.borrow(&t1); // pins the cell's brand to `t1`'s region
    brand_scope(|t2| {
        let _ = cell.borrow(&t2); // ERROR: `t2`'s brand ≠ the cell's brand
    });
});
```

The aliasing rules on the *single* owning token transitively police every cell
of the brand: a shared `&token` permits reads, an exclusive `&mut token` permits
writes, and the borrow checker's XOR discipline on that one value means the two
can never coexist. That is the entire mechanism — there is no flag, no atomic,
no lock. Next, [Token Families](token_families.md) catalogues the token types
that build on the brand.

[`brand_scope`]: https://docs.rs/melinoe/latest/melinoe/fn.brand_scope.html
