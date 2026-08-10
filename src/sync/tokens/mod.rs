mod region;
mod thread_local;

#[cfg(feature = "std")]
pub(crate) use region::SyncRegionFamily;
pub use region::{sync_region_scope, SyncRegionToken};
pub use thread_local::{thread_local_scope, ThreadLocalToken};
