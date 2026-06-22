//! Value-semantic contract tests for `halo::BrandedVec`.
//!
//! The harness checks Melinoe permit-gated access, zero-copy slice layout, and
//! conditional `Cow` borrow/retain behavior.
// Test item names are behavior specifications; public API documentation is
// enforced on the library crate.
#![allow(missing_docs)]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::cell::Cell;

use halo::BrandedVec;
#[cfg(feature = "std")]
use melinoe::sync::PartitionPlan;
use melinoe::{brand_scope, Borrowed, Retained};

#[derive(Debug, Eq, PartialEq)]
struct CountedClone<'a> {
    value: u32,
    clones: &'a Cell<usize>,
}

impl Clone for CountedClone<'_> {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            value: self.value,
            clones: self.clones,
        }
    }
}

#[test]
fn slice_access_is_value_semantic_and_zero_copy() {
    brand_scope(|mut token| {
        let mut values = BrandedVec::with_capacity(4);
        values.extend([10_u32, 20, 30, 40]);

        values.as_mut_slice(&mut token)[2] = 300;

        let snapshot = token.share();
        let slice = values.as_slice(snapshot);
        let cell_ptr = values.as_cells().as_ptr().cast::<u32>();

        assert_eq!(slice, &[10, 20, 300, 40]);
        assert_eq!(slice.as_ptr(), cell_ptr);
        assert_eq!(values.len(), 4);
        assert_eq!(values.capacity(), 4);
    });
}

#[test]
fn element_access_uses_melinoe_permits() {
    brand_scope(|mut token| {
        let values = BrandedVec::from_iter([1_i32, 2, 3]);

        {
            let mut middle = match values.get_mut(1, &mut token) {
                Some(value) => value,
                None => panic!("index 1 must exist in a three-element vector"),
            };
            *middle = 22;
        }

        let first = match values.get(0, &token) {
            Some(value) => *value,
            None => panic!("index 0 must exist in a three-element vector"),
        };

        assert_eq!(first, 1);
        assert_eq!(values.as_slice(&token), &[1, 22, 3]);
    });
}

#[test]
fn cow_policies_preserve_borrow_or_clone_contract() {
    brand_scope(|token| {
        let clones = Cell::new(0);
        let values = BrandedVec::from_iter([
            CountedClone {
                value: 1,
                clones: &clones,
            },
            CountedClone {
                value: 2,
                clones: &clones,
            },
        ]);

        let borrowed = values.cow_with(&token, Borrowed);
        let borrowed_ptr = match &borrowed {
            Cow::Borrowed(slice) => slice.as_ptr(),
            Cow::Owned(_) => panic!("Borrowed policy must not allocate"),
        };
        assert_eq!(borrowed_ptr, values.as_slice(&token).as_ptr());
        assert_eq!(clones.get(), 0);

        let retained = values.cow_with(&token, Retained);
        match &retained {
            Cow::Borrowed(_) => panic!("Retained policy must allocate owned storage"),
            Cow::Owned(owned) => {
                assert_eq!(
                    owned.iter().map(|item| item.value).collect::<Vec<_>>(),
                    [1, 2]
                );
                assert_ne!(owned.as_ptr(), values.as_slice(&token).as_ptr());
            }
        }
        assert_eq!(clones.get(), 2);
    });
}

#[test]
fn into_vec_returns_owned_values() {
    brand_scope(|_token| {
        let values = BrandedVec::from_iter([5_u8, 8, 13]);
        assert_eq!(values.into_vec(), [5, 8, 13]);
    });
}

#[test]
fn structural_vec_operations_preserve_owned_values() {
    brand_scope(|token| {
        let mut values = BrandedVec::from_iter([1_i32, 2, 3]);

        values.insert(1, 10);
        values.swap(0, 2);
        assert_eq!(values.as_slice(&token), &[2, 10, 1, 3]);

        let removed = values.remove(1);
        assert_eq!(removed, 10);
        assert_eq!(values.as_slice(&token), &[2, 1, 3]);

        let swapped = values.swap_remove(0);
        assert_eq!(swapped, 2);
        assert_eq!(values.as_slice(&token), &[3, 1]);

        values.resize_with(5, || 9);
        values.retain_mut(|value| {
            *value += 1;
            *value != 10
        });
        assert_eq!(values.as_slice(&token), &[4, 2]);

        assert_eq!(values.pop(), Some(2));
        values.truncate(0);
        assert_eq!(values.pop(), None);
        assert_eq!(values.as_slice(&token), &[]);
    });
}

