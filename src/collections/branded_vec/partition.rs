use alloc::vec::Vec;

use crate::ReadPermit;

use super::BrandedVec;

impl<'brand, T> BrandedVec<'brand, T> {
    /// Split the vector into disjoint writer shards and mutate them
    /// concurrently according to `plan`.
    ///
    /// This method delegates to Melinoe's partition driver. No token is needed:
    /// `&mut self` already proves unique ownership of the vector storage, and
    /// Melinoe's [`WriterShard`](crate::WriterShard) proves every worker sees
    /// a non-overlapping subslice.
    #[inline]
    pub fn partition_for_each_mut_with<F>(&mut self, plan: crate::sync::PartitionPlan, f: F)
    where
        T: Send,
        F: Fn(usize, &mut [T]) + Sync,
    {
        crate::sync::partition_for_each_with(&mut self.cells, plan, |start, mut shard| {
            f(start, shard.as_mut_slice());
        });
    }

    /// Split the vector into disjoint writer shards and return one result per
    /// shard in partition order.
    #[inline]
    pub fn partition_map_mut_with<R, F>(&mut self, plan: crate::sync::PartitionPlan, f: F) -> Vec<R>
    where
        T: Send,
        R: Send,
        F: Fn(usize, &mut [T]) -> R + Sync,
    {
        crate::sync::partition_map_with(&mut self.cells, plan, |start, mut shard| {
            f(start, shard.as_mut_slice())
        })
    }

    /// Split a permit-gated shared slice into disjoint read shards and run `f`
    /// on each shard concurrently.
    ///
    /// This is the read-side counterpart to
    /// [`partition_map_mut_with`](Self::partition_map_mut_with): Melinoe proves
    /// the whole slice view is read-only through `permit`, while
    /// [`PartitionPlan`](crate::sync::PartitionPlan) controls chunking.
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
        crate::sync::partition_read_map_with(self.as_slice(permit), plan, f)
    }

    /// Split a permit-gated shared slice into disjoint read shards and run `f`
    /// on each shard concurrently, discarding results.
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
        crate::sync::partition_read_for_each_with(self.as_slice(permit), plan, f);
    }
}
