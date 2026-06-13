# Checklist — melinoe

Target version: 0.7.0

## Current micro-sprint (0.7.0)

- [x] [patch] Split `region` into `shard` and `chunks` leaf modules, preserving
  public re-exports while separating shard capability logic from exact-size
  chunk iteration.
- [x] [patch] Harden `partition_driver` benchmark inputs with `black_box`,
  rerun the group, and refresh `BENCHMARKS.md` partition-driver figures.
- [x] [minor] Add `thread_cached!` as the shared per-thread `Copy` value-cache
  primitive for Atlas consumers, with nightly TLS cfg support and stable
  `std::thread_local!` fallback.
- [x] [patch] Add and rerun `thread_cached_4096x` Criterion coverage for
  cached hit, overwrite, and invalidation paths; update `BENCHMARKS.md`.
- [x] [patch] Remove generated unsafe from `thread_cached!` nightly TLS access
  by using `#[thread_local] Cell<Option<T>>` storage and inline accessors.
- [x] [patch] Audit registered partition executor dispatch and remove the
  aliased `&mut Context` reconstruction from each task; task wrappers now read a
  shared context and write only their disjoint result slot.
- [x] [patch] Split the `std` partition implementation into vertical
  `plan`/`executor`/`driver` leaf modules without changing public exports.
- [x] [patch] Add default `parallel` and `mnemosyne-memory` feature markers;
  `mnemosyne-memory` forwards to `alloc` for branded Cow/cell memory-boundary
  support without depending back on Mnemosyne.
- [x] Evidence: `cargo metadata --no-deps --locked --format-version 1`; full
  Atlas feature-policy metadata audit; `cargo fmt --check`; `git diff --check`.
  Residual: compile/test gates were blocked before rustc by denied access to
  `target/debug/.cargo-lock`.
- [x] [patch] Add `ParallelExecutorFn` and `register_parallel_executor` for
  `std` partition drivers so Moirai can provide the shard executor instead of
  Melinoe always spawning raw scoped threads.
- [x] [patch] Add a value-semantic registered-executor partition test proving
  the executor receives the resolved shard count and that disjoint branded
  writes preserve the identity mapping.
- [x] [patch] Add Apollo-facing `tests/apollo_boundary.rs` contract tests for
  branded `Cow` scratch boundaries: static borrowed policy performs zero clones
  and pointer-identical borrow; static retained policy clones exactly once per
  element into independent owned storage.
- [x] [minor] Implement `ExactSizeIterator` + exact `size_hint` for
  `region::ShardChunks` (`ceil(remaining / chunk)`, decrementing as consumed).
- [x] [patch] Make the partition driver derive worker-handle capacity from the
  `ShardChunks` exact size; remove the duplicated `shard_count` helper and the
  internal `ResolvedPartitionPlan` struct (single source of truth for shard count).
- [x] [patch] Add value-semantic tests for the exact-size contract and the
  empty-region zero-shard case.
- [x] [patch] Gate `examples/codegen.rs` with `required-features = ["alloc"]`
  (it uses the alloc-gated `borrow_cow`); fixes the broken `--no-default-features`
  example build.
- [x] [patch] Refresh `BENCHMARKS.md` partition-driver section and `empty_region`
  figure; bump version to 0.6.0 and sync CHANGELOG, backlog, gap audit.
- [x] [patch] Run local gates: `cargo fmt --check`, `cargo clippy --all-targets
  -- -D warnings`, `cargo test --features std`, `cargo doc --no-deps`, and the
  feature matrix (`--no-default-features`, `--no-default-features --features alloc`).
- [x] [patch] Rerun the `partition_driver` Criterion group (fast sweep); confirm
  no regression (`empty_region` ~42 ns, still sub-µs / no spawn).

## Prior micro-sprint (0.5.0)

- [x] [minor] Add direct `CellCowExt::borrow_cow` / `retain_cow` methods for
  common static zero-copy and retain-once boundary cases.
- [x] [patch] Extend codegen probes for direct `borrow_cow`, ZST atomic ordering,
  and read-permit-gated raw atomic interop.
- [x] [patch] Route `BrandedAtomic::*_with` ZST-ordering methods directly through
  associated constants and the sealed atomic mediation surface.
- [x] [patch] Consolidate direct, ZST-policy, and runtime-decision Cow entry
  points through the sealed `Borrowed` / `Retained` policy bodies.
- [x] [patch] Expand benchmarks for generic ZST-policy Cow paths and
  read-permit-gated `BrandedAtomic::as_atomic` raw interop; rerun targeted
  Criterion groups and update `BENCHMARKS.md`.