#[cfg(feature = "std")]
#[test]
fn partition_for_each_mut_writes_disjoint_shards() {
    brand_scope(|token| {
        let mut values = BrandedVec::from_iter([0_usize; 8]);

        values.partition_for_each_mut_with(PartitionPlan::chunk_size(2), |start, shard| {
            for (offset, value) in shard.iter_mut().enumerate() {
                *value = start + offset;
            }
        });

        assert_eq!(values.as_slice(&token), &[0, 1, 2, 3, 4, 5, 6, 7]);
    });
}

#[cfg(feature = "std")]
#[test]
fn partition_map_mut_returns_partition_order_results() {
    brand_scope(|token| {
        let mut values = BrandedVec::from_iter([1_usize, 1, 1, 1, 1, 1]);

        let lengths =
            values.partition_map_mut_with(PartitionPlan::chunk_size(2), |start, shard| {
                for value in shard.iter_mut() {
                    *value += start;
                }
                shard.len()
            });

        assert_eq!(lengths, [2, 2, 2]);
        assert_eq!(values.as_slice(&token), &[1, 1, 3, 3, 5, 5]);
    });
}

#[cfg(feature = "std")]
#[test]
fn partition_map_reads_shared_shards_in_order() {
    brand_scope(|token| {
        let values = BrandedVec::from_iter(0_usize..10);

        let sums =
            values.partition_map_with(&token, PartitionPlan::chunk_size(4), |start, shard| {
                assert_eq!(start % 4, 0);
                shard.iter().sum::<usize>()
            });

        assert_eq!(sums, [6, 22, 17]);
        assert_eq!(values.as_slice(&token), &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    });
}

#[cfg(feature = "std")]
#[test]
fn partition_for_each_reads_all_shared_shards() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static VISITED: AtomicUsize = AtomicUsize::new(0);
    VISITED.store(0, Ordering::SeqCst);

    brand_scope(|token| {
        let values = BrandedVec::from_iter([1_usize, 2, 3, 4, 5, 6]);

        values.partition_for_each_with(&token, PartitionPlan::chunk_size(2), |_start, shard| {
            VISITED.fetch_add(shard.iter().sum::<usize>(), Ordering::SeqCst);
        });

        assert_eq!(VISITED.load(Ordering::SeqCst), 21);
        assert_eq!(values.as_slice(&token), &[1, 2, 3, 4, 5, 6]);
    });
}

#[test]
fn new_vec_apis_drain_split_off_append() {
    brand_scope(|token| {
        let mut values = BrandedVec::from_iter([1_i32, 2, 3, 4, 5]);

        // test drain
        {
            let drained: Vec<i32> = values.drain(1..4).collect();
            assert_eq!(drained, [2, 3, 4]);
            assert_eq!(values.as_slice(&token), &[1, 5]);
        }

        // test split_off
        let mut other = values.split_off(1);
        assert_eq!(values.as_slice(&token), &[1]);
        assert_eq!(other.as_slice(&token), &[5]);

        // test append
        values.append(&mut other);
        assert_eq!(values.as_slice(&token), &[1, 5]);
        assert_eq!(other.as_slice(&token), &[]);
    });
}

#[test]
fn test_zero_copy_conversions_preserve_buffer_pointer() {
    brand_scope(|token| {
        let mut original = Vec::with_capacity(10);
        original.extend([42_i32, 43, 44]);
        let original_ptr = original.as_ptr();

        let branded = BrandedVec::from(original);
        assert_eq!(branded.len(), 3);
        assert_eq!(branded.capacity(), 10);
        assert_eq!(branded.as_slice(&token), &[42, 43, 44]);

        let converted = branded.into_vec();
        assert_eq!(converted.len(), 3);
        assert_eq!(converted.capacity(), 10);
        assert_eq!(converted.as_ptr(), original_ptr);
        assert_eq!(converted, &[42, 43, 44]);
    });
}

#[test]
fn test_branded_vec_clone_with() {
    brand_scope(|token| {
        let values = BrandedVec::from_iter([10_i32, 20, 30]);
        let cloned = values.clone_with(&token);
        assert_eq!(cloned.len(), 3);
        assert_eq!(cloned.as_slice(&token), &[10, 20, 30]);
    });
}
