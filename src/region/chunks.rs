use crate::cell::MelinoeCell;

use super::WriterShard;

/// Iterator over disjoint [`WriterShard`]s produced by [`WriterShard::chunks`].
///
/// Each yielded shard owns a non-overlapping sub-slice carved off the front of
/// the remaining region with [`slice::split_at_mut`], so all yielded shards are
/// mutually disjoint and may be written in parallel.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ShardChunks<'a, 'brand, T> {
    pub(super) rest: Option<&'a mut [MelinoeCell<'brand, T>]>,
    pub(super) chunk: usize,
}

impl<'a, 'brand, T> Iterator for ShardChunks<'a, 'brand, T> {
    type Item = WriterShard<'a, 'brand, T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let slice = self.rest.take()?;
        if slice.is_empty() {
            return None;
        }
        let mid = self.chunk.min(slice.len());
        let (head, tail) = slice.split_at_mut(mid);
        self.rest = Some(tail);
        Some(WriterShard::new(head))
    }

    /// Exact remaining shard count: `ceil(remaining / chunk)`, the single source
    /// of truth the partition driver reserves worker capacity from.
    ///
    /// `chunk` is `>= 1` (clamped in [`WriterShard::chunks`]), so the division is
    /// total. Written as `1 + (rem - 1) / chunk` to compute the ceiling without
    /// the `rem + chunk - 1` form, which can overflow for adversarial `rem`.
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.rest.as_ref().map_or(0, |s| s.len());
        let shards = if remaining == 0 {
            0
        } else {
            1 + (remaining - 1) / self.chunk
        };
        (shards, Some(shards))
    }
}

// `size_hint` returns an exact, consistent bound (the remaining shard count is
// fully determined by the unconsumed length and the fixed `chunk`), so the
// iterator is exact-size; the default `len` derives from `size_hint`.
impl<'a, 'brand, T> ExactSizeIterator for ShardChunks<'a, 'brand, T> {}

impl<'a, 'brand, T> core::fmt::Debug for ShardChunks<'a, 'brand, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let remaining = self.rest.as_ref().map_or(0, |s| s.len());
        f.debug_struct("ShardChunks")
            .field("remaining", &remaining)
            .field("chunk", &self.chunk)
            .finish()
    }
}
