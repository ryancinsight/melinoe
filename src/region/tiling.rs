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
