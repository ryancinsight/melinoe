//! Value-semantic tests for the `thread_cached!` per-thread cache macro.
#![cfg_attr(nightly_tls_active, feature(thread_local))]

melinoe::thread_cached! {
    /// Per-thread test cache.
    pub mod cached_value: u64;
}

#[test]
fn first_access_initializes_and_later_accesses_reuse() {
    let value = cached_value::get_or_init(|| 41);
    assert_eq!(value, 41);
    // Init closure must not run again on the same thread.
    assert_eq!(cached_value::get_or_init(|| panic!("re-init")), 41);
}

#[test]
fn set_overwrites_for_the_calling_thread() {
    cached_value::set(7);
    assert_eq!(cached_value::get_or_init(|| panic!("re-init")), 7);
    cached_value::set(9);
    assert_eq!(cached_value::get_or_init(|| panic!("re-init")), 9);
}

#[test]
fn threads_have_independent_caches() {
    cached_value::set(100);
    let other = std::thread::spawn(|| {
        // Fresh thread: uninitialized, runs its own init.
        let initial = cached_value::get_or_init(|| 200);
        cached_value::set(201);
        (initial, cached_value::get_or_init(|| panic!("re-init")))
    })
    .join()
    .expect("invariant: spawned test thread completes");
    assert_eq!(other, (200, 201));
    // Spawned thread's writes never leak into this thread's slot.
    assert_eq!(cached_value::get_or_init(|| panic!("re-init")), 100);
}
