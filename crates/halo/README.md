# halo

`halo` is the Melinoe workspace crate for protective branded collections.
Its public storage primitives delegate brand, permit, zero-copy slice, and
conditional `Cow` semantics to `melinoe` instead of carrying a second token
implementation.

The first migrated surface is `BrandedVec<'brand, T>`, a `Vec` of
`MelinoeCell<'brand, T>` with permit-gated element, slice, and `Cow` access.
With the default `std` feature, it also exposes Melinoe partition-driver
adapters for disjoint concurrent reads over `&[T]` shards and lock-free,
disjoint concurrent mutation over `&mut [T]` shards.

The remaining upstream Halo collections are tracked for staged migration in the
workspace backlog and gap audit.
