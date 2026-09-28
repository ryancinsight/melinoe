//! Concurrent disjoint-write tests for `WriterShard` and the partition drivers.

// Test code is exempt from `clippy::unwrap_used`: a panic here is the
// assertion, not a defect escaping into a consumer's process.
#![allow(clippy::unwrap_used)]

use melinoe::region::WriterShard;
use melinoe::{brand_scope, MelinoeCell};

/// The executor registry is process-global: every test that calls a partition
/// driver must hold the guard, both to serialize registration windows and to
/// guarantee a clean registry baseline. A guard dropped during a test's unwind
/// poisons the lock; later tests recover it so one genuine failure never
/// cascades spurious failures into its neighbors.
#[cfg(feature = "std")]
static EXECUTOR_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(feature = "std")]
struct ExecutorTestGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(feature = "std")]
impl ExecutorTestGuard {
    fn acquire() -> Self {
        let lock = EXECUTOR_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        melinoe::sync::clear_parallel_executor();
        Self { _lock: lock }
    }
}

#[cfg(feature = "std")]
impl Drop for ExecutorTestGuard {
    fn drop(&mut self) {
        melinoe::sync::clear_parallel_executor();
    }
}

/// Single-threaded split: two disjoint shards write their halves; the whole
/// region reads back correctly via the token afterwards.
#[test]
fn split_writes_disjoint_halves() {
    brand_scope(|token| {
        let mut cells: [MelinoeCell<'_, usize>; 8] = core::array::from_fn(|_| MelinoeCell::new(0));

        let (mut lo, mut hi) = WriterShard::new(&mut cells).split_at(4);
        for (j, slot) in lo.iter_mut().enumerate() {
            *slot = j;
        }
        for (j, slot) in hi.iter_mut().enumerate() {
            *slot = 100 + j;
        }

        let snap = token.share();
        let seen: [usize; 8] = core::array::from_fn(|k| *cells[k].borrow(snap));
        assert_eq!(seen, [0, 1, 2, 3, 100, 101, 102, 103]);
    });
}

/// `chunks` yields strictly disjoint, gap-free, fully-covering shards.
#[test]
fn chunks_cover_region_without_overlap() {
    brand_scope(|token| {
        let mut cells: Vec<MelinoeCell<'_, usize>> = (0..10).map(|_| MelinoeCell::new(0)).collect();

        let mut total = 0;
        for (chunk_idx, mut shard) in WriterShard::new(&mut cells).chunks(3).enumerate() {
            total += shard.len();
            for slot in &mut shard {
                *slot = chunk_idx;
            }
        }
        assert_eq!(total, 10);

        // Chunk size 3 over 10 cells → shards of len 3,3,3,1 tagged 0,1,2,3.
        let snap = token.share();
        let tags: Vec<usize> = cells.iter().map(|c| *c.borrow(snap)).collect();
        assert_eq!(tags, vec![0, 0, 0, 1, 1, 1, 2, 2, 2, 3]);
    });
}

/// `chunks` reports its exact remaining shard count up front and as it is
/// consumed, so a driver can reserve worker capacity from the iterator alone.
#[test]
fn chunks_report_exact_size() {
    brand_scope(|_token| {
        let mut cells: Vec<MelinoeCell<'_, usize>> = (0..10).map(|_| MelinoeCell::new(0)).collect();

        // 10 cells / chunk 3 → ceil = 4 shards (3,3,3,1).
        let mut chunks = WriterShard::new(&mut cells).chunks(3);
        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks.size_hint(), (4, Some(4)));

        // The reported count decrements exactly as shards are yielded.
        let mut observed = 0;
        let mut expected_remaining = 4;
        while let Some(shard) = chunks.next() {
            let _ = shard;
            expected_remaining -= 1;
            observed += 1;
            assert_eq!(chunks.len(), expected_remaining);
        }
        assert_eq!(observed, 4);
        assert_eq!(chunks.len(), 0);
    });
}

