//! The parallel-executor seam: how Melinoe hands a partition's tasks to an
//! external scheduler without depending on it.
//!
//! Melinoe is a foundation crate with no dependencies and a `#![no_std]`
//! posture, so it cannot name a scheduler. It instead defines the *contract* a
//! scheduler must satisfy ([`ParallelExecutor`]) and a process-global slot
//! holding one implementation ([`register_parallel_executor`]). A scheduler
//! implements the trait and registers itself; Melinoe's partition drivers then
//! run their shards on it.
//!
//! # Why a trait and not a function pointer
//!
//! The seam used to be a bare
//! `unsafe fn(usize, unsafe fn(usize, *mut ()), *mut ())` stored in an
//! `AtomicPtr<()>`. That shape worked, but it leaked cost into every
//! implementor:
//!
//! * The contract ("invoke every index exactly once, and return only when the
//!   last invocation has completed") could not be stated anywhere a
//!   implementor would read it — it lived in a doc comment on an unrelated
//!   constructor.
//! * `Send` was inexpressible. An implementor that moves its context pointer
//!   onto worker threads had to launder it through `usize` to satisfy the
//!   compiler, which is exactly the kind of cast that defeats the check.
//! * Melinoe's own `MaybeUninit` out-buffer trusts the contract absolutely. A
//!   caller could install any function of the right *arity*, and the machine
//!   that contains the fallout ([`super::driver_core`]'s drop guard) exists
//!   because that trust cannot be verified.
//!
//! Expressing the seam as a trait does not make the contract verifiable — that
//! is not possible across a crate boundary — but it moves it to one place an
//! implementor cannot miss. The entry point is an associated function rather
//! than a receiver method: the global slot stores no scheduler value, so an
//! implementation cannot accidentally observe fabricated or uninitialized
//! receiver storage.
//!
//! # The zero-alloc global slot
//!
//! A global slot cannot hold a generic type, and `dyn Trait` needs `alloc`,
//! which is an optional feature here. The slot therefore holds a *monomorphized
//! shim*: [`register`] generates one trampoline per implementing type with the
//! same ABI the old design used, and stores its address. The ABI is an
//! implementation detail of this module now, not the public interface.
//!
//! # Registration is process-global and order-sensitive
//!
//! There is one slot for the whole process, and it is read afresh on every
//! partition call. Two consequences follow.
//!
//! **Register before partitioning.** The scoped-thread fallback costs roughly
//! 30 µs per shard — it spawns `min(parts, len) − 1` OS threads per call and
//! joins them — against a pool dispatch that spawns nothing. The gap is large
//! enough that a fine-grained workload can be an order of magnitude slower on
//! the fallback. Because registration is lazily performed by the scheduling
//! layer (a scheduler typically registers from its own first-access
//! initializer), a program whose first partition call happens *before* it
//! touches that scheduler will silently take the fallback path for that call.
//! Registering, or touching the scheduler, at startup avoids this.
//!
//! **A later registration does not retroactively change a call in flight.**
//! Each call loads the slot once; calls already running keep the driver they
//! started with.

use core::sync::atomic::{AtomicPtr, Ordering};

/// A scheduler that can run a partition's independent tasks.
///
/// This is the contract a scheduler honours to drive Melinoe's partitioning.
/// Implement its associated entry point, then pass the type to
/// [`register_parallel_executor`]. The global slot stores only a
/// monomorphized function pointer; no scheduler value or receiver is created.
///
/// # Contract
///
/// An implementation of [`run_indexed`](ParallelExecutor::run_indexed) **must**:
///
/// 1. Invoke `task(index, context)` for **every** `index` in `0..num_tasks`
///    **exactly once**.
/// 2. Return only after the last invocation has completed — no invocation may
///    still be running, or able to start, once `run_indexed` returns.
/// 3. On unwind, either complete every remaining index or unwind through the
///    caller; it must not return normally having skipped one.
///
/// Melinoe depends on (1) and (2) for soundness, not merely correctness: it
/// hands each task a pointer into a `MaybeUninit` out-buffer and reconstructs
/// a fully-initialized `Vec` on return, so a skipped index reads uninitialized
/// memory and a torn return aliases it. Violating the contract is undefined
/// behaviour, which is why [`run_indexed`](ParallelExecutor::run_indexed) is
/// `unsafe` to *invoke* and why implementations are expected to document how
/// they discharge it.
///
/// # `Send` and the context pointer
///
/// `context` is a raw pointer because the tasks must remain type-erased across
/// the global slot. An implementation that moves it to worker threads must do
/// so soundly; where the pointer was laundered through `usize` to pass the
/// compiler, prefer a `Send` wrapper type so the obligation is visible.
///
/// # Safety
///
/// `run_indexed` is unsafe to invoke because its *caller* must uphold the
/// mirror-image obligations: `context` must be a live pointer of the type the
/// task function expects, valid for the whole call, and the task function must
/// expect it. Implementors of the trait do not choose those; Melinoe does.
///
/// # Example
///
/// A sequential stand-in, useful in tests:
///
/// ```
/// # use melinoe::sync::ParallelExecutor;
/// struct Sequentially;
///
/// // SAFETY: the loop runs every index in `0..num_tasks` exactly once and
/// // returns only after the last invocation, as the contract requires.
/// unsafe impl ParallelExecutor for Sequentially {
///     unsafe fn run_indexed(
///         num_tasks: usize,
///         task: unsafe fn(usize, *mut ()),
///         context: *mut (),
///     ) {
///         for index in 0..num_tasks {
///             // SAFETY: forwarded from the caller; this implementation invokes
///             // each index exactly once with the caller's context.
///             unsafe { task(index, context) };
///         }
///     }
/// }
/// ```
pub unsafe trait ParallelExecutor: Sized {
    /// Run `task` for every index in `0..num_tasks`, blocking until all have
    /// completed.
    ///
    /// # Safety
    ///
    /// The caller must pass a `context` that is a live pointer of the type
    /// `task` expects, valid for the entire call, and must not use the pointer
    /// again until this method returns. See the trait's own `# Safety`.
    unsafe fn run_indexed(num_tasks: usize, task: unsafe fn(usize, *mut ()), context: *mut ());
}

