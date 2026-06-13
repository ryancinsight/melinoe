use core::num::NonZeroUsize;

/// Shard sizing policy for partitioned scoped-thread execution.
///
/// The plan controls only how a region is tiled into non-empty
/// [`WriterShard`](crate::region::WriterShard)s. It does not introduce locks,
/// atomics, queues, or worker pools; each shard is still moved into one worker
/// and joined before the call returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartitionPlan {
    /// Split into at most this many non-empty shards.
    Parts(NonZeroUsize),
    /// Split into at most `std::thread::available_parallelism()` non-empty
    /// shards, falling back to one shard if the platform cannot report it.
    AvailableParallelism,
    /// Split into non-empty shards containing at most this many cells.
    ChunkSize(NonZeroUsize),
}

impl PartitionPlan {
    /// Create a fixed-part plan, clamping zero to one.
    #[inline]
    #[must_use]
    pub fn parts(parts: usize) -> Self {
        Self::Parts(nonzero_or_one(parts))
    }

    /// Create a plan based on the process's reported hardware parallelism.
    #[inline]
    #[must_use]
    pub const fn available_parallelism() -> Self {
        Self::AvailableParallelism
    }

    /// Create a fixed-chunk-size plan, clamping zero to one.
    #[inline]
    #[must_use]
    pub fn chunk_size(chunk_size: usize) -> Self {
        Self::ChunkSize(nonzero_or_one(chunk_size))
    }

    /// Resolve the plan to a concrete per-shard `chunk` size for `len` cells.
    ///
    /// The shard *count* is intentionally not computed here: it is derived once,
    /// at the single source of truth, from the [`ShardChunks`](crate::region)
    /// iterator's exact size when the driver reserves worker capacity.
    #[inline]
    pub(super) fn resolve(self, len: usize) -> usize {
        match self {
            Self::Parts(parts) => chunk_for_parts(len, parts.get()),
            Self::AvailableParallelism => {
                let parts = std::thread::available_parallelism().map_or(1, NonZeroUsize::get);
                chunk_for_parts(len, parts)
            }
            Self::ChunkSize(chunk_size) => chunk_size.get(),
        }
    }
}

#[inline]
fn nonzero_or_one(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| NonZeroUsize::new(1).expect("1 is non-zero"))
}

#[inline]
fn chunk_for_parts(len: usize, parts: usize) -> usize {
    if len == 0 {
        1
    } else {
        // ceil(len / parts), clamped to >= 1. Written as
        // `1 + (len - 1) / parts` because `usize::div_ceil` is not in the MSRV
        // and `len + parts - 1` can overflow for adversarial inputs.
        1 + (len - 1) / parts
    }
}
