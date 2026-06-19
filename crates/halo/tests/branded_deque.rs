//! Value-semantic contract tests for `halo::BrandedVecDeque`.
//!
//! The harness checks Melinoe permit-gated access, zero-copy split slice layout,
//! and double-ended queue properties.
#![allow(missing_docs)]

extern crate alloc;

use alloc::collections::VecDeque;
use halo::BrandedVecDeque;
use melinoe::brand_scope;

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
            let mut middle = match deque.get_mut(1, &mut token) {
                Some(val) => val,
                None => panic!("index 1 must exist"),
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
        let mut deque = BrandedVecDeque::from_iter([100_i32, 200, 300]);
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
