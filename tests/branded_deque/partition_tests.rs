//! Std-gated partitioned-access tests for `BrandedVecDeque`.

use super::*;

#[cfg(feature = "std")]
#[test]
fn partition_map_reads_wrapped_deque_correctly() {
    use melinoe::sync::PartitionPlan;
    brand_scope(|token| {
        let mut deque = BrandedVecDeque::with_capacity(8);
        for i in 0..6 {
            deque.push_back(i);
        }
        for _ in 0..3 {
            deque.pop_front();
        }
        for i in 6..9 {
            deque.push_back(i);
        }
        // Logical queue should have values: [3, 4, 5, 6, 7, 8]
        assert_eq!(deque.len(), 6);

        let sums =
            deque.partition_map_with(&token, PartitionPlan::chunk_size(2), |start, shard| {
                (start, shard.iter().sum::<usize>())
            });

        // The first slice of len 5 is partitioned into: chunk 1 (offset 0, len 2, sum 3+4=7), chunk 2 (offset 2, len 2, sum 5+6=11), chunk 3 (offset 4, len 1, sum 7).
        // The second slice of len 1 is partitioned into: chunk 1 (offset 5, len 1, sum 8).
        assert_eq!(sums, vec![(0, 7), (2, 11), (4, 7), (5, 8)]);
    });
}

#[cfg(feature = "std")]
#[test]
fn partition_for_each_reads_all_shared_shards() {
    use melinoe::sync::PartitionPlan;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static VISITED: AtomicUsize = AtomicUsize::new(0);
    VISITED.store(0, Ordering::SeqCst);

    brand_scope(|token| {
        let mut original = VecDeque::new();
        original.extend([1_usize, 2, 3, 4, 5, 6]);
        let deque = BrandedVecDeque::from(original);

        deque.partition_for_each_with(&token, PartitionPlan::chunk_size(2), |_start, shard| {
            VISITED.fetch_add(shard.iter().sum::<usize>(), Ordering::SeqCst);
        });

        assert_eq!(VISITED.load(Ordering::SeqCst), 21);
    });
}

#[cfg(feature = "std")]
#[test]
fn partition_map_contiguous_deque_correctness() {
    use melinoe::sync::PartitionPlan;
    brand_scope(|token| {
        let mut original = VecDeque::new();
        original.extend([10_usize, 20, 30, 40]);
        let deque = BrandedVecDeque::from(original);

        // Deque is contiguous, s2 should be empty and skipped.
        let sums =
            deque.partition_map_with(&token, PartitionPlan::chunk_size(2), |start, shard| {
                (start, shard.iter().sum::<usize>())
            });

        assert_eq!(sums, vec![(0, 30), (2, 70)]);
    });
}

#[cfg(feature = "std")]
#[test]
fn partition_map_uses_one_logical_plan_for_contiguous_deque() {
    use melinoe::sync::PartitionPlan;

    brand_scope(|token| {
        let deque = (0_usize..6).collect::<BrandedVecDeque<_>>();
        let (front, back) = deque.as_slices(&token);
        assert_eq!(front, &[0, 1, 2, 3, 4, 5]);
        assert_eq!(back, &[]);

        let shards = deque.partition_map_with(&token, PartitionPlan::parts(2), |start, shard| {
            (start, shard.to_vec())
        });

        assert_eq!(shards, vec![(0, vec![0, 1, 2]), (3, vec![3, 4, 5])]);
    });
}

#[cfg(feature = "std")]
#[test]
fn partition_map_uses_one_logical_plan_for_wrapped_deque() {
    use melinoe::sync::PartitionPlan;

    brand_scope(|token| {
        let deque = BrandedVecDeque::from(wrapped_three_three_queue());
        let (front, back) = deque.as_slices(&token);
        assert_eq!(front, &[5, 6, 7]);
        assert_eq!(back, &[8, 9, 10]);

        let shards = deque.partition_map_with(&token, PartitionPlan::parts(2), |start, shard| {
            (start, shard.to_vec())
        });

        assert_eq!(shards, vec![(0, vec![5, 6, 7]), (3, vec![8, 9, 10])]);
    });
}

