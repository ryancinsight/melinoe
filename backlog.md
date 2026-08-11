# Backlog — melinoe

## Atlas in-house replacement roadmap — melinoe slice [minor]

melinoe is the capability/ownership-proof foundation. The Atlas GPU program
(the `hephaestus` device substrate — atlas ADR 0001 — used by coeus/apollo on wgpu +
CUDA, with mnemosyne device pools) wants compile-time proofs for device-buffer ownership:
- [x] [minor] Stage D1 support: a documented device-buffer ownership-transfer pattern —
  moving a `SyncRegionToken` transfers write capability across host/device/stream; a
  `SharedReadToken` fans out concurrent device reads; `BrandedAtomic` covers fence/
  counter values touched on both sides. Add a real contract test over the lowest
  available device/stream abstraction; do not substitute a mock buffer.

## Active

- [x] [minor] Add generic branded-vector generation through
  `BrandedVec::from_fn` and `collections::with_generated`. The fresh
  higher-ranked brand remains inside the callback while its result escapes;
  generated cells compose with the existing partition driver, and
  `into_boxed_cells` supports Themis's branded placement container. Keep
  topology providers above Melinoe: Themis-derived worker counts enter through
  `PartitionPlan::parts`, not a reverse dependency. Evidence: all-feature
  Nextest 125/125, alloc-only Nextest 79/79, strict Clippy for both feature
  surfaces, offline check, documentation and semver gates, Themis branded
  placement compile, and CFDrs `cfd-core` Nextest 269/269 through the Atlas
  overlay.

- [x] [patch] Consolidate conditional-atomic ordering resolution behind one
  generic `OrderingSource` strategy. Runtime `Ordering` and sealed ZST
  `AtomicOrder` policies now share the same operation bodies; static policy
  monomorphizations retain their associated ordering constants without a
  runtime policy branch. No public API or consumer migration is required.
  Evidence: ordering-role unit coverage, all-feature Nextest 126/126, strict
  Clippy, and offline check.

- [x] [patch] Consolidate fresh higher-ranked brand minting behind the private
  GAT-based `TokenFamily` factory. Exclusive, cross-thread region, thread-local,
  scoped-worker, and reentrant entry points now share one brand proof boundary
  while retaining their distinct token auto-trait postures. Public signatures
  and consumer behavior remain unchanged. Evidence: all-feature Nextest
  126/126, alloc-only Nextest 80/80, strict Clippy for both feature surfaces,
  offline check, 31 doctests, rustdoc, and diff checks.

- [x] [patch] Repair the conditional-atomic README link after the atomic
  module hierarchy split; the documented `BrandedAtomic` source now resolves to
  `src/atomic/branded.rs`.

- [x] [patch] Split the 513-line `BrandedVec` implementation into named
  generation, operation, view, partition, iterator, and manifest modules.
  Public exports and signatures remain unchanged; the largest resulting leaf
  is 157 lines. Evidence: all-feature Nextest 126/126, alloc-only Nextest
  80/80, strict Clippy for both feature surfaces, 31 doctests, rustdoc,
  rustfmt, offline check, and diff checks.

- [x] [minor] Move `CellCowExt`'s `Clone` requirement from the public trait
  boundary to the four methods that construct `Cow` values. Non-`Clone`
  branded cells can now satisfy the capability boundary while clone-dependent
  operations retain explicit method bounds. Evidence: all-feature Nextest
  127/127, alloc-only Nextest 81/81, strict Clippy for both feature surfaces,
  31 doctests, rustdoc, offline check, rustfmt, and diff checks.

- [x] [patch] Consolidate wrapped
  `BrandedVecDeque` `Cow` construction through one generic segment helper and
  document the transparent-storage safety contracts for deque conversions.
  Preserve the existing borrow/retain policy semantics and public methods;
  prove contiguous and wrapped value behavior through the existing deque Cow
  suite plus focused feature gates. Evidence: focused deque Nextest 22/22,
  all-feature Nextest 127/127, alloc-only Nextest 81/81, strict Clippy on both
  feature surfaces, 31 doctests, rustdoc, rustfmt, and diff checks.

- [x] [patch] Harden registered partition panic recovery against mutex poisoning.
  `sync::scoped::partition::driver_core` now recovers the first captured panic
  payload with `PoisonError::into_inner` both when task wrappers report a panic
  and when the executor tears down the manually managed result buffer. This
  prevents a poisoned payload mutex from masking the original panic; the
  existing panic-safety drop-count tests continue to cover initialized-result
  cleanup. A focused regression poisons the payload mutex, reports a second
  panic, and verifies the first payload remains recoverable. Evidence
  (2026-08-06): `cargo check --all-features`, strict
  `cargo clippy --all-targets --all-features -- -D warnings`, Nextest **122/122**,
  doctests **29/29**, `cargo check --no-default-features`, rustfmt, and diff
  checks all pass. Implementation scope is one provider-local source file;
  no consumer or peer-owned files changed.

