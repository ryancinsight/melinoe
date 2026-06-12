# Gap Audit — melinoe

Audit date: 2026-06-04. Scope: correctness/soundness, performance, memory
efficiency, branding capability surface, testing, benchmarking, documentation.

## Method

Full read of every source, test, and bench module; baseline `cargo test`,
`cargo clippy --all-targets`, and `cargo miri test` across all paths (Stacked
Borrows default + Tree Borrows on the projection/branding paths).
The 0.6.0 increment audited the full source tree again end to end; the access
core, token families, guards, atomics, and Cow/slice paths remain optimal and
unchanged. Two gaps were found and closed: a shard-count SSOT duplication in the
partition driver (`src/region/mod.rs`, `src/sync/partition.rs`) and a feature-gate
defect in `examples/codegen.rs`. The prior increment audited `src/cell/cow.rs`,
`src/atomic.rs`, `src/static_assertions.rs`, `tests/conditional_cow.rs`,
`tests/conditional_atomics.rs`, and the Mnemosyne / conditional-atomic Criterion
harnesses.

The Apollo provider increment adds `tests/apollo_boundary.rs` as an explicit
consumer contract for branded scratch boundaries. It verifies that the static
`Borrowed` ZST policy returns a pointer-identical `Cow::Borrowed` with zero
element clones, while the static `Retained` ZST policy produces independent
owned storage with exactly one clone per element. Evidence tier:
value-semantic integration tests plus the existing ZST/type-level policy
surface.

## Findings

### Default provider feature policy — closed

Melinoe did not expose the Atlas-wide default `parallel` and
`mnemosyne-memory` feature contract. Added zero-dependency `parallel` and a
`mnemosyne-memory` feature forwarding to `alloc`, which is the minimum memory
surface required by branded Cow/cell ownership boundaries. Evidence tier:
Cargo metadata audit across Apollo, Leto, Hermes, Mnemosyne, Moirai, Melinoe,
Themis, and Hephaestus; formatting and diff checks. Compile/test residual:
target lockfile access denied before rustc.

### Soundness — clean

The four `unsafe` sites (token minting, cell access, `from_mut`, `Send`/`Sync`
impls) each discharge their obligation through a higher-ranked invariant lifetime
or the `#[repr(transparent)]` layout chain, with inline `// SAFETY:` reasoning.
Miri reports no aliasing or data-race violations on any access, slice-view,
partition, cross-thread, or projection path. Evidence tier: **machine-checked**
(Miri) on top of type-level encoding.

### Performance / memory — already optimal on the access core

Token access lowers to a bare load/store (confirmed by `examples/codegen.rs` and
the `access` benchmarks); tokens and guards are ZST / `#[repr(transparent)]`
(pinned by `src/static_assertions.rs`). There is no synchronization instruction
to remove on the hot path. The remaining memory-efficiency lever was **reaching a
sub-field of a large branded payload through a permit** without copying the whole
payload — previously only expressible by cloning the payload out. Closed below.

### Capability surface — one gap closed

`MelinoeCell` exposed whole-value `borrow` / `borrow_mut` and whole-slice
`CellSliceExt`, but no way to project a guard onto a component (the `Ref::map`
analogue) or to derive two disjoint `&mut` from one write permit. Added
`MelinoeRef`/`MelinoeMut` `map` and `map_split` ([0.2.0]).

### Partition driver memory — gap closed

`sync::partition_map` could reserve `parts` join handles even though it can
spawn only non-empty shards. For empty input this reserved needless capacity;
for `parts > len` it could amplify allocation beyond useful work. The old
ceiling division also used `len + parts - 1`, which is overflow-prone at
`usize::MAX` inputs. Fixed in [0.2.1]: chunk size now uses
`1 + (len - 1) / requested_parts`, and the handle vector capacity is the actual
non-empty shard count. Evidence tier: value-semantic integration tests plus
Criterion scheduling benchmarks.

### Multithreading ergonomics — gap closed

