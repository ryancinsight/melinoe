//! Driver concurrency tests. `register_parallel_executor`/`clear` mutate
//! process-global state that every partition call reads, so EVERY test in
//! this module that touches the driver acquires `ExecutorTestGuard` — never
//! just the registration tests. Without that, an unsynchronized concurrent
//! test can observe a registered executor mid-flight, dirties the shared
//! `EXECUTED_TASKS` side-channel, and breaks the guard tests' driver
//! assertions; the guard then poisons the lock and cascades into the next
//! guard test. Serialization costs nothing here: each test still exercises
//! real concurrency inside its own partition calls.

use super::*;
use melinoe::sync::{
    clear_parallel_executor, partition_for_each, partition_for_each_available,
    partition_for_each_with, partition_map, partition_map_available, partition_map_with,
    register_parallel_executor, ParallelExecutor, PartitionPlan,
};
use std::sync::atomic::{AtomicUsize, Ordering};

static EXECUTED_TASKS: AtomicUsize = AtomicUsize::new(0);

/// A stand-in scheduler that runs tasks in ascending index order on the
/// calling thread, recording how many it was asked for.
struct Deterministic;

// SAFETY: `run_indexed` invokes every index in ascending order exactly once
// and returns only after the last invocation completes, as the contract
// requires. Running on the calling thread adds no concurrency, so it cannot
// observe the context pointer from anywhere the caller did not expect.
unsafe impl ParallelExecutor for Deterministic {
    unsafe fn run_indexed(num_tasks: usize, task: unsafe fn(usize, *mut ()), context: *mut ()) {
        EXECUTED_TASKS.store(num_tasks, Ordering::SeqCst);
        for index in 0..num_tasks {
            // SAFETY: forwarded from the caller; this implementation invokes
            // each index exactly once with the caller's context.
            unsafe { task(index, context) };
        }
    }
}

/// A non-zero-sized implementation proves registration does not fabricate
/// a receiver value or borrow storage with the wrong layout.
struct NonZeroSized([u8; 64]);

// SAFETY: the implementation delegates to the deterministic scheduler,
// which invokes every index exactly once and blocks until completion.
unsafe impl ParallelExecutor for NonZeroSized {
    unsafe fn run_indexed(num_tasks: usize, task: unsafe fn(usize, *mut ()), context: *mut ()) {
        let marker = Self([0; 64]);
        let _ = marker.0[0];
        // SAFETY: the delegated implementation receives the same valid
        // task and context and discharges the executor contract.
        unsafe { <Deterministic as ParallelExecutor>::run_indexed(num_tasks, task, context) };
    }
}

/// Four threads concurrently fill disjoint partitions with global indices;
/// the joined region equals the identity mapping.
#[test]
fn concurrent_disjoint_writes_fill_region() {
    let _guard = ExecutorTestGuard::acquire();
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
    let _guard = ExecutorTestGuard::acquire();
    const N: usize = 1_000;
    brand_scope(|_token| {
        let mut cells: Vec<MelinoeCell<'_, u64>> = (0..N).map(|_| MelinoeCell::new(0)).collect();

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
    register_parallel_executor::<Deterministic>();

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
fn non_zero_sized_executor_registers_without_receiver_storage() {
    const N: usize = 8;
    let _guard = ExecutorTestGuard::acquire();
    EXECUTED_TASKS.store(0, Ordering::SeqCst);
    register_parallel_executor::<NonZeroSized>();

    brand_scope(|token| {
        let mut cells: Vec<MelinoeCell<'_, usize>> = (0..N).map(MelinoeCell::new).collect();
        partition_for_each_with(
            &mut cells,
            PartitionPlan::chunk_size(2),
            |start, mut shard| {
                for (offset, value) in shard.iter_mut().enumerate() {
                    *value += start + offset;
                }
            },
        );
        let snapshot = token.share();
        let values: Vec<usize> = cells.iter().map(|cell| *cell.borrow(snapshot)).collect();
        assert_eq!(values, (0..N).map(|index| index * 2).collect::<Vec<_>>());
    });

    assert_eq!(EXECUTED_TASKS.load(Ordering::SeqCst), 4);
}

#[test]
fn clearing_registered_executor_restores_default_driver() {
    const N: usize = 8;
    let _guard = ExecutorTestGuard::acquire();
    EXECUTED_TASKS.store(0, Ordering::SeqCst);
    register_parallel_executor::<Deterministic>();
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
    let _guard = ExecutorTestGuard::acquire();
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
    let _guard = ExecutorTestGuard::acquire();
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
    let _guard = ExecutorTestGuard::acquire();
    const N: usize = 33;
    let fill = |v: usize| v.wrapping_mul(11).wrapping_add(5);

    let legacy = brand_scope(|token| {
        let mut cells: Vec<MelinoeCell<'_, usize>> = (0..N).map(|_| MelinoeCell::new(0)).collect();
        partition_for_each(&mut cells, 4, |start, mut shard| {
            for (j, slot) in shard.iter_mut().enumerate() {
                *slot = fill(start + j);
            }
        });
        let snap = token.share();
        cells.iter().map(|c| *c.borrow(snap)).collect::<Vec<_>>()
    });

    let planned = brand_scope(|token| {
        let mut cells: Vec<MelinoeCell<'_, usize>> = (0..N).map(|_| MelinoeCell::new(0)).collect();
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
    let _guard = ExecutorTestGuard::acquire();
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
    let _guard = ExecutorTestGuard::acquire();
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
    let _guard = ExecutorTestGuard::acquire();
    const N: usize = 64;
    brand_scope(|token| {
        let mut cells: Vec<MelinoeCell<'_, usize>> = (0..N).map(|_| MelinoeCell::new(0)).collect();

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
    let _guard = ExecutorTestGuard::acquire();
    const N: usize = 4_096;
    let fill = |v: usize| (v * 7 + 3) % 251;

    // Sequential reference.
    let sequential: Vec<usize> = (0..N).map(fill).collect();

    // Concurrent via shards.
    let concurrent = brand_scope(|token| {
        let mut cells: Vec<MelinoeCell<'_, usize>> = (0..N).map(|_| MelinoeCell::new(0)).collect();
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
    let _guard = ExecutorTestGuard::acquire();
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
    let _guard = ExecutorTestGuard::acquire();
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
    let _guard = ExecutorTestGuard::acquire();

    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);
    DROP_COUNT.store(0, Ordering::SeqCst);

    struct DropItem;
    impl Drop for DropItem {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    register_parallel_executor::<Deterministic>();

    let mut cells: Vec<MelinoeCell<'_, usize>> = (0..4).map(|_| MelinoeCell::new(0)).collect();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        partition_map_with(&mut cells, PartitionPlan::chunk_size(1), |index, _shard| {
            assert!(index != 2, "Task 2 failed");
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
    let _guard = ExecutorTestGuard::acquire();

    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);
    DROP_COUNT.store(0, Ordering::SeqCst);

    struct DropItem;
    impl Drop for DropItem {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    register_parallel_executor::<Deterministic>();

    let values: Vec<usize> = vec![0; 4];

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        melinoe::sync::partition_read_map_with(
            &values,
            PartitionPlan::chunk_size(1),
            |index, _shard| {
                assert!(index != 2, "Task 2 failed");
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
