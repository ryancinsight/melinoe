//! `std`-gated drivers that run one [`WriterShard`] per thread concurrently.

use core::num::NonZeroUsize;
use core::sync::atomic::{AtomicPtr, Ordering};
use std::vec::Vec;

use crate::cell::MelinoeCell;
use crate::region::WriterShard;

/// Shard sizing policy for partitioned scoped-thread execution.
///
/// The plan controls only how a region is tiled into non-empty
/// [`WriterShard`]s. It does not introduce locks, atomics, queues, or worker
/// pools; each shard is still moved into one `std::thread::scope` worker and
/// joined before the call returns.
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
    fn resolve(self, len: usize) -> usize {
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

/// Split `cells` into `parts` disjoint shards and run `f` on each concurrently,
/// returning the per-shard results in partition order.
///
/// Each invocation of `f` receives the global start index of its partition (the
/// offset of the shard's first cell within `cells`) and the [`WriterShard`]
/// itself. Because the shards are non-overlapping, the writes proceed in
/// parallel with no atomics and no locks; the only synchronization is the
/// thread join at the end of the scope.
///
/// `parts` is clamped to at least `1`. The number of shards is
/// `min(parts, cells.len())` (no empty shards are produced).
///
/// # Panics
///
/// Propagates (re-raises) any panic that unwinds out of `f` on a worker thread.
///
/// # Examples
///
/// ```
/// use melinoe::sync::partition_map;
/// use melinoe::{brand_scope, MelinoeCell};
///
/// brand_scope(|token| {
///     let mut cells: Vec<MelinoeCell<'_, usize>> =
///         (0..8).map(|_| MelinoeCell::new(0)).collect();
///
///     // Four threads each fill their disjoint partition with global indices.
///     let written: Vec<usize> = partition_map(&mut cells, 4, |start, mut shard| {
///         for (j, slot) in shard.iter_mut().enumerate() {
///             *slot = start + j;
///         }
///         shard.len()
///     });
///     assert_eq!(written.iter().sum::<usize>(), 8);
///
///     // Read the whole region back via the token: every cell holds its index.
///     let snap = token.share();
///     for (k, c) in cells.iter().enumerate() {
///         assert_eq!(*c.borrow(snap), k);
///     }
/// });
/// ```
pub fn partition_map<'brand, T, R, F>(
    cells: &mut [MelinoeCell<'brand, T>],
    parts: usize,
    f: F,
) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(usize, WriterShard<'_, 'brand, T>) -> R + Sync,
{
    partition_map_with(cells, PartitionPlan::parts(parts), f)
}

/// Signature for a custom parallel executor.
///
/// # Safety
/// The executor must block the calling thread until all `num_tasks` invocations
/// of `task_fn` have completed. Each task is invoked with a unique index in
/// `0..num_tasks` and the provided `data` raw pointer. It must not call an
/// index more than once, omit an index, or return before every task call has
/// either completed or unwound through the executor.
pub type ParallelExecutorFn =
    unsafe fn(num_tasks: usize, task_fn: unsafe fn(usize, *mut ()), data: *mut ());

static PARALLEL_EXECUTOR: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

/// Register a global parallel executor to run `partition_map` chunks.
///
/// If registered, `partition_map_with` will execute chunks on the provided
/// executor instead of spawning raw OS threads via `std::thread::scope`.
#[inline]
pub fn register_parallel_executor(executor: ParallelExecutorFn) {
    PARALLEL_EXECUTOR.store(executor as *mut (), Ordering::Release);
}

/// Split `cells` according to `plan` and run `f` on each disjoint shard
/// concurrently, returning per-shard results in partition order.
///
/// Use [`PartitionPlan::available_parallelism`] when the caller wants the
/// current process's reported hardware parallelism, or
/// [`PartitionPlan::chunk_size`] when cache/NUMA tiling is more important than
/// a fixed worker count.
///
/// # Panics
///
/// Propagates (re-raises) any panic that unwinds out of `f` on a worker thread.
#[inline]
pub fn partition_map_with<'brand, T, R, F>(
    cells: &mut [MelinoeCell<'brand, T>],
    plan: PartitionPlan,
    f: F,
) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(usize, WriterShard<'_, 'brand, T>) -> R + Sync,
{
    let chunk = plan.resolve(cells.len());
    let chunks = WriterShard::new(cells).chunks(chunk);
    let num_chunks = chunks.len();
    if num_chunks == 0 {
        return Vec::new();
    }

    let executor_ptr = PARALLEL_EXECUTOR.load(Ordering::Acquire);
    if !executor_ptr.is_null() {
        let executor: ParallelExecutorFn = unsafe { core::mem::transmute(executor_ptr) };

        let mut out: Vec<core::mem::MaybeUninit<R>> = Vec::with_capacity(num_chunks);
        unsafe {
            out.set_len(num_chunks);
        }

        struct Context<'a, 'brand, T, R, F> {
            cells_ptr: *mut MelinoeCell<'brand, T>,
            cells_len: usize,
            chunk_size: usize,
            f: &'a F,
            out_ptr: *mut core::mem::MaybeUninit<R>,
        }

        let mut ctx = Context {
            cells_ptr: cells.as_mut_ptr(),
            cells_len: cells.len(),
            chunk_size: chunk,
            f: &f,
            out_ptr: out.as_mut_ptr(),
        };

        unsafe fn task_wrapper<'brand, T, R, F>(index: usize, data: *mut ())
        where
            T: Send,
            R: Send,
            F: Fn(usize, WriterShard<'_, 'brand, T>) -> R + Sync,
        {
            // SAFETY: `partition_map_with` passes a pointer to a live `Context`
            // and the executor safety contract requires all tasks to complete
            // before returning. The context fields are read-only during task
            // execution; per-task mutation happens only through disjoint output
            // slots and non-overlapping cell ranges below.
            let ctx = unsafe { &*(data as *const Context<'_, 'brand, T, R, F>) };
            let start = index * ctx.chunk_size;
            let end = (start + ctx.chunk_size).min(ctx.cells_len);

            // SAFETY: `index < num_tasks`, and `num_tasks` is exactly the
            // `ShardChunks` count for `chunk_size`, so each computed range is
            // in-bounds, non-empty, and disjoint from every other task range.
            let chunk_ref =
                unsafe { core::slice::from_raw_parts_mut(ctx.cells_ptr.add(start), end - start) };
            let shard = WriterShard::new(chunk_ref);
            let result = (ctx.f)(start, shard);
            // SAFETY: the executor invokes each task index in `0..num_tasks`
            // exactly once. Each task writes only its own result slot, so the
            // writes are disjoint even when tasks run concurrently.
            unsafe {
                ctx.out_ptr
                    .add(index)
                    .write(core::mem::MaybeUninit::new(result));
            }
        }

        unsafe {
            executor(
                num_chunks,
                task_wrapper::<T, R, F>,
                &mut ctx as *mut Context<'_, 'brand, T, R, F> as *mut (),
            );
        }

        // SAFETY: the custom parallel executor blocks until all tasks complete.
        // Therefore, every slot in the `out` vector has been initialized.
        let mut out = core::mem::ManuallyDrop::new(out);
        return unsafe {
            Vec::from_raw_parts(out.as_mut_ptr().cast::<R>(), num_chunks, out.capacity())
        };
    }

    std::thread::scope(|scope| {
        let f = &f;
        let mut handles = Vec::with_capacity(num_chunks);
        let mut start = 0usize;
        for shard in chunks {
            let shard_start = start;
            start += shard.len();
            handles.push(scope.spawn(move || f(shard_start, shard)));
        }
        handles
            .into_iter()
            .map(|h| match h.join() {
                Ok(value) => value,
                Err(payload) => std::panic::resume_unwind(payload),
            })
            .collect()
    })
}

/// Split `cells` using the process's reported hardware parallelism and run `f`
/// on each disjoint shard concurrently.
///
/// Equivalent to `partition_map_with(cells,
/// PartitionPlan::available_parallelism(), f)`.
#[inline]
pub fn partition_map_available<'brand, T, R, F>(
    cells: &mut [MelinoeCell<'brand, T>],
    f: F,
) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(usize, WriterShard<'_, 'brand, T>) -> R + Sync,
{
    partition_map_with(cells, PartitionPlan::available_parallelism(), f)
}

/// Split `cells` into `parts` disjoint shards and run `f` on each concurrently,
/// discarding results.
///
/// Convenience wrapper over [`partition_map`] for the common write-only case.
///
/// # Panics
///
/// Propagates any panic from a worker thread, as [`partition_map`].
#[inline]
pub fn partition_for_each<'brand, T, F>(cells: &mut [MelinoeCell<'brand, T>], parts: usize, f: F)
where
    T: Send,
    F: Fn(usize, WriterShard<'_, 'brand, T>) + Sync,
{
    partition_map(cells, parts, f);
}

/// Split `cells` according to `plan` and run `f` on each disjoint shard
/// concurrently, discarding results.
///
/// # Panics
///
/// Propagates any panic from a worker thread, as [`partition_map_with`].
#[inline]
pub fn partition_for_each_with<'brand, T, F>(
    cells: &mut [MelinoeCell<'brand, T>],
    plan: PartitionPlan,
    f: F,
) where
    T: Send,
    F: Fn(usize, WriterShard<'_, 'brand, T>) + Sync,
{
    partition_map_with(cells, plan, f);
}

/// Split `cells` using the process's reported hardware parallelism and run `f`
/// on each disjoint shard concurrently, discarding results.
#[inline]
pub fn partition_for_each_available<'brand, T, F>(cells: &mut [MelinoeCell<'brand, T>], f: F)
where
    T: Send,
    F: Fn(usize, WriterShard<'_, 'brand, T>) + Sync,
{
    partition_map_available(cells, f);
}
