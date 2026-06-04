# Checklist — melinoe

Target version: 0.3.0

## Current micro-sprint

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
- [x] [patch] Compile updated benchmark harnesses: `cargo bench --bench access
  --no-run` and `cargo bench --bench mnemosyne --no-run`.
- [x] [patch] Run Miri partition suite under Stacked Borrows and Tree Borrows.

## Next concrete increment

- [ ] [minor] Install or provide `cargo-semver-checks`, then run it before
  tagging 0.3.0. Current environment reports `no such command: semver-checks`.
- [ ] [patch] Run Miri on projection paths after the partition suite.
- [ ] [patch] Run nightly-only `cargo clippy --all-targets --all-features -- -D
  warnings` on a nightly toolchain; stable fails because the documented
  `nightly` feature enables `#![feature(doc_cfg)]`.
