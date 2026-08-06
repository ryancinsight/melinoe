# Changelog

All notable changes to `melinoe` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/) and the project adheres to
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- [patch] Harden registered-partition panic recovery against mutex poisoning:
  `sync::scoped::partition::driver_core` recovers the first captured panic
  payload with `PoisonError::into_inner` both when task wrappers report a
  panic and when the executor tears down the manually managed result buffer.
  Previously the `if let Ok` path silently dropped the first panic payload if
  the payload mutex was poisoned; the first panic cause is now always
  preserved. A focused regression poisons the payload mutex, reports a second
  panic, and verifies the first payload remains recoverable.
- [patch] `BrandedVecDeque::as_slices`/`as_mut_slices` now route the ring-buffer
  segment cast through `MelinoeCell::slice_as_unsafe_cell`, the crate's SSOT for
  interior-mutability-provenance-preserving pointer conversion, instead of a
  bare `*const MelinoeCell<T> as *const T` cast. Behavior is unchanged; the
  shared and exclusive slice views now carry the same `UnsafeCell` provenance as
  `CellSliceExt::borrow_slice*` and the region shards.

- [patch] Site-local `#[allow(clippy::mut_from_ref)]` suppressions on the
  branded `&self -> &mut [T]` interior-mutability boundaries converted to
  `#[expect(clippy::mut_from_ref, reason = "...")]` so an accidental removal of
  the triggering shape is caught by `unfulfilled_lint_expectations` rather than
  silently retiring the suppression.

- [patch] Added a GitHub Release workflow that validates crate identity and
  package contents before publishing through crates.io Trusted Publishing.

- [patch] Added crate-root `#![deny(missing_docs)]` attribute to align with the
  atlas stack convention (aequitas, harmonia, horae, themis). The lint was
  already enforced via `[lints.rust]` in `Cargo.toml`; this makes it visible at
  the crate root where the rest of the stack places it.

### Changed

- [patch] Consolidated `crates/halo` into the root `melinoe` crate. The public
  types `BrandedVec`, `BrandedVecDeque`, `BrandedDrain`, and
  `BrandedVecDequeDrain` now live in `melinoe::collections` (gated on
  `alloc`). Re-exported at the crate root as `melinoe::BrandedVec` etc. for
  ergonomic access. The `halo` sub-crate and its workspace entry are removed;
  all tests, benches, and impl modules are migrated into the trunk crate.
  Single-crate workspace reduces build complexity and eliminates the
  cross-crate dependency boundary.

### Breaking

- [major] Replaced the `ParallelExecutorFn` alias with the zero-cost
  `ParallelExecutor` capability. Integrators construct the capability through
  `unsafe { ParallelExecutor::new(executor) }`, discharging exact-once index,
  blocking-completion, and context-lifetime obligations once; safe registration
  accepts only the validated value. No compatibility alias remains.

### Added

- [minor] `BrandedVecDeque` gained sharded parallel read/write operations —
  `partition_map_with`/`partition_for_each_with` (permit-gated shared reads)
  and `partition_for_each_mut_with`/`partition_map_mut_with` (exclusive
  mutation) — each taking an explicit `melinoe::sync::PartitionPlan`. Built
  on a `DequeShardPlan` that maps a flat `0..front_len+back_len` chunk range
  onto the deque's front/back ring segments and delegates dispatch to
  `melinoe::sync::partition_read_map_with`, so contiguous and wrapped-around
  deques share one sharding algorithm; a logical shard crossing the ring
  wrap is exposed as two physical subshards with stable logical offsets.
  Covered by contiguous and wrapped-deque correctness tests plus a
  same-logical-plan consistency check for both the mutation and read paths.

## [0.8.0] — 2026-07-05

### Added

- [minor] Migrated `halo::BrandedVecDeque<'brand, T>` as the next staged Halo
  collection. The surface stores `VecDeque<MelinoeCell<'brand, T>>`, delegates
  element/split-slice/`Cow` access to Melinoe read/write permits, and routes
  default-`std` read/write partition adapters through Melinoe partition drivers
  instead of a Halo-local `GhostToken` / `GhostCell` layer.
- [patch] Added the `halo` `branded_deque` Criterion harness covering
  contiguous split-slice reads, wrapped split-slice reads, read partitions, and
  write partitions for `BrandedVecDeque`.
