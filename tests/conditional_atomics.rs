//! Conditional-atomic (`BrandedAtomic`) tests: plain access in the exclusive
//! phase, atomic access in the shared phase, and the cross-thread transition.

use core::sync::atomic::Ordering;

use melinoe::atomic::{AcqRel, BrandedAtomic, Relaxed, SeqCst};
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
fn zst_ordering_policies_match_runtime_ordering_paths() {
    brand_scope(|token| {
        let a: BrandedAtomic<'_, core::sync::atomic::AtomicU64> = BrandedAtomic::new(1);
        let snap = token.share();

        assert_eq!(a.load_with(snap, Relaxed), 1);
        a.store_with(2, snap, Relaxed);
        assert_eq!(a.swap_with(3, snap, AcqRel), 2);
        assert_eq!(a.fetch_add_with(4, snap, SeqCst), 3);
        assert_eq!(a.fetch_sub_with(2, snap, Relaxed), 7);
        assert_eq!(a.fetch_and_with(0b0111, snap, Relaxed), 5);
        assert_eq!(a.fetch_or_with(0b1000, snap, Relaxed), 5);
        assert_eq!(a.compare_exchange_with(13, 21, snap, AcqRel), Ok(13));
        assert_eq!(a.load_with(snap, SeqCst), 21);
    });
}

#[test]
fn from_mut_brands_existing_atomic_in_place() {
    use core::sync::atomic::AtomicU64;
    let mut raw = AtomicU64::new(40);
    brand_scope(|mut token| {
        let branded: &mut BrandedAtomic<'_, AtomicU64> = BrandedAtomic::from_mut(&mut raw);
        branded.with_exclusive(&mut token, |v| *v += 2);
        assert_eq!(branded.load(token.share(), Ordering::Relaxed), 42);
    });
    // The original atomic carries the result — no copy was made.
    assert_eq!(raw.load(Ordering::Relaxed), 42);
}

#[test]
fn raw_atomic_views_are_zero_copy_and_value_preserving() {
    use core::sync::atomic::AtomicU64;

    brand_scope(|token| {
        let branded: BrandedAtomic<'_, AtomicU64> = BrandedAtomic::new(7);
        let snap = token.share();

        let raw = branded.as_atomic(snap);
        assert_eq!(raw.load(Ordering::Relaxed), 7);
        assert_eq!(
            core::ptr::from_ref(raw) as usize,
            core::ptr::addr_of!(branded) as usize
        );
        raw.store(11, Ordering::Relaxed);
        assert_eq!(branded.load(snap, Ordering::Relaxed), 11);
    });

    let mut owned: BrandedAtomic<'static, AtomicU64> = BrandedAtomic::new(3);
    owned.as_atomic_mut().store(5, Ordering::Relaxed);
    let raw = owned.into_atomic();
    assert_eq!(raw.load(Ordering::Relaxed), 5);
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

#[test]
fn test_new_branded_atomic_operations() {
    brand_scope(|token| {
        let a: BrandedAtomic<'_, core::sync::atomic::AtomicU64> = BrandedAtomic::new(0b1100);
        let snap = token.share();

        // 1. fetch_xor and fetch_xor_with
        assert_eq!(a.fetch_xor(0b1010, snap, Ordering::Relaxed), 0b1100); // 0b1100 ^ 0b1010 = 0b0110
        assert_eq!(a.fetch_xor_with(0b0011, snap, Relaxed), 0b0110); // 0b0110 ^ 0b0011 = 0b0101

        // 2. fetch_nand and fetch_nand_with
        // current value is 0b0101
        assert_eq!(a.fetch_nand(0b1111, snap, Ordering::Relaxed), 0b0101); // !(0b0101 & 0b1111) = !0b0101 = 0b1010 (on 64 bits: !5 = -6)
                                                                           // Let's store a clean value to test nand_with easily
        a.store(0b0101, snap, Ordering::Relaxed);
        assert_eq!(a.fetch_nand_with(0b0011, snap, Relaxed), 0b0101); // !(0b0101 & 0b0011) = !(0b0001) = 0xffff_ffff_ffff_fffe

        // 3. fetch_max and fetch_max_with
        a.store(10, snap, Ordering::Relaxed);
        assert_eq!(a.fetch_max(5, snap, Ordering::Relaxed), 10); // max(10, 5) = 10, value stays 10
        assert_eq!(a.fetch_max_with(20, snap, Relaxed), 10); // max(10, 20) = 20, value becomes 20
        assert_eq!(a.load(snap, Ordering::Relaxed), 20);

        // 4. fetch_min and fetch_min_with
        assert_eq!(a.fetch_min(30, snap, Ordering::Relaxed), 20); // min(20, 30) = 20, value stays 20
        assert_eq!(a.fetch_min_with(10, snap, Relaxed), 20); // min(20, 10) = 10, value becomes 10
        assert_eq!(a.load(snap, Ordering::Relaxed), 10);

        // 5. fetch_update and fetch_update_with (integer)
        assert_eq!(
            a.fetch_update(Ordering::Relaxed, Ordering::Relaxed, snap, |v| Some(v + 5)),
            Ok(10)
        );
        assert_eq!(a.load(snap, Ordering::Relaxed), 15);
        assert_eq!(a.fetch_update_with(snap, Relaxed, |v| Some(v * 2)), Ok(15));
        assert_eq!(a.load(snap, Ordering::Relaxed), 30);

        // 6. fetch_update on AtomicBool
        let flag: BrandedAtomic<'_, core::sync::atomic::AtomicBool> = BrandedAtomic::new(false);
        assert_eq!(
            flag.fetch_update(Ordering::Relaxed, Ordering::Relaxed, snap, |v| Some(!v)),
            Ok(false)
        );
        assert!(flag.load(snap, Ordering::Relaxed));
        assert_eq!(
            flag.fetch_update_with(snap, Relaxed, |v| Some(!v)),
            Ok(true)
        );
        assert!(!flag.load(snap, Ordering::Relaxed));
    });
}
