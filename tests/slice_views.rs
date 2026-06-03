//! Zero-copy branded-slice view tests (`CellSliceExt`).

use melinoe::{brand_scope, CellSliceExt, MelinoeCell};

#[test]
fn borrow_slice_is_zero_copy() {
    brand_scope(|token| {
        let cells: [MelinoeCell<'_, u32>; 4] = core::array::from_fn(|i| MelinoeCell::new(i as u32));

        let view = cells.borrow_slice(&token);
        // Same backing memory — no copy was made.
        assert_eq!(view.as_ptr() as usize, cells.as_ptr() as usize);
        assert_eq!(view, &[0, 1, 2, 3]);
    });
}

#[test]
fn borrow_slice_mut_bulk_writes() {
    brand_scope(|mut token| {
        let cells: [MelinoeCell<'_, u64>; 6] = core::array::from_fn(|_| MelinoeCell::new(0));

        // Bulk, native-slice mutation through the exclusive token.
        cells
            .borrow_slice_mut(&mut token)
            .copy_from_slice(&[10, 20, 30, 40, 50, 60]);
        cells
            .borrow_slice_mut(&mut token)
            .iter_mut()
            .for_each(|x| *x += 1);

        let sum: u64 = cells.borrow_slice(&token).iter().sum();
        assert_eq!(sum, 10 + 20 + 30 + 40 + 50 + 60 + 6);
    });
}

#[test]
fn slice_view_round_trips_with_cell_borrow() {
    brand_scope(|mut token| {
        let cells: Vec<MelinoeCell<'_, i32>> = (0..8).map(|_| MelinoeCell::new(0)).collect();

        // Write via the bulk slice view…
        for (i, slot) in cells.borrow_slice_mut(&mut token).iter_mut().enumerate() {
            *slot = i as i32 * 3;
        }
        // …read back via the per-cell token API: the two views agree.
        let snap = token.share();
        for (i, c) in cells.iter().enumerate() {
            assert_eq!(*c.borrow(snap), i as i32 * 3);
        }
    });
}

#[test]
fn empty_region_views_are_valid() {
    brand_scope(|mut token| {
        let cells: [MelinoeCell<'_, u8>; 0] = [];
        assert!(cells.borrow_slice(&token).is_empty());
        assert!(cells.borrow_slice_mut(&mut token).is_empty());
    });
}
