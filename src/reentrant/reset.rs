use crate::reentrant::error::Reentered;
use core::cell::Cell;

/// Holds a gate flag `true` for its lifetime and clears it on scope exit,
/// including a panic unwind. The single point where the re-entrancy flag is
/// acquired and released, shared by both gate types.
pub(super) struct Reset<'a>(&'a Cell<bool>);

impl<'a> Reset<'a> {
    /// Arm the gate, returning the clearing guard, or [`Reentered`] if it is
    /// already held.
    #[inline]
    pub(super) fn acquire(active: &'a Cell<bool>) -> Result<Self, Reentered> {
        if active.get() {
            return Err(Reentered);
        }
        active.set(true);
        Ok(Self(active))
    }
}

impl Drop for Reset<'_> {
    #[inline]
    fn drop(&mut self) {
        self.0.set(false);
    }
}
