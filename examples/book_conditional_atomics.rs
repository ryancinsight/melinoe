//! Conditional atomics: plain store under exclusive ownership, atomic ops
//! under shared access — pay for synchronization only while sharing.
//!
//! [`BrandedAtomic`] selects the access cost through the token type:
//! - [`ExclusiveToken`] → plain (non-atomic) store/load — no `lock` prefix.
//! - [`SharedReadToken`] → true atomic operation — `lock xadd`, `cmpxchg`, etc.
//!
//! The ZST ordering policies [`Relaxed`], [`AcqRel`], [`SeqCst`] monomorphize
//! the call site so the ordering is embedded in the instruction selection, not
//! passed as a runtime `Ordering` argument.

#![expect(
    clippy::print_stdout,
    reason = "book example: stdout is the demonstrated output"
)]

extern crate melinoe;

use core::sync::atomic::AtomicU64;
use melinoe::{brand_scope, AcqRel, BrandedAtomic, Relaxed};

fn main() {
    // ── Plain store under exclusive permit ──
    brand_scope(|mut token| {
        let counter: BrandedAtomic<'_, AtomicU64> = BrandedAtomic::new(0);

        // Exclusive phase: plain non-atomic write (no `lock` prefix in asm).
        counter.store_exclusive(42, &mut token);

        let snap = token.share();
        // Shared phase: atomic load.
        let value = counter.load_with(snap, Relaxed);
        println!("after exclusive store: {value}");
        assert_eq!(value, 42);
    });

    // ── Fetch-add under shared permit: true atomic RMW ──
    brand_scope(|mut token| {
        let counter: BrandedAtomic<'_, AtomicU64> = BrandedAtomic::new(100);

        // Seed value via exclusive write (cheap).
        counter.store_exclusive(100, &mut token);

        let snap = token.share();

        // fetch_add under shared permit: `lock xaddq` on x86-64.
        let prev = counter.fetch_add_with(5, snap, Relaxed);
        println!(
            "fetch_add(5): prev={prev}, now={}",
            counter.load_with(snap, Relaxed)
        );
        assert_eq!(prev, 100);
        assert_eq!(counter.load_with(snap, Relaxed), 105);

        // AcqRel ordering for a publish-subscribe hand-off.
        let prev2 = counter.fetch_add_with(10, snap, AcqRel);
        println!(
            "fetch_add(10) AcqRel: prev={prev2}, now={}",
            counter.load_with(snap, Relaxed)
        );
        assert_eq!(counter.load_with(snap, Relaxed), 115);
    });

    // ── as_atomic: fall through to the standard atomic API ──
    brand_scope(|mut token| {
        let a: BrandedAtomic<'_, AtomicU64> = BrandedAtomic::new(0);
        counter_increment_interop(&a, &mut token, 7);
        let snap = token.share();
        println!("interop fetch_add: {}", a.load_with(snap, Relaxed));
        assert_eq!(a.load_with(snap, Relaxed), 7);
    });

    println!("all conditional-atomic assertions passed");
}

/// Uses `as_atomic` to hand the inner `AtomicU64` to an existing API that
/// accepts `&AtomicU64` — branded → raw interop with zero overhead.
fn counter_increment_interop<'b>(
    a: &BrandedAtomic<'b, AtomicU64>,
    token: &mut melinoe::ExclusiveToken<'b>,
    delta: u64,
) {
    let snap = token.share();
    a.as_atomic(snap)
        .fetch_add(delta, core::sync::atomic::Ordering::Relaxed);
}
