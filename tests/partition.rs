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
        clear_parallel_executor, partition_for_each, partition_for_each_available,
        partition_for_each_with, partition_map, partition_map_available, partition_map_with,
        register_parallel_executor, ParallelExecutor, PartitionPlan,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, MutexGuard};

    static EXECUTED_TASKS: AtomicUsize = AtomicUsize::new(0);
    static EXECUTOR_TEST_LOCK: Mutex<()> = Mutex::new(());

    struct ExecutorTestGuard {
        _lock: MutexGuard<'static, ()>,
    }

    impl ExecutorTestGuard {
        fn acquire() -> Self {
            let lock = EXECUTOR_TEST_LOCK
                .lock()
                .expect("invariant: executor-state tests do not poison their lock");
            clear_parallel_executor();
            Self { _lock: lock }
        }
    }

    impl Drop for ExecutorTestGuard {
        fn drop(&mut self) {
            clear_parallel_executor();
        }
    }

    unsafe fn deterministic_executor(
        num_tasks: usize,
        task_fn: unsafe fn(usize, *mut ()),
        data: *mut (),
    ) {
        EXECUTED_TASKS.store(num_tasks, Ordering::SeqCst);
        for index in 0..num_tasks {
            // SAFETY: this deterministic executor runs every task index exactly
            // once before returning, satisfying `ParallelExecutor`.
            unsafe {
                task_fn(index, data);
            }
        }
    }

    // SAFETY: `deterministic_executor` invokes every index in ascending order
    // exactly once and returns only after the last invocation completes.
    const DETERMINISTIC_EXECUTOR: ParallelExecutor =
        unsafe { ParallelExecutor::new(deterministic_executor) };

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
        let _guard = ExecutorTestGuard::acquire();
        EXECUTED_TASKS.store(0, Ordering::SeqCst);
        register_parallel_executor(DETERMINISTIC_EXECUTOR);

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

    #[test]
    fn clearing_registered_executor_restores_default_driver() {
        const N: usize = 8;
        let _guard = ExecutorTestGuard::acquire();
        EXECUTED_TASKS.store(0, Ordering::SeqCst);
        register_parallel_executor(DETERMINISTIC_EXECUTOR);
        clear_parallel_executor();

        brand_scope(|token| {
            let mut cells: Vec<MelinoeCell<'_, usize>> =
                (0..N).map(|_| MelinoeCell::new(usize::MAX)).collect();

            let lengths = partition_map(&mut cells, 4, |start, mut shard| {
                for (offset, slot) in shard.iter_mut().enumerate() {
                    *slot = start + offset;
                }
                shard.len()
            });

            assert_eq!(EXECUTED_TASKS.load(Ordering::SeqCst), 0);
            assert_eq!(lengths, vec![2, 2, 2, 2]);
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

    /// Public plan resolution gives downstream crates the same overflow-safe
    /// chunk sizing as Melinoe's partition driver.
    #[test]
    fn partition_plan_chunk_len_for_matches_driver_tiling() {
        assert_eq!(PartitionPlan::parts(4).chunk_len_for(10), 3);
        assert_eq!(PartitionPlan::parts(32).chunk_len_for(5), 1);
        assert_eq!(PartitionPlan::parts(0).chunk_len_for(9), 9);
        assert_eq!(PartitionPlan::chunk_size(0).chunk_len_for(9), 1);
        assert_eq!(PartitionPlan::chunk_size(4).chunk_len_for(10), 4);
        assert_eq!(PartitionPlan::parts(4).chunk_len_for(0), 1);
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

    #[test]
    fn read_partition_map_with_delegates_to_driver() {
        let values: Vec<usize> = (0..16).collect();
        let sums = melinoe::sync::partition_read_map_with(
            &values,
            PartitionPlan::chunk_size(4),
            |_start, shard| shard.iter().sum::<usize>(),
        );
        assert_eq!(sums, [6, 22, 38, 54]);
    }

    #[test]
    fn read_partition_for_each_with_delegates_to_driver() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let values: Vec<usize> = (0..16).collect();
        let sum = AtomicUsize::new(0);
        melinoe::sync::partition_read_for_each_with(
            &values,
            PartitionPlan::chunk_size(4),
            |_start, shard| {
                sum.fetch_add(shard.iter().sum::<usize>(), Ordering::SeqCst);
            },
        );
        assert_eq!(sum.load(Ordering::SeqCst), 120);
    }

    #[test]
    fn custom_executor_panic_safety_drops_success_elements() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);
        DROP_COUNT.store(0, Ordering::SeqCst);

        struct DropItem;
        impl Drop for DropItem {
            fn drop(&mut self) {
                DROP_COUNT.fetch_add(1, Ordering::SeqCst);
            }
        }

        register_parallel_executor(DETERMINISTIC_EXECUTOR);

        let mut cells: Vec<MelinoeCell<'_, usize>> = (0..4).map(|_| MelinoeCell::new(0)).collect();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            partition_map_with(&mut cells, PartitionPlan::chunk_size(1), |index, _shard| {
                if index == 2 {
                    panic!("Task 2 failed");
                }
                DropItem
            });
        }));

        clear_parallel_executor();

        let payload = result.expect_err("partition task 2 must propagate its panic");
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"Task 2 failed"));
        // Tasks 0, 1, 3 succeeded and produced DropItem. Task 2 panicked.
        // Therefore, exactly 3 DropItem instances should have been created and dropped by the panic guard.
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn read_custom_executor_panic_safety_drops_success_elements() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);
        DROP_COUNT.store(0, Ordering::SeqCst);

        struct DropItem;
        impl Drop for DropItem {
            fn drop(&mut self) {
                DROP_COUNT.fetch_add(1, Ordering::SeqCst);
            }
        }

        register_parallel_executor(DETERMINISTIC_EXECUTOR);

        let values: Vec<usize> = vec![0; 4];

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            melinoe::sync::partition_read_map_with(
                &values,
                PartitionPlan::chunk_size(1),
                |index, _shard| {
                    if index == 2 {
                        panic!("Task 2 failed");
                    }
                    DropItem
                },
            );
        }));

        clear_parallel_executor();

        let payload = result.expect_err("read partition task 2 must propagate its panic");
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"Task 2 failed"));
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn partition_plan_const_constructors() {
        const PLAN_PARTS: PartitionPlan = PartitionPlan::parts(4);
        const PLAN_CHUNK: PartitionPlan = PartitionPlan::chunk_size(1024);
        const PLAN_AVAIL: PartitionPlan = PartitionPlan::available_parallelism();

        assert!(matches!(PLAN_PARTS, PartitionPlan::Parts(_)));
        assert!(matches!(PLAN_CHUNK, PartitionPlan::ChunkSize(_)));
        assert!(matches!(PLAN_AVAIL, PartitionPlan::AvailableParallelism));
    }
}

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
        let parts = raw_parts.min(n);
        let data: Vec<u64> = (0..n as u64).collect();
        let sums: Vec<u64> =
            melinoe::sync::partition_read_map(&data, parts, |_start, shard| shard.iter().sum());
        proptest::prop_assert!(!sums.is_empty() && sums.len() <= parts);
        let expected = (n as u64) * (n as u64 - 1) / 2;
        proptest::prop_assert_eq!(sums.iter().sum::<u64>(), expected);
    }
}
