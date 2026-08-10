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

/// Internal ordering source shared by runtime and compile-time policies.
///
/// Runtime [`Ordering`] values preserve their caller-selected order for every
/// operation. A sealed [`AtomicOrder`] policy instead resolves each operation
/// to an associated constant, so the same generic operation body
/// monomorphizes to the fixed ordering without retaining a runtime policy
/// branch.
pub(crate) trait OrderingSource: Copy {
    /// Resolve a load ordering.
    fn load_order(self) -> Ordering;
    /// Resolve a store ordering.
    fn store_order(self) -> Ordering;
    /// Resolve a read-modify-write ordering.
    fn rmw_order(self) -> Ordering;
    /// Resolve a failed-update ordering.
    fn failure_order(self) -> Ordering;
}

impl OrderingSource for Ordering {
    #[inline]
    fn load_order(self) -> Ordering {
        self
    }

    #[inline]
    fn store_order(self) -> Ordering {
        self
    }

    #[inline]
    fn rmw_order(self) -> Ordering {
        self
    }

    #[inline]
    fn failure_order(self) -> Ordering {
        self
    }
}

impl<O: AtomicOrder> OrderingSource for O {
    #[inline]
    fn load_order(self) -> Ordering {
        O::LOAD
    }

    #[inline]
    fn store_order(self) -> Ordering {
        O::STORE
    }

    #[inline]
    fn rmw_order(self) -> Ordering {
        O::RMW
    }

    #[inline]
    fn failure_order(self) -> Ordering {
        O::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_sources_preserve_runtime_and_policy_roles() {
        assert_eq!(Ordering::Relaxed.load_order(), Ordering::Relaxed);
        assert_eq!(Ordering::Acquire.store_order(), Ordering::Acquire);
        assert_eq!(Ordering::Release.rmw_order(), Ordering::Release);
        assert_eq!(Ordering::AcqRel.failure_order(), Ordering::AcqRel);

        assert_eq!(Relaxed.load_order(), Ordering::Relaxed);
        assert_eq!(AcqRel.store_order(), Ordering::Release);
        assert_eq!(AcqRel.rmw_order(), Ordering::AcqRel);
        assert_eq!(AcqRel.failure_order(), Ordering::Acquire);
        assert_eq!(SeqCst.failure_order(), Ordering::SeqCst);
    }
}
