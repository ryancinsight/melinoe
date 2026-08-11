# 7. Cow at the Ownership Boundary

Inside a brand scope, [`CellSliceExt`] already produces a zero-copy `&[T]` view
of a branded slab for the price of a read permit. `Cow` belongs at the
**boundary**: the point where a consumer must decide whether it only reads the
slab transiently, or must retain an owned copy that outlives the brand window.
[`CellCowExt`] makes that retain decision explicit and, when the decision is
static, moves it to compile time.

Requires the `alloc` feature (`Cow<'_, [T]>` is `alloc::borrow::Cow`).

## The boundary question: borrow or retain

Given a permit, a branded slab can be handed out in two shapes:

- **Borrowed** — `Cow::Borrowed(&[T])`. A zero-allocation, zero-copy view of
  the cells through the shared token. It dies when the permit borrow ends.
- **Owned** — `Cow::Owned(Vec<T>)`. A clone of the contents that can escape
  the brand scope, stored in un-branded owned memory.

The API gives both shapes, plus a runtime decision when the answer depends on
the data:

| Method | Selection | Cost |
| --- | --- | --- |
| [`borrow_cow`] | always borrowed | no allocation, no clone |
| [`retain_cow`] | always owned | one clone |
| [`borrow_cow_with`] | compile-time ZST policy | per policy |
| [`borrow_cow_if`] | runtime [`RetainDecision`] | per decision |

```rust
extern crate melinoe;
use std::borrow::Cow;
use melinoe::{
    brand_scope, Borrowed, CellCowExt, CellSliceExt, MelinoeCell, RetainDecision,
    Retained,
};

brand_scope(|mut token| {
    let cells: [MelinoeCell<'_, u32>; 4] = core::array::from_fn(|_| MelinoeCell::new(0));
    cells.borrow_slice_mut(&mut token).copy_from_slice(&[1, 2, 3, 4]);

    // Read-only consumer: a zero-cost view of the slab.
    let view = cells.borrow_cow(&token);
    assert!(matches!(view, Cow::Borrowed(_)));
    assert_eq!(view.iter().sum::<u32>(), 10);

    // A consumer that must keep the data past the brand window: one clone.
    let owned = cells.retain_cow(&token);
    assert!(matches!(owned, Cow::Owned(_)));

    // The same two choices through the ZST policy, resolved at compile time.
    assert!(matches!(
        cells.borrow_cow_with(&token, Borrowed),
        Cow::Borrowed(_)
    ));
    assert!(matches!(
        cells.borrow_cow_with(&token, Retained),
        Cow::Owned(_)
    ));

    // A data-dependent decision, taken at runtime.
    let chosen = cells.borrow_cow_if(&token, RetainDecision::Borrow);
    assert_eq!(chosen.first(), Some(&1));

    // The owned Cow moves out of the brand scope into plain memory.
    let escaped: Vec<u32> = owned.into_owned();
    assert_eq!(escaped, vec![1, 2, 3, 4]);
});
```

## Compile-time policy: sealed ZSTs

[`CowPolicy`] is a sealed trait implemented only by zero-sized marker types.
Each implementation owns its `Cow` construction body, so the two
monomorphizations contain exactly what they promise:

- [`Borrowed`] returns `Cow::Borrowed(slice)` — no allocation, no element
  clone, no clone path in the compiled body.
- [`Retained`] returns `Cow::Owned(slice.to_vec())` — exactly one clone path.

Passing `policy: C` to [`borrow_cow_with`] therefore selects the path at
compile time with **no runtime branch**: the optimizer erases the inactive
monomorph. This is the [phase discipline](conditional_atomics.md) applied to
ownership — when the boundary posture is known statically, the decision costs
nothing at run time.

## Runtime decision: RetainDecision

When the escape decision is data-dependent — a consumer that clones only when
the slab is small, or when a cache is cold — [`RetainDecision`] covers the
choice with two variants, [`Borrow`](RetainDecision::Borrow) and
[`Retain`](RetainDecision::Retain). Use the ZST policies instead whenever the
decision is static, so the runtime branch disappears.

## Segmented collections

The segmented collection adapters share the same ownership decision. A
single-segment slab delegates to the selected policy; a two-segment slab (the
wrapped halves of a `BrandedVecDeque`) cannot be expressed as one borrowed
slice, so the segments are concatenated into owned storage regardless of
policy. This sharing lives behind a crate-internal helper; the public decision
remains `Borrowed` / `Retained` / `RetainDecision`.

## The pattern

`CellCowExt` sits on top of the [zero-copy slice view](melinoe_cell.md): a
library that only reads the slab takes the borrowed `Cow`; one that needs an
owned buffer takes the retained `Cow`; and the choice of path — static or
dynamic — is explicit at the call site instead of being a hidden fallback.
The return value's lifetime is the boundary itself: `Cow<'a, [T]>` is tied to
the permit-bearing `&'a self` borrow, so a borrowed view cannot outlive the
permit that authorized it.

[`CellCowExt`]: https://docs.rs/melinoe/latest/melinoe/trait.CellCowExt.html
[`CellSliceExt`]: https://docs.rs/melinoe/latest/melinoe/trait.CellSliceExt.html
[`CowPolicy`]: https://docs.rs/melinoe/latest/melinoe/trait.CowPolicy.html
[`Borrowed`]: https://docs.rs/melinoe/latest/melinoe/struct.Borrowed.html
[`Retained`]: https://docs.rs/melinoe/latest/melinoe/struct.Retained.html
[`RetainDecision`]: https://docs.rs/melinoe/latest/melinoe/enum.RetainDecision.html
[`borrow_cow`]: https://docs.rs/melinoe/latest/melinoe/trait.CellCowExt.html#method.borrow_cow
[`retain_cow`]: https://docs.rs/melinoe/latest/melinoe/trait.CellCowExt.html#method.retain_cow
[`borrow_cow_with`]: https://docs.rs/melinoe/latest/melinoe/trait.CellCowExt.html#method.borrow_cow_with
[`borrow_cow_if`]: https://docs.rs/melinoe/latest/melinoe/trait.CellCowExt.html#method.borrow_cow_if
