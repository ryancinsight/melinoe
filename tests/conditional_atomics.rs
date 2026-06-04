//! Conditional-atomic (`BrandedAtomic`) tests: plain access in the exclusive
//! phase, atomic access in the shared phase, and the cross-thread transition.

use core::sync::atomic::Ordering;

use melinoe::atomic::BrandedAtomic;
use melinoe::sync::sync_region_scope;
use melinoe::{brand_scope, ExclusiveToken};

#[test]
fn plain_exclusive_then_atomic_shared_single_thread() {
    brand_scope(|mut token| {
        let a: BrandedAtomic<'_, core::sync::atomic::AtomicU64> = BrandedAtomic::new(0);

        // Exclusive phase: plain, non-atomic.
        a.store_exclusive(100, &mut token);
        a.with_exclusive(&mut token, |v| *v += 23);
        assert_eq!(a.load_exclusive(&mut token), 123);

        // Shared phase: atomic (single-threaded here, but via the atomic path).
        let snap = token.share();
        assert_eq!(a.fetch_add(7, snap, Ordering::Relaxed), 123);
        assert_eq!(a.load(snap, Ordering::Relaxed), 130);
    });
}

#[test]
fn concurrent_atomic_writes_after_exclusive_init() {
    const THREADS: usize = 8;
    const PER: u64 = 10_000;

    let total = sync_region_scope(|mut token| {
        let a: BrandedAtomic<'_, core::sync::atomic::AtomicU64> = BrandedAtomic::new(0);

        // Exclusive phase: plain init (no atomics).
        a.store_exclusive(1000, &mut token);

        // Shared phase: many threads CAS/fetch-add concurrently.
        let snap = token.share();
        let a_ref = &a;
        std::thread::scope(|s| {
            for _ in 0..THREADS {
                s.spawn(move || {
                    for _ in 0..PER {
                        a_ref.fetch_add(1, snap, Ordering::Relaxed);
                    }
                });
            }
        });

        // Back to the exclusive phase (the scope join re-grants `&mut token`):
        // a plain read observes every atomic increment.
        a.load_exclusive(&mut token)
    });

    assert_eq!(total, 1000 + THREADS as u64 * PER);
}

#[test]
fn compare_exchange_in_shared_phase() {
    brand_scope(|token| {
        let a: BrandedAtomic<'_, core::sync::atomic::AtomicUsize> = BrandedAtomic::new(5);
        let snap = token.share();
        assert_eq!(
            a.compare_exchange(5, 9, Ordering::AcqRel, Ordering::Acquire, snap),
            Ok(5)
        );
        assert_eq!(
            a.compare_exchange(5, 0, Ordering::AcqRel, Ordering::Acquire, snap),
            Err(9)
        );
        assert_eq!(a.load(snap, Ordering::Relaxed), 9);
    });
}

#[test]
fn bool_flag_and_get_mut() {
    brand_scope(|mut token| {
        let flag: BrandedAtomic<'_, core::sync::atomic::AtomicBool> = BrandedAtomic::new(false);
        flag.store_exclusive(true, &mut token);
        assert!(flag.load(token.share(), Ordering::Relaxed));
    });

    let mut owned: BrandedAtomic<'static, core::sync::atomic::AtomicU32> = BrandedAtomic::new(1);
    *owned.get_mut() += 41; // unique ownership, no token
    assert_eq!(owned.into_inner(), 42);
}

/// A `WritePermit` (plain access) cannot be formed while a shared token is live,
/// so plain and atomic access can never overlap.
#[test]
fn exclusive_token_send_posture() {
    fn assert_send_sync<T: Send + Sync>() {}
    // The cell is `Send + Sync` (it wraps a real atomic), enabling shared-phase
    // cross-thread access.
    assert_send_sync::<BrandedAtomic<'static, core::sync::atomic::AtomicU64>>();
    // Compile-time confirmation the exclusive token threads through write access.
    let _ = |t: &mut ExclusiveToken<'static>,
             a: &BrandedAtomic<'static, core::sync::atomic::AtomicU64>| {
        a.store_exclusive(1, t);
    };
}
