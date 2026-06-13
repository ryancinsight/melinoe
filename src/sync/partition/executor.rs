use core::sync::atomic::{AtomicPtr, Ordering};

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

#[inline]
pub(super) fn registered_parallel_executor() -> Option<ParallelExecutorFn> {
    let executor_ptr = PARALLEL_EXECUTOR.load(Ordering::Acquire);
    if executor_ptr.is_null() {
        None
    } else {
        // SAFETY: `register_parallel_executor` stores only values originally
        // created from `ParallelExecutorFn`; the atomic load observes that same
        // bit pattern before reconstructing the function pointer.
        Some(unsafe { core::mem::transmute::<*mut (), ParallelExecutorFn>(executor_ptr) })
    }
}
