# Benchmarks: Melinoe vs `Mutex` / `RwLock` / `AtomicU64`

Reproduce:

```sh
cargo bench --bench access
# faster, lower-confidence sweep:
cargo bench --bench access -- --warm-up-time 0.3 --measurement-time 1.0 --sample-size 30
```

Harness: [`benches/access.rs`](benches/access.rs) (Criterion). Equivalence of the
work measured is pinned by [`tests/differential.rs`](tests/differential.rs),
which asserts all four mechanisms compute identical results — the comparison is
over the *same* computation, not coincidentally-similar ones.

## What is and isn't being compared

Melinoe discharges the `T xor &mut T xor &T` exclusion at **compile time**; the
others discharge it at **run time**. The benchmarks measure the cost of one
access *once exclusion is already guaranteed* by each mechanism. Melinoe is not a
drop-in for runtime-contended shared **mutation** (it forbids concurrent writers
at compile time rather than serializing them), so the contended-write case is
deliberately omitted as non-substitutable.

## Representative results

Single run, `x86_64-pc-windows-gnu`, release. Absolute numbers are
hardware-dependent; the **ratios** are the signal. Median of the Criterion
interval, normalised to per-operation.

### Single-threaded read-modify-write (`increment_1024x`)

| Mechanism        | Time / 1024 ops | Per op    | vs Melinoe |
|------------------|-----------------|-----------|-----------:|
| **Melinoe**      | 219.7 ns        | ~0.21 ns  |       1.0× |
| `AtomicU64` (Relaxed) | 6.26 µs    | ~6.1 ns   |      ~29×  |
| `RwLock` (write) | 8.78 µs         | ~8.6 ns   |      ~40×  |
| `Mutex`          | 11.93 µs        | ~11.6 ns  |      ~54×  |

### Single-threaded read (`read_1024x`)

| Mechanism        | Time / 1024 ops | Note |
|------------------|-----------------|------|
| **Melinoe**      | ~220 ps         | The loop-invariant `&T` read is hoisted out of the loop — exactly the optimization a bare reference permits and a lock/atomic forbids. |
| `AtomicU64` (Relaxed) | 106 ns     | Relaxed load is cheap but not freely hoistable. |
| `Mutex`          | 8.03 µs         | A lock acquire/release per read. |
| `RwLock` (read)  | 9.01 µs         | A read-lock (atomic RMW on the lock word) per read. |

The sub-nanosecond Melinoe figure is **not** a realistic single-access latency;
it is the optimizer proving the repeated read redundant because token access
carries no side effect. That transparency to optimization *is* the zero-cost
property — see the assembly in [`examples/codegen.rs`](examples/codegen.rs),
where one access is a single `mov`.

### Concurrent reads, 4 threads × 4096 reads (`concurrent_reads_4threads`)

| Mechanism        | Wall time / sample | Throughput |
|------------------|--------------------|-----------:|
| **Melinoe** (`SharedReadToken`) | 264 µs  | ~62 Melem/s |
| `AtomicU64` (Relaxed) | 249 µs        | ~66 Melem/s |
| `Mutex`          | 755 µs             | ~22 Melem/s |
| `RwLock` (read)  | 834 µs             | ~20 Melem/s |

Melinoe shared reads match a relaxed atomic load and run **~3× the throughput**
of `Mutex`/`RwLock`, whose lock words bounce between cores under read traffic.
Caveat: each Criterion sample re-spawns the four threads, so this figure is
dominated by `thread::scope` spawn/join overhead; the lock-based rows carry the
contention penalty *on top* of that shared baseline. The single-threaded rows
above are the cleaner per-access evidence.

### Concurrent disjoint writes, compute-bound, 4 threads × ~1M elements (`partitioned_compute_1m`)

The sound form of "concurrent writes": each thread owns a disjoint
[`WriterShard`](src/region/mod.rs) partition (via `split_at_mut`), so writes run
in parallel with no atomics and no locks. A non-trivial per-element kernel makes
the work compute-bound (a simple-store version would be memory-bandwidth-bound
and dominated by thread-spawn overhead, measuring nothing useful).

| Mechanism | Time | Speedup vs 1 thread |
|-----------|------|--------------------:|
| `single_thread` (baseline) | 14.6 ms | 1.00× |
| **Melinoe** disjoint shards | 5.16 ms | **2.83×** |
| `AtomicU64` disjoint stores | 5.11 ms | 2.86× |
| `Mutex<Vec>` (lock across writers) | 17.3 ms | **0.84×** (slower than serial) |

* Melinoe shards achieve real parallel speedup (~2.8× on 4 cores) using **plain
  stores** — no synchronization on the write path.
* They match lock-free atomics here because the heavy per-element compute hides
  the atomic-store cost; when stores dominate, Melinoe's plain store is far
  cheaper (see the ~29× single-threaded RMW gap above).
* A `Mutex<Vec>` — the idiomatic way to *share* a `Vec` across threads, since it
  cannot express disjoint `&mut` — serializes the writers and ends up **slower
  than the single-threaded baseline** (lock + spawn overhead, zero parallelism).
  This is the case the shard model is designed to replace.

Caveat: each Criterion sample re-spawns the worker threads via `thread::scope`;
with a persistent pool the parallel rows would improve further. The point is the
*relative* behaviour: lock-free disjoint shards scale, a shared lock does not.

## Mnemosyne access patterns (`cargo bench --bench mnemosyne`)

Patterns a branded allocator exercises: bulk slab init/scan via per-cell token
access vs Melinoe's zero-copy [`CellSliceExt`](src/cell/slice.rs) views, and a
`Cow` borrow-or-spill at the ownership boundary.

| Benchmark (64k u64 slab) | per-cell token | slice view | Result |
|--------------------------|----------------|------------|--------|
| `slab_fill` (write all)  | 6.15 µs | 6.21 µs | **parity** |
| `slab_scan` (sum all)    | 6.44 µs | 6.26 µs | **parity** |

The slice view *matches* the per-cell path rather than beating it — which is the
point: per-cell token access is already zero-cost and autovectorizes to the same
memory-bandwidth-bound code (~85 GB/s here). The `&[T]` / `&mut [T]` view adds no
overhead while giving native-slice ergonomics (`fill`, `copy_from_slice`, SIMD,
FFI hand-off) and a contiguity guarantee. Confirmation of zero-cost, not a
speedup claim.

| `cow_escape` (4k u8, 1-in-8 must own) | time | |
|---------------------------------------|------|--|
| `always_owned` (clone every call)     | 65.6 ns | 1.00× |
| `cow_borrow_mostly` (clone 1/8 calls) | 33.7 ns | **1.95× faster** |

`Cow` nearly halves cost by borrowing the branded slab zero-copy on the common
transient path and cloning only when a buffer must outlive the brand scope. It
lives at the ownership boundary by design: inside the zero-cost access core a
branded borrow is *always* zero-copy, so a `Cow` there would be a degenerate
always-`Borrowed`.

## Interpretation

* Melinoe access is a bare load/store: zero synchronization instructions, and
  fully transparent to the optimizer (hoisting, vectorization, constant folding).
* The runtime primitives each pay for their dynamic guarantee on every access —
  an atomic RMW (`Atomic`, `RwLock`), or a lock acquire/release (`Mutex`).
* Melinoe buys this by requiring the exclusion to be *statically provable*. Where
  it is (single-writer regions, branded heaps, scoped handoff), the runtime cost
  is eliminated, not merely reduced.
