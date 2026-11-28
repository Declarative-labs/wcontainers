//! Runs the memory-shard suite on a new shard and on a split's second half, and the memory table suite on the table.

use wcontainers_capability_table::CapabilityTable;
use wcontainers_memory_capability_table::MemoryCapabilityTable;
use wcontainers_memory_shard::{MemoryPermissions, MemoryShardConfig};
use wcontainers_memory_shard_shared_cells::{
    SharedCellsMemoryCapabilityTable, SharedCellsMemoryShard,
};

/// The length of the shards the suite runs on: three pages of four kibibytes.
///
/// https://developer.arm.com/documentation/101811/0104/Translation-granule
const LENGTH: u64 = 3 * 4096;

/// Returns the config the suite's shards are made from.
fn config() -> MemoryShardConfig {
    MemoryShardConfig {
        length: LENGTH,
        permissions: MemoryPermissions {
            read: true,
            write: true,
            execute: false,
        },
    }
}

/// Returns a new shard, made by a table that lives as long as the program, and the config it was made from.
fn made() -> (&'static SharedCellsMemoryShard, MemoryShardConfig) {
    let table: &'static mut SharedCellsMemoryCapabilityTable =
        Box::leak(Box::new(SharedCellsMemoryCapabilityTable::new()));
    assert!(table.create(u64::MIN, config()).is_ok());
    (
        table.get(&u64::MIN).expect("the table holds what it made"),
        config(),
    )
}

/// Returns the second half of a split shard twice the suite's length, and the config a shard of its shape is made from.
///
/// Its view starts inside its allocation, not at the start.
fn made_second_half() -> (&'static SharedCellsMemoryShard, MemoryShardConfig) {
    let table: &'static mut SharedCellsMemoryCapabilityTable =
        Box::leak(Box::new(SharedCellsMemoryCapabilityTable::new()));
    let whole = MemoryShardConfig {
        length: LENGTH + LENGTH,
        permissions: config().permissions,
    };
    assert!(table.create(u64::MIN, whole).is_ok());
    assert!(table.split(&u64::MIN, LENGTH).is_ok());
    (
        table
            .get(&LENGTH)
            .expect("the split holds its second half at the length"),
        config(),
    )
}

wcontainers_memory_shard_conformance::tests!(crate::made());

mod second_half {
    wcontainers_memory_shard_conformance::tests!(crate::made_second_half());
}

mod memory_capability_table {
    wcontainers_memory_capability_table_conformance::tests!(
        wcontainers_memory_shard_shared_cells::SharedCellsMemoryCapabilityTable::new()
    );
}
