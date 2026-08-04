# Example: Branded Cells

**Crate**: `melinoe`
**Source**: `examples/book_branded_cell.rs`

`brand_scope` mints a unique `'brand` lifetime on each call.
`MelinoeCell<'brand, T>` cells are only accessible through a token carrying
that same brand — the borrow checker enforces exclusive vs. shared access
across the entire cell family with **zero runtime cost**.

## Source

```rust
{{#include ../../../examples/book_branded_cell.rs}}
```

## Output

```text
counter = 14, step = 7
pair = (42, 99)
a = 101, b = 201
all branded-cell assertions passed
```

## What to notice

- `borrow_mut(&mut token)` takes `&mut` of the *token*, not of the cell.
  The cell is `&`-accessible; the *permit* is what gets exclusively borrowed.
  This means multiple cells can exist as `&MelinoeCell` in normal Rust
  references while the token enforces the aliasing contract.

- `token.share()` coerces the `ExclusiveToken` into a `Copy`
  `SharedReadToken`.  Once shared, `borrow_mut` would need the exclusive token
  back — the compiler rejects a write attempt while the shared snapshot is
  live.

- `MelinoeMut::map_split` produces two `&mut` projections from one write
  permit.  Both are live simultaneously because the compiler can see they
  point to disjoint fields.  Without this primitive you would need to drop
  the first `borrow_mut` before taking the second.

- Nested `brand_scope` calls produce *non-unifiable* `'brand` lifetimes.
  `t1` governs `a` and `t2` governs `b`; holding `&mut t1` and `&mut t2` at
  the same time is safe because they are different types.
