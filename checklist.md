# Checklist — melinoe

Target version: 0.9.0

## Trusted publishing

- [x] Add and validate the release workflow, then register `melinoe` against
  `ryancinsight/melinoe/.github/workflows/rust-release.yml` in crates.io.

## MSRV verification (Unreleased)

- [ ] [patch] Run the declared Rust 1.65 floor in a standalone locked
  `cargo check --all-features` workflow; do not treat the Atlas development
  overlay as MSRV evidence.

## Generativity continuation (Unreleased)

- [x] [minor] Add `BrandedVec::from_fn` for index-sensitive generation and
  `collections::with_generated` for fresh-brand collection workflows.
- [x] Keep the higher-ranked callback boundary generic: only the callback
  result may escape; the generated vector and exclusive token remain scoped.
- [x] Compose generated storage with the existing Melinoe partition driver and
  retain direct `MelinoeCell` access plus the `into_boxed_cells` handoff for
  downstream providers; do not add a second scheduler or an upward dependency
  on Themis.
- [x] Synchronize public exports, README, changelog, and value-semantic tests.
- [x] Evidence: all-feature Nextest 125/125, alloc-only Nextest 79/79,
  all-target/all-feature Clippy, alloc-only Clippy, offline check, rustfmt,
  doctests, rustdoc, semver checks (196 pass, 57 skip), and locked
  no-dependency metadata. Downstream evidence: Themis branded placement and
  CFDrs `cfd-core` compile through the Atlas overlay; CFDrs Nextest 269/269.

## Conditional-atomic genericity cleanup (Unreleased)

- [x] [patch] Consolidate runtime `Ordering` and compile-time `AtomicOrder`
  resolution behind the private generic `OrderingSource` strategy. Keep the
  public runtime and ZST entry points unchanged while sharing the operation
  bodies and preserving role-specific load/store/RMW/failure orderings.
- [x] Add value-semantic coverage for runtime and ZST ordering-role mapping.
- [x] Evidence: all-feature Nextest 126/126, strict all-target/all-feature
  Clippy, offline check, and rustfmt.

## Fresh-brand factory cleanup (Unreleased)

- [x] [patch] Centralize fresh brand construction behind the private
  higher-ranked `TokenFamily`/`with_fresh_token` factory. Preserve separate
  exclusive, cross-thread region, and thread-local token families, including
  the thread-local non-`Send` marker.
- [x] Replace repeated scope-local unsafe minting in public branding, scoped
  worker, and reentrant entry points without changing their public contracts.
- [x] Evidence: all-feature Nextest 126/126, alloc-only Nextest 80/80,
  all-target/all-feature and alloc-only Clippy, offline check, rustfmt,
  31 doctests, and rustdoc.

## Documentation residual cleanup (Unreleased)

- [x] Repair the stale `BrandedAtomic` source link in `README.md` after the
  atomic module hierarchy split.
- [x] Evidence: repository search confirms no remaining `src/atomic.rs` link;
  `git diff --check` passes.

## Branded vector module cleanup (Unreleased)

- [x] Split the oversized `BrandedVec` implementation into dedicated
  generation, operation, view, partition, and iterator leaves while retaining
  the existing `collections::BrandedVec` and `collections::with_generated`
  exports.
- [x] Add explicit safety rationale to the moved transparent-layout ownership
  conversions.
- [x] Evidence: all-feature Nextest 126/126, alloc-only Nextest 80/80,
  all-target/all-feature and alloc-only Clippy, 31 doctests, rustdoc, offline
  check, rustfmt, and diff checks. The largest leaf is 157 lines.

## Cow trait genericity cleanup (Unreleased)

- [x] Remove the trait-level `T: Clone` bound from `CellCowExt`.
- [x] Keep `T: Clone` on every Cow-producing method and its implementation.
- [x] Add a non-`Clone` trait-bound compile regression.
- [x] Evidence: all-feature Nextest 127/127, alloc-only Nextest 81/81,
  strict Clippy for both feature surfaces, 31 doctests, rustdoc, offline
  check, rustfmt, and diff checks. `cargo semver-checks` remains blocked by
  the peer-owned manifest's simultaneous path and registry `melinoe@0.9.0`
  specifications.

## Wrapped deque Cow consolidation (Unreleased)

- [x] Claim the provider-local slice and preserve the existing public Cow APIs.
- [x] Route contiguous and wrapped segment assembly through one generic helper
  parameterized by the existing `CowPolicy` ZST seam.
- [x] Add the missing safety rationale for `VecDeque<T>` to
  `VecDeque<MelinoeCell<'brand, T>>` ownership conversions.
- [x] Verify the existing contiguous/wrapped Cow value and pointer contracts,
  feature builds, strict Clippy, doctests, rustdoc, formatting, and diff checks:
  focused deque Nextest 22/22; all-feature Nextest 127/127; alloc-only Nextest
  81/81; strict Clippy on both feature surfaces; 31 doctests; rustdoc; rustfmt;
  and diff checks.

