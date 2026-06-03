//! Differential equivalence between Melinoe and the runtime primitives it is
//! benchmarked against. If these pass, the `benches/access.rs` comparison is
//! over identical computations, not coincidentally-similar ones.

#![cfg(feature = "std")]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};

use melinoe::sync::sync_region_scope;
use melinoe::{brand_scope, MelinoeCell};

const N: u64 = 10_000;

/// `N` sequential increments yield `N` under every mechanism.
#[test]
fn increment_equivalence() {
    let atomic = {
        let a = AtomicU64::new(0);
        for _ in 0..N {
            a.fetch_add(1, Ordering::Relaxed);
        }
        a.load(Ordering::Relaxed)
    };

    let mutex = {
        let m = Mutex::new(0u64);
        for _ in 0..N {
            *m.lock().unwrap() += 1;
        }
        m.into_inner().unwrap()
    };

    let rwlock = {
        let l = RwLock::new(0u64);
        for _ in 0..N {
            *l.write().unwrap() += 1;
        }
        l.into_inner().unwrap()
    };

    let melinoe = brand_scope(|mut token| {
        let cell = MelinoeCell::new(0u64);
        for _ in 0..N {
            *cell.borrow_mut(&mut token) += 1;
        }
        *cell.borrow(&token)
    });

    assert_eq!(atomic, N);
    assert_eq!(mutex, N);
    assert_eq!(rwlock, N);
    assert_eq!(melinoe, N);
}

/// Sum `N` reads via `load`. Generic so each call site's closure—and the borrow
/// it captures—is monomorphized independently.
fn read_sum(load: impl Fn() -> u64) -> u64 {
    (0..N).fold(0u64, |acc, _| acc.wrapping_add(load()))
}

/// Summing `N` reads of a constant yields `N * value` under every mechanism.
#[test]
fn read_sum_equivalence() {
    const VALUE: u64 = 7;
    let expected = N * VALUE;

    let atomic = {
        let a = AtomicU64::new(VALUE);
        read_sum(|| a.load(Ordering::Relaxed))
    };
    let mutex = {
        let m = Mutex::new(VALUE);
        read_sum(|| *m.lock().unwrap())
    };
    let rwlock = {
        let l = RwLock::new(VALUE);
        read_sum(|| *l.read().unwrap())
    };
    let melinoe = brand_scope(|token| {
        let cell = MelinoeCell::new(VALUE);
        read_sum(|| *cell.borrow(&token))
    });

    assert_eq!(atomic, expected);
    assert_eq!(mutex, expected);
    assert_eq!(rwlock, expected);
    assert_eq!(melinoe, expected);
}

/// Concurrent shared reads (Melinoe `SharedReadToken` vs `RwLock`) observe the
/// same value from every thread and agree on the aggregate.
#[test]
fn concurrent_read_equivalence() {
    const THREADS: usize = 4;
    const PER: u64 = 2_000;
    const VALUE: u64 = 13;
    let expected = THREADS as u64 * PER * VALUE;

    let rwlock = {
        let l = RwLock::new(VALUE);
        std::thread::scope(|s| {
            let hs: Vec<_> = (0..THREADS)
                .map(|_| s.spawn(|| (0..PER).fold(0u64, |a, _| a + *l.read().unwrap())))
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).sum::<u64>()
        })
    };

    let melinoe = sync_region_scope(|token| {
        let cell = MelinoeCell::new(VALUE);
        let snap = token.share();
        let cell = &cell;
        std::thread::scope(|s| {
            let hs: Vec<_> = (0..THREADS)
                .map(|_| s.spawn(move || (0..PER).fold(0u64, |a, _| a + *cell.borrow(snap))))
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).sum::<u64>()
        })
    });

    assert_eq!(rwlock, expected);
    assert_eq!(melinoe, expected);
}
