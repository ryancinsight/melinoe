//! [`MelinoeCell`] — branded interior mutability with token-mediated access.

mod melinoe_cell;
mod reference;
mod slice;

#[cfg(feature = "alloc")]
mod cow;

#[cfg(feature = "alloc")]
pub(crate) use cow::cow_from_segments;
#[cfg(feature = "alloc")]
pub use cow::{Borrowed, CellCowExt, CowPolicy, RetainDecision, Retained};
pub use melinoe_cell::MelinoeCell;
pub use reference::{MelinoeMut, MelinoeRef};
pub use slice::CellSliceExt;
