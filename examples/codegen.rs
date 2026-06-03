//! Codegen probe: confirms token-mediated access lowers to the same store as a
//! raw pointer write (run via `cargo rustc --release --example codegen -- --emit asm`).
use core::cell::UnsafeCell;
use melinoe::{ExclusiveToken, MelinoeCell};

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

fn main() {
    // The probes are never called (their `'static` brands are not callable with
    // a real scope token); `#[no_mangle]` forces their symbols to be emitted, so
    // an empty `main` suffices. Inspect the emitted `.s` for the bodies.
}
