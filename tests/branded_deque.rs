//! Value-semantic contract tests for `melinoe::collections::BrandedVecDeque`.
//!
//! The harness checks Melinoe permit-gated access, zero-copy split slice layout,
//! and double-ended queue properties.
#![allow(missing_docs)]

extern crate alloc;

use alloc::collections::VecDeque;
use melinoe::brand_scope;
use melinoe::collections::BrandedVecDeque;

#[cfg(feature = "std")]
fn wrapped_three_three_queue() -> VecDeque<usize> {
    let mut values = VecDeque::with_capacity(8);
    values.extend(0_usize..8);
    for _ in 0..5 {
        values.pop_front();
    }
    values.extend(8_usize..11);
    values
}

#[test]
fn double_ended_operations_preserve_logical_ordering() {
    brand_scope(|token| {
        let mut deque = BrandedVecDeque::new();
        assert!(deque.is_empty());
        assert_eq!(deque.len(), 0);

        deque.push_back(10_i32);
        deque.push_front(20);
        deque.push_back(30);
        deque.push_front(40);

        // Logical layout should be: [40, 20, 10, 30]
        assert_eq!(deque.len(), 4);
        assert!(!deque.is_empty());

        let (s1, s2) = deque.as_slices(&token);
        let mut combined = VecDeque::new();
        combined.extend(s1.iter().copied());
        combined.extend(s2.iter().copied());
        assert_eq!(combined, &[40, 20, 10, 30]);

        assert_eq!(deque.pop_front(), Some(40));
        assert_eq!(deque.pop_back(), Some(30));
        assert_eq!(deque.len(), 2);

        assert_eq!(deque.pop_front(), Some(20));
        assert_eq!(deque.pop_back(), Some(10));
        assert_eq!(deque.pop_front(), None);
        assert_eq!(deque.pop_back(), None);
    });
}

#[test]
fn element_access_uses_melinoe_permits() {
    brand_scope(|mut token| {
        let mut deque = BrandedVecDeque::new();
        deque.push_back(1_u8);
        deque.push_back(2);
        deque.push_back(3);

        {
            let Some(mut middle) = deque.get_mut(1, &mut token) else {
                panic!("index 1 must exist")
            };
            *middle = 20;
        }

        let first = match deque.get(0, &token) {
            Some(val) => *val,
            None => panic!("index 0 must exist"),
        };

        assert_eq!(first, 1);
        let (s1, s2) = deque.as_slices(&token);
        assert_eq!(s1, &[1, 20, 3]);
        assert_eq!(s2, &[]);
    });
}

#[test]
fn split_slice_access_supports_mutations_under_write_permit() {
    brand_scope(|mut token| {
        let mut deque = BrandedVecDeque::with_capacity(8);
        // Force the ring buffer to wrap around by pushing/popping
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
        // In VecDeque, this typically splits into two contiguous slice segments.
        assert_eq!(deque.len(), 6);

        {
            let (s1, s2) = deque.as_mut_slices(&mut token);
            for val in s1.iter_mut() {
                *val += 100;
            }
            for val in s2.iter_mut() {
                *val += 100;
            }
        }

        let (s1, s2) = deque.as_slices(&token);
        let mut result = alloc::vec::Vec::new();
        result.extend(s1.iter().copied());
        result.extend(s2.iter().copied());
        assert_eq!(result, &[103, 104, 105, 106, 107, 108]);
    });
}

#[test]
fn structural_vecdeque_operations() {
    brand_scope(|token| {
        let mut deque = [100_i32, 200, 300]
            .into_iter()
            .collect::<BrandedVecDeque<_>>();
        assert_eq!(deque.len(), 3);
        assert!(deque.capacity() >= 3);

        deque.swap(0, 2);
        let (s1, _) = deque.as_slices(&token);
        assert_eq!(s1[0], 300);
        assert_eq!(s1[2], 100);

        deque.reserve(10);
        assert!(deque.capacity() >= 13);

        deque.truncate(2);
        assert_eq!(deque.len(), 2);

        deque.clear();
        assert_eq!(deque.len(), 0);
        assert!(deque.is_empty());
    });
}

#[test]
fn new_deque_apis_drain_split_off_append_retain_mut_extend() {
    brand_scope(|token| {
        let mut deque = [1_i32, 2, 3, 4, 5]
            .into_iter()
            .collect::<BrandedVecDeque<_>>();

        // test drain
        {
            let drained: alloc::vec::Vec<i32> = deque.drain(1..4).collect();
            assert_eq!(drained, [2, 3, 4]);
            let (s1, s2) = deque.as_slices(&token);
            assert_eq!(s1, &[1, 5]);
            assert_eq!(s2, &[]);
        }

        // test split_off
        let mut other = deque.split_off(1);
        let (s1, _) = deque.as_slices(&token);
        assert_eq!(s1, &[1]);
        let (o1, _) = other.as_slices(&token);
        assert_eq!(o1, &[5]);

        // test append
        deque.append(&mut other);
        let (s1, _) = deque.as_slices(&token);
        assert_eq!(s1, &[1, 5]);
        let (o1, _) = other.as_slices(&token);
        assert_eq!(o1, &[]);

        // test retain_mut
        deque.retain_mut(|x| {
            *x += 1;
            *x != 6
        });
        let (s1, _) = deque.as_slices(&token);
        assert_eq!(s1, &[2]);

        // test extend
        deque.extend([10, 20]);
        let (s1, _) = deque.as_slices(&token);
        assert_eq!(s1, &[2, 10, 20]);
    });
}

#[test]
fn test_branded_deque_into_iterator() {
    let mut deque = BrandedVecDeque::new();
    deque.push_back(100_i32);
    deque.push_back(200);
    deque.push_front(50);

    let collected: alloc::vec::Vec<i32> = deque.into_iter().collect();
    assert_eq!(collected, &[50, 100, 200]);
}

