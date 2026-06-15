//! Value-semantic contract tests for `halo::BrandedVec`.
//!
//! The harness checks Melinoe permit-gated access, zero-copy slice layout, and
//! conditional `Cow` borrow/retain behavior.
// Test item names are behavior specifications; public API documentation is
// enforced on the library crate.
#![allow(missing_docs)]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::cell::Cell;

use halo::BrandedVec;
use melinoe::{brand_scope, Borrowed, Retained};

#[derive(Debug, Eq, PartialEq)]
struct CountedClone<'a> {
    value: u32,
    clones: &'a Cell<usize>,
}

impl Clone for CountedClone<'_> {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            value: self.value,
            clones: self.clones,
        }
    }
}

#[test]
fn slice_access_is_value_semantic_and_zero_copy() {
    brand_scope(|mut token| {
        let mut values = BrandedVec::with_capacity(4);
        values.extend([10_u32, 20, 30, 40]);

        values.as_mut_slice(&mut token)[2] = 300;

        let snapshot = token.share();
        let slice = values.as_slice(snapshot);
        let cell_ptr = values.as_cells().as_ptr().cast::<u32>();

        assert_eq!(slice, &[10, 20, 300, 40]);
        assert_eq!(slice.as_ptr(), cell_ptr);
        assert_eq!(values.len(), 4);
        assert_eq!(values.capacity(), 4);
    });
}

#[test]
fn element_access_uses_melinoe_permits() {
    brand_scope(|mut token| {
        let values = BrandedVec::from_iter([1_i32, 2, 3]);

        {
            let mut middle = match values.get_mut(1, &mut token) {
                Some(value) => value,
                None => panic!("index 1 must exist in a three-element vector"),
            };
            *middle = 22;
        }

        let first = match values.get(0, &token) {
            Some(value) => *value,
            None => panic!("index 0 must exist in a three-element vector"),
        };

        assert_eq!(first, 1);
        assert_eq!(values.as_slice(&token), &[1, 22, 3]);
    });
}

#[test]
fn cow_policies_preserve_borrow_or_clone_contract() {
    brand_scope(|token| {
        let clones = Cell::new(0);
        let values = BrandedVec::from_iter([
            CountedClone {
                value: 1,
                clones: &clones,
            },
            CountedClone {
                value: 2,
                clones: &clones,
            },
        ]);

        let borrowed = values.cow_with(&token, Borrowed);
        let borrowed_ptr = match &borrowed {
            Cow::Borrowed(slice) => slice.as_ptr(),
            Cow::Owned(_) => panic!("Borrowed policy must not allocate"),
        };
        assert_eq!(borrowed_ptr, values.as_slice(&token).as_ptr());
        assert_eq!(clones.get(), 0);

        let retained = values.cow_with(&token, Retained);
        match &retained {
            Cow::Borrowed(_) => panic!("Retained policy must allocate owned storage"),
            Cow::Owned(owned) => {
                assert_eq!(
                    owned.iter().map(|item| item.value).collect::<Vec<_>>(),
                    [1, 2]
                );
                assert_ne!(owned.as_ptr(), values.as_slice(&token).as_ptr());
            }
        }
        assert_eq!(clones.get(), 2);
    });
}

#[test]
fn into_vec_returns_owned_values() {
    brand_scope(|_token| {
        let values = BrandedVec::from_iter([5_u8, 8, 13]);
        assert_eq!(values.into_vec(), [5, 8, 13]);
    });
}
