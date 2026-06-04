# Backlog — melinoe

## Active

- <a id="partition-plan"></a>[minor] Typed multithreading plan surface. Status:
  implemented for 0.3.0. `PartitionPlan` supports fixed parts, reported
  hardware parallelism, and fixed chunk sizes, with planned map/for-each APIs
  and available-parallel convenience wrappers. Evidence: integration tests and
  access benchmark compilation.

## Next

- <a id="semver-check"></a>[minor] Install or provide `cargo-semver-checks`,
  then run it before a 0.3.0 release tag. Current environment reports
  `no such command: semver-checks`.
- <a id="miri-partition-projection"></a>[patch] Re-run Miri on partition and
  projection paths after stable gates. Evidence target: machine-checked Stacked
  Borrows / Tree Borrows where toolchain support is available.
- <a id="feature-hygiene"></a>[patch] Verify feature matrix: default, `alloc`,
  `--no-default-features`, and documented nightly-only `--all-features`
  behavior.

## Closed

- <a id="partition-driver-memory"></a>[patch] Partition driver memory discipline
  delivered in 0.2.1. `partition_map` uses overflow-safe ceiling division and
  reserves worker handles to the actual non-empty shard count.
- <a id="guard-projection"></a>[minor] Zero-copy guard projection delivered in
  0.2.0 with `MelinoeRef`/`MelinoeMut` `map` and `map_split`.
