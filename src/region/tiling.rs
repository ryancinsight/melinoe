//! How many fixed-size shards tile a region.

/// Number of `chunk`-sized partitions tiling `len` items: `ceil(len / chunk)`,
/// or `0` when `len == 0`.
///
/// `chunk` must be `>= 1`, so the division is total. Written as `1 + (len - 1) /
/// chunk` to compute the ceiling without the `len + chunk - 1` form, which can
/// overflow for adversarial `len`. This is the single source of truth for the
/// count shared by `ParChunks::len`, `ShardChunks::size_hint`, and the scoped
/// partition drivers, so they can never disagree on how many shards tile a
/// region.
#[inline]
pub(crate) const fn partition_count(len: usize, chunk: usize) -> usize {
    if len == 0 {
        0
    } else {
        1 + (len - 1) / chunk
    }
}

/// Bounds of a partition, clamped to the region for all indices.
///
/// Saturation before clamping is necessary for zero-sized elements: their
/// slices may have lengths near `usize::MAX` without allocating storage.
#[inline]
pub(crate) const fn partition_range(len: usize, chunk: usize, index: usize) -> (usize, usize) {
    let start = index.saturating_mul(chunk);
    let end = start.saturating_add(chunk);
    let start = if start < len { start } else { len };
    let end = if end < len { end } else { len };
    (start, end)
}