- [minor] Added `WriterShard::par_chunks(chunk_size) -> ParChunks<'a, 'brand, T>`,
  the *indexed* random-access counterpart to the sequential `WriterShard::chunks`
  (`ShardChunks`) lending iterator. `ParChunks::len()` reports the exact partition
  count `ceil(region_len / chunk_size)` and the `# Safety`-documented
  `ParChunks::get_unchecked_chunk(index)` returns the disjoint `WriterShard` for a
  partition without sequential `&mut` threading — the shape a work-stealing pool
  (e.g. `moirai-parallel`) needs. It is the single authoritative home for the
  `from_raw_parts_mut` disjoint sub-slice range math those consumers otherwise
  hand-roll. Disjointness (distinct indices → non-overlapping ranges, each
  requested at most once) is the caller's contract, stated in one `// SAFETY:`;
  the returned shard carries the region's `'a`/`'brand` so it cannot outlive the
  brand (enforced by a `compile_fail` doctest). Miri-verified (Stacked/Tree
  Borrows clean) that two indices held live simultaneously do not alias.

### Changed

- [patch] Consolidated the duplicated executor-path scaffolding of the two
  partition drivers into one generic engine `sync::scoped::partition::driver_core`
  (`drive`). The mutable-shard (`partition_map*`) and shared-slice
  (`partition_read_map*`) drivers previously each carried a near-identical copy of
  the `MaybeUninit` out-buffer allocation, `ExecutorDropGuard` leak/panic handling,
  panic mutex, and `Vec::from_raw_parts` teardown; they now differ only in the
  per-index task closure they hand to `drive`. The write path drives partitioning
  through the new `WriterShard::par_chunks`/`ParChunks` primitive instead of
  re-deriving `from_raw_parts_mut` ranges internally. The `driver.rs` file was
  split by concern into `driver_core`/`map`/`read_map` leaf modules; public exports
  and the drop-only-initialized-slots + free-backing-on-unwind behavior are
  unchanged (all existing partition tests, including the panic-safety drop-count
  tests, pass unmodified).

- [minor] Completed the `BrandedAtomic` shared-phase operation surface to full
  std-atomic parity: `fetch_xor`, `fetch_nand`, `fetch_max`, `fetch_min`, and the
  general `fetch_update` (each with a compile-time ZST-ordering `_with` variant),
  plus the backing `Atomic`/`AtomicInt` trait methods over every standard atomic.
  The exclusive phase remains plain `&mut` under a `WritePermit`; the shared phase
  is atomic under a `ReadPermit`. Soundness of the one `unsafe` interior-mutability
  path (`value_ptr` deref in `with_exclusive`) is **Miri-verified** (Stacked/Tree
  Borrows clean across all conditional-atomic tests, including the 8-thread
  concurrent phase-transition stress test). The `&AtomicU64 -> *mut` derivation
  uses a manual cast rather than `as_ptr()` to preserve the 1.65 MSRV (`as_ptr`
  stabilized in 1.70).

## [0.7.0] — 2026-06-12

### Added

- [minor] Added the `halo` workspace crate as the Melinoe-backed protective
  collection layer. The initial `halo::BrandedVec<'brand, T>` surface stores
  `Vec<MelinoeCell<'brand, T>>` and exposes permit-gated element, slice, and
  conditional `Cow` access through Melinoe's canonical primitives.
- [minor] Extended `halo::BrandedVec` with owned vector structural operations
  and default-`std` partitioned mutation adapters over Melinoe
  `PartitionPlan` / `WriterShard` shards.
- [minor] Exposed `PartitionPlan::chunk_len_for` and added Halo read-side
  partition adapters for concurrent shared-slice mapping/iteration without a
  second scheduler.
- [minor] `thread_cached!` macro (`thread_cached` module): per-thread cached
  value with `get_or_init`/`set`/`get`/`clear`, expanding to the
  nightly-`#[thread_local]` / stable-`thread_local!` cfg pair. Consolidates the
  identical caching pattern themis (`CACHED_NODE`) and mnemosyne
  (`CACHED_CPU_ID`) carried independently; storage is `Option<T>`, never a
  sentinel. Build-script `nightly_tls_active` detection added (probe no longer
  gated behind the `nightly` feature).
- [patch] Default `parallel` and `mnemosyne-memory` feature markers. The
  `mnemosyne-memory` feature forwards to `alloc`, preserving branded Cow/cell
  memory-boundary support without introducing a dependency cycle back to
  Mnemosyne.
- [patch] `std` partition drivers can register a custom blocking parallel executor, allowing Moirai to route branded `partition_map` shards through its scheduler while preserving disjoint `WriterShard` semantics.
- [minor] Added `clear_parallel_executor` to restore the default scoped-thread
  partition driver after a custom process-global executor was registered.
- [patch] Apollo-facing branded `Cow` boundary contract tests proving zero-copy
  borrowed scratch views and exactly-once retained ownership.
### Changed

- [patch] Split `region` into `region::shard` and `region::chunks` leaf modules
  while preserving the public `region::WriterShard` / `region::ShardChunks`
  exports. The module root is now documentation plus exports only; shard access
  and exact-size chunk iteration each have one implementation home.
- [patch] Hardened the `partition_driver` benchmark by black-boxing the input
  slice, preventing the empty-region row from collapsing to a compile-time-known
  `Vec::new()` result; refreshed the partition-driver measurements.
