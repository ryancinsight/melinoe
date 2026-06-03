//! Multi-token / multi-brand integration tests: simultaneous disjoint exclusive
//! access (multi-XOR), two-brand AND-cells, and the re-entrancy gate.

use melinoe::reentrant::{GuardedCell, Reentered, ReentrancyCell};
use melinoe::{brand_scope2, brand_scope3, MelinoeCell, MelinoeCell2};

#[test]
fn two_brands_grant_simultaneous_disjoint_mut() {
    let result = brand_scope2(|mut ta, mut tb| {
        let a = MelinoeCell::new(10_u64);
        let b = MelinoeCell::new(32_u64);
        // Two live &mut into different brands held at once — multi-XOR.
        let mut ma = a.borrow_mut(&mut ta);
        let mb = b.borrow_mut(&mut tb);
        *ma += *mb;
        *a.borrow(&ta)
    });
    assert_eq!(result, 42);
}

#[test]
fn three_brands_independent() {
    let sum = brand_scope3(|mut ta, mut tb, mut tc| {
        let a = MelinoeCell::new(1_u64);
        let b = MelinoeCell::new(2_u64);
        let c = MelinoeCell::new(3_u64);
        *a.borrow_mut(&mut ta) += 10;
        *b.borrow_mut(&mut tb) += 10;
        *c.borrow_mut(&mut tc) += 10;
        *a.borrow(&ta) + *b.borrow(&tb) + *c.borrow(&tc)
    });
    assert_eq!(sum, 36);
}

#[test]
fn pair_cell_requires_both_brands() {
    let value = brand_scope2(|mut ta, mut tb| {
        let cell: MelinoeCell2<'_, '_, u64> = MelinoeCell2::new(0);
        *cell.borrow_mut(&mut ta, &mut tb) = 100;
        *cell.borrow_mut(&mut ta, &mut tb) += 23;
        *cell.borrow(&ta, &tb)
    });
    assert_eq!(value, 123);
}

#[test]
fn pair_cell_get_mut_needs_no_tokens() {
    let mut cell: MelinoeCell2<'static, 'static, i32> = MelinoeCell2::new(5);
    *cell.get_mut() += 1;
    assert_eq!(cell.into_inner(), 6);
}

#[test]
fn reentrancy_gate_grants_token_and_refuses_nested() {
    let gate = ReentrancyCell::new();
    assert!(!gate.is_active());

    let out = gate.enter(|mut token| {
        assert!(gate.is_active());
        let slot = MelinoeCell::new(0_u64);
        *slot.borrow_mut(&mut token) = 7;

        // Re-entrant acquisition is refused, not aliased.
        assert_eq!(gate.enter(|_| ()).unwrap_err(), Reentered);

        *slot.borrow(&token)
    });

    assert_eq!(out, Ok(7));
    assert!(!gate.is_active()); // cleared on exit
}

#[test]
fn reentrancy_gate_clears_flag_after_panic() {
    let gate = ReentrancyCell::new();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = gate.enter(|_token| panic!("boom"));
    }));
    assert!(r.is_err());
    // Flag was reset by the drop guard despite the unwind, so the gate is reusable.
    assert!(!gate.is_active());
    assert_eq!(gate.enter(|_| 5), Ok(5));
}

#[test]
fn reentrancy_gate_sequential_reuse() {
    let gate = ReentrancyCell::new();
    let a = gate.enter(|_| 1).unwrap();
    let b = gate.enter(|_| 2).unwrap();
    assert_eq!(a + b, 3);
}

#[test]
fn guarded_cell_exclusive_mut_and_reentry_refused() {
    let cache = GuardedCell::new(vec![1, 2, 3]);
    let len = cache
        .enter(|v| {
            v.push(4);
            // Re-entrant borrow refused, not aliased.
            assert_eq!(cache.enter(|_| ()).unwrap_err(), Reentered);
            v.len()
        })
        .unwrap();
    assert_eq!(len, 4);
    assert_eq!(cache.into_inner(), vec![1, 2, 3, 4]);
}

#[test]
fn guarded_cell_clears_flag_after_panic() {
    let cache = GuardedCell::new(0_u64);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = cache.enter(|_| panic!("boom"));
    }));
    assert!(r.is_err());
    // The hand-rolled `is_allocating` idiom would stay poisoned here; the drop
    // guard cleared it, so the cell is reusable.
    assert!(!cache.is_active());
    assert_eq!(cache.enter(|n| *n + 9), Ok(9));
}

#[test]
fn guarded_cell_unguarded_skips_flag() {
    let cache = GuardedCell::new(10_i32);
    // SAFETY: the closure does not re-enter the cell.
    let doubled = unsafe { cache.enter_unguarded(|n| *n * 2) }.unwrap();
    assert_eq!(doubled, 20);
    assert!(!cache.is_active());
}