/// An empty region yields zero shards — the exact size is `0`, so a driver
/// reserves no capacity and spawns no worker for it.
#[test]
fn empty_region_chunks_report_zero() {
    brand_scope(|_token| {
        let mut cells: [MelinoeCell<'_, usize>; 0] = [];
        let chunks = WriterShard::new(&mut cells).chunks(8);
        assert_eq!(chunks.len(), 0);
        assert_eq!(chunks.size_hint(), (0, Some(0)));
        assert_eq!(chunks.count(), 0);
    });
}

/// Read capability is available through `&shard`; write through `&mut shard`.
#[test]
fn shard_read_and_write_capabilities() {
    brand_scope(|_token| {
        let mut cells: [MelinoeCell<'_, i32>; 3] =
            core::array::from_fn(|i| MelinoeCell::new(i as i32));
        let mut shard = WriterShard::new(&mut cells);

        // read via &self
        assert_eq!(shard.as_slice(), &[0, 1, 2]);
        assert_eq!(shard.get(1), Some(&1));

        // write via &mut self (which also still reads)
        *shard.get_mut(1).unwrap() = 42;
        assert_eq!(shard.as_slice(), &[0, 42, 2]);
    });
}

/// A shard is iterable directly via `IntoIterator` for `&`/`&mut` references.
#[test]
fn shard_into_iterator() {
    brand_scope(|_token| {
        let mut cells: [MelinoeCell<'_, i32>; 4] =
            core::array::from_fn(|i| MelinoeCell::new(i as i32));
        let mut shard = WriterShard::new(&mut cells);

        for slot in &mut shard {
            *slot *= 10;
        }
        let sum: i32 = (&shard).into_iter().sum();
        assert_eq!(sum, 60); // 0 + 10 + 20 + 30
    });
}

#[cfg(feature = "std")]
#[path = "partition/concurrent.rs"]
mod concurrent;

// ── Property-based partition correctness: disjoint, complete coverage ──
//
// Generalizes the fixed-size `partition_map` examples over arbitrary cell and
// partition counts. Writing each cell's global index across `parts` shards must
// (a) cover every index exactly once — the per-shard partial sums equal the
// closed form 0+1+..+(n-1) — and (b) leave every cell holding its own index,
// i.e. the partition is disjoint and complete for any (n, parts).

#[cfg(feature = "std")]
proptest::proptest! {
    #[test]
    fn prop_partition_map_covers_every_index_disjointly(
        n in 1usize..256,
        raw_parts in 1usize..32,
    ) {
        let _guard = ExecutorTestGuard::acquire();
        let parts = raw_parts.min(n);
        brand_scope(|token| {
            let mut cells: Vec<MelinoeCell<'_, u64>> =
                (0..n).map(|_| MelinoeCell::new(0)).collect();
            let sums: Vec<u64> =
                melinoe::sync::partition_map(&mut cells, parts, |start, mut shard| {
                    let mut local = 0u64;
                    for (j, slot) in shard.iter_mut().enumerate() {
                        let v = (start + j) as u64;
                        *slot = v;
                        local += v;
                    }
                    local
                });
            // (a) per-shard partial sums add to 0+1+..+(n-1) → complete coverage.
            let expected = (n as u64) * (n as u64 - 1) / 2;
            assert_eq!(sums.iter().sum::<u64>(), expected);
            // (b) every cell holds its own global index → disjoint, no overwrites.
            let snap = token.share();
            for (k, c) in cells.iter().enumerate() {
                assert_eq!(*c.borrow(snap), k as u64);
            }
        });
    }

    /// Read-side partition over a plain slice: the disjoint shared shards passed
    /// to `f` must tile the whole slice in order — each element appears in
    /// exactly one shard at its correct global offset, and the shard lengths sum
    /// to the slice length for any (n, parts). No `brand_scope` needed.
    #[test]
    fn prop_partition_read_tiles_slice_in_order(
        n in 0usize..256,
        raw_parts in 1usize..32,
    ) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let _guard = ExecutorTestGuard::acquire();
        let parts = raw_parts.min(n.max(1));
        let data: Vec<usize> = (0..n).collect();
        let covered = AtomicUsize::new(0);
        melinoe::sync::partition_read_for_each(&data, parts, |start, shard| {
            // each element equals its global index → shard sits at offset `start`,
            // contents are in order, and shards do not overlap.
            for (j, &v) in shard.iter().enumerate() {
                assert_eq!(v, start + j);
            }
            covered.fetch_add(shard.len(), Ordering::Relaxed);
        });
        // every element visited exactly once → complete, disjoint coverage.
        proptest::prop_assert_eq!(covered.load(Ordering::Relaxed), n);
    }

    /// `partition_read_map` returns one result per shard in partition order.
    /// `parts` is a *target*: the actual shard count is `ceil(n / ceil(n/parts))`,
    /// which lies in `[1, parts]`. For any (n, parts), folding each shard to its
    /// element-sum yields partial sums whose total is the closed form
    /// `0+1+..+(n-1)` — i.e. the shards tile the slice disjointly and completely,
    /// and the per-shard results are returned (not dropped).
    #[test]
    fn prop_partition_read_map_returns_disjoint_shard_sums(
        n in 1usize..256,
        raw_parts in 1usize..32,
    ) {
        let _guard = ExecutorTestGuard::acquire();
        let parts = raw_parts.min(n);
        let data: Vec<u64> = (0..n as u64).collect();
        let sums: Vec<u64> =
            melinoe::sync::partition_read_map(&data, parts, |_start, shard| shard.iter().sum());
        proptest::prop_assert!(!sums.is_empty() && sums.len() <= parts);
        let expected = (n as u64) * (n as u64 - 1) / 2;
        proptest::prop_assert_eq!(sums.iter().sum::<u64>(), expected);
    }
}
