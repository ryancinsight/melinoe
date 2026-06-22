//! Branded collection adapters backed by Melinoe primitives.

mod branded_deque;
mod branded_vec;

#[doc(inline)]
pub use branded_deque::{BrandedVecDeque, BrandedVecDequeDrain};
#[doc(inline)]
pub use branded_vec::{BrandedDrain, BrandedVec};
