//! Conditional `Cow` boundary tests for branded slices.

#![cfg(feature = "alloc")]

use std::borrow::Cow;

use melinoe::{brand_scope, Borrowed, CellCowExt, MelinoeCell, RetainDecision, Retained};

#[test]
fn borrowed_policy_returns_zero_copy_borrow() {
    brand_scope(|token| {
        let cells: Vec<MelinoeCell<'_, u8>> = (0..4).map(MelinoeCell::new).collect();
        let cow = cells.borrow_cow_with(&token, Borrowed);

        match cow {
            Cow::Borrowed(slice) => {
                assert_eq!(slice, &[0, 1, 2, 3]);
                assert_eq!(slice.as_ptr() as usize, cells.as_ptr() as usize);
            }
            Cow::Owned(_) => panic!("Borrowed policy must not clone"),
        }
    });
}

#[test]
fn retained_policy_returns_owned_copy() {
    brand_scope(|token| {
        let cells: Vec<MelinoeCell<'_, u8>> = (0..4).map(MelinoeCell::new).collect();
        let cow = cells.borrow_cow_with(&token, Retained);

        match cow {
            Cow::Borrowed(_) => panic!("Retained policy must clone"),
            Cow::Owned(values) => {
                assert_eq!(values, vec![0, 1, 2, 3]);
                assert_ne!(values.as_ptr() as usize, cells.as_ptr() as usize);
            }
        }
    });
}

#[test]
fn runtime_decision_selects_borrow_or_retain() {
    brand_scope(|token| {
        let cells: Vec<MelinoeCell<'_, u16>> = (10..14).map(MelinoeCell::new).collect();

        let borrowed = cells.borrow_cow_if(&token, RetainDecision::Borrow);
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        assert_eq!(&*borrowed, &[10, 11, 12, 13]);

        let retained = cells.borrow_cow_if(&token, RetainDecision::Retain);
        assert!(matches!(retained, Cow::Owned(_)));
        assert_eq!(&*retained, &[10, 11, 12, 13]);
    });
}
