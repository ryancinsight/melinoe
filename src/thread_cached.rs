//! Per-thread value caching, consolidated from the identical
//! nightly-`#[thread_local]` / stable-`thread_local!` pairs that themis
//! (`CACHED_NODE`) and mnemosyne (`CACHED_CPU_ID`) carried independently.
//!
//! Thread-local statics cannot be expressed as a generic type — the storage
//! must be declared per site with the toolchain-appropriate attribute — so the
//! single authoritative implementation is the
//! [`thread_cached!`](crate::thread_cached!) macro, which
//! expands to a module owning the cfg-paired storage plus a typed accessor
//! surface. Boilerplate generation is the sanctioned macro use here: the
//! variation (nightly fast path vs stable fallback) is a *declaration-site*
//! dimension that traits and generics cannot capture.
//!
//! The cache always stores `Option<T>` — "uninitialized" is a real state, not
//! a sentinel value carved out of `T`'s domain.

/// Declares a per-thread cached value with `get_or_init` / `set` / `get` /
/// `clear` accessors.
///
/// Expands to a module named `$name` containing the thread-local storage and
/// four functions:
///
/// - `get_or_init(init: impl FnOnce() -> T) -> T` — returns the cached value,
///   computing and caching it on first access from the calling thread.
/// - `set(value: T)` — overwrites the calling thread's cached value.
/// - `get() -> Option<T>` — reads the calling thread's cached value without
///   initializing it.
/// - `clear()` — returns the calling thread's cache to the uninitialized state.
///
/// `T` must be `Copy`.
///
/// # Consumer requirements
///
/// On nightly toolchains the expansion uses `#[thread_local]` statics, so the
/// consuming crate must carry:
///
/// - a build script emitting `cargo:rustc-check-cfg=cfg(nightly_tls_active)`
///   and `cargo:rustc-cfg=nightly_tls_active` when the compiler is nightly
///   (themis and mnemosyne-local already do), and
/// - `#![cfg_attr(nightly_tls_active, feature(thread_local))]` at crate root.
///
/// On stable the expansion falls back to `std::thread_local!`, so a stable
/// consumer must link `std`.
///
/// # Example
///
/// ```
/// #![cfg_attr(nightly_tls_active, feature(thread_local))]
/// melinoe::thread_cached! {
///     /// Cached worker shard index for the calling thread.
///     pub mod cached_shard: u32;
/// }
///
/// assert_eq!(cached_shard::get_or_init(|| 7), 7);
/// cached_shard::set(11);
/// assert_eq!(cached_shard::get(), Some(11));
/// cached_shard::clear();
/// assert_eq!(cached_shard::get(), None);
/// assert_eq!(cached_shard::get_or_init(|| 13), 13);
/// ```
#[macro_export]
macro_rules! thread_cached {
    ($(#[$meta:meta])* $vis:vis mod $name:ident: $ty:ty;) => {
        $(#[$meta])*
        $vis mod $name {
            #[allow(unused_imports)]
            use super::*;

            #[cfg(nightly_tls_active)]
            #[thread_local]
            static mut VALUE: Option<$ty> = None;

            #[cfg(not(nightly_tls_active))]
            ::std::thread_local! {
                static VALUE: ::core::cell::Cell<Option<$ty>> =
                    const { ::core::cell::Cell::new(None) };
            }

            /// Returns the cached value, initializing it with `init` on the
            /// calling thread's first access.
            #[inline]
            pub fn get_or_init(init: impl FnOnce() -> $ty) -> $ty {
                #[cfg(nightly_tls_active)]
                // SAFETY: `VALUE` is `#[thread_local]`: the calling thread
                // owns its instance exclusively and no reference to it
                // escapes this function, so the read/write cannot race.
                unsafe {
                    if let Some(value) = VALUE {
                        value
                    } else {
                        let value = init();
                        VALUE = Some(value);
                        value
                    }
                }
                #[cfg(not(nightly_tls_active))]
                VALUE.with(|cell| {
                    if let Some(value) = cell.get() {
                        value
                    } else {
                        let value = init();
                        cell.set(Some(value));
                        value
                    }
                })
            }

            /// Overwrites the cached value for the calling thread.
            #[inline]
            pub fn set(value: $ty) {
                #[cfg(nightly_tls_active)]
                // SAFETY: thread-exclusive `#[thread_local]` slot; see
                // `get_or_init`.
                unsafe {
                    VALUE = Some(value);
                }
                #[cfg(not(nightly_tls_active))]
                VALUE.with(|cell| cell.set(Some(value)));
            }

            /// Returns the cached value if initialized, otherwise returns `None`.
            #[inline]
            pub fn get() -> Option<$ty> {
                #[cfg(nightly_tls_active)]
                // SAFETY: thread-exclusive `#[thread_local]` slot; see
                // `get_or_init`.
                unsafe {
                    VALUE
                }
                #[cfg(not(nightly_tls_active))]
                VALUE.with(|cell| cell.get())
            }

            /// Clears the cached value for the calling thread.
            #[inline]
            pub fn clear() {
                #[cfg(nightly_tls_active)]
                // SAFETY: thread-exclusive `#[thread_local]` slot; see
                // `get_or_init`.
                unsafe {
                    VALUE = None;
                }
                #[cfg(not(nightly_tls_active))]
                VALUE.with(|cell| cell.set(None));
            }
        }
    };
}
