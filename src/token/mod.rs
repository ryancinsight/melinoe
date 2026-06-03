//! The Melinoe token system: zero-sized, branded capability evidence.
//!
//! Every token is a ZST parameterised by an invariant `'brand` lifetime. The
//! brand fuses tokens to the [`MelinoeCell`](crate::MelinoeCell)s minted under
//! the same scope, and the token's Rust auto-traits (`Send`/`Sync`) encode its
//! thread-safety posture. Tokens differ only in *how many may exist* and *where
//! they may travel*; they share one unified permit interface
//! ([`ReadPermit`]/[`WritePermit`]).
//!
//! | Token | Cardinality | `Send`/`Sync` | Permits |
//! |-------|-------------|---------------|---------|
//! | [`ExclusiveToken`] | exactly one per brand | both | read + write |
//! | [`SharedReadToken`] | many (`Copy`) | both | read |
//! | [`ThreadLocalToken`](crate::sync::ThreadLocalToken) | one per brand | neither | read + write |
//! | [`SyncRegionToken`](crate::sync::SyncRegionToken) | one per brand | both | read + write |

mod brand;
pub(crate) mod capability;
mod exclusive;
mod shared;

pub(crate) use brand::InvariantLifetime;

pub use brand::brand_scope;
pub use capability::{ReadPermit, WritePermit};
pub use exclusive::ExclusiveToken;
pub use shared::SharedReadToken;
