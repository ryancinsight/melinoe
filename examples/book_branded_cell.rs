//! Brand-scoped cells: zero-cost interior mutability without runtime checks.
//!
//! A [`brand_scope`] mints one [`ExclusiveToken`] for a fresh, compiler-unique
//! lifetime brand.  [`MelinoeCell`]s created inside that scope reveal their
//! contents only to a matching token; the borrow checker enforces aliasing
//! rules across the *entire cell family* with no runtime flags.
//!
//! [`MelinoeMut::map_split`] projects one write permit onto two disjoint
//! fields, so both fields can be mutated in the same expression.

#![expect(
    clippy::print_stdout,
    reason = "book example: stdout is the demonstrated output"
)]

extern crate melinoe;

use melinoe::{brand_scope, MelinoeCell, MelinoeMut};

fn main() {
    // ── Exclusive write followed by shared read ──
    brand_scope(|mut token| {
        let counter = MelinoeCell::new(0_u64);
        let step = MelinoeCell::new(7_u64);

        // Mutate through the exclusive (write) permit.
        *counter.borrow_mut(&mut token) += *step.borrow_mut(&mut token);
        *counter.borrow_mut(&mut token) += *step.borrow_mut(&mut token);

        // Fan-out shared (read) permit: `token.share()` returns a `Copy` snapshot.
        let snap = token.share();
        println!(
            "counter = {}, step = {}",
            *counter.borrow(snap),
            *step.borrow(snap)
        );
        assert_eq!(*counter.borrow(snap), 14);
        assert_eq!(*step.borrow(snap), 7);
    });

    // ── map_split: two disjoint field writers from one permit ──
    brand_scope(|mut token| {
        let pair = MelinoeCell::new((0_u32, 0_u32));

        {
            let (mut left, mut right) =
                MelinoeMut::map_split(pair.borrow_mut(&mut token), |t| (&mut t.0, &mut t.1));
            *left = 42;
            *right = 99;
        }

        let snap = token.share();
        println!("pair = {:?}", *pair.borrow(snap));
        assert_eq!(*pair.borrow(snap), (42, 99));
    });

    // ── Nested brands: two independent exclusion domains ──
    brand_scope(|mut t1| {
        brand_scope(|mut t2| {
            let a = MelinoeCell::new(100_i32);
            let b = MelinoeCell::new(200_i32);

            // `t1` governs `a`, `t2` governs `b`; both are live at once.
            *a.borrow_mut(&mut t1) += 1;
            *b.borrow_mut(&mut t2) += 1;

            let s1 = t1.share();
            let s2 = t2.share();
            println!("a = {}, b = {}", *a.borrow(s1), *b.borrow(s2));
            assert_eq!(*a.borrow(s1), 101);
            assert_eq!(*b.borrow(s2), 201);
        });
    });

    println!("all branded-cell assertions passed");
}
