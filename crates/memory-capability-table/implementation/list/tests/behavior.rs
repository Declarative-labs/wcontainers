use wcontainers_capability_table::{CapabilityTable, CreateError};
use wcontainers_memory_capability_table::{MemoryCapabilityTable, MemoryError, SplitError};
use wcontainers_memory_capability_table_list::ListMemoryCapabilityTable;
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard, MemoryShardConfig};
use wcontainers_volatile_bytes::VolatileBytes;

const ADDRESS: u64 = 0x1000;
const LENGTH: u64 = 4096;
const OTHER_ADDRESS: u64 = 0x9000;

fn read_write() -> MemoryPermissions {
    MemoryPermissions { read: true, write: true, execute: false }
}

fn read_execute() -> MemoryPermissions {
    MemoryPermissions { read: true, write: false, execute: true }
}

fn config() -> MemoryShardConfig {
    MemoryShardConfig { length: LENGTH, permissions: read_write() }
}

fn permissions_of(table: &ListMemoryCapabilityTable, address: u64) -> (bool, bool, bool) {
    let permissions = table.get(&address).expect("a held shard").permissions();
    (permissions.read, permissions.write, permissions.execute)
}

fn write(table: &ListMemoryCapabilityTable, address: u64, offset: u64, bytes: &[u8]) {
    let shard = table.get(&address).expect("a held shard");
    let view = shard.borrow(offset, bytes.len() as u64).expect("bytes inside the shard");
    assert_eq!(view.write(bytes), bytes.len());
}

fn read(table: &ListMemoryCapabilityTable, address: u64, offset: u64, length: usize) -> Vec<u8> {
    let shard = table.get(&address).expect("a held shard");
    let view = shard.borrow(offset, length as u64).expect("bytes inside the shard");
    let mut bytes = vec![0; length];
    assert_eq!(view.read(&mut bytes), length);
    bytes
}

fn table_with_a_shard() -> ListMemoryCapabilityTable {
    let mut table = ListMemoryCapabilityTable::new();
    table.create(ADDRESS, config()).expect("create a shard");
    table
}

#[test]
fn a_new_shard_has_the_asked_length_and_permissions() {
    let table = table_with_a_shard();
    assert_eq!(table.get(&ADDRESS).expect("a held shard").len(), LENGTH);
    assert_eq!(permissions_of(&table, ADDRESS), (true, true, false));
}

#[test]
fn a_create_at_a_taken_address_is_refused() {
    let mut table = table_with_a_shard();
    assert!(matches!(table.create(ADDRESS, config()), Err(CreateError::IdentifierTaken)));
}

#[test]
fn an_overlapping_create_is_refused() {
    let mut table = table_with_a_shard();
    assert!(matches!(
        table.create(ADDRESS + LENGTH / 2, config()),
        Err(CreateError::Failed(MemoryError::Overlap))
    ));
    assert!(matches!(
        table.create(ADDRESS - LENGTH / 2, config()),
        Err(CreateError::Failed(MemoryError::Overlap))
    ));
    assert!(table.create(ADDRESS + LENGTH, config()).is_ok());
}

#[test]
fn bytes_written_through_one_entry_are_read_through_its_duplicate_in_another_table() {
    let first = table_with_a_shard();
    let mut second = ListMemoryCapabilityTable::new();
    second.duplicate_from(&first, &ADDRESS, OTHER_ADDRESS).expect("duplicate");
    write(&first, ADDRESS, 10, b"shared");
    assert_eq!(read(&second, OTHER_ADDRESS, 10, 6), b"shared");
    write(&second, OTHER_ADDRESS, 100, b"back");
    assert_eq!(read(&first, ADDRESS, 100, 4), b"back");
}

#[test]
fn bytes_written_before_set_permissions_are_read_after_it() {
    let mut first = table_with_a_shard();
    let mut second = ListMemoryCapabilityTable::new();
    second.duplicate_from(&first, &ADDRESS, OTHER_ADDRESS).expect("duplicate");
    write(&first, ADDRESS, 0, b"kept");
    first.set_permissions(&ADDRESS, read_execute()).expect("set permissions");
    assert_eq!(read(&first, ADDRESS, 0, 4), b"kept");
    assert_eq!(permissions_of(&first, ADDRESS), (true, false, true));
    assert_eq!(permissions_of(&second, OTHER_ADDRESS), (true, true, false));
    write(&second, OTHER_ADDRESS, 0, b"seen");
    assert_eq!(read(&first, ADDRESS, 0, 4), b"seen");
}

#[test]
fn bytes_written_before_split_are_read_through_both_halves() {
    let mut first = table_with_a_shard();
    let mut second = ListMemoryCapabilityTable::new();
    second.duplicate_from(&first, &ADDRESS, OTHER_ADDRESS).expect("duplicate");
    write(&first, ADDRESS, 2046, b"abcd");
    first.split(&ADDRESS, 2048).expect("split");
    assert_eq!(first.get(&ADDRESS).expect("first half").len(), 2048);
    assert_eq!(first.get(&(ADDRESS + 2048)).expect("second half").len(), 2048);
    assert_eq!(read(&first, ADDRESS, 2046, 2), b"ab");
    assert_eq!(read(&first, ADDRESS + 2048, 0, 2), b"cd");
    assert_eq!(permissions_of(&first, ADDRESS + 2048), (true, true, false));
    write(&first, ADDRESS + 2048, 1, b"X");
    assert_eq!(read(&second, OTHER_ADDRESS, 2046, 4), b"abcX");
}

#[test]
fn a_split_at_zero_or_at_the_length_is_refused() {
    let mut table = table_with_a_shard();
    assert!(matches!(table.split(&ADDRESS, 0), Err(SplitError::OutsideShard)));
    assert!(matches!(table.split(&ADDRESS, LENGTH), Err(SplitError::OutsideShard)));
    assert!(matches!(table.split(&OTHER_ADDRESS, 1), Err(SplitError::NothingHeld)));
    assert_eq!(table.get(&ADDRESS).expect("a held shard").len(), LENGTH);
}

#[test]
fn a_dropped_entry_leaves_its_duplicate_usable() {
    let mut first = table_with_a_shard();
    let mut second = ListMemoryCapabilityTable::new();
    second.duplicate_from(&first, &ADDRESS, OTHER_ADDRESS).expect("duplicate");
    write(&first, ADDRESS, 0, b"left");
    first.drop(&ADDRESS);
    assert!(first.get(&ADDRESS).is_none());
    assert_eq!(read(&second, OTHER_ADDRESS, 0, 4), b"left");
    write(&second, OTHER_ADDRESS, 0, b"more");
    assert_eq!(read(&second, OTHER_ADDRESS, 0, 4), b"more");
}

#[test]
fn a_duplicate_within_shares_the_bytes_and_refuses_an_overlap() {
    let mut table = table_with_a_shard();
    table.duplicate_within(&ADDRESS, OTHER_ADDRESS).expect("duplicate");
    write(&table, ADDRESS, 5, b"same");
    assert_eq!(read(&table, OTHER_ADDRESS, 5, 4), b"same");
    assert!(table.duplicate_within(&ADDRESS, ADDRESS + 1).is_err());
}
