//! Cross-thread integration tests: the static thread-safety proofs in action.
//!
//! These exercise the auto-trait posture of each token family and the
//! `Send`/`Sync` impls of [`MelinoeCell`], which together constitute the
//! crate's compile-time thread-safety guarantees.

#![cfg(feature = "std")]

use std::thread;

use melinoe::sync::{scope_exclusive, sync_region_scope, SyncRegionToken, ThreadLocalToken};
use melinoe::{brand_scope, ExclusiveToken, MelinoeCell, SharedReadToken};

/// Compile-time assertion helpers.
fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

#[test]
fn token_autotrait_posture_is_correct() {
    // Portable / shareable tokens are `Send + Sync`.
    assert_send::<ExclusiveToken<'static>>();
    assert_sync::<ExclusiveToken<'static>>();
    assert_send::<SyncRegionToken<'static>>();
    assert_sync::<SyncRegionToken<'static>>();
    assert_send::<SharedReadToken<'static, 'static>>();
    assert_sync::<SharedReadToken<'static, 'static>>();

    // A `Send + Sync` payload makes the cell `Send + Sync`.
    assert_send::<MelinoeCell<'static, u64>>();
    assert_sync::<MelinoeCell<'static, u64>>();
}

#[test]
fn exclusive_handoff_runs_branded_work_on_a_worker_thread() {
    // `scope_exclusive` moves the sole write capability to a spawned thread.
    let result = scope_exclusive(|mut token| {
        let cell = MelinoeCell::new(0_i64);
        for i in 1..=10 {
            *cell.borrow_mut(&mut token) += i;
        }
        *cell.borrow(&token)
    });
    assert_eq!(result, 55);
}

#[test]
fn shared_fan_out_reads_concurrently_across_threads() {
    // Send the region token + share `&cells` to many readers at once.
    let total = sync_region_scope(|token| {
        let cells: Vec<MelinoeCell<'_, u32>> = (0..8).map(MelinoeCell::new).collect();
        let cells_ref = &cells;
        let token_ref = &token;

        thread::scope(|scope| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    scope.spawn(move || cells_ref.iter().map(|c| *c.borrow(token_ref)).sum::<u32>())
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).sum::<u32>()
        })
    });
    // Four threads each summing 0..8 = 28.
    assert_eq!(total, 28 * 4);
}

#[test]
fn region_share_fans_read_capability_across_threads() {
    // `SyncRegionToken::share` mints a `Copy + Send + Sync` read token.
    let total = sync_region_scope(|token| {
        let cells: Vec<MelinoeCell<'_, u32>> = (1..=5).map(MelinoeCell::new).collect();
        let snap = token.share();
        let cells_ref = &cells;

        thread::scope(|scope| {
            (0..3)
                .map(|_| {
                    scope.spawn(move || cells_ref.iter().map(|c| *c.borrow(snap)).sum::<u32>())
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|h| h.join().unwrap())
                .sum::<u32>()
        })
    });
    // Three threads each summing 1..=5 = 15.
    assert_eq!(total, 15 * 3);
}

#[test]
fn brand_scope_result_returns_to_caller() {
    let v = brand_scope(|mut token| {
        let cell = MelinoeCell::new(3_usize);
        *cell.borrow_mut(&mut token) = 99;
        *cell.borrow(&token)
    });
    assert_eq!(v, 99);
}

/// Negative trait check: a `ThreadLocalToken` must be neither `Send` nor `Sync`.
/// We cannot assert `!Send` at runtime, but a `static_assertions`-style
/// compile-fail doctest is impossible here; instead we document the intent and
/// rely on the type's `*const ()` phantom (covered by `compile_fail` doctests in
/// the crate root if extended). This test merely confirms the value is usable.
#[test]
fn thread_local_token_is_constructible_and_usable() {
    let n = melinoe::sync::thread_local_scope(|mut token: ThreadLocalToken<'_>| {
        let cell = MelinoeCell::new(0_i32);
        *cell.borrow_mut(&mut token) = 7;
        *cell.borrow(&token)
    });
    assert_eq!(n, 7);
}
