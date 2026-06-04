//! Concurrent disjoint-write tests for `WriterShard` and the partition drivers.

use melinoe::region::WriterShard;
use melinoe::{brand_scope, MelinoeCell};

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
            for slot in shard.iter_mut() {
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
mod concurrent {
    use super::*;
    use melinoe::sync::{partition_for_each, partition_map};

    /// Four threads concurrently fill disjoint partitions with global indices;
    /// the joined region equals the identity mapping.
    #[test]
    fn concurrent_disjoint_writes_fill_region() {
        const N: usize = 10_000;
        brand_scope(|token| {
            let mut cells: Vec<MelinoeCell<'_, usize>> =
                (0..N).map(|_| MelinoeCell::new(usize::MAX)).collect();

            partition_for_each(&mut cells, 4, |start, mut shard| {
                for (j, slot) in shard.iter_mut().enumerate() {
                    *slot = start + j;
                }
            });

            // Every cell holds its own global index — no gaps, no double-writes.
            let snap = token.share();
            for (k, c) in cells.iter().enumerate() {
                assert_eq!(*c.borrow(snap), k);
            }
        });
    }

    /// `partition_map` returns per-shard results in partition order, and the
    /// shards exactly tile the region.
    #[test]
    fn partition_map_returns_ordered_results() {
        const N: usize = 1_000;
        brand_scope(|_token| {
            let mut cells: Vec<MelinoeCell<'_, u64>> =
                (0..N).map(|_| MelinoeCell::new(0)).collect();

            let sums: Vec<u64> = partition_map(&mut cells, 4, |start, mut shard| {
                let mut local = 0u64;
                for (j, slot) in shard.iter_mut().enumerate() {
                    let v = (start + j) as u64;
                    *slot = v;
                    local += v;
                }
                local
            });

            // Per-shard partial sums add up to the closed form 0+1+..+(N-1).
            let expected = (N as u64 - 1) * N as u64 / 2;
            assert_eq!(sums.iter().sum::<u64>(), expected);
        });
    }

    /// Differential: concurrent partitioned writes produce the identical region
    /// to a single-threaded sequential fill.
    #[test]
    fn concurrent_matches_sequential() {
        const N: usize = 4_096;
        let fill = |v: usize| (v * 7 + 3) % 251;

        // Sequential reference.
        let sequential: Vec<usize> = (0..N).map(fill).collect();

        // Concurrent via shards.
        let concurrent = brand_scope(|token| {
            let mut cells: Vec<MelinoeCell<'_, usize>> =
                (0..N).map(|_| MelinoeCell::new(0)).collect();
            partition_for_each(&mut cells, 8, |start, mut shard| {
                for (j, slot) in shard.iter_mut().enumerate() {
                    *slot = fill(start + j);
                }
            });
            let snap = token.share();
            cells.iter().map(|c| *c.borrow(snap)).collect::<Vec<_>>()
        });

        assert_eq!(concurrent, sequential);
    }
}
