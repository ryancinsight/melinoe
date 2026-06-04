# Backlog — melinoe

## Active

- <a id="zst-boundary-policies"></a>[minor] ZST boundary and synchronization
  policies. Status: implemented for 0.4.0. `CellCowExt` covers conditional
  borrow-or-retain at the ownership boundary; `AtomicOrder` covers
  monomorphized atomic orderings. Evidence: value-semantic tests, static
  assertions, and benchmark compilation.

## Next

- <a id="semver-check"></a>[minor] Provide a SemVer baseline for
  `cargo-semver-checks` before a 0.4.0 release tag. The tool is installed, but
  default registry lookup fails because `melinoe` is not found in crates.io.
- <a id="miri-partition-projection"></a>[patch] Re-run Miri on partition and
  projection paths after stable gates. Evidence target: machine-checked Stacked
  Borrows / Tree Borrows where toolchain support is available.
- <a id="feature-hygiene"></a>[patch] Verify feature matrix: default, `alloc`,
  `--no-default-features`, and documented nightly-only `--all-features`
  behavior.

## Closed

- <a id="partition-plan"></a>[minor] Typed multithreading plan surface delivered
  in 0.3.0. `PartitionPlan` supports fixed parts, reported hardware
  parallelism, and fixed chunk sizes.
- <a id="partition-driver-memory"></a>[patch] Partition driver memory discipline
  delivered in 0.2.1. `partition_map` uses overflow-safe ceiling division and
  reserves worker handles to the actual non-empty shard count.
- <a id="guard-projection"></a>[minor] Zero-copy guard projection delivered in
  0.2.0 with `MelinoeRef`/`MelinoeMut` `map` and `map_split`.
