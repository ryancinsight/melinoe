# 5. Conditional Atomics

*Chapter prose deferred — DoR item.*

<!-- Key points to cover:
  - BrandedAtomic<'brand, A>: wraps any A: Atomic; routes through the token
  - WritePermit → plain store_exclusive: no lock prefix; exclusive ownership
    is the proof that no concurrent reader exists
  - ReadPermit → true atomic op: fetch_add, fetch_sub, cmpxchg, etc.
    with a runtime Ordering or a ZST ordering policy
  - ZST ordering policies: Relaxed, AcqRel, SeqCst — monomorphize the call
    site so the ordering is a compile-time constant, not a runtime branch
  - as_atomic(tok): gives a &A so existing APIs that accept &AtomicU64
    can be called without unsafe code — branded → raw interop
  - load_exclusive: non-atomic load under WritePermit (no lock, faster than
    a shared atomic load when exclusive ownership is proven)
-->