- [x] [patch] Move segmented `Cow` assembly into the existing `cell::cow`
  policy owner. `BrandedVecDeque` remains a consumer of the sealed `CowPolicy`
  seam; contiguous zero-copy and wrapped owned behavior remain unchanged, with
  no public API or peer-owned file changes. Evidence: focused branded-deque
  Nextest 22/22, all-feature and alloc-only Nextest, strict Clippy on both
  feature surfaces, 31 doctests, rustdoc, rustfmt, and diff checks.

- [x] [patch] Publish future releases through a pinned GitHub Actions workflow
  using crates.io OIDC Trusted Publishing and no stored registry credential.

- [ ] [patch] Fix executor-state test interference in `tests/partition.rs`.
  Under a parallel `cargo test --all-features --test partition` the same two
  tests fail deterministically — `clearing_registered_executor_restores_default_driver`
  and `registered_executor_drives_partition_map` — as an assertion under one
  guard-protected test while a non-guard test mutates the shared registered
  executor; the failed guard then poisons `EXECUTOR_TEST_LOCK` and the
  `PoisonError` cascades into the second. Serial (`--test-threads=1`) passes
  17/17. Pre-existing on `origin/main` (file identical to main); unrelated to
  the book/`mdbook-test` change.

- No Melinoe-local item remains in progress; the 0.9.0 executor capability is
  ready for upstream publication and downstream Moirai lock refresh.

## Next

- <a id="semver-registry"></a>[patch] After registry publication, switch
  `cargo-semver-checks` from the `--baseline-rev` git workflow (now established)
  to the default crates.io baseline, and re-run once semver-checks supports the
  newer rustdoc-JSON format so its lints execute rather than skip.

## Closed

- <a id="parallel-executor-capability"></a>[major] Replaced the
  `ParallelExecutorFn` domain alias with a transparent validating capability.
  Evidence: compile-time layout assertion, 121/121 nextest, 30/30 doctests,
  Clippy/rustdoc, three focused Miri tests, and major-change semver
  classification. Decision: ADR 0001.

- <a id="atlas-device-contract"></a>[minor] Added the Atlas device-buffer
  ownership-transfer contract crate in commit `375108b`; the workspace now
  carries the real Hephaestus-backed contract instead of an uncommitted plan.

- <a id="halo-workspace-crate"></a>[minor] Added `crates/halo` as the
  Melinoe-backed protective collection crate. The first migrated vertical slice
  is `halo::BrandedVec<'brand, T>`, backed directly by
  `Vec<MelinoeCell<'brand, T>>` and Melinoe's permit, zero-copy slice, and
  conditional `Cow` traits. Evidence: `cargo check -p halo` plus targeted tests,
  docs, and benchmark harness verification in the delivering change.
- <a id="halo-branded-vec-ops"></a>[minor] Extended `halo::BrandedVec` with
  owned vector structural operations and `std`-gated partitioned mutation/map
  adapters over Melinoe `PartitionPlan` shards. Evidence: default and
  `--no-default-features` Halo builds, value-semantic structural/concurrent
  tests, workspace nextest/clippy/doc gates, and benchmark harness compilation.
- <a id="halo-read-partitions"></a>[minor] Exposed
  `PartitionPlan::chunk_len_for` for downstream chunk planning and added
  `halo::BrandedVec` read-side partition map/for-each adapters over
  permit-gated shared slices. Evidence: Melinoe plan-resolution tests, Halo
  shared-shard tests, workspace gates, and benchmark harness compilation.
- <a id="halo-branded-vecdeque"></a>[minor] Migrated the next lowest-risk
  upstream Halo collection as `halo::BrandedVecDeque<'brand, T>`:
  `std::collections::VecDeque` maps directly to one owned standard container,
  unlike the remaining hash/tree/graph collections with broader invariants.
  Storage is `VecDeque<MelinoeCell<'brand, T>>`; element, split-slice, `Cow`,
  clone, read-partition, and write-partition access are gated through Melinoe
  permits/cells instead of a Halo-local `GhostToken` / `GhostCell` layer.
  Evidence: value-semantic deque tests and the `branded_deque` Criterion
  harness.
- <a id="halo-branded-deque-ops"></a>[minor] Extended `halo::BrandedVecDeque`
  with the same `std`-gated partitioned mutation/map adapters as `BrandedVec`
  (`partition_map_with`/`partition_for_each_with` for shared reads,
  `partition_for_each_mut_with`/`partition_map_mut_with` for exclusive
  mutation), via a `DequeShardPlan` that maps the flat logical index range
  onto the deque's front/back ring segments — a shard crossing the wrap
  boundary is split into two physical subshards sharing one logical offset.
  Evidence: contiguous and wrapped-deque correctness tests, a same-logical-
  plan consistency check across both mutation and read paths, workspace
  nextest/clippy/fmt gates.