## Segmented Cow policy ownership (Unreleased)

- [x] Claim the provider-local cleanup without touching peer-owned manifest,
  changelog, or partition-plan files.
- [x] Move the generic two-segment `Cow` assembly helper beside `CowPolicy` and
  route the deque view through that owner without changing public methods.
- [x] Preserve zero-copy contiguous borrowing, retained ownership, and wrapped
  value semantics through the existing branded collection tests.
- [x] Evidence: focused branded-deque Nextest 22/22; all-feature and alloc-only
  Nextest; strict Clippy on both feature surfaces; 31 doctests; rustdoc;
  rustfmt; and diff checks.

## Current micro-sprint (0.9.0)

- [x] [major] Record ADR 0001: replace the raw executor alias with a transparent
  validated capability; reject unsafe-at-every-registration and trait-object
  alternatives.
- [x] [major] Encode the implementer obligation in
  `ParallelExecutor::new`, preserve safe registration, pin function-pointer
  layout at compile time, and remove `ParallelExecutorFn` completely.
- [x] [major] Migrate Melinoe contracts and Moirai's registration boundary.
- [x] Evidence: workspace Clippy; 121/121 nextest; 30/30 doctests; rustdoc;
  three focused registered-executor Miri tests; `cargo semver-checks` classifies
  0.8.0 to 0.9.0 as a major change.

## Prior micro-sprint (0.8.0)

- [x] [minor] Migrate the next lowest-risk upstream Halo collection:
  `halo::BrandedVecDeque<'brand, T>`. Upstream `vec_deque` maps directly to
  `std::collections::VecDeque`, unlike hash/tree/graph collections with broader
  invariants, so the Melinoe-backed slice stores `VecDeque<MelinoeCell<'brand,
  T>>` and routes element/split-slice/`Cow`/partition access through Melinoe
  permits. Value-semantic deque tests cover logical ordering, permit-gated
  element mutation, wrapped split-slice mutation, structural operations,
  zero-copy conversion, pointer-identical contiguous `Cow` borrowing, retained
  owned `Cow`, read partitions, and write partitions.
- [x] [patch] Register and add the `halo` `branded_deque` Criterion harness for
  contiguous split-slice reads, wrapped split-slice reads, write partitions, and
  read partitions; sync README, backlog, gap audit, changelog, and benchmarks.

## Prior micro-sprint (0.7.0)

- [x] [minor] CR-7: add `WriterShard::par_chunks` / `region::ParChunks`, the
  indexed disjoint-shard accessor (`len`, `# Safety` `get_unchecked_chunk`) that
  encapsulates the `from_raw_parts_mut` range math consumers (moirai-parallel)
  hand-roll; value-semantic tests (exact `len`, partition coverage, disjoint
  non-aliasing writes, single-partition, `Send`/`Sync`), a `compile_fail`
  brand-escape doctest, and a `ShardChunks` len-parity unit test. Miri-clean.
- [x] [patch] M-3: consolidate the two partition drivers' duplicated executor-path
  scaffolding into one generic `driver_core::drive`; split `driver.rs` into
  `driver_core`/`map`/`read_map` leaf modules; drive write-path partitioning
  through `par_chunks`. Public exports and unwind/init-tracking behavior byte-for-
  byte preserved (existing partition + panic-safety tests pass unmodified).
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
- [x] [minor] Add `clear_parallel_executor` and value-semantic coverage proving
  registered executor state can be reset to the default scoped-thread driver.
- [x] [minor] Add `crates/halo` as the Melinoe-backed protective collection
  crate. Initial migrated surface: `halo::BrandedVec<'brand, T>` over
  `Vec<MelinoeCell<'brand, T>>`, with permit-gated element/slice access,
  conditional `Cow`, value-semantic tests, and a Criterion benchmark harness.
- [x] [minor] Extend `halo::BrandedVec` with owned structural operations
  (`pop`, `insert`, `remove`, `swap_remove`, `swap`, `truncate`,
  `resize_with`, `retain_mut`) and `std`-gated partitioned mutation/map
  adapters using Melinoe's existing disjoint-shard driver.
- [x] [minor] Expose `PartitionPlan::chunk_len_for` for downstream read-side
  chunking and add `halo::BrandedVec` shared partition map/for-each adapters
  for concurrent `&[T]` processing under a Melinoe read permit.
- [x] [patch] Split the `std` partition implementation into vertical
  `plan`/`executor`/`driver` leaf modules without changing public exports.
- [x] [patch] Remove remaining existence-only error assertions from reentrancy
  tests/Rustdoc; assert concrete `Reentered` and sentinel panic payloads.
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
