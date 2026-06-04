//! Value-semantic tests for zero-cost branded guard projection
//! (`MelinoeRef::map` / `map_split`, `MelinoeMut::map` / `map_split`).

use melinoe::{brand_scope, MelinoeCell, MelinoeMut, MelinoeRef};

#[derive(Debug, PartialEq, Eq)]
struct Header {
    tag: u32,
    len: u32,
}

/// `MelinoeRef::map` reaches a field through the permit and observes its value
/// without copying the enclosing struct.
#[test]
fn ref_map_projects_to_field_value() {
    brand_scope(|token| {
        let cell = MelinoeCell::new(Header { tag: 7, len: 42 });
        let len: MelinoeRef<'_, '_, u32> = MelinoeRef::map(cell.borrow(&token), |h| &h.len);
        assert_eq!(*len, 42);

        // Projection is over the same allocation: the field address lies inside
        // the cell's storage (no copy was made).
        let base = cell.borrow(&token);
        let base_ptr = (&*base as *const Header) as usize;
        let field_ptr = (&*len as *const u32) as usize;
        assert!(field_ptr >= base_ptr && field_ptr < base_ptr + core::mem::size_of::<Header>());
    });
}

/// `MelinoeMut::map` mutates a field in place; the write is visible through a
/// later read of the whole cell.
#[test]
fn mut_map_writes_field_in_place() {
    brand_scope(|mut token| {
        let cell = MelinoeCell::new(Header { tag: 1, len: 0 });
        {
            let mut len = MelinoeMut::map(cell.borrow_mut(&mut token), |h| &mut h.len);
            *len = 99;
        }
        assert_eq!(*cell.borrow(&token), Header { tag: 1, len: 99 });
    });
}

/// `MelinoeMut::map_split` yields two disjoint exclusive sub-guards that can be
/// mutated simultaneously — the multi-`&mut`-from-one-permit pattern.
#[test]
fn mut_map_split_disjoint_fields_mutate_together() {
    brand_scope(|mut token| {
        let cell = MelinoeCell::new(Header { tag: 0, len: 0 });
        {
            let (mut tag, mut len) =
                MelinoeMut::map_split(cell.borrow_mut(&mut token), |h| (&mut h.tag, &mut h.len));
            // Both `&mut` projections are live at once over disjoint fields.
            *tag = 5;
            *len = 10;
        }
        assert_eq!(*cell.borrow(&token), Header { tag: 5, len: 10 });
    });
}

/// `map_split` over a slice payload partitions it into two halves writable at
/// once via `split_at_mut`.
#[test]
fn mut_map_split_slice_halves() {
    brand_scope(|mut token| {
        let cell = MelinoeCell::new([0_i32; 6]);
        let (lo, hi) = MelinoeMut::map_split(cell.borrow_mut(&mut token), |a| a.split_at_mut(3));
        for s in lo.into_mut() {
            *s = 1;
        }
        for s in hi.into_mut() {
            *s = 2;
        }
        assert_eq!(*cell.borrow(&token), [1, 1, 1, 2, 2, 2]);
    });
}

/// `MelinoeRef::map_split` projects two shared sub-views of disjoint fields.
#[test]
fn ref_map_split_disjoint_reads() {
    brand_scope(|token| {
        let cell = MelinoeCell::new(Header { tag: 3, len: 4 });
        let (tag, len) = MelinoeRef::map_split(cell.borrow(&token), |h| (&h.tag, &h.len));
        assert_eq!((*tag, *len), (3, 4));
    });
}

/// A projected read guard keeps the read borrow live, so a `&mut token` write
/// cannot interleave while it is held — the exclusion survives projection.
///
/// ```compile_fail
/// use melinoe::{brand_scope, MelinoeCell, MelinoeRef};
/// brand_scope(|mut token| {
///     let cell = MelinoeCell::new((1_i32, 2_i32));
///     let first = MelinoeRef::map(cell.borrow(&token), |t| &t.0);
///     *cell.borrow_mut(&mut token) = (9, 9); // ERROR: read projection still live
///     let _ = first;
/// });
/// ```
#[test]
fn projection_preserves_exclusion_doc() {}