The fixed `parts: usize` API forced callers to compute a worker count outside
the crate and could not express cache/tile-oriented chunk sizing directly.
Added `PartitionPlan` in [0.3.0] with fixed part count, reported hardware
parallelism, and fixed chunk-size variants. `partition_map_with` /
`partition_for_each_with` execute the same disjoint-shard engine with a typed
plan, and `partition_map_available` / `partition_for_each_available` use
`std::thread::available_parallelism()` for the common hardware-parallel case.
Evidence tier: type-level API plus value-semantic tests for plan equivalence,
chunk tiling, and platform-independent full coverage.

### Boundary policy monomorphization — gap closed

Conditional ownership and atomic ordering were previously expressed either by
ad hoc benchmark-local `Cow` branching or by runtime `Ordering` arguments.
Added [0.4.0] ZST policy surfaces: `Borrowed` / `Retained` for static
borrow-or-retain decisions, and `Relaxed` / `AcqRel` / `SeqCst` for atomic
ordering contracts. The runtime `RetainDecision` and `Ordering` APIs remain for
data-dependent cases. Evidence tier: compile-time ZST size assertions plus
value-semantic tests for borrowed pointer identity, retained copy independence,
runtime retain decisions, and ZST atomic ordering equivalence.

Refinement pass: `AtomicOrder` is sealed to the crate's audited policy set;
`BrandedAtomic::get_mut` / `into_inner` use the standard atomic unique/owned
APIs rather than pointer reads; static `Cow` policies dispatch through policy
method bodies, so the borrowed monomorph contains no clone branch. Added
read-permit-gated `BrandedAtomic::as_atomic` for zero-copy interop with raw
atomic APIs while preserving the shared-phase token proof; `as_atomic_mut` and
`into_atomic` cover unique/owned extraction. Latest patch routes
`BrandedAtomic::*_with` methods directly through the sealed atomic mediation
surface with `AtomicOrder` associated constants, avoiding runtime-ordering
wrapper calls in static policy monomorphs. Latest Cow refinement routes direct
`borrow_cow` / `retain_cow`, generic-policy `borrow_cow_with`, and runtime
`borrow_cow_if` through the same sealed `Borrowed` / `Retained` policy bodies,
so clone/no-clone behavior has one implementation source. Benchmark expansion
adds direct-vs-ZST-policy Cow rows and read-permit-gated `as_atomic` interop;
targeted Criterion reruns show static Cow policy rows match direct methods
within local run noise and shared atomic interop matches raw atomic throughput.

### Partition shard-count SSOT — gap closed (0.6.0)

The shard count was computed in two places: the private `shard_count(len, chunk)`
ceiling-division helper (used to size the worker-handle `Vec`) and, implicitly,
the `ShardChunks` iterator that actually yields the shards. The two agreed, but
the duplication was a latent SSOT/DRY hazard — a future change to chunking could
desynchronize the reserved capacity from the real yield. `ShardChunks` now
implements `ExactSizeIterator` with an exact `size_hint`
(`ceil(remaining / chunk)`, decrementing as consumed), and `partition_map_with`
reserves capacity from `chunks.len()`. The helper and the `ResolvedPartitionPlan`
struct are removed; `PartitionPlan::resolve` returns only the chunk size. The
empty/over-partitioned memory-efficiency contract is unchanged (the iterator
reports `0` for an empty region) and is pinned by both the new exact-size tests
and the `partition_driver/empty_region` benchmark (~42 ns, no spawn). The new
`ExactSizeIterator` impl is additive public API ([minor]). Evidence tier:
value-semantic tests plus Criterion confirmation of no regression.

### Feature hygiene — gap closed (0.6.0)

`examples/codegen.rs` used the alloc-gated `CellCowExt::borrow_cow` but carried no
`required-features`, so `cargo test --no-default-features` failed to compile the
example despite the checklist claiming the gate passed. Fixed by declaring
`required-features = ["alloc"]` for the example in `Cargo.toml`. The full feature
matrix (`--no-default-features`, `--no-default-features --features alloc`,
`--features std`) now builds and tests clean.

## Residual risk / non-goals

- Projecting arbitrary *separate* cells (not sub-components of one payload) to
  simultaneous `&mut` is intentionally **not** added: distinctness of two
  independent `MelinoeCell`s is not provable to the borrow checker without a
  runtime pointer check, which would violate the zero-cost invariant. The slice
  (`CellSliceExt` + `split_at_mut`), `WriterShard`, and `map_split` paths cover
  the disjoint-`&mut` need where disjointness is structurally provable.