#[cfg(feature = "std")]
#[test]
fn partition_for_each_mut_contiguous_deque_correctness() {
    use melinoe::sync::PartitionPlan;
    brand_scope(|token| {
        let mut original = VecDeque::new();
        original.extend([0_usize; 8]);
        let mut deque = BrandedVecDeque::from(original);

        deque.partition_for_each_mut_with(PartitionPlan::chunk_size(2), |start, shard| {
            for (offset, value) in shard.iter_mut().enumerate() {
                *value = start + offset;
            }
        });

        let (s1, s2) = deque.as_slices(&token);
        assert_eq!(s1, &[0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(s2, &[]);
    });
}

#[cfg(feature = "std")]
#[test]
fn partition_for_each_mut_wrapped_deque_correctness() {
    use melinoe::sync::PartitionPlan;
    brand_scope(|token| {
        let mut original = VecDeque::with_capacity(8);
        for _ in 0..6 {
            original.push_back(0);
        }
        for _ in 0..3 {
            original.pop_front();
        }
        for _ in 6..9 {
            original.push_back(0);
        }
        // Logical queue has length 6 (all zeros)
        let mut deque = BrandedVecDeque::from(original);

        deque.partition_for_each_mut_with(PartitionPlan::chunk_size(2), |start, shard| {
            for (offset, value) in shard.iter_mut().enumerate() {
                *value = start + offset;
            }
        });

        let (s1, s2) = deque.as_slices(&token);
        let mut result = alloc::vec::Vec::new();
        result.extend(s1.iter().copied());
        result.extend(s2.iter().copied());
        assert_eq!(result, &[0, 1, 2, 3, 4, 5]);
    });
}

#[cfg(feature = "std")]
#[test]
fn partition_map_mut_contiguous_deque_correctness() {
    use melinoe::sync::PartitionPlan;
    brand_scope(|token| {
        let mut original = VecDeque::new();
        original.extend([1_usize; 6]);
        let mut deque = BrandedVecDeque::from(original);

        let lengths = deque.partition_map_mut_with(PartitionPlan::chunk_size(2), |start, shard| {
            for value in shard.iter_mut() {
                *value += start;
            }
            shard.len()
        });

        assert_eq!(lengths, [2, 2, 2]);
        let (s1, s2) = deque.as_slices(&token);
        assert_eq!(s1, &[1, 1, 3, 3, 5, 5]);
        assert_eq!(s2, &[]);
    });
}

#[cfg(feature = "std")]
#[test]
fn partition_map_mut_wrapped_deque_correctness() {
    use melinoe::sync::PartitionPlan;
    brand_scope(|token| {
        let mut original = VecDeque::with_capacity(8);
        for _ in 0..6 {
            original.push_back(1);
        }
        for _ in 0..3 {
            original.pop_front();
        }
        for _ in 6..9 {
            original.push_back(1);
        }
        let mut deque = BrandedVecDeque::from(original);

        let lengths = deque.partition_map_mut_with(PartitionPlan::chunk_size(2), |start, shard| {
            for value in shard.iter_mut() {
                *value += start;
            }
            shard.len()
        });

        assert_eq!(lengths, [2, 2, 1, 1]);
        let (s1, s2) = deque.as_slices(&token);
        assert_eq!(s1, &[1, 1, 3, 3, 5]);
        assert_eq!(s2, &[6]);
    });
}

#[cfg(feature = "std")]
#[test]
fn partition_map_mut_uses_one_logical_plan_for_contiguous_deque() {
    use melinoe::sync::PartitionPlan;

    brand_scope(|token| {
        let mut deque = [1_usize; 6].into_iter().collect::<BrandedVecDeque<_>>();

        let lengths = deque.partition_map_mut_with(PartitionPlan::parts(2), |start, shard| {
            for (offset, value) in shard.iter_mut().enumerate() {
                *value = start + offset;
            }
            shard.len()
        });

        assert_eq!(lengths, [3, 3]);
        let (front, back) = deque.as_slices(&token);
        assert_eq!(front, &[0, 1, 2, 3, 4, 5]);
        assert_eq!(back, &[]);
    });
}

#[cfg(feature = "std")]
#[test]
fn partition_map_mut_uses_one_logical_plan_for_wrapped_deque() {
    use melinoe::sync::PartitionPlan;

    brand_scope(|token| {
        let mut deque = BrandedVecDeque::from(wrapped_three_three_queue());

        let lengths = deque.partition_map_mut_with(PartitionPlan::parts(2), |start, shard| {
            for (offset, value) in shard.iter_mut().enumerate() {
                *value = start + offset;
            }
            shard.len()
        });

        assert_eq!(lengths, [3, 3]);
        let (front, back) = deque.as_slices(&token);
        assert_eq!(front, &[0, 1, 2]);
        assert_eq!(back, &[3, 4, 5]);
    });
}