#[test]
fn test_zero_copy_vec_deque_conversions() {
    let mut original = VecDeque::with_capacity(10);
    original.push_back(42_i32);
    original.push_back(43);
    original.push_front(41);

    let cap_before = original.capacity();

    let branded = BrandedVecDeque::from(original);
    assert_eq!(branded.len(), 3);
    assert_eq!(branded.capacity(), cap_before);

    let converted = branded.into_vec_deque();
    assert_eq!(converted.len(), 3);
    assert_eq!(converted.capacity(), cap_before);
    assert_eq!(
        converted.into_iter().collect::<alloc::vec::Vec<_>>(),
        &[41, 42, 43]
    );
}

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

#[test]
fn test_branded_deque_cow_boundary_helpers() {
    use alloc::borrow::Cow;
    use melinoe::RetainDecision;

    brand_scope(|token| {
        // 1. Contiguous case (no wrap-around)
        let mut original = VecDeque::new();
        original.extend([1_i32, 2, 3]);
        let deque = BrandedVecDeque::from(original);

        let cow = deque.borrow_cow(&token);
        assert!(matches!(cow, Cow::Borrowed(_)));
        assert_eq!(&*cow, &[1, 2, 3]);

        let retained = deque.retain_cow(&token);
        assert!(matches!(retained, Cow::Owned(_)));
        assert_eq!(&*retained, &[1, 2, 3]);

        let cow_if_borrow = deque.cow_if(&token, RetainDecision::Borrow);
        assert!(matches!(cow_if_borrow, Cow::Borrowed(_)));

        let cow_if_retain = deque.cow_if(&token, RetainDecision::Retain);
        assert!(matches!(cow_if_retain, Cow::Owned(_)));

        // 2. Wrapped case (force wrap-around)
        let mut original_wrapped = VecDeque::with_capacity(8);
        for i in 0..6 {
            original_wrapped.push_back(i);
        }
        for _ in 0..3 {
            original_wrapped.pop_front();
        }
        for i in 6..9 {
            original_wrapped.push_back(i);
        }
        // Logical queue: [3, 4, 5, 6, 7, 8]
        let deque_wrapped = BrandedVecDeque::from(original_wrapped);

        let cow_wrapped = deque_wrapped.borrow_cow(&token);
        // Wrapped deques must allocate and return Cow::Owned
        assert!(matches!(cow_wrapped, Cow::Owned(_)));
        assert_eq!(&*cow_wrapped, &[3, 4, 5, 6, 7, 8]);
    });
}

#[test]
fn contiguous_cow_borrows_preserve_pointer_identity_and_retained_cow_owns() {
    use alloc::borrow::Cow;

    brand_scope(|token| {
        let deque = [11_i32, 13, 17, 19]
            .into_iter()
            .collect::<BrandedVecDeque<_>>();
        let (front, back) = deque.as_slices(&token);
        assert_eq!(front, &[11, 13, 17, 19]);
        assert_eq!(back, &[]);

        let borrowed = deque.borrow_cow(&token);
        match borrowed {
            Cow::Borrowed(values) => {
                assert_eq!(values, &[11, 13, 17, 19]);
                assert_eq!(values.as_ptr(), front.as_ptr());
            }
            Cow::Owned(values) => panic!("contiguous borrow_cow must borrow, got {values:?}"),
        }

        let retained = deque.retain_cow(&token);
        match retained {
            Cow::Owned(values) => {
                assert_eq!(values, &[11, 13, 17, 19]);
                assert_ne!(values.as_ptr(), front.as_ptr());
            }
            Cow::Borrowed(values) => panic!("retain_cow must own, got {values:?}"),
        }
    });
}

#[test]
fn test_branded_deque_cow_with() {
    use alloc::borrow::Cow;
    use melinoe::{Borrowed, Retained};

    brand_scope(|token| {
        // 1. Contiguous case
        let mut original = VecDeque::new();
        original.extend([10_i32, 20, 30]);
        let deque = BrandedVecDeque::from(original);

        let cow_borrowed = deque.cow_with(&token, Borrowed);
        assert!(matches!(cow_borrowed, Cow::Borrowed(_)));
        assert_eq!(&*cow_borrowed, &[10, 20, 30]);

        let cow_retained = deque.cow_with(&token, Retained);
        assert!(matches!(cow_retained, Cow::Owned(_)));
        assert_eq!(&*cow_retained, &[10, 20, 30]);

        // 2. Wrapped case
        let mut original_wrapped = VecDeque::with_capacity(8);
        for i in 0..6 {
            original_wrapped.push_back(i);
        }
        for _ in 0..3 {
            original_wrapped.pop_front();
        }
        for i in 6..9 {
            original_wrapped.push_back(i);
        }
        let deque_wrapped = BrandedVecDeque::from(original_wrapped);

        let cow_borrowed_wrapped = deque_wrapped.cow_with(&token, Borrowed);
        // Wrapped must allocate and return Cow::Owned
        assert!(matches!(cow_borrowed_wrapped, Cow::Owned(_)));
        assert_eq!(&*cow_borrowed_wrapped, &[3, 4, 5, 6, 7, 8]);
    });
}

#[test]
fn test_branded_deque_clone_with() {
    brand_scope(|token| {
        let mut original = VecDeque::new();
        original.extend([100_i32, 200, 300]);
        let deque = BrandedVecDeque::from(original);

        let cloned = deque.clone_with(&token);
        assert_eq!(cloned.len(), 3);
        let (s1, s2) = cloned.as_slices(&token);
        assert_eq!(s1, &[100, 200, 300]);
        assert_eq!(s2, &[]);
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
