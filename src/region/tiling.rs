//! Partition arithmetic for fixed-size shards tiling a region: how many
//! shards there are and where each one starts and ends.

/// Number of `chunk`-sized partitions tiling `len` items: `ceil(len / chunk)`,
/// or `0` when `len == 0`.
///
/// `chunk` must be `>= 1`; `chunk == 0` divides by zero. Written as
/// `1 + (len - 1) / chunk` to compute the ceiling without the
/// `len + chunk - 1` form, which can overflow for adversarial `len`. This is
/// the single source of truth for the count shared by `ParChunks::len`,
/// `ShardChunks::size_hint`, and the scoped partition drivers, so they can
/// never disagree on how many shards tile a region.
#[inline]
pub(crate) const fn partition_count(len: usize, chunk: usize) -> usize {
    if len == 0 {
        0
    } else {
        1 + (len - 1) / chunk
    }
}

/// Bounds `[start, end)` of partition `index` of a `len`-item region cut into
/// `chunk`-sized partitions, clamped to `len` for every `index`.
///
/// `chunk` must be `>= 1`; with `chunk == 0` every range is empty. Indices at
/// or past [`partition_count`] yield the empty range `(len, len)`. Saturation
/// before clamping is necessary for zero-sized elements: their slices may have
/// lengths near `usize::MAX` without allocating storage.
#[inline]
pub(crate) const fn partition_range(len: usize, chunk: usize, index: usize) -> (usize, usize) {
    let start = index.saturating_mul(chunk);
    let end = start.saturating_add(chunk);
    let start = if start < len { start } else { len };
    let end = if end < len { end } else { len };
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::{partition_count, partition_range};

    #[test]
    fn count_is_the_ceiling_without_overflow() {
        assert_eq!(partition_count(0, 4), 0);
        assert_eq!(partition_count(7, 4), 2);
        assert_eq!(partition_count(8, 4), 2);
        assert_eq!(partition_count(usize::MAX, usize::MAX / 2 + 1), 2);
    }

    #[test]
    fn range_tiles_the_region_and_clamps_the_tail() {
        assert_eq!(partition_range(3, 2, 0), (0, 2));
        assert_eq!(partition_range(3, 2, 1), (2, 3));
        assert_eq!(partition_range(3, 2, 2), (3, 3));
    }

    #[test]
    fn range_saturates_the_start_for_out_of_range_indices() {
        // A wrapping product would give index * chunk = 0 here, an in-region
        // range overlapping partition 0.
        let wrapping_index = usize::MAX / 2 + 1;
        assert_eq!(partition_range(3, 2, wrapping_index), (3, 3));
        assert_eq!(partition_range(3, 2, usize::MAX), (3, 3));
    }

    #[test]
    fn range_clamps_the_end_for_huge_chunks() {
        let chunk = usize::MAX / 2 + 1;
        assert_eq!(partition_range(usize::MAX, chunk, 0), (0, chunk));
        assert_eq!(partition_range(usize::MAX, chunk, 1), (chunk, usize::MAX));
        assert_eq!(
            partition_range(usize::MAX, usize::MAX, 1),
            (usize::MAX, usize::MAX)
        );
    }
}
