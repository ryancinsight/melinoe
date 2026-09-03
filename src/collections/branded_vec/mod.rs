//! `Vec` storage whose element access is gated by a Melinoe brand permit.

mod generation;
mod iter;
mod ops;
#[cfg(feature = "std")]
mod partition;
mod vec;
mod views;

#[doc(inline)]
pub use generation::with_generated;
#[doc(inline)]
pub use iter::BrandedDrain;
pub use vec::BrandedVec;
