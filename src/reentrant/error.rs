use core::fmt;

/// Returned by [`ReentrancyCell::enter`](crate::reentrant::ReentrancyCell::enter) when the gate is already held — i.e. a
/// re-entrant call on the same thread. Callers take a fallback path rather than
/// aliasing the guarded state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Reentered;

impl fmt::Display for Reentered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("re-entrant ReentrancyCell::enter (gate already held on this thread)")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Reentered {}