- [patch] Added `thread_cached_4096x` benchmark coverage for `get`,
  `get_or_init`, `set`, and `clear`/`set` on the stable TLS fallback path.
- [patch] `thread_cached!` nightly storage now uses
  `#[thread_local] Cell<Option<T>>` instead of `static mut Option<T>`, removing
  the macro's generated unsafe blocks on the nightly TLS path.
- [patch] Registered partition executor task dispatch now reconstructs a shared
  read-only context from the executor payload instead of an aliased `&mut`
  context, preserving the disjoint raw output writes without invalid mutable
  aliasing when custom executors run shards concurrently.
- [patch] Split the `std` partition implementation into
  `sync::partition::{plan, executor, driver}` leaf modules, preserving all
  public `melinoe::sync::*` re-exports while isolating sizing policy, executor
  registration, and driver execution.
- [patch] Reentrancy panic tests and Rustdoc examples now assert concrete
  `Reentered` / panic-payload values instead of existence-only error predicates.
- [patch] `build.rs` now declares and emits `nightly_tls_active` independently
  from `doc_cfg_active`, so TLS fast-path cfg is available without requiring the
  `nightly` documentation feature.

## [0.6.0] — 2026-06-04

### Added

- `region::ShardChunks` now implements `ExactSizeIterator` with an exact
  `size_hint`. The remaining shard count (`ceil(remaining / chunk_size)`) is
  reported up front and decrements as shards are yielded, so consumers that
  `collect()` shards reserve capacity exactly and avoid reallocation.
- Value-semantic tests pinning the exact-size contract, including the
  decrement-as-consumed property and the empty-region (`0` shards) case.

### Changed

- The partition driver (`sync::partition_map_with` and its wrappers) now derives
  worker-handle capacity from the `ShardChunks` iterator's exact size, making the
  iterator the single source of truth for the shard count. The previously
  duplicated `shard_count` ceiling-division helper and the internal
  `ResolvedPartitionPlan` struct are removed; `PartitionPlan::resolve` now
  returns only the per-shard chunk size. Behavior is unchanged: empty and
  over-partitioned regions still reserve no surplus capacity and spawn no surplus
  workers (pinned by the `partition_driver/empty_region` benchmark).

### Fixed

- `examples/codegen.rs` now declares `required-features = ["alloc"]` (it exercises
  the alloc-gated `CellCowExt::borrow_cow` boundary). `cargo test
  --no-default-features` previously failed to compile the example; the full
  feature matrix now builds cleanly.

## [0.5.0] — 2026-06-04

### Added

- `CellCowExt::borrow_cow` and `CellCowExt::retain_cow` as direct, branch-free
  convenience methods for the common static boundary cases. `borrow_cow` returns
  zero-copy `Cow::Borrowed`; `retain_cow` clones the branded slice exactly once.
- Value-semantic tests for direct borrowed pointer identity and retained owned
  copy independence.
- Mnemosyne `cow_escape` benchmark rows now exercise the direct `borrow_cow` /
  `retain_cow` methods for the static zero-copy and retain-once cases.
- `examples/codegen.rs` now probes `borrow_cow`, `fetch_add_with(Relaxed)`, and
  raw atomic interop through `as_atomic` against their raw equivalents.
- `BrandedAtomic::*_with` methods now call the sealed atomic mediation surface
  directly with ZST ordering associated constants instead of delegating through
  runtime-`Ordering` wrapper methods.
- `CellCowExt` direct, generic-policy, and runtime-decision entry points now
  share the sealed `Borrowed` / `Retained` policy bodies as their single
  clone/no-clone implementation source.
- Mnemosyne benchmarks now include generic ZST-policy Cow rows
  (`cow_policy_borrow`, `cow_policy_retain`) alongside direct Cow methods, and
  conditional-atomic benchmarks now include read-permit-gated raw interop through
  `BrandedAtomic::as_atomic`.

## [0.4.0] — 2026-06-04

### Added

- `CellCowExt` plus `Borrowed`, `Retained`, and `RetainDecision` for conditional
  `Cow` at branded slice ownership boundaries. Static retain decisions are ZST
  policies; data-dependent retain decisions use the explicit runtime enum.
- `AtomicOrder` plus `Relaxed`, `AcqRel`, and `SeqCst` ZST ordering policies for
  monomorphized `BrandedAtomic` shared-phase operations.
- `BrandedAtomic::*_with` methods for compile-time ordering policies:
  `load_with`, `store_with`, `swap_with`, `compare_exchange_with`,
  `fetch_add_with`, `fetch_sub_with`, `fetch_and_with`, and `fetch_or_with`.
- Zero-copy raw atomic interop on `BrandedAtomic`: `as_atomic` (shared phase,
  read-permit gated), `as_atomic_mut` (unique wrapper access), and `into_atomic`
  (owned extraction).
