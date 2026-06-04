# Changelog

All notable changes to `melinoe` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/) and the project adheres to
[Semantic Versioning](https://semver.org/).

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

[0.3.0]: https://github.com/ryancinsight/melinoe/releases/tag/v0.3.0
[0.2.1]: https://github.com/ryancinsight/melinoe/releases/tag/v0.2.1
[0.2.0]: https://github.com/ryancinsight/melinoe/releases/tag/v0.2.0
[0.1.0]: https://github.com/ryancinsight/melinoe/releases/tag/v0.1.0
