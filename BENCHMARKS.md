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

### Concurrent read scaling (`cargo bench --bench concurrent_reads`)

Each thread sweeps a shared 1024×`u64` buffer `PASSES` times; spawn is amortised
(many sweeps per spawn) and each sweep re-reads behind `black_box` so the loads
are real. Reported as **throughput, Gelem/s** across thread counts (24-core host).

> An earlier `concurrent_reads_4threads` group spawned threads *inside* every
> sample and did only a few reads — it measured `thread::scope` spawn overhead,
> not read throughput, and wrongly showed Melinoe merely ≈ atomics. That group
> was removed; this is the corrected measurement.

| threads | **melinoe** | `RwLock` read | `Mutex` | `AtomicU64` (per-elem) |
|--------:|------------:|--------------:|--------:|-----------------------:|
| 1  | 13.3 | 12.0 | 12.2 | 7.6  |
| 2  | 24.3 | 10.3 | 9.3  | 15.0 |
| 4  | 45.3 | 10.4 | 8.0  | 29.3 |
| 8  | 74.0 | 11.6 | 6.9  | 46.6 |
| 16 | **103.5** | 9.8 | 6.7 | 72.9 |

* **Melinoe scales near-linearly** (≈7.8× from 1→16 threads): a branded read is a
  plain load with *zero shared mutable state*, so cores never contend.
* **`RwLock` does not scale** — it is flat-to-degrading (~10 Gelem/s at every
  thread count) because `read()` does an atomic RMW on the shared reader count,
  whose cache line ping-pongs between cores. At 16 threads Melinoe is **~10.5×**
  `RwLock` and **~15×** `Mutex`.
* **Atomics scale** (lock-free, read-only) but trail Melinoe by ~40% because
  per-element atomic loads do not autovectorize.
* `melinoe_per_cell` ≈ `melinoe_slice`: the per-cell `borrow` loop already
  autovectorizes (~4 `u64`/cycle single-thread), so the slice view is an
  ergonomic convenience here, not a speedup. The read path is already optimal —
  this benchmark is the evidence, not a code change.

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

## Disjoint per-thread counters: false sharing & memory (`cargo bench --bench false_sharing`)

Pattern: 8 threads each accumulate into their *own* counter, results read after
the join (per-thread allocator statistics). The decisive property is whether the
write can be register-promoted.

| Variant | Throughput | Memory / counter |
|---------|-----------:|-----------------:|
| `raw_split_mut` (`&mut u64`) | 9.2 Gelem/s | 8 B |
| **`melinoe_shards`** (`MelinoeCell<u64>`) | **10.1 Gelem/s** | **8 B** |
| `atomic_adjacent` (`AtomicU64`) | 0.12 Gelem/s | 8 B |
| `atomic_padded` (`#[repr(align(128))]`) | 1.06 Gelem/s | 128 B |

* **Melinoe matches raw `split_at_mut`** — the disjoint shard is zero-cost.
* It is **~83× faster than adjacent atomics** and **~9.5× faster than padded
  atomics**, at **8 B/counter** (no padding). Because the type system proves
  single-writer, the compiler keeps the counter in a register and writes back
  once, so the shared cache line is never touched mid-loop — no false sharing,
  no memory RMW.
* An `AtomicU64` cannot be register-promoted: every `fetch_add` is a real memory
  RMW, so adjacent counters bounce their shared line (false sharing). Recovering
  throughput needs cache-line padding — **16× the memory** — and still trails
  Melinoe.

The takeaway is a *non-change*: Melinoe needs no cache-line padding for disjoint
per-thread state, so the dense (8 B) layout is also the fast one. Adding a padded
cell type would be the wrong fix — it would trade away the memory efficiency the
single-writer proof already buys for free.

## Interpretation

* Melinoe access is a bare load/store: zero synchronization instructions, and
  fully transparent to the optimizer (hoisting, vectorization, constant folding).
* The runtime primitives each pay for their dynamic guarantee on every access —
  an atomic RMW (`Atomic`, `RwLock`), or a lock acquire/release (`Mutex`).
* Melinoe buys this by requiring the exclusion to be *statically provable*. Where
  it is (single-writer regions, branded heaps, scoped handoff), the runtime cost
  is eliminated, not merely reduced.
