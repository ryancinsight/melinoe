# Backlog — melinoe

## Active

(none — 0.6.0 increment closed; see Closed.)

## Next

- <a id="semver-registry"></a>[patch] After registry publication, switch
  `cargo-semver-checks` from the `--baseline-rev` git workflow (now established)
  to the default crates.io baseline, and re-run once semver-checks supports the
  newer rustdoc-JSON format so its lints execute rather than skip.

## Closed

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