- Value-semantic tests for conditional `Cow` borrowed/retained paths and ZST
  atomic ordering policies.
- Benchmark rows for static `Cow` policies and ZST atomic ordering calls.

### Changed

- `AtomicOrder` is sealed; only the crate's audited ZST ordering policies can
  implement it.
- Static `Cow` policy dispatch is implementation-driven rather than a const-bool
  branch: `Borrowed` contains no clone path, `Retained` contains exactly one
  clone path.
- `BrandedAtomic::get_mut` and `BrandedAtomic::into_inner` now route through the
  standard atomic `get_mut` / `into_inner` APIs, removing avoidable unsafe from
  unique/owned access.

## [0.3.0] — 2026-06-04

### Added

- `sync::PartitionPlan`, a typed scheduling policy for scoped disjoint writes:
  fixed part count, reported hardware parallelism, or fixed chunk size.
- `sync::partition_map_with` and `sync::partition_for_each_with` to run the
  existing lock-free shard executor with an explicit `PartitionPlan`.
- `sync::partition_map_available` and `sync::partition_for_each_available` as
  hardware-parallel convenience wrappers using
  `std::thread::available_parallelism()`.
- Integration tests for fixed-plan equivalence, chunk-size tiling,
  hardware-parallel full coverage, and the available-parallel write-only
  wrapper.
- Benchmark variants for fixed-part, hardware-parallel, and chunk-size
  partitioned writes, plus scheduler-only `available_parallelism` and
  `chunk_size_16` rows.

## [0.2.1] — 2026-06-04

### Fixed

- `sync::partition_map` now reserves worker handles to the actual non-empty
  shard count rather than the requested `parts` value. Empty regions allocate no
  worker handle capacity, and over-partitioned regions reserve/spawn only
  non-empty shards.
- Replaced overflow-prone ceiling division in the partition driver with
  `1 + (len - 1) / requested_parts`, preserving the MSRV while avoiding
  `len + parts - 1` overflow for adversarial inputs.

### Added

- Regression tests for empty partitioned regions and `parts > len`
  over-partitioning.
- `partition_driver` Criterion benchmark group isolating scheduling/allocation
  overhead from compute-bound partitioned writes.

## [0.2.0] — 2026-06-03

### Added

- **Zero-copy branded guard projection.** `MelinoeRef::map` / `map_split` and
  `MelinoeMut::map` / `map_split` narrow a borrow guard onto a component of its
  payload (typically a field, or a `split_at_mut` half) while carrying the brand
  evidence through — the branded analogues of `Ref::map` / `RefMut::map`. The
  permit is threaded through the projection's lifetime, so the brand's
  read/write exclusion is preserved by the borrow checker alone, at zero copy.
  `MelinoeMut::map_split` yields two disjoint `&mut` projections from a single
  write permit (the multi-field-writer pattern). Provided as associated
  functions (not methods) to avoid colliding with `Deref` access.
- Static-assertion coverage: projected guards remain pointer-sized with their
  null-pointer niche intact (`src/static_assertions.rs`).
- `tests/projection.rs`: value-semantic tests for all four projection forms,
  plus a `compile_fail` doctest pinning that a live read projection still
  excludes a concurrent write of the brand. Verified under Miri (Stacked Borrows
  and Tree Borrows), including the disjoint-`&mut` `map_split` path.
- `benches/mnemosyne.rs`: `projection_1024x` group contrasting `borrow + map`
  (zero copy) against cloning a 512-byte block to reach one field.

### Verification

- Whole test suite (unit, integration, doctests) re-confirmed under `cargo miri
  test`; no Stacked/Tree Borrows violations on any access, partition, or
  projection path — the unsafe foundation rests on machine-checked evidence.

## [0.1.0]

- Initial release: branded multi-token phantom capabilities (`ExclusiveToken`,
  `SharedReadToken`, `ThreadLocalToken`, `SyncRegionToken`), `MelinoeCell`
  interior mutability, `CellSliceExt` zero-copy slice views, and `WriterShard`
  disjoint concurrent-write partitioning.

[0.6.0]: https://github.com/ryancinsight/melinoe/releases/tag/v0.6.0
[0.5.0]: https://github.com/ryancinsight/melinoe/releases/tag/v0.5.0
[0.4.0]: https://github.com/ryancinsight/melinoe/releases/tag/v0.4.0
[0.3.0]: https://github.com/ryancinsight/melinoe/releases/tag/v0.3.0
[0.2.1]: https://github.com/ryancinsight/melinoe/releases/tag/v0.2.1
[0.2.0]: https://github.com/ryancinsight/melinoe/releases/tag/v0.2.0
[0.1.0]: https://github.com/ryancinsight/melinoe/releases/tag/v0.1.0
