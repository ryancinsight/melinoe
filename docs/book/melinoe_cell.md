# 4. MelinoeCell and Borrow Guards

*Chapter prose deferred — DoR item.*

<!-- Key points to cover:
  - MelinoeCell<'brand, T>: transparent wrapper; size_of = size_of::<T>
  - borrow(&token) → MelinoeRef<'_, T>: read guard; Deref<Target=T>
  - borrow_mut(&mut token) → MelinoeMut<'_, T>: write guard; DerefMut
  - MelinoeRef::map / MelinoeMut::map: project onto a field without re-
    presenting the permit (branded analogue of Ref::map / RefMut::map)
  - MelinoeMut::map_split: two disjoint &mut projections from one write
    permit; the borrow checker can prove they cover distinct memory
  - Cell family: slice operations via CellSliceExt::borrow_slice(tok)
    → &[T] zero-copy view of a &[MelinoeCell<'b, T>]
-->
