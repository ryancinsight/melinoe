use core::sync::atomic::Ordering;

pub(crate) mod sealed {
    pub trait Sealed {}
}

/// ZST ordering policy for atomic load/store/swap/fetch operations.
///
/// Use this when the ordering contract is fixed by the algorithm. The policy is
/// a zero-sized type; monomorphization substitutes the associated constants at
/// compile time. The trait is sealed so downstream code cannot introduce an
/// ordering combination outside this crate's audited policy set.
pub trait AtomicOrder: sealed::Sealed + Copy {
    /// Ordering for load operations.
    const LOAD: Ordering;
    /// Ordering for store operations.
    const STORE: Ordering;
    /// Ordering for read-modify-write operations.
    const RMW: Ordering;
    /// Failure ordering for compare-exchange operations.
    const FAILURE: Ordering;
}

/// Relaxed atomic ordering policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Relaxed;

/// Acquire load / release store / acquire-release RMW ordering policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcqRel;

/// Sequentially consistent ordering policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeqCst;

impl AtomicOrder for Relaxed {
    const LOAD: Ordering = Ordering::Relaxed;
    const STORE: Ordering = Ordering::Relaxed;
    const RMW: Ordering = Ordering::Relaxed;
    const FAILURE: Ordering = Ordering::Relaxed;
}

impl AtomicOrder for AcqRel {
    const LOAD: Ordering = Ordering::Acquire;
    const STORE: Ordering = Ordering::Release;
    const RMW: Ordering = Ordering::AcqRel;
    const FAILURE: Ordering = Ordering::Acquire;
}

impl AtomicOrder for SeqCst {
    const LOAD: Ordering = Ordering::SeqCst;
    const STORE: Ordering = Ordering::SeqCst;
    const RMW: Ordering = Ordering::SeqCst;
    const FAILURE: Ordering = Ordering::SeqCst;
}

impl sealed::Sealed for Relaxed {}
impl sealed::Sealed for AcqRel {}
impl sealed::Sealed for SeqCst {}
