//! `std`-gated drivers that run one [`WriterShard`] per thread concurrently.
//!
//! The module is split by responsibility: [`plan`] owns shard sizing policies,
//! [`executor`] owns custom scheduler registration, and [`driver`] owns the
//! zero-copy partition execution paths.

mod driver;
mod executor;
mod plan;

pub use driver::{
    partition_for_each, partition_for_each_available, partition_for_each_with, partition_map,
    partition_map_available, partition_map_with,
};
pub use executor::{register_parallel_executor, ParallelExecutorFn};
pub use plan::PartitionPlan;
