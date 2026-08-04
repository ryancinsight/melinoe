# 6. Guarded and Reentrant Cells

*Chapter prose deferred — DoR item.*

<!-- Key points to cover:
  - GuardedCell<T>: one runtime boolean flag; get_mut() checks and sets the
    flag, returning &mut T on first entry and panicking on re-entry; flag is
    cleared on drop of the returned guard
  - ReentrancyCell<T>: similar to GuardedCell but returns a brand token so
    the exclusive borrow is checked at compile time (inside the guard scope),
    not just at the re-entrancy boundary
  - Use case: ambient thread-local state that must not be aliased; a single
    static mut is replaced by a GuardedCell which enforces the "one caller
    at a time" invariant at runtime with a known, one-instruction cost
  - vs. RefCell: RefCell tracks borrow count at runtime; GuardedCell tracks
    only "locked / not locked"; the difference matters when the count is
    always 0 or 1 (which it is for exclusive ambient state)
-->
