//! The single generic engine behind every partition driver.
//!
//! Both the mutable-shard ([`super::map`]) and shared-slice ([`super::read_map`])
//! drivers reduce to the same shape: *run `num_chunks` independent tasks, each
//! producing an `R`, and collect the results in index order, propagating the
//! first panic*. The only thing that varies between them is how task `index`
//! turns into an `R` — the mutable path builds a
//! [`WriterShard`](crate::region::WriterShard) from a disjoint sub-slice and
//! calls the user closure; the shared path builds a `&[T]`. That variation is
//! captured entirely by the `run: Fn(usize) -> R` closure each driver supplies,
//! so the delicate machinery — the `MaybeUninit` out-buffer, the
//! [`ExecutorDropGuard`] unwind handling, the panic mutex, and the
//! `Vec::from_raw_parts` teardown — lives here exactly once.

use std::vec::Vec;

use super::executor::registered_parallel_executor;

type PanicPayload = Option<std::boxed::Box<dyn std::any::Any + Send>>;

type PanicPayloadMutex = std::sync::Mutex<PanicPayload>;

fn lock_panic_payload(mutex: &PanicPayloadMutex) -> std::sync::MutexGuard<'_, PanicPayload> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn take_panic_payload(mutex: PanicPayloadMutex) -> PanicPayload {
    mutex
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Guards the raw `MaybeUninit` out-buffer while the registered executor runs.
///
/// If a task panics and unwinds *through* the executor call (rather than being
/// caught by the per-task `catch_unwind`), this guard runs on unwind and (a)
/// drops exactly the slots the tasks marked initialized and (b) frees the
/// backing allocation, so no leak or double-drop occurs. On the normal path the
/// driver sets `active = false` after the executor returns and takes ownership of
/// the buffer itself.
struct ExecutorDropGuard<R> {
    out_ptr: *mut core::mem::MaybeUninit<R>,
    capacity: usize,
    successful_ptr: *const bool,
    num_chunks: usize,
    active: bool,
}

impl<R> Drop for ExecutorDropGuard<R> {
    fn drop(&mut self) {
        if self.active {
            for index in 0..self.num_chunks {
                // SAFETY: `successful_ptr` points at a live `[bool; num_chunks]`
                // (the driver's `successful` vec, borrowed for the executor call),
                // and `out_ptr` at a `MaybeUninit<R>` allocation of `capacity >=
                // num_chunks`. A `true` flag was set only after the matching slot
                // was `write`-initialized with a valid `R`, so dropping it in
                // place is sound and happens at most once.
                unsafe {
                    if *self.successful_ptr.add(index) {
                        self.out_ptr.add(index).cast::<R>().drop_in_place();
                    }
                }
            }
            // SAFETY: `out_ptr`/`capacity` are the pointer and capacity of the
            // `Vec<MaybeUninit<R>>` the driver `forget`-leaked; reconstructing it
            // with length `0` frees the backing allocation without dropping any
            // element (initialized ones were dropped above).
            unsafe {
                let _ = Vec::from_raw_parts(self.out_ptr, 0, self.capacity);
            }
        }
    }
}

/// Context handed to the raw task wrapper through the executor's `*mut ()`.
///
/// Holds only shared/read-only state plus the per-slot output pointers; each
/// task writes solely to its own `index` slot and flag, so no field is a shared
/// mutable aliasing hazard.
struct TaskContext<'a, R, Run> {
    run: &'a Run,
    out_ptr: *mut core::mem::MaybeUninit<R>,
    successful_ptr: *mut bool,
    panic_payload: &'a PanicPayloadMutex,
}