/// The ABI of the registered shim: type-erased, monomorphized per implementor
/// type by [`register`].
type ExecutorFn = unsafe fn(usize, unsafe fn(usize, *mut ()), *mut ());

/// A validated, process-global parallel executor.
///
/// Construct one with [`Executor::new`] and hand it to
/// [`register_parallel_executor`] when an integration needs to hold the
/// function-pointer capability before registration.
#[must_use]
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Executor(ExecutorFn);

const _: () = assert!(core::mem::size_of::<Executor>() == core::mem::size_of::<ExecutorFn>());

impl Executor {
    /// Build a registrable executor from an implementation of the contract.
    ///
    /// This is safe to *call*: the shim generated here forwards faithfully to
    /// `E::run_indexed`, so whatever safety obligations the implementation
    /// carries are the implementation's to discharge — and implementing
    /// [`ParallelExecutor`] is itself `unsafe`.
    pub const fn new<E: ParallelExecutor + 'static>() -> Self {
        // The shim is monomorphized per `E`, so the global slot can stay a
        // single pointer with no allocation and no `dyn`.
        unsafe fn shim<E: ParallelExecutor + 'static>(
            num_tasks: usize,
            task: unsafe fn(usize, *mut ()),
            context: *mut (),
        ) {
            // SAFETY: `E`'s contract, established by its `unsafe impl`, is that
            // `run_indexed` invokes every index in `0..num_tasks` exactly once
            // and returns only afterwards. Forwarding the caller's `task` and
            // `context` unchanged is the identity case of that obligation, so
            // the shim upholds `run_indexed`'s own requirement.
            //
            unsafe { E::run_indexed(num_tasks, task, context) };
        }
        Self(shim::<E>)
    }

    #[inline]
    fn as_ptr(self) -> *mut () {
        self.0 as *mut ()
    }

    #[inline]
    unsafe fn from_ptr(executor: *mut ()) -> Self {
        // SAFETY: callers load only non-null pointers previously produced by
        // `Executor::as_ptr`, preserving the function-pointer bits.
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
        // contract established when the shim was generated by `Executor::new`.
        unsafe { (self.0)(num_tasks, task_fn, data) }
    }
}

static PARALLEL_EXECUTOR: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

/// Register a global parallel executor to run partition tasks.
///
/// If registered, Melinoe's partition drivers execute their shards on `E`
/// instead of spawning raw OS threads via `std::thread::scope`.
///
/// See the module documentation for why registration is process-global and
/// order-sensitive.
///
/// # Example
///
/// ```
/// # use melinoe::sync::{ParallelExecutor, register_parallel_executor};
/// struct Sequentially;
///
/// // SAFETY: every index in `0..num_tasks` is invoked exactly once, and the
/// // call returns only after the last invocation completes.
/// unsafe impl ParallelExecutor for Sequentially {
///     unsafe fn run_indexed(
///         num_tasks: usize,
///         task: unsafe fn(usize, *mut ()),
///         context: *mut (),
///     ) {
///         for index in 0..num_tasks {
///             // SAFETY: forwarded from the caller.
///             unsafe { task(index, context) };
///         }
///     }
/// }
///
/// register_parallel_executor::<Sequentially>();
/// ```
#[inline]
pub fn register_parallel_executor<E: ParallelExecutor + 'static>() {
    PARALLEL_EXECUTOR.store(Executor::new::<E>().as_ptr(), Ordering::Release);
}

/// Clear the registered parallel executor, restoring the default scoped-thread
/// partition driver.
///
/// This is primarily a lifecycle and test-isolation hook for integrations that
/// install a process-global scheduler temporarily. Existing partition calls
/// that have already loaded the executor continue under that call's chosen
/// driver; later calls use the default path.
#[inline]
pub fn clear_parallel_executor() {
    PARALLEL_EXECUTOR.store(core::ptr::null_mut(), Ordering::Release);
}

#[inline]
pub(super) fn registered_parallel_executor() -> Option<Executor> {
    let executor_ptr = PARALLEL_EXECUTOR.load(Ordering::Acquire);
    if executor_ptr.is_null() {
        None
    } else {
        // SAFETY: registration stores only pointers produced by
        // `Executor::as_ptr`; the acquire load observes those function pointer
        // bits after the matching release store.
        Some(unsafe { Executor::from_ptr(executor_ptr) })
    }
}
