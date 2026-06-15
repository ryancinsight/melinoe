//! Melinoe-backed branded collection adapters.
//!
//! `halo` sits above `melinoe`: it does not mint a second ghost-token system.
//! Collection storage is built from [`melinoe::MelinoeCell`], and access is
//! mediated by Melinoe's [`melinoe::ReadPermit`] / [`melinoe::WritePermit`]
//! traits. The current crate surface is intentionally narrow while the larger
//! upstream Halo repository is migrated one verified collection at a time.
//!
//! ```
//! use halo::BrandedVec;
//! use melinoe::brand_scope;
//!
//! brand_scope(|mut token| {
//!     let mut values = BrandedVec::from_iter([1_u32, 2, 3]);
//!     values.as_mut_slice(&mut token)[1] = 20;
//!     assert_eq!(values.as_slice(&token), &[1, 20, 3]);
//! });
//! ```

#![no_std]

extern crate alloc;

pub mod collections;

#[doc(inline)]
pub use collections::BrandedVec;

/// Common imports for Melinoe-backed Halo collection code.
pub mod prelude {
    #[doc(no_inline)]
    pub use melinoe::{brand_scope, ExclusiveToken, ReadPermit, SharedReadToken, WritePermit};

    #[doc(no_inline)]
    pub use crate::BrandedVec;
}
