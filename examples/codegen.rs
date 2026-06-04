//! Codegen probe: confirms every branded access path lowers to the same machine
//! code as the equivalent raw pointer / atomic op — the zero-cost claim, made
//! checkable. Run:
//!
//! ```sh
//! cargo rustc --release --example codegen -- --emit asm
//! # then read target/release/examples/codegen-*.s
//! ```
//!
//! Each probe is `#[no_mangle] #[inline(never)]` so its symbol is emitted and
//! the body can be read in isolation; the matching `raw_*` probe is the speed
//! limit. The probes are never called (their `'static` brands are not callable
//! with a real scope token), so an empty `main` suffices.
//!
//! # Verified results (release, x86-64)
//!
//! Every branded path lowers to the *same* machine code as its raw equivalent —
//! so much so that the linker folds them into one symbol:
//!
//! * `melinoe_write` = `branded_atomic_plain_store` — a single plain `movq`
//!   store (a branded `&mut` write and a `BrandedAtomic` exclusive store are
//!   byte-identical to a raw `UnsafeCell` write; no atomic, no flag).
//! * `melinoe_read` — a single `movq` load.
//! * `raw_atomic_fetch_add` = `branded_atomic_fetch_add` — one `lock xaddq`
//!   (the shared atomic path is a real atomic, identical to raw).
//! * `melinoe_slice_sum` — instruction-for-instruction identical to
//!   `raw_slice_sum` (the same vectorised reduction over a native `&[u64]`).
//! * `guarded_get_mut` — a plain load/inc/store; the re-entrancy flag occupies a
//!   byte but is not touched on the access path.
//!
//! The branding, permits, and phantom markers leave **no trace** in the emitted
//! code. This is the machine-code-identity tier of the zero-cost claim.
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU64, Ordering};

use melinoe::atomic::BrandedAtomic;
use melinoe::reentrant::GuardedCell;
use melinoe::{CellSliceExt, ExclusiveToken, MelinoeCell, SharedReadToken};

// ── Single-cell token-mediated access vs raw `UnsafeCell` ──

#[no_mangle]
#[inline(never)]
fn melinoe_write(cell: &MelinoeCell<'static, u64>, tok: &mut ExclusiveToken<'static>, v: u64) {
    *cell.borrow_mut(tok) = v;
}

#[no_mangle]
#[inline(never)]
fn raw_write(p: &UnsafeCell<u64>, v: u64) {
    // SAFETY: exclusive access by contract of this probe.
    unsafe { *p.get() = v };
}

#[no_mangle]
#[inline(never)]
fn melinoe_read(cell: &MelinoeCell<'static, u64>, tok: &ExclusiveToken<'static>) -> u64 {
    *cell.borrow(tok)
}

// ── Zero-copy slice view vs raw slice access ──

#[no_mangle]
#[inline(never)]
fn melinoe_slice_sum(
    cells: &[MelinoeCell<'static, u64>],
    tok: SharedReadToken<'static, 'static>,
) -> u64 {
    cells.borrow_slice(tok).iter().sum()
}

#[no_mangle]
#[inline(never)]
fn raw_slice_sum(s: &[u64]) -> u64 {
    s.iter().sum()
}

// ── Conditional atomic: exclusive (plain) vs raw store, shared (atomic) vs raw ──

#[no_mangle]
#[inline(never)]
fn branded_atomic_plain_store(
    a: &BrandedAtomic<'static, AtomicU64>,
    tok: &mut ExclusiveToken<'static>,
    v: u64,
) {
    a.store_exclusive(v, tok);
}

#[no_mangle]
#[inline(never)]
fn branded_atomic_fetch_add(
    a: &BrandedAtomic<'static, AtomicU64>,
    tok: SharedReadToken<'static, 'static>,
    v: u64,
) -> u64 {
    a.fetch_add(v, tok, Ordering::Relaxed)
}

#[no_mangle]
#[inline(never)]
fn raw_atomic_fetch_add(a: &AtomicU64, v: u64) -> u64 {
    a.fetch_add(v, Ordering::Relaxed)
}

// ── Guarded-cell access vs raw `UnsafeCell` ──

#[no_mangle]
#[inline(never)]
fn guarded_get_mut(cell: &mut GuardedCell<u64>) -> u64 {
    let v = cell.get_mut();
    *v += 1;
    *v
}

fn main() {}
