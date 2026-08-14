# ADR 0001: Parallel executor capability

Status: Accepted

Change class: [major]. Delivered in 0.9.0.

## Context

The registered partition executor controls concurrent writes into raw
`MaybeUninit` result slots. Correctness requires exact-once index coverage on
normal return and a blocking lifetime contract on return or unwind, but the
public `ParallelExecutorFn` alias and safe
registration function admitted any unsafe function pointer without requiring
the integrator to discharge those obligations.

## Decision

Replace the alias with `#[repr(transparent)] ParallelExecutor`. Its unsafe
constructor is the single validation boundary; registration accepts only the
validated newtype and remains safe. The newtype is `Copy`, occupies one function
pointer, and delegates without allocation or dynamic dispatch.

Moirai constructs the capability next to its executor bridge with a safety proof
covering exact-once indexed dispatch, completion, and context lifetime. No old
alias, conversion shim, or parallel registration path remains.

## Rejected alternatives

- Making registration unsafe repeats the proof obligation at every install site
  instead of encoding the validated executor as a reusable value.
- Retaining the alias preserves the possibility of passing an unvalidated raw
  executor through safe code.
- Trait-object registration adds vtable dispatch and does not strengthen the
  exact-once or lifetime contract.

## Verification

Compile-time API shape prevents safe construction from a raw executor. Existing
value-semantic partition and panic tests exercise valid executors. Miri checks
the raw-slot lifecycle, and the Moirai conformance test verifies real scheduler
routing. `size_of::<ParallelExecutor>() == size_of::<ExecutorFn>()` is pinned by
a compile-time layout assertion.
