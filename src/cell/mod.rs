//! [`MelinoeCell`] — branded interior mutability with token-mediated access.

mod reference;
mod slice;

#[cfg(feature = "alloc")]
mod cow;

#[cfg(feature = "alloc")]
pub use cow::{Borrowed, CellCowExt, CowPolicy, RetainDecision, Retained};
pub use reference::{MelinoeMut, MelinoeRef};
pub use slice::CellSliceExt;

use core::cell::UnsafeCell;
use core::fmt;
use core::marker::PhantomData;

use crate::token::{InvariantLifetime, ReadPermit, WritePermit};

/// A cell whose interior mutability is unlocked only by a brand-matching token.
///
/// `MelinoeCell<'brand, T>` is the storage counterpart to the
/// [token system](crate::token): it holds a `T` but exposes it solely through
/// methods that demand a [`ReadPermit`] or [`WritePermit`] for the *same*
/// `'brand`. Because the brand is an invariant lifetime, a cell minted in one
/// [`brand_scope`](crate::brand_scope) can never be unlocked by another scope's
/// token.
///
/// Unlike [`RefCell`](core::cell::RefCell), there is **no runtime flag and no
/// `borrow` panic path**: the aliasing discipline is discharged entirely by the
/// borrow checker acting on the unique owning token, so access compiles to a
/// bare pointer dereference. Unlike [`Cell`](core::cell::Cell), it yields real
/// `&T`/`&mut T` references and works with non-`Copy` payloads.
///
/// # Layout
///
/// `#[repr(transparent)]` over `UnsafeCell<T>`, so a `&mut T` can be reborrowed
/// as `&mut MelinoeCell<'brand, T>` via [`from_mut`](Self::from_mut) at zero
/// cost—the basis for branding pre-existing allocator storage in place.
#[repr(transparent)]
pub struct MelinoeCell<'brand, T: ?Sized> {
    _brand: InvariantLifetime<'brand>,
    value: UnsafeCell<T>,
}

impl<'brand, T> MelinoeCell<'brand, T> {
    /// Wrap `value` in a cell branded with the ambient `'brand`.
    #[inline]
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self {
            _brand: PhantomData,
            value: UnsafeCell::new(value),
        }
    }

    /// Consume the cell, returning the contained value.
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }

    /// Replace the contents, returning the previous value. Requires a write
    /// permit; performs no allocation.
    #[inline]
    pub fn replace<'a, P>(&'a self, value: T, permit: P) -> T
    where
        P: WritePermit<'brand> + 'a,
    {
        core::mem::replace(&mut *self.borrow_mut(permit), value)
    }
}

impl<'brand, T: ?Sized> MelinoeCell<'brand, T> {
    /// Reborrow an exclusive reference as a branded cell, in place.
    ///
    /// Sound because `MelinoeCell` is `#[repr(transparent)]` over
    /// `UnsafeCell<T>`, which is itself `#[repr(transparent)]` over `T`.
    #[inline]
    #[must_use]
    pub fn from_mut(value: &mut T) -> &mut Self {
        // SAFETY: `Self` shares the layout of `T` (transparent over
        // `UnsafeCell<T>`, transparent over `T`). The unique `&mut T` becomes a
        // unique `&mut Self`; no aliasing is introduced.
        unsafe { &mut *(value as *mut T as *mut Self) }
    }

    /// Acquire a shared, branded view of the contents.
    ///
    /// Any [`ReadPermit`] for `'brand` suffices—a shared borrow of an owning
    /// token, or a [`SharedReadToken`](crate::SharedReadToken). The returned
    /// [`MelinoeRef`] borrows both the cell and the permit for `'a`.
    #[inline]
    pub fn borrow<'a, P>(&'a self, _permit: P) -> MelinoeRef<'a, 'brand, T>
    where
        P: ReadPermit<'brand> + 'a,
    {
        // The borrow the permit carries is held for `'a` by the `P: 'a` bound
        // and the returned guard's lifetime—no runtime use of `_permit` needed.
        // SAFETY: a live `ReadPermit<'brand>` proves (via the borrow checker on
        // the brand's unique token) that no `&mut` view of this brand can exist
        // for `'a`; therefore forming a `&T` here cannot alias a `&mut T`.
        MelinoeRef::new(unsafe { &*self.value.get() })
    }

    /// Acquire an exclusive, branded view of the contents.
    ///
    /// Requires a [`WritePermit`] for `'brand` (a `&mut` borrow of the brand's
    /// unique owning token). The returned [`MelinoeMut`] borrows both the cell
    /// and the permit for `'a`.
    #[inline]
    pub fn borrow_mut<'a, P>(&'a self, _permit: P) -> MelinoeMut<'a, 'brand, T>
    where
        P: WritePermit<'brand> + 'a,
    {
        // The exclusive borrow `_permit` carries is held for `'a` by the
        // `P: 'a` bound and the returned guard's lifetime.
        // SAFETY: a live `WritePermit<'brand>` is an exclusive borrow of the
        // brand's unique token; while it is held no other read or write permit
        // of this brand can exist, so this `&mut T` is unaliased for `'a`.
        MelinoeMut::new(unsafe { &mut *self.value.get() })
    }

    /// Acquire `&mut T` from unique ownership of the cell—no token required.
    ///
    /// `&mut self` already proves there are no other references to the cell, so
    /// access needs no capability evidence.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }
}

impl<'brand, T> MelinoeCell<'brand, T> {
    /// Reinterpret a branded cell slice as a single `UnsafeCell<[T]>`.
    ///
    /// This is the provenance-correct bridge for zero-copy slice views: because
    /// `MelinoeCell<'brand, T>` is `#[repr(transparent)]` over `UnsafeCell<T>`
    /// over `T`, `[MelinoeCell<'brand, T>]` and `UnsafeCell<[T]>` share a layout,
    /// and `UnsafeCell::get` yields a `*mut [T]` carrying interior-mutability
    /// provenance over the whole region (which a plain `&[_] -> *mut` cast would
    /// not). Used by [`CellSliceExt`](crate::cell::CellSliceExt).
    #[inline]
    pub(crate) fn slice_as_unsafe_cell(slice: &[Self]) -> &UnsafeCell<[T]> {
        let ptr = slice as *const [MelinoeCell<'brand, T>] as *const UnsafeCell<[T]>;
        // SAFETY: identical layout via the transparent chain above; the fat
        // pointer's length metadata is preserved by the cast.
        unsafe { &*ptr }
    }
}

// SAFETY: moving a cell moves its `T`; this is sound exactly when `T: Send`.
// The brand phantom is `Send + Sync` and irrelevant to the bound.
unsafe impl<'brand, T: ?Sized + Send> Send for MelinoeCell<'brand, T> {}

// SAFETY: `&MelinoeCell` can yield `&T` (to readers holding shared token
// borrows, possibly on several threads at once) and, after a token is moved to
// one thread, `&mut T` (to that single writer). Concurrent readers require
// `T: Sync`; transferring the writer requires `T: Send`. Both together are
// necessary and sufficient—identical to the `GhostCell` bound proven sound by
// RustBelt.
unsafe impl<'brand, T: ?Sized + Send + Sync> Sync for MelinoeCell<'brand, T> {}

impl<'brand, T: Default> Default for MelinoeCell<'brand, T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<'brand, T> From<T> for MelinoeCell<'brand, T> {
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<'brand, T: ?Sized> fmt::Debug for MelinoeCell<'brand, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MelinoeCell").finish_non_exhaustive()
    }
}
