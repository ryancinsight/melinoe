//! Value-semantic integration tests for the Melinoe branding model.

use melinoe::sync::{sync_region_scope, thread_local_scope};
use melinoe::{brand_scope, MelinoeCell};

#[test]
fn exclusive_write_then_shared_reads_observe_value() {
    let observed = brand_scope(|mut token| {
        let cell = MelinoeCell::new(100_u64);
        *cell.borrow_mut(&mut token) += 23;

        // Copyable read permit fans out to many independent reads.
        let snap = token.share();
        let a = *cell.borrow(snap);
        let b = *cell.borrow(snap);
        (a, b)
    });
    assert_eq!(observed, (123, 123));
}

#[test]
fn one_token_governs_every_cell_in_the_region() {
    let sum = sync_region_scope(|mut token| {
        let cells: [MelinoeCell<'_, i32>; 4] = [
            MelinoeCell::new(0),
            MelinoeCell::new(0),
            MelinoeCell::new(0),
            MelinoeCell::new(0),
        ];

        for (i, cell) in cells.iter().enumerate() {
            *cell.borrow_mut(&mut token) = i as i32 * 10;
        }
        cells.iter().map(|c| *c.borrow(&token)).sum::<i32>()
    });
    assert_eq!(sum, 60);
}

#[test]
fn replace_returns_previous_value() {
    let prev = brand_scope(|mut token| {
        let cell = MelinoeCell::new(String::from("old"));
        cell.replace(String::from("new"), &mut token)
    });
    assert_eq!(prev, "old");
}

#[test]
fn get_mut_needs_no_token() {
    let mut cell = MelinoeCell::<'static, _>::new(7_i32);
    *cell.get_mut() += 1;
    assert_eq!(cell.into_inner(), 8);
}

#[test]
fn from_mut_brands_existing_storage_in_place() {
    brand_scope(|token| {
        let mut backing = 41_i32;
        let cell = MelinoeCell::from_mut(&mut backing);
        // Read through the branded view…
        assert_eq!(*cell.borrow(&token), 41);
    });
    // `backing` is untouched in value, only reborrowed.
}

#[test]
fn thread_local_brand_supports_full_read_write_cycle() {
    let final_value = thread_local_scope(|mut token| {
        let cell = MelinoeCell::new(vec![1, 2, 3]);
        cell.borrow_mut(&mut token).push(4);
        cell.borrow(&token).iter().sum::<i32>()
    });
    assert_eq!(final_value, 10);
}

#[test]
fn guards_deref_and_unwrap() {
    brand_scope(|mut token| {
        let cell = MelinoeCell::new(5_i32);
        {
            let mut guard = cell.borrow_mut(&mut token);
            *guard *= 4;
            assert_eq!(*guard.into_mut(), 20);
        }
        let r = cell.borrow(&token);
        assert_eq!(*r.into_ref(), 20);
    });
}
