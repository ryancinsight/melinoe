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

Replace the alias with the `unsafe` `ParallelExecutor` trait. Its associated
`run_indexed` function is the single scheduler entry point, and
`register_parallel_executor::<E>()` generates a monomorphized shim that stores
one function pointer without allocation or dynamic dispatch. `Executor` remains
the transparent, copyable function-pointer capability for integrations that
need to hold the shim before registration.

The trait has no receiver. The registration slot stores no scheduler value, so
an implementation cannot observe fabricated storage or rely on a receiver
lifetime that Melinoe does not own. Moirai discharges the exact-once indexed
dispatch, blocking completion, and context-lifetime proof at its bridge. No old
alias, conversion shim, or parallel registration path remains.

### Revision — 2026-09-13

The first trait draft fabricated `&E` by casting a static unit value. That was
unsound for any non-zero-sized implementation even when its method did not read
the receiver. The associated-function form removes that invalid reference
construction. A non-zero-sized executor registration test exercises the public
boundary.

## Rejected alternatives

- Keeping a receiver and requiring every implementation to be zero-sized would
  leave the invalid-reference proof at the generic shim and make the restriction
  unenforceable for downstream crates.
- Retaining the alias preserves the possibility of passing an unvalidated raw
  executor through safe code.
- Trait-object registration adds vtable dispatch and does not strengthen the
  exact-once or lifetime contract.

## Verification

Compile-time API shape prevents safe construction from a raw executor. Existing
value-semantic partition and panic tests exercise valid executors, including a
non-zero-sized implementation. Miri checks the raw-slot lifecycle, and the
Moirai conformance test verifies real scheduler routing. The transparent
`Executor` capability remains pinned to one function pointer by a compile-time
layout assertion.
