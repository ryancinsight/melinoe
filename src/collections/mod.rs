//! Branded collection adapters backed by Melinoe primitives.

mod branded_vec;
mod deque;

#[doc(inline)]
pub use branded_vec::{with_generated, BrandedDrain, BrandedVec};
#[doc(inline)]
pub use deque::{BrandedVecDeque, BrandedVecDequeDrain};
