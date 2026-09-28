//! Contract crate for Atlas device-buffer ownership transfer.
//!
//! It intentionally lives outside Melinoe's library dependency graph because
//! Hephaestus depends back on Melinoe through the Atlas memory stack.
//!
//! The contract surface is exercised end-to-end by
//! `tests/device_buffer.rs`: a real Hephaestus WGPU device/stream buffer is
//! moved across a `SyncRegionToken` handoff, and the fence is read back through
//! a `BrandedAtomic`. Keeping the library item-less is deliberate — the test is
//! the contract, and any re-export here would drag the device backend into
//! Melinoe's public graph.
#![deny(missing_docs)]