- <a id="halo-upstream-migration"></a>[major] Consolidated `crates/halo` into
  the root `melinoe` crate (`2e9bf87`). `halo` workspace member removed;
  `BrandedVec`, `BrandedVecDeque`, `BrandedDrain`, `BrandedVecDequeDrain` live
  in `melinoe::collections` (re-exported at crate root under `alloc` gate).
  Single-crate workspace. 121/121 nextest, clippy/rustdoc clean.
- <a id="region-module-hierarchy"></a>[patch] Region module hierarchy split
  delivered in 0.6.0. `src/region/mod.rs` is now the documentation/re-export
  root, `src/region/shard.rs` owns `WriterShard`, and
  `src/region/chunks.rs` owns `ShardChunks` exact-size iteration. Public exports
  are unchanged; evidence: partition integration suite and stable gates.
- <a id="default-provider-feature-policy"></a>[patch] Default `parallel` and
  `mnemosyne-memory` feature markers delivered. `mnemosyne-memory` forwards to
  `alloc`; no dependency cycle to Mnemosyne is introduced. Evidence: Atlas
  feature-policy metadata audit, fmt, and diff checks.
- <a id="apollo-boundary-contract"></a>[patch] Apollo-facing zero-copy scratch
  boundary contract tests delivered. `Borrowed` ZST policy returns a
  pointer-identical `Cow::Borrowed` with zero element clones; `Retained` ZST
  policy returns independent owned storage with exactly one clone per element.
  Evidence: value-semantic integration tests in `tests/apollo_boundary.rs`.
- <a id="residuals-0-6-0"></a>[patch] 0.6.0 verification residuals resolved:
  (1) `cargo-semver-checks` baseline via `--baseline-rev HEAD` — v0.5.0→v0.6.0
  reports no semver update required; (2) Miri clean across all nine test suites
  (no UB / no data races), covering the previously-pending partition and
  projection paths; (3) nightly `cargo clippy --all-targets --all-features -- -D
  warnings` clean (the MSYS2 nightly needs `RUSTC_BOOTSTRAP=1` for the
  `doc_cfg` feature gate). Feature matrix verified: default, `alloc`,
  `--no-default-features`, and nightly `--all-features` build.
- <a id="shard-chunks-exact-size"></a>[minor] `ShardChunks: ExactSizeIterator`
  with exact `size_hint`, delivered in 0.6.0. The partition driver reserves
  worker capacity from the iterator's exact size, making it the single source of
  truth for the shard count; the duplicated `shard_count` helper and
  `ResolvedPartitionPlan` struct are removed. Evidence: exact-size and
  empty-region value-semantic tests; `partition_driver/empty_region` benchmark
  pins the no-spawn / zero-capacity contract.
- <a id="codegen-example-alloc-gate"></a>[patch] `examples/codegen.rs` gated on
  `required-features = ["alloc"]` in 0.6.0; restores a clean
  `cargo test --no-default-features` build (the example uses alloc-gated
  `borrow_cow`).
- <a id="cell-cow-direct"></a>[minor] Direct conditional-Cow boundary methods
  (`borrow_cow` / `retain_cow`) delivered in 0.5.0, covering common static
  borrow/retain cases without a generic policy parameter.
- <a id="zst-boundary-policies"></a>[minor] ZST boundary and synchronization
  policies delivered in 0.4.0. `CellCowExt` covers conditional borrow-or-retain
  at the ownership boundary; `AtomicOrder` covers monomorphized atomic
  orderings.
- <a id="partition-plan"></a>[minor] Typed multithreading plan surface delivered
  in 0.3.0. `PartitionPlan` supports fixed parts, reported hardware
  parallelism, and fixed chunk sizes.
- <a id="partition-driver-memory"></a>[patch] Partition driver memory discipline
  delivered in 0.2.1. `partition_map` uses overflow-safe ceiling division and
  reserves worker handles to the actual non-empty shard count.
- <a id="guard-projection"></a>[minor] Zero-copy guard projection delivered in
  0.2.0 with `MelinoeRef`/`MelinoeMut` `map` and `map_split`.

## Cross-repo filing (2026-06-12 stack audit)

- [x] [minor] (0.7.0) Shared thread-local value-cache utility delivered as
  `thread_cached!` (macro: TLS statics are declaration-site constructs no
  generic type can capture; same sanctioned route as moirai's
  `thread_local_static!`). themis `CACHED_NODE` and mnemosyne `CACHED_CPU_ID`
  adopt it in the same coordinated change. moirai's `thread_local_static!`
  remains separate by design: it serves no_std targets with a different
  fallback shape (evaluated 2026-06-12).
