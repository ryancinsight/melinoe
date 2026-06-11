use core::sync::atomic::{
    AtomicBool, AtomicI32, AtomicI64, AtomicIsize, AtomicU32, AtomicU64, AtomicUsize, Ordering,
};

pub(crate) mod sealed {
    pub trait Sealed {}
}

/// An atomic primitive abstracted over its value type, so [`crate::BrandedAtomic`] is
/// one generic implementation rather than a type-numbered family.
///
/// Sealed: implemented only for the standard-library atomics. The methods are
/// the crate-internal mediation surface; users call [`crate::BrandedAtomic`].
pub trait Atomic: sealed::Sealed {
    /// The plain value carried by this atomic (e.g. `u64` for `AtomicU64`).
    type Value: Copy;

    #[doc(hidden)]
    fn new_atomic(value: Self::Value) -> Self;
    #[doc(hidden)]
    fn atomic_load(&self, order: Ordering) -> Self::Value;
    #[doc(hidden)]
    fn atomic_store(&self, value: Self::Value, order: Ordering);
    #[doc(hidden)]
    fn atomic_swap(&self, value: Self::Value, order: Ordering) -> Self::Value;
    #[doc(hidden)]
    fn atomic_compare_exchange(
        &self,
        current: Self::Value,
        new: Self::Value,
        success: Ordering,
        failure: Ordering,
    ) -> Result<Self::Value, Self::Value>;
    #[doc(hidden)]
    fn atomic_get_mut(&mut self) -> &mut Self::Value;
    #[doc(hidden)]
    fn atomic_into_inner(self) -> Self::Value;
    /// Pointer to the underlying value for plain (non-atomic) access.
    ///
    /// Sound to dereference only under a proof of exclusivity. An atomic has the
    /// same size/bit-validity as its value and interior mutability, so the cast
    /// is layout-valid and the pointer carries interior-mutable provenance.
    #[doc(hidden)]
    fn value_ptr(&self) -> *mut Self::Value;
}

/// Integer atomics, which additionally support arithmetic/bitwise RMW.
pub trait AtomicInt: Atomic {
    #[doc(hidden)]
    fn atomic_fetch_add(&self, value: Self::Value, order: Ordering) -> Self::Value;
    #[doc(hidden)]
    fn atomic_fetch_sub(&self, value: Self::Value, order: Ordering) -> Self::Value;
    #[doc(hidden)]
    fn atomic_fetch_and(&self, value: Self::Value, order: Ordering) -> Self::Value;
    #[doc(hidden)]
    fn atomic_fetch_or(&self, value: Self::Value, order: Ordering) -> Self::Value;
}

macro_rules! impl_atomic_int {
    ($atomic:ty, $value:ty) => {
        impl sealed::Sealed for $atomic {}

        impl Atomic for $atomic {
            type Value = $value;

            #[inline]
            fn new_atomic(value: $value) -> Self {
                <$atomic>::new(value)
            }
            #[inline]
            fn atomic_load(&self, order: Ordering) -> $value {
                self.load(order)
            }
            #[inline]
            fn atomic_store(&self, value: $value, order: Ordering) {
                self.store(value, order);
            }
            #[inline]
            fn atomic_swap(&self, value: $value, order: Ordering) -> $value {
                self.swap(value, order)
            }
            #[inline]
            fn atomic_compare_exchange(
                &self,
                current: $value,
                new: $value,
                success: Ordering,
                failure: Ordering,
            ) -> Result<$value, $value> {
                self.compare_exchange(current, new, success, failure)
            }
            #[inline]
            fn atomic_get_mut(&mut self) -> &mut $value {
                self.get_mut()
            }
            #[inline]
            fn atomic_into_inner(self) -> $value {
                self.into_inner()
            }
            #[inline]
            fn value_ptr(&self) -> *mut $value {
                // SAFETY of later deref: same layout as `$value`, interior-mutable.
                self as *const Self as *mut $value
            }
        }

        impl AtomicInt for $atomic {
            #[inline]
            fn atomic_fetch_add(&self, value: $value, order: Ordering) -> $value {
                self.fetch_add(value, order)
            }
            #[inline]
            fn atomic_fetch_sub(&self, value: $value, order: Ordering) -> $value {
                self.fetch_sub(value, order)
            }
            #[inline]
            fn atomic_fetch_and(&self, value: $value, order: Ordering) -> $value {
                self.fetch_and(value, order)
            }
            #[inline]
            fn atomic_fetch_or(&self, value: $value, order: Ordering) -> $value {
                self.fetch_or(value, order)
            }
        }
    };
}

impl_atomic_int!(AtomicUsize, usize);
impl_atomic_int!(AtomicIsize, isize);
impl_atomic_int!(AtomicU32, u32);
impl_atomic_int!(AtomicI32, i32);
#[cfg(target_has_atomic = "64")]
impl_atomic_int!(AtomicU64, u64);
#[cfg(target_has_atomic = "64")]
impl_atomic_int!(AtomicI64, i64);

impl sealed::Sealed for AtomicBool {}

impl Atomic for AtomicBool {
    type Value = bool;

    #[inline]
    fn new_atomic(value: bool) -> Self {
        Self::new(value)
    }
    #[inline]
    fn atomic_load(&self, order: Ordering) -> bool {
        self.load(order)
    }
    #[inline]
    fn atomic_store(&self, value: bool, order: Ordering) {
        self.store(value, order);
    }
    #[inline]
    fn atomic_swap(&self, value: bool, order: Ordering) -> bool {
        self.swap(value, order)
    }
    #[inline]
    fn atomic_compare_exchange(
        &self,
        current: bool,
        new: bool,
        success: Ordering,
        failure: Ordering,
    ) -> Result<bool, bool> {
        self.compare_exchange(current, new, success, failure)
    }
    #[inline]
    fn atomic_get_mut(&mut self) -> &mut bool {
        self.get_mut()
    }
    #[inline]
    fn atomic_into_inner(self) -> bool {
        self.into_inner()
    }
    #[inline]
    fn value_ptr(&self) -> *mut bool {
        self as *const Self as *mut bool
    }
}
