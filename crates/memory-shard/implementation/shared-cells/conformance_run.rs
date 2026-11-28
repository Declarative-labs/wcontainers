//! Runs the memory-shard suite on shards a table makes, and kernel-core's memory table suite on the table.

use wcontainers_capability_table::CapabilityTable;
use wcontainers_memory_shard::{MemoryPermissions, MemoryShardConfig};
use wcontainers_memory_shard_shared_cells::{SharedCellsMemoryCapabilityTable, SharedCellsMemoryShard};

/// The length of the shards the suite runs on: three pages of four kibibytes.
///
/// https://developer.arm.com/documentation/101811/0104/Translation-granule
const LENGTH: u64 = 3 * 4096;

/// Returns the config the suite's shards are made from.
fn config() -> MemoryShardConfig {
    MemoryShardConfig { length: LENGTH, permissions: MemoryPermissions { read: true, write: true, execute: false } }
}

/// Returns a new shard, made by a table that lives as long as the program, and the config it was made from.
fn made() -> (&'static SharedCellsMemoryShard, MemoryShardConfig) {
    let table: &'static mut SharedCellsMemoryCapabilityTable = Box::leak(Box::default());
    assert!(table.create(u64::MIN, config()).is_ok());
    (table.get(&u64::MIN).expect("the table holds what it made"), config())
}

wcontainers_memory_shard_conformance::tests!(crate::made());

mod memory_capability_table {
    wcontainers_kernel_core_conformance::tests!(
        wcontainers_memory_shard_shared_cells::SharedCellsMemoryCapabilityTable::default()
    );
}
