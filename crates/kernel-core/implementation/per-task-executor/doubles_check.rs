//! Checks the test table of sparse shards on the parts of a shard the kernel core suite does not reach.

mod doubles;

use wcontainers_capability_table::CapabilityTable;
use wcontainers_memory_capability_table::MemoryCapabilityTable;
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard, MemoryShardConfig};
use wcontainers_volatile_bytes::VolatileBytes;

use crate::doubles::{SparseShard, SparseTable};

const START: u64 = 0x100;
const LENGTH: u64 = 16;
const READ_WRITE: MemoryPermissions = MemoryPermissions {
    read: true,
    write: true,
    execute: false,
};

fn create_table() -> SparseTable {
    let mut table = SparseTable::default();
    assert!(
        table
            .create(
                START,
                MemoryShardConfig {
                    length: LENGTH,
                    permissions: READ_WRITE
                }
            )
            .is_ok()
    );
    table
}

fn get(table: &SparseTable, start: u64) -> &SparseShard {
    table.get(&start).expect("a shard at the start")
}

#[test]
fn lends_only_bytes_inside_the_shard() {
    let table = create_table();
    let shard = get(&table, START);
    let view = shard.borrow(8, 8).expect("the last eight bytes");
    assert_eq!(view.len(), 8);
    assert!(shard.borrow(8, 9).is_err());
}

#[test]
fn reads_and_writes_at_offsets_of_a_view() {
    let table = create_table();
    let shard = get(&table, START);
    let view = shard.borrow(4, 8).expect("eight bytes four in");
    assert_eq!(view.write_at(2, &[1, 2]), 2);
    let mut part = [0; 2];
    assert_eq!(view.read_at(2, &mut part), 2);
    assert_eq!(part, [1, 2]);
    let mut whole = [u8::MAX; LENGTH as usize];
    assert_eq!(
        shard
            .borrow(0, LENGTH)
            .expect("the whole shard")
            .read_at(0, &mut whole),
        whole.len()
    );
    assert_eq!(whole, [0, 0, 0, 0, 0, 0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn streams_a_whole_view() {
    let table = create_table();
    let view = get(&table, START).borrow(4, 4).expect("four bytes four in");
    assert!(matches!(view.write_from(&mut &[9, 8, 7, 6][..]), Ok(4)));
    let mut sink = Vec::new();
    assert!(matches!(view.read_to(&mut sink), Ok(4)));
    assert_eq!(sink, [9, 8, 7, 6]);
}

#[test]
fn splits_into_views_of_the_same_bytes() {
    let mut table = create_table();
    assert_eq!(
        get(&table, START)
            .borrow(10, 1)
            .expect("one byte ten in")
            .write_at(0, &[5]),
        1
    );
    assert!(table.split(&START, 8).is_ok());
    let mut byte = [0];
    assert_eq!(
        get(&table, START + 8)
            .borrow(2, 1)
            .expect("one byte two in")
            .read_at(0, &mut byte),
        1
    );
    assert_eq!(byte, [5]);
}
