#[cfg(feature = "std")]
use alloc::vec::Vec;

#[cfg(feature = "std")]
use crate::MelinoeCell;
#[cfg(feature = "std")]
use crate::{ReadPermit, WriterShard};

use super::BrandedVecDeque;

#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DequeSegment {
    Front,
    Back,
}

#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DequeShard {
    start: usize,
    segment: DequeSegment,
    offset: usize,
    len: usize,
}

#[cfg(feature = "std")]
struct DequeShardPlan {
    shards: Vec<DequeShard>,
}

#[cfg(feature = "std")]
impl DequeShardPlan {
    fn from_lengths(front_len: usize, back_len: usize, plan: crate::sync::PartitionPlan) -> Self {
        let total_len = front_len + back_len;
        let chunk_len = plan.chunk_len_for(total_len).max(1);
        let num_chunks = (total_len + chunk_len - 1) / chunk_len;
        let mut shards = Vec::with_capacity(num_chunks * 2);
        let mut start = 0;

        while start < total_len {
            let end = (start + chunk_len).min(total_len);
            if start < front_len {
                let front_end = end.min(front_len);
                shards.push(DequeShard {
                    start,
                    segment: DequeSegment::Front,
                    offset: start,
                    len: front_end - start,
                });
            }
            if end > front_len {
                let back_start = start.max(front_len) - front_len;
                shards.push(DequeShard {
                    start: front_len + back_start,
                    segment: DequeSegment::Back,
                    offset: back_start,
                    len: end - start.max(front_len),
                });
            }
            start = end;
        }

        Self { shards }
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.shards.is_empty()
    }

    fn map_read<T, R, F>(&self, front: &[T], back: &[T], f: F) -> Vec<R>
    where
        T: Sync,
        R: Send,
        F: Fn(usize, &[T]) -> R + Sync,
    {
        if self.is_empty() {
            return Vec::new();
        }

        crate::sync::partition_read_map_with(
            &self.shards,
            crate::sync::PartitionPlan::chunk_size(1),
            |_, shard| {
                debug_assert_eq!(shard.len(), 1);
                let shard = shard[0];
                f(shard.start, shard.read_slice(front, back))
            },
        )
    }

    fn for_each_read<T, F>(&self, front: &[T], back: &[T], f: F)
    where
        T: Sync,
        F: Fn(usize, &[T]) + Sync,
    {
        self.map_read(front, back, |start, shard| f(start, shard));
    }

    fn map_mut<'brand, T, R, F>(
        &self,
        front: &mut [MelinoeCell<'brand, T>],
        back: &mut [MelinoeCell<'brand, T>],
        f: F,
    ) -> Vec<R>
    where
        T: Send,
        R: Send,
        F: Fn(usize, &mut [T]) -> R + Sync,
    {
        if self.is_empty() {
            return Vec::new();
        }

        let slices = DequeMutSlices::new(front, back);
        crate::sync::partition_read_map_with(
            &self.shards,
            crate::sync::PartitionPlan::chunk_size(1),
            |_, shard| {
                debug_assert_eq!(shard.len(), 1);
                // SAFETY: this driver partitions `self.shards` with chunk size 1,
                // so each descriptor is requested once. `from_lengths` emits
                // non-overlapping descriptors in ascending logical order, and
                // `DequeMutSlices` points at the two disjoint VecDeque storage
                // slices borrowed for this call.
                unsafe { slices.with_shard_mut(shard[0], |start, values| f(start, values)) }
            },
        )
    }

    fn for_each_mut<'brand, T, F>(
        &self,
        front: &mut [MelinoeCell<'brand, T>],
        back: &mut [MelinoeCell<'brand, T>],
        f: F,
    ) where
        T: Send,
        F: Fn(usize, &mut [T]) + Sync,
    {
        self.map_mut(front, back, |start, shard| f(start, shard));
    }
}

#[cfg(feature = "std")]
impl DequeShard {
    #[inline]
    fn read_slice<'a, T>(self, front: &'a [T], back: &'a [T]) -> &'a [T] {
        match self.segment {
            DequeSegment::Front => &front[self.offset..self.offset + self.len],
            DequeSegment::Back => &back[self.offset..self.offset + self.len],
        }
    }
}

#[cfg(feature = "std")]
struct DequeMutSlices<'a, 'brand, T> {
    front: *mut MelinoeCell<'brand, T>,
    front_len: usize,
    back: *mut MelinoeCell<'brand, T>,
    back_len: usize,
    _borrow: core::marker::PhantomData<&'a mut [MelinoeCell<'brand, T>]>,
}

#[cfg(feature = "std")]
impl<'a, 'brand, T> DequeMutSlices<'a, 'brand, T> {
    fn new(
        front: &'a mut [MelinoeCell<'brand, T>],
        back: &'a mut [MelinoeCell<'brand, T>],
    ) -> Self {
        Self {
            front: front.as_mut_ptr(),
            front_len: front.len(),
            back: back.as_mut_ptr(),
            back_len: back.len(),
            _borrow: core::marker::PhantomData,
        }
    }

    unsafe fn with_shard_mut<R>(
        &self,
        shard: DequeShard,
        f: impl FnOnce(usize, &mut [T]) -> R,
    ) -> R {
        let (base, segment_len) = match shard.segment {
            DequeSegment::Front => (self.front, self.front_len),
            DequeSegment::Back => (self.back, self.back_len),
        };
        debug_assert!(shard.offset <= segment_len);
        debug_assert!(shard.len <= segment_len - shard.offset);

        // SAFETY: the caller guarantees each descriptor is used at most once and
        // descriptors do not overlap. Bounds are established by `from_lengths`
        // and checked above in debug builds before forming the sub-slice.
        let cells = unsafe { core::slice::from_raw_parts_mut(base.add(shard.offset), shard.len) };
        let mut writer = WriterShard::new(cells);
        f(shard.start, writer.as_mut_slice())
    }
}