- `--all-features` requires a nightly toolchain (the `nightly` feature gates
  `feature(doc_cfg)`); this is by design. Stable builds use the default feature
  set. Not a defect.

## Status

0.6.0 increment implemented and tracked in `checklist.md` / `backlog.md`.
Stable gates green: `fmt --check`, `clippy --all-targets -- -D warnings`,
`test --features std` (88 passed), `doc --no-deps`, and the full feature matrix
(`--no-default-features`, `--no-default-features --features alloc`). The
`partition_driver` Criterion group was rerun (fast sweep); `empty_region`
(~42 ns) confirms the no-spawn / zero-capacity contract survives the SSOT
refactor. Version bumped 0.5.0 → 0.6.0 ([minor], additive public API:
`ShardChunks: ExactSizeIterator`). CHANGELOG synchronized.

All prior verification residuals are now resolved. **Miri** is clean across the
full suite (no UB, no data races): `projection` (6), `partition` (15, including
the new exact-size tests under real `std::thread::scope`), `threads` (6),
`conditional_atomics` (8), `conditional_cow` (5), `branding` (7), `multi_token`
(8), `slice_views` (4), `differential` (3) — evidence tier: machine-checked.
**`cargo-semver-checks`** runs via the git-rev baseline (`--baseline-rev HEAD`):
v0.5.0 → v0.6.0 reports no semver update required, confirming the [minor]
classification; default registry comparison awaits publication, and
semver-checks 0.48.0 skips its lints against the current nightly rustdoc-JSON
format (tool/format mismatch, not a crate defect). **Nightly clippy**
`--all-targets --all-features -- -D warnings` is clean (the local MSYS2 nightly
bakes the stable channel, so the `doc_cfg` feature gate needs
`RUSTC_BOOTSTRAP=1`).

### Prior increments (historical)

Current minor increment implemented and tracked in `checklist.md` /
`backlog.md`. Stable gates green: `fmt --check`, `clippy --all-targets -D
warnings`, `test`, `doc --no-deps`, no-default feature tests, and benchmark
compilation for both Criterion harnesses. `cargo miri test --test partition`
passes under Stacked Borrows and Tree Borrows. Version bumped 0.2.1 → 0.3.0
([minor], additive public API). CHANGELOG synchronized.

Current target is now 0.4.0 ([minor], additive public API) for conditional
`Cow` boundary policies and monomorphized atomic ordering policies. Stable gates
are green: `fmt --check`, `clippy --all-targets -D warnings`, `test`,
`doc --no-deps`, no-default feature tests, and benchmark compilation for all
five Criterion harnesses. Miri passes for conditional atomic / conditional Cow
tests under Stacked Borrows and Tree Borrows.

`cargo-semver-checks` is installed, but default comparison fails because
`melinoe` is not found in crates.io; a baseline rev or registry release is
needed before tagging. Stable `--all-features` still fails at the documented
nightly `doc_cfg` feature gate.

Benchmark note: the access suite was run repeatedly here (incl. a
`--sample-size 200` sweep; the `--measurement-time 10` variant crashed on the
constant-folding Melinoe read micro-benchmark, which balloons to ~51e9 estimated
iterations — `measurement-time 5` is the safe ceiling for this suite). This
machine proved load-saturated: single-threaded Melinoe micro-figures were stable
but the multithreaded/lock-contended absolutes swung 15–60% run-to-run (the
`single_thread` partition baseline ranged 15–25 ms). Because this work was
rebased onto a parallel branch that had independently **refreshed all
benchmark numbers** on a cleaner machine, the merged BENCHMARKS.md retains that
branch's internally-consistent figures (e.g. `AtomicU64` increment ~30×) rather
than this session's noisy re-measurements; only the genuinely new rows
(`projection_1024x`, `partition_driver`, and the `PartitionPlan`
available-parallelism / fixed-chunk rows) carry this session's data, with the
core-count-dependent ones labelled "measure locally." Ratios are the durable
signal across both machines.

[0.4.0]: CHANGELOG.md
[0.3.0]: CHANGELOG.md
[0.2.1]: CHANGELOG.md
[0.2.0]: CHANGELOG.md
