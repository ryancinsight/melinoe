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
mod concurrent {
    use super::*;
    use melinoe::sync::{
        partition_for_each, partition_for_each_available, partition_for_each_with, partition_map,
        partition_map_available, partition_map_with, register_parallel_executor, PartitionPlan,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    static EXECUTED_TASKS: AtomicUsize = AtomicUsize::new(0);

    unsafe fn deterministic_executor(
        num_tasks: usize,
        task_fn: unsafe fn(usize, *mut ()),
        data: *mut (),
    ) {
        EXECUTED_TASKS.store(num_tasks, Ordering::SeqCst);
        for index in 0..num_tasks {
            // SAFETY: this deterministic executor runs every task index exactly
            // once before returning, satisfying `ParallelExecutorFn`.
            unsafe {
                task_fn(index, data);
            }
        }
    }

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

    #[test]
    fn registered_executor_drives_partition_map() {
        const N: usize = 32;
        EXECUTED_TASKS.store(0, Ordering::SeqCst);
        register_parallel_executor(deterministic_executor);

        brand_scope(|token| {
            let mut cells: Vec<MelinoeCell<'_, usize>> =
                (0..N).map(|_| MelinoeCell::new(usize::MAX)).collect();

            let lengths = partition_map(&mut cells, 4, |start, mut shard| {
                for (offset, slot) in shard.iter_mut().enumerate() {
                    *slot = start + offset;
                }
                shard.len()
            });

            assert_eq!(EXECUTED_TASKS.load(Ordering::SeqCst), 4);
            assert_eq!(lengths, vec![8, 8, 8, 8]);
            let snap = token.share();
            for (index, cell) in cells.iter().enumerate() {
                assert_eq!(*cell.borrow(snap), index);
            }
        });
    }

    /// Empty regions spawn no shards and therefore never invoke the worker.
    #[test]
    fn partition_map_empty_region_returns_empty_results() {
        brand_scope(|_token| {
            let mut cells: Vec<MelinoeCell<'_, u64>> = Vec::new();

            let results: Vec<u64> = partition_map(&mut cells, 8, |_start, _shard| {
                panic!("empty regions must not produce worker shards");
            });

            assert!(results.is_empty());
        });
    }

    /// Requesting more partitions than cells still produces only non-empty
    /// shards, in order, with exact full coverage.
    #[test]
    fn partition_map_overpartitioning_produces_no_empty_shards() {
        const N: usize = 5;
        brand_scope(|token| {
            let mut cells: Vec<MelinoeCell<'_, usize>> =
                (0..N).map(|_| MelinoeCell::new(usize::MAX)).collect();

            let lengths: Vec<usize> = partition_map(&mut cells, 32, |start, mut shard| {
                assert!(!shard.is_empty());
                for (j, slot) in shard.iter_mut().enumerate() {
                    *slot = start + j;
                }
                shard.len()
            });

            assert_eq!(lengths, vec![1, 1, 1, 1, 1]);
            let snap = token.share();
            let seen: Vec<usize> = cells.iter().map(|c| *c.borrow(snap)).collect();
            assert_eq!(seen, vec![0, 1, 2, 3, 4]);
        });
    }

    /// The typed fixed-part plan is equivalent to the legacy `parts` argument
    /// while making the scheduling policy explicit at the call site.
    #[test]
    fn partition_map_with_fixed_parts_matches_legacy_partition_map() {
        const N: usize = 33;
        let fill = |v: usize| v.wrapping_mul(11).wrapping_add(5);

        let legacy = brand_scope(|token| {
            let mut cells: Vec<MelinoeCell<'_, usize>> =
                (0..N).map(|_| MelinoeCell::new(0)).collect();
            partition_for_each(&mut cells, 4, |start, mut shard| {
                for (j, slot) in shard.iter_mut().enumerate() {
                    *slot = fill(start + j);
                }
            });
            let snap = token.share();
            cells.iter().map(|c| *c.borrow(snap)).collect::<Vec<_>>()
        });

        let planned = brand_scope(|token| {
            let mut cells: Vec<MelinoeCell<'_, usize>> =
                (0..N).map(|_| MelinoeCell::new(0)).collect();
            partition_for_each_with(&mut cells, PartitionPlan::parts(4), |start, mut shard| {
                for (j, slot) in shard.iter_mut().enumerate() {
                    *slot = fill(start + j);
                }
            });
            let snap = token.share();
            cells.iter().map(|c| *c.borrow(snap)).collect::<Vec<_>>()
        });

        assert_eq!(planned, legacy);
    }

    /// Chunk-size plans expose cache/tile-oriented scheduling directly.
    #[test]
    fn partition_map_with_chunk_size_tiles_region() {
        const N: usize = 10;
        brand_scope(|token| {
            let mut cells: Vec<MelinoeCell<'_, usize>> =
                (0..N).map(|_| MelinoeCell::new(usize::MAX)).collect();

            let lengths: Vec<usize> = partition_map_with(
                &mut cells,
                PartitionPlan::chunk_size(4),
                |start, mut shard| {
                    for (j, slot) in shard.iter_mut().enumerate() {
                        *slot = start + j;
                    }
                    shard.len()
                },
            );

            assert_eq!(lengths, vec![4, 4, 2]);
            let snap = token.share();
            let seen: Vec<usize> = cells.iter().map(|c| *c.borrow(snap)).collect();
            assert_eq!(seen, (0..N).collect::<Vec<_>>());
        });
    }

    /// Hardware-parallel planning must remain value-equivalent independent of
    /// the platform's reported CPU count.
    #[test]
    fn available_parallelism_plan_covers_region_once() {
        const N: usize = 257;
        brand_scope(|token| {
            let mut cells: Vec<MelinoeCell<'_, usize>> =
                (0..N).map(|_| MelinoeCell::new(usize::MAX)).collect();

            let lengths: Vec<usize> = partition_map_available(&mut cells, |start, mut shard| {
                assert!(!shard.is_empty());
                for (j, slot) in shard.iter_mut().enumerate() {
                    *slot = (start + j).wrapping_mul(3);
                }
                shard.len()
            });

            assert_eq!(lengths.iter().sum::<usize>(), N);
            let snap = token.share();
            for (index, cell) in cells.iter().enumerate() {
                assert_eq!(*cell.borrow(snap), index * 3);
            }
        });
    }

    /// The available-parallel for-each convenience function is a write-only
    /// wrapper over the same shard plan.
    #[test]
    fn partition_for_each_available_writes_region() {
        const N: usize = 64;
        brand_scope(|token| {
            let mut cells: Vec<MelinoeCell<'_, usize>> =
                (0..N).map(|_| MelinoeCell::new(0)).collect();

            partition_for_each_available(&mut cells, |start, mut shard| {
                for (j, slot) in shard.iter_mut().enumerate() {
                    *slot = start + j + 1;
                }
            });

            let snap = token.share();
            let seen: Vec<usize> = cells.iter().map(|c| *c.borrow(snap)).collect();
            assert_eq!(seen, (1..=N).collect::<Vec<_>>());
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