- [x] [minor] Add `CellCowExt` conditional `Cow` boundary API with `Borrowed` /
  `Retained` ZST policies and `RetainDecision` runtime policy.
- [x] [minor] Add `AtomicOrder` ZST policies (`Relaxed`, `AcqRel`, `SeqCst`)
  and monomorphized `BrandedAtomic::*_with` methods.
- [x] [minor] Add value-semantic tests for conditional `Cow` and ZST atomic
  ordering paths.
- [x] [minor] Extend Mnemosyne and conditional-atomic benchmarks for the new
  policy APIs.
- [x] [minor] Pin policy ZST layout with compile-time assertions.
- [x] [minor] Seal `AtomicOrder` to the audited ZST policy set.
- [x] [minor] Add read-permit-gated zero-copy raw atomic interop via
  `BrandedAtomic::as_atomic`, plus unique/owned `as_atomic_mut` / `into_atomic`.
- [x] [patch] Remove avoidable unsafe from `BrandedAtomic` unique/owned access
  by using standard atomic `get_mut` / `into_inner`.
- [x] [patch] Make static `Cow` policy dispatch branch-free by policy body.
- [x] [minor] Add typed partition planning for fixed parts, hardware
  parallelism, and fixed chunk sizes.
- [x] [minor] Export planned map/for-each APIs and available-parallelism
  convenience wrappers.
- [x] [minor] Add value-semantic tests for plan equivalence, chunk tiling, and
  available-parallel region coverage.
- [x] [minor] Extend access benchmarks with fixed-part, hardware-parallel,
  chunk-size, and scheduler-only plan rows.
- [x] [minor] Synchronize README, BENCHMARKS, CHANGELOG, backlog, and gap audit.
- [x] [patch] Audit partitioned-write scheduling for avoidable allocation and
  overflow risk.
- [x] [patch] Reserve `partition_map` worker handles to the actual non-empty
  shard count.
- [x] [patch] Add value-semantic tests for empty regions and over-partitioned
  regions.
- [x] [patch] Add `partition_driver` benchmarks for scheduling/allocation paths.
- [x] [patch] Synchronize README, BENCHMARKS, CHANGELOG, backlog, and gap audit.
- [x] [patch] Run local gates: `cargo fmt --check`, stable `cargo clippy
  --all-targets -- -D warnings`, `cargo test`, `cargo doc --no-deps`.
- [x] [patch] Verify feature builds: `cargo test --no-default-features` and
  `cargo test --no-default-features --features alloc`.
- [x] [patch] Compile benchmark harnesses: `access`, `concurrent_reads`,
  `mnemosyne`, `conditional_atomics`, and `false_sharing` with `--no-run`.
- [x] [patch] Run Miri partition suite under Stacked Borrows and Tree Borrows.
- [x] [patch] Run Miri conditional atomic / conditional Cow suites under Stacked
  Borrows and Tree Borrows.

## Residuals — resolved (0.6.0)

- [x] [minor] `cargo-semver-checks` baseline established via git rev:
  `cargo semver-checks check-release --baseline-rev HEAD` builds and parses both
  v0.5.0 (baseline) and v0.6.0 (current) rustdoc and reports **no semver update
  required** (0.6.0 introduces no breaking change). Default registry comparison
  is still unavailable (crate unpublished); the `--baseline-rev` workflow is the
  standing substitute. Note: semver-checks 0.48.0 skips all 253 lints against the
  current nightly rustdoc-JSON format (a tool/format mismatch, not a crate
  issue); the comparison nonetheless completes cleanly.
- [x] [patch] Miri clean across the full suite under this nightly (no UB, no data
  races): `projection` (6), `partition` (15, incl. the new exact-size tests with
  real threads), `threads` (6), `conditional_atomics` (8), `conditional_cow` (5),
  `branding` (7), `multi_token` (8), `slice_views` (4), `differential` (3).
- [x] [patch] Nightly `cargo clippy --all-targets --all-features -- -D warnings`
  is clean. The local MSYS2-packaged nightly bakes the stable release channel, so
  `#![feature(doc_cfg)]` requires `RUSTC_BOOTSTRAP=1`; with that set the
  all-features lint passes with zero warnings.

## Next concrete increment

- [ ] [patch] On registry publication, switch `cargo-semver-checks` from the
  `--baseline-rev` workflow to the default crates.io baseline, and re-run once
  semver-checks supports the newer rustdoc-JSON format so lints execute rather
  than skip.
