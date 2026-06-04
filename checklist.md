# Checklist — melinoe

Target version: 0.5.0

## Current micro-sprint

- [x] [minor] Add direct `CellCowExt::borrow_cow` / `retain_cow` methods for
  common static zero-copy and retain-once boundary cases.
- [x] [patch] Extend codegen probes for direct `borrow_cow`, ZST atomic ordering,
  and read-permit-gated raw atomic interop.
- [x] [patch] Route `BrandedAtomic::*_with` ZST-ordering methods directly through
  associated constants and the sealed atomic mediation surface.
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

## Next concrete increment

- [ ] [minor] Provide a baseline rev or registry release for
  `cargo-semver-checks`. Tool is installed, but `melinoe` is not found in the
  registry, so default comparison cannot run.
- [ ] [patch] Run Miri on projection paths after the partition suite.
- [ ] [patch] Run nightly-only `cargo clippy --all-targets --all-features -- -D
  warnings` on a nightly toolchain; stable fails because the documented
  `nightly` feature enables `#![feature(doc_cfg)]`.
