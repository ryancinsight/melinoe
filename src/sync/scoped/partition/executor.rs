use core::sync::atomic::{AtomicPtr, Ordering};

type ExecutorFn = unsafe fn(usize, unsafe fn(usize, *mut ()), *mut ());

/// Validated process-global parallel executor capability.
///
/// The transparent newtype separates a raw executor function from one whose
/// scheduling contract has been discharged. It has the same representation and
/// runtime cost as the function pointer it contains.
#[must_use]
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct ParallelExecutor(ExecutorFn);

impl ParallelExecutor {
    /// Validate a raw parallel executor function.
    ///
    /// # Safety
    ///
    /// On normal return, the executor must have invoked every index in
    /// `0..num_tasks` exactly once with the provided `data` pointer. On return
    /// or unwind, no invocation may still access that pointer. An executor may
    /// omit unfinished indices only when it unwinds through the caller.
    #[inline]
    pub const unsafe fn new(executor: ExecutorFn) -> Self {
        Self(executor)
    }

    #[inline]
    fn as_ptr(self) -> *mut () {
        self.0 as *mut ()
    }

    #[inline]
    unsafe fn from_ptr(executor: *mut ()) -> Self {
        // SAFETY: callers load only non-null pointers previously produced by
        // `ParallelExecutor::as_ptr`, preserving the function-pointer bits.
        Self(unsafe { core::mem::transmute::<*mut (), ExecutorFn>(executor) })
    }

    #[inline]
    pub(super) unsafe fn execute(
        self,
        num_tasks: usize,
        task_fn: unsafe fn(usize, *mut ()),
        data: *mut (),
    ) {
        // SAFETY: the caller supplies a task and context satisfying the
        // validated executor contract established by `new`.
        unsafe { (self.0)(num_tasks, task_fn, data) }
    }
}

const _: () =
    assert!(core::mem::size_of::<ParallelExecutor>() == core::mem::size_of::<ExecutorFn>());

static PARALLEL_EXECUTOR: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

/// Register a global parallel executor to run `partition_map` chunks.
///
/// If registered, `partition_map_with` will execute chunks on the provided
/// executor instead of spawning raw OS threads via `std::thread::scope`.
///
/// # Registration is process-global and order-sensitive
///
/// There is one slot for the whole process, and it is read afresh on every
/// partition call. Two consequences follow.
///
/// **Register before partitioning.** The scoped-thread fallback costs roughly
/// 30 µs per shard — it spawns `min(parts, len) − 1` OS threads per call and
/// joins them — against a pool dispatch that spawns nothing. The gap is large
/// enough that a fine-grained workload can be an order of magnitude slower on
/// the fallback. Because registration is lazily performed by the scheduling
/// layer (moirai registers from its own first-access initializer), a program
/// whose first partition call happens *before* it touches that scheduler will
/// silently take the fallback path for that call. Registering, or touching the
/// scheduler, at startup avoids this.
///
/// **A later registration does not retroactively change a call in flight.**
/// Each call loads the slot once; calls already running keep the driver they
/// started with.
///
/// # Example
///
/// ```
/// # use melinoe::sync::register_parallel_executor;
/// unsafe fn run_tasks(num_tasks: usize, task: unsafe fn(usize, *mut ()), data: *mut ()) {
///     for index in 0..num_tasks {
///         // SAFETY: the caller upholds the `ParallelExecutor` contract; this
///         // sequential stand-in invokes each index exactly once.
///         unsafe { task(index, data) };
///     }
/// }
///
/// // SAFETY: `run_tasks` drives every index in `0..num_tasks` exactly once and
/// // returns only after the last one, as `ParallelExecutor::new` requires.
/// register_parallel_executor(unsafe {
///     melinoe::sync::ParallelExecutor::new(run_tasks)
/// });
/// ```
#[inline]
pub fn register_parallel_executor(executor: ParallelExecutor) {
    PARALLEL_EXECUTOR.store(executor.as_ptr(), Ordering::Release);
}

/// Clear the registered parallel executor, restoring the default scoped-thread
/// partition driver.
///
/// This is primarily a lifecycle and test-isolation hook for integrations that
/// install a process-global scheduler temporarily. Existing `partition_map`
/// calls that have already loaded the executor continue under that call's
/// chosen driver; later calls use the default path.
#[inline]
pub fn clear_parallel_executor() {
    PARALLEL_EXECUTOR.store(core::ptr::null_mut(), Ordering::Release);
}

#[inline]
pub(super) fn registered_parallel_executor() -> Option<ParallelExecutor> {
    let executor_ptr = PARALLEL_EXECUTOR.load(Ordering::Acquire);
    if executor_ptr.is_null() {
        None
    } else {
        // SAFETY: registration stores only pointers produced by
        // `ParallelExecutor::as_ptr`; the acquire load observes those function
        // pointer bits after the matching release store.
        Some(unsafe { ParallelExecutor::from_ptr(executor_ptr) })
    }
}
