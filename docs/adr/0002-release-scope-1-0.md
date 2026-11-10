# ADR 0002: Melinoe release scope for 1.0

- Status: Accepted
- Date: 2026-11-10
- Class: `[arch]` `[minor]`

## Context

Melinoe (0.10.0) provides branded, multi-token phantom capabilities and branded
collection adapters for compile-time access proofs in the Atlas stack. The crate
ships tokens, cells, branded collections, and partitioning primitives but has no
milestone ladder to 1.0.

## Decision

Adopt a two-milestone ladder to 1.0 recorded in `backlog/milestones.md`:

- **M-branded-collections** (0.11.0): users construct and operate branded
  vectors/deques and execute partitioned read/write operations through
  compile-time permits.
- **M-1.0** (1.0.0): stable release of the branded capability and collection
  surface (authority-gated), with documented contracts and release gates
  satisfied.

## Alternatives

- Declare 1.0 without a capability ladder: rejected because stack policy
  requires runnable, capability-oriented milestone exit checks.
- Defer ladder creation: rejected because never-drafted ladders block roadmap
  progress to 1.0.

## Consequences

- Future feature items name the exit check they serve and carry `milestone:`
  assignments.
- Milestone closure requires exit checks passing on fetched default commits;
  1.0 release/publication remains authority-gated.
- This ADR is rewritten in place as scope evidence changes.
