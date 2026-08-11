# 4. MelinoeCell and Borrow Guards

[`MelinoeCell<'brand, T>`] is the storage counterpart to the token system: it
holds a `T` but exposes it solely through methods that demand a
[`ReadPermit`] or [`WritePermit`] for the *same* `'brand`.

## Layout and cost

`MelinoeCell` is `#[repr(transparent)]` over `UnsafeCell<T>` (which is itself
transparent over `T`), with an `InvariantLifetime<'brand>` alongside:

```rust
extern crate melinoe;
use core::cell::UnsafeCell;
use melinoe::InvariantLifetime;

pub struct MelinoeCell<'brand, T: ?Sized> {
    _brand: InvariantLifetime<'brand>,
    value: UnsafeCell<T>,
}
```

So `size_of::<MelinoeCell<'_, T>>() == size_of::<T>()`. Access compiles to a
bare pointer dereference: there is **no runtime flag and no `borrow` panic
path**, unlike `RefCell`. Because the cell yields real `&T`/`&mut T`
references, it also works with non-`Copy` payloads, unlike `Cell`.

The transparent layout is what makes [`MelinoeCell::from_mut`] possible:
reborrow a `&mut T` as `&mut MelinoeCell<'_, T>` in place — the basis for
branding pre-existing allocator storage without moving it.

## Borrowing

- [`borrow(&token)`](MelinoeCell::borrow) takes any `ReadPermit` and returns a
  [`MelinoeRef<'_, 'brand, T>`] — a shared guard that `Deref`s to `T`.
- [`borrow_mut(&mut token)`](MelinoeCell::borrow_mut) takes a `WritePermit` and
  returns a [`MelinoeMut<'_, 'brand, T>`] — an exclusive guard that
  `DerefMut`s to `T`.

Both guards are `#[repr(transparent)]` over the underlying reference, so they
are ABI-identical to the bare reference, preserve the null-pointer niche
(`Option<MelinoeRef<'_, '_, T>>` stays pointer-sized), and carry the brand
evidence in their type: a value of type `MelinoeMut<'a, 'brand, T>` is itself
proof that exclusive access to `'brand`-branded data was lawfully obtained.

```rust
extern crate melinoe;
use melinoe::{brand_scope, MelinoeCell};

brand_scope(|mut token| {
    let cell = MelinoeCell::new(0_i32);
    *cell.borrow_mut(&mut token) += 10; // exclusive write
    let snap = token.share();
    assert_eq!(*cell.borrow(snap), 10); // shared read
});
```

Notice that `borrow_mut` takes `&mut` of the *token*, not of the cell: the cell
may be held behind `&` references while the permit enforces the aliasing
contract. See the [branded cells example](examples/branded_cell.md) for the
full picture.

## Projection: MelinoeRef::map / MelinoeMut::map

A borrow guard can be narrowed onto a component of its payload **without copying
and without re-presenting the permit** — the branded analogue of
`Ref::map` / `RefMut::map`. The original capability is threaded through the
returned guard's lifetime:

```rust
extern crate melinoe;
use melinoe::{brand_scope, MelinoeCell, MelinoeMut};

struct Header { tag: u32, len: u32 }

brand_scope(|mut token| {
    let cell = MelinoeCell::new(Header { tag: 7, len: 0 });
    let mut len: MelinoeMut<'_, '_, u32> =
        MelinoeMut::map(cell.borrow_mut(&mut token), |h| &mut h.len);
    *len = 42;
    drop(len);
    assert_eq!(cell.borrow(&token).len, 42);
});
```

Projection is provided as an associated function (not a method) so it does not
collide with field/method access reached through `Deref`/`DerefMut`.

## map_split: two disjoint projections from one write permit

[`MelinoeMut::map_split`] yields two **non-overlapping** `&mut` projections
from a single write permit — e.g. two fields of a struct, or the two halves of
`slice::split_at_mut`. Both live simultaneously because the compiler can see
they point to disjoint memory. Without this primitive you would need to drop
the first `borrow_mut` before taking the second:

```rust
extern crate melinoe;
use melinoe::{brand_scope, MelinoeCell, MelinoeMut};

brand_scope(|mut token| {
    let cell = MelinoeCell::new((0_u32, 0_u32));
    let (mut a, mut b) =
        MelinoeMut::map_split(cell.borrow_mut(&mut token), |t| (&mut t.0, &mut t.1));
    *a = 1;
    *b = 2;
    drop((a, b));
    assert_eq!(*cell.borrow(&token), (1, 2));
});
```

Disjointness is the caller's `f` contract — exactly as in the standard library —
and is what makes the two simultaneous `&mut` projections sound. The shared
counterpart `MelinoeRef::map_split` splits one read guard into two independent
shared sub-guards over distinct parts of the contents.

## Cell family: zero-copy slice views

Reading or writing a branded slab one cell at a time costs a bounds check and a
permit pass per element and defeats autovectorization. [`CellSliceExt`] exposes
the whole `[MelinoeCell<'brand, T>]` as a plain `&[T]` / `&mut [T]` once a
permit is presented:

```rust
extern crate melinoe;
use melinoe::{brand_scope, CellSliceExt, MelinoeCell};

brand_scope(|mut token| {
    let cells: [MelinoeCell<'_, u32>; 4] = core::array::from_fn(|_| MelinoeCell::new(0));
    cells.borrow_slice_mut(&mut token).fill(7);       // bulk write, zero copy
    let total: u32 = cells.borrow_slice(&token).iter().sum(); // bulk read, SIMD-friendly
    assert_eq!(total, 28);
});
```

`borrow_slice`/`borrow_slice_mut` return the region as ordinary slices with
standard ergonomics — `fill`, `copy_from_slice`, `iter().sum()`, SIMD — at zero
copy and zero added cost. This is the primitive an allocator uses to
bulk-initialise or scan a slab held as branded cells. Because the trait is
implemented for the slice type itself, it applies to arrays, `Vec`s, and
sub-slices uniformly.

At the ownership boundary, these slice views feed the conditional-`Cow` helpers
of [chapter 7](cow_boundary.md); on the synchronization side,
[chapter 5](conditional_atomics.md) applies the same phase discipline to
atomics.
