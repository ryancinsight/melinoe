//! Apollo-facing zero-copy boundary contracts.

#![cfg(feature = "alloc")]

use std::borrow::Cow;
use std::cell::Cell;
use std::rc::Rc;

use melinoe::{brand_scope, Borrowed, CellCowExt, MelinoeCell, Retained};

#[derive(Debug, Eq, PartialEq)]
struct Sample {
    value: usize,
    clones: Rc<Cell<usize>>,
}

impl Clone for Sample {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            value: self.value,
            clones: Rc::clone(&self.clones),
        }
    }
}

#[test]
fn apollo_scratch_borrow_boundary_is_zero_copy() {
    brand_scope(|token| {
        let clones = Rc::new(Cell::new(0));
        let cells: Vec<MelinoeCell<'_, Sample>> = (0..4)
            .map(|value| {
                MelinoeCell::new(Sample {
                    value,
                    clones: Rc::clone(&clones),
                })
            })
            .collect();

        let cow = cells.borrow_cow_with(&token, Borrowed);

        match cow {
            Cow::Borrowed(slice) => {
                assert_eq!(
                    slice.iter().map(|sample| sample.value).collect::<Vec<_>>(),
                    vec![0, 1, 2, 3]
                );
                assert_eq!(slice.as_ptr() as usize, cells.as_ptr() as usize);
                assert_eq!(clones.get(), 0);
            }
            Cow::Owned(_) => panic!("Apollo borrowed scratch boundary must not clone"),
        }
    });
}

#[test]
fn apollo_scratch_retain_boundary_clones_once_per_element() {
    brand_scope(|token| {
        let clones = Rc::new(Cell::new(0));
        let cells: Vec<MelinoeCell<'_, Sample>> = (10..14)
            .map(|value| {
                MelinoeCell::new(Sample {
                    value,
                    clones: Rc::clone(&clones),
                })
            })
            .collect();

        let cow = cells.borrow_cow_with(&token, Retained);

        match cow {
            Cow::Borrowed(_) => panic!("Apollo retained scratch boundary must own"),
            Cow::Owned(values) => {
                assert_eq!(
                    values.iter().map(|sample| sample.value).collect::<Vec<_>>(),
                    vec![10, 11, 12, 13]
                );
                assert_ne!(values.as_ptr() as usize, cells.as_ptr() as usize);
                assert_eq!(clones.get(), values.len());
            }
        }
    });
}