/// The raw per-task entry point handed to a registered executor.
///
/// # Safety
///
/// `data` must be a live `*mut TaskContext<'_, R, Run>` valid for the whole
/// executor call, and the executor must invoke each `index` in
/// `0..num_chunks` at most once (the [`ParallelExecutor`](super::ParallelExecutor)
/// contract), so no two invocations touch the same `out_ptr`/`successful_ptr`
/// slot.
unsafe fn task_wrapper<R, Run>(index: usize, data: *mut ())
where
    R: Send,
    Run: Fn(usize) -> R + Sync,
{
    // SAFETY: by the function contract `data` is a live `TaskContext<'_, R, Run>`
    // borrowed for the duration of the executor call; the fields read here
    // (`run`, `panic_payload`) are shared-immutable, and the per-slot writes
    // below target this task's unique `index`.
    let ctx = unsafe { &*(data as *const TaskContext<'_, R, Run>) };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (ctx.run)(index)));

    match result {
        Ok(val) => {
            // SAFETY: `index` is unique to this task (executor contract), so this
            // is the sole writer of slot `index`; the slot is uninitialized
            // `MaybeUninit<R>` and is written exactly once, after which its flag
            // is set so teardown knows to drop it.
            unsafe {
                ctx.out_ptr
                    .add(index)
                    .write(core::mem::MaybeUninit::new(val));
                *ctx.successful_ptr.add(index) = true;
            }
        }
        Err(payload) => {
            let mut g = lock_panic_payload(ctx.panic_payload);
            if g.is_none() {
                *g = Some(payload);
            }
        }
    }
}

/// Run `num_chunks` tasks that each turn an index into an `R`, collecting the
/// results in index order and re-raising the first task panic.
///
/// `num_chunks` must be `>= 1` (callers return early for the empty region).
/// `run(index)` is invoked once per `index` in `0..num_chunks`; the closure owns
/// whatever per-index shard construction the driver needs. When a parallel
/// executor is registered it drives the tasks; otherwise the work runs on scoped
/// OS threads (one task inline on the current thread).
///
/// # Panics
///
/// Re-raises (via [`resume_unwind`](std::panic::resume_unwind)) the first panic
/// observed from any `run` invocation, after dropping every successfully
/// produced result so nothing leaks.
pub(super) fn drive<R, Run>(num_chunks: usize, run: Run) -> Vec<R>
where
    R: Send,
    Run: Fn(usize) -> R + Sync,
{
    debug_assert!(num_chunks >= 1, "drive requires at least one chunk");

    if let Some(executor) = registered_parallel_executor() {
        let mut out: Vec<core::mem::MaybeUninit<R>> = Vec::with_capacity(num_chunks);
        // SAFETY: `capacity == num_chunks`; the elements are `MaybeUninit`, so
        // extending the length to expose the (uninitialized) slots is sound —
        // each slot is initialized exactly once by its task before being read.
        unsafe {
            out.set_len(num_chunks);
        }
        let out_ptr = out.as_mut_ptr();
        let capacity = out.capacity();
        core::mem::forget(out);

        let mut successful = std::vec![false; num_chunks];
        let panic_payload = PanicPayloadMutex::new(None);

        let mut guard = ExecutorDropGuard {
            out_ptr,
            capacity,
            successful_ptr: successful.as_ptr(),
            num_chunks,
            active: true,
        };

        let mut ctx = TaskContext {
            run: &run,
            out_ptr,
            successful_ptr: successful.as_mut_ptr(),
            panic_payload: &panic_payload,
        };

        // SAFETY: `task_wrapper::<R, Run>` matches the executor's task signature;
        // `&mut ctx` is a live `TaskContext<'_, R, Run>` valid for the whole call
        // (the executor blocks until every task completes). The executor contract
        // guarantees each `index` runs at most once, upholding `task_wrapper`'s
        // own safety requirement.
        unsafe {
            executor.execute(
                num_chunks,
                task_wrapper::<R, Run>,
                core::ptr::addr_of_mut!(ctx).cast::<()>(),
            );
        }

        // The executor has returned: every task completed or unwound into the
        // panic mutex. Take manual ownership of the buffer away from the guard.
        guard.active = false;

        if let Some(payload) = take_panic_payload(panic_payload) {
            for (index, &success) in successful.iter().enumerate() {
                if success {
                    // SAFETY: `success` is set only after slot `index` was
                    // initialized with a valid `R`; drop it once to avoid leaking
                    // the results of the tasks that finished before the panic.
                    unsafe {
                        out_ptr.add(index).cast::<R>().drop_in_place();
                    }
                }
            }
            // SAFETY: reconstruct the leaked buffer with length `0` to free its
            // backing allocation; initialized slots were just dropped above.
            unsafe {
                let _ = Vec::from_raw_parts(out_ptr, 0, capacity);
            }
            std::panic::resume_unwind(payload);
        }

        // SAFETY: on the panic-free path every one of the `num_chunks` slots was
        // written exactly once (executor ran each index), so the buffer is a
        // fully-initialized `[R; num_chunks]`; `MaybeUninit<R>` shares `R`'s
        // layout, and `capacity` is the original allocation capacity, so this
        // reconstitutes the owning `Vec<R>` without copy.
        return unsafe { Vec::from_raw_parts(out_ptr.cast::<R>(), num_chunks, capacity) };
    }

    if num_chunks == 1 {
        return std::vec![run(0)];
    }

    std::thread::scope(|scope| {
        let run = &run;
        let mut handles = Vec::with_capacity(num_chunks - 1);
        for index in 0..(num_chunks - 1) {
            handles.push(scope.spawn(move || run(index)));
        }

        let last = run(num_chunks - 1);

        let mut results = Vec::with_capacity(num_chunks);
        for h in handles {
            match h.join() {
                Ok(value) => results.push(value),
                Err(payload) => std::panic::resume_unwind(payload),
            }
        }
        results.push(last);
        results
    })
}

#[cfg(test)]
// Test code is exempt from `clippy::unwrap_used`: a panic here is the
// assertion, not a defect escaping into a consumer's process.
#[allow(clippy::unwrap_used)]
mod tests {
    use std::boxed::Box;

    use super::{lock_panic_payload, take_panic_payload, PanicPayloadMutex, TaskContext};

    #[test]
    fn poisoned_payload_mutex_preserves_the_first_panic() {
        let mutex = PanicPayloadMutex::new(None);
        let poison = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut payload = mutex.lock().unwrap();
            *payload = Some(Box::new("first panic"));
            panic!("poison payload mutex");
        }));
        assert!(poison.is_err());

        let run: fn(usize) = |_| panic!("second panic");
        let mut successful = false;
        let mut context = TaskContext::<(), fn(usize)> {
            run: &run,
            out_ptr: core::ptr::null_mut(),
            successful_ptr: &mut successful,
            panic_payload: &mutex,
        };

        // The task wrapper's recovery path must not mask the first payload or
        // panic again merely because another task reports a panic afterward.
        unsafe {
            super::task_wrapper::<(), fn(usize)>(0, core::ptr::addr_of_mut!(context).cast::<()>());
        }

        let payload = take_panic_payload(mutex).expect("first panic payload survives poisoning");
        assert_eq!(payload.downcast_ref::<&'static str>(), Some(&"first panic"));

        // Keep the lock helper exercised directly as well: poisoned state is a
        // recoverable condition for this payload-only mutex.
        let mutex = PanicPayloadMutex::new(None);
        let poison = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = mutex.lock().unwrap();
            panic!("poison empty payload mutex");
        }));
        assert!(poison.is_err());
        assert!(lock_panic_payload(&mutex).is_none());
    }
}