#[cfg(feature = "std")]
// SAFETY: `DequeMutSlices` exposes mutation only through disjoint shard
// descriptors generated by `DequeShardPlan`; `T: Send` matches Melinoe's
// `WriterShard` cross-thread write requirement.
unsafe impl<'a, 'brand, T: Send> Sync for DequeMutSlices<'a, 'brand, T> {}

impl<'brand, T> BrandedVecDeque<'brand, T> {
    /// Split a permit-gated shared queue into disjoint read shards and run `f`
    /// on each shard concurrently, returning per-shard results.
    ///
    /// The shard plan is derived once from the queue's total logical length.
    /// When a planned logical shard crosses the ring-buffer wrap, it is exposed
    /// as two physical subshards with stable logical start offsets.
    #[cfg(feature = "std")]
    #[inline]
    pub fn partition_map_with<'a, P, R, F>(
        &'a self,
        permit: P,
        plan: crate::sync::PartitionPlan,
        f: F,
    ) -> Vec<R>
    where
        P: ReadPermit<'brand> + 'a,
        T: Sync,
        R: Send,
        F: Fn(usize, &[T]) -> R + Sync,
    {
        let (s1, s2) = self.as_slices(permit);
        DequeShardPlan::from_lengths(s1.len(), s2.len(), plan).map_read(s1, s2, f)
    }

    /// Split a permit-gated shared queue into disjoint read shards and run `f`
    /// on each shard concurrently, discarding results.
    #[cfg(feature = "std")]
    #[inline]
    pub fn partition_for_each_with<'a, P, F>(
        &'a self,
        permit: P,
        plan: crate::sync::PartitionPlan,
        f: F,
    ) where
        P: ReadPermit<'brand> + 'a,
        T: Sync,
        F: Fn(usize, &[T]) + Sync,
    {
        let (s1, s2) = self.as_slices(permit);
        DequeShardPlan::from_lengths(s1.len(), s2.len(), plan).for_each_read(s1, s2, f);
    }

    /// Split the queue into disjoint writer shards and mutate them
    /// concurrently according to `plan`.
    ///
    /// The shard plan is derived once from the queue's total logical length.
    /// When a planned logical shard crosses the ring-buffer wrap, it is exposed
    /// as two physical subshards with stable logical start offsets.
    #[cfg(feature = "std")]
    #[inline]
    pub fn partition_for_each_mut_with<F>(&mut self, plan: crate::sync::PartitionPlan, f: F)
    where
        T: Send,
        F: Fn(usize, &mut [T]) + Sync,
    {
        let (s1, s2) = self.cells.as_mut_slices();
        DequeShardPlan::from_lengths(s1.len(), s2.len(), plan).for_each_mut(s1, s2, f);
    }

    /// Split the queue into disjoint writer shards and return one result per
    /// shard in partition order.
    #[cfg(feature = "std")]
    #[inline]
    pub fn partition_map_mut_with<R, F>(&mut self, plan: crate::sync::PartitionPlan, f: F) -> Vec<R>
    where
        T: Send,
        R: Send,
        F: Fn(usize, &mut [T]) -> R + Sync,
    {
        let (s1, s2) = self.cells.as_mut_slices();
        DequeShardPlan::from_lengths(s1.len(), s2.len(), plan).map_mut(s1, s2, f)
    }
}
