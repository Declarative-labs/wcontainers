use wcontainers_capability_table::{CapabilityTable, CreateError};
use wcontainers_memory_capability_table::{MemoryCapabilityTable, MemoryError, SplitError};
use wcontainers_memory_capability_table_ordered::OrderedMemoryCapabilityTable;
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard, MemoryShardConfig};
use wcontainers_volatile_bytes::VolatileBytes;

const PAGE: u64 = 4096;
const READ_WRITE: MemoryPermissions = MemoryPermissions {
    read: true,
    write: true,
    execute: false,
};
const READ_ONLY: MemoryPermissions = MemoryPermissions {
    read: true,
    write: false,
    execute: false,
};

fn config(length: u64) -> MemoryShardConfig {
    MemoryShardConfig {
        length,
        permissions: READ_WRITE,
    }
}

fn table_with_shard(address: u64, length: u64) -> OrderedMemoryCapabilityTable {
    let mut table = OrderedMemoryCapabilityTable::new();
    assert!(table.create(address, config(length)).is_ok());
    table
}

fn write(table: &OrderedMemoryCapabilityTable, address: u64, offset: u64, bytes: &[u8]) {
    let shard = table.get(&address).expect("a shard at the address");
    let view = shard.borrow(offset, bytes.len() as u64).expect("bytes inside the shard");
    assert_eq!(view.write(bytes), bytes.len());
}

fn read(table: &OrderedMemoryCapabilityTable, address: u64, offset: u64, length: usize) -> Vec<u8> {
    let shard = table.get(&address).expect("a shard at the address");
    let view = shard.borrow(offset, length as u64).expect("bytes inside the shard");
    let mut bytes = vec![0; length];
    assert_eq!(view.read(&mut bytes), length);
    bytes
}

fn same(first: MemoryPermissions, second: MemoryPermissions) -> bool {
    first.read == second.read && first.write == second.write && first.execute == second.execute
}

#[test]
fn a_new_shard_has_the_asked_length_and_permissions() {
    let mut table = OrderedMemoryCapabilityTable::new();
    let asked = MemoryShardConfig {
        length: 3 * PAGE,
        permissions: READ_ONLY,
    };
    assert!(table.create(PAGE, asked).is_ok());
    let shard = table.get(&PAGE).expect("the new shard");
    assert_eq!(shard.len(), 3 * PAGE);
    assert!(same(shard.permissions(), READ_ONLY));
    assert_eq!(read(&table, PAGE, 0, 16), vec![0; 16]);
}

#[test]
fn a_create_at_a_taken_address_or_over_a_held_shard_is_refused() {
    let mut table = table_with_shard(PAGE, 2 * PAGE);
    assert!(matches!(
        table.create(PAGE, config(PAGE)),
        Err(CreateError::IdentifierTaken)
    ));
    assert!(matches!(
        table.create(2 * PAGE, config(PAGE)),
        Err(CreateError::Failed(MemoryError::Overlap))
    ));
    assert!(matches!(
        table.create(0, config(PAGE + 1)),
        Err(CreateError::Failed(MemoryError::Overlap))
    ));
    assert!(table.create(0, config(PAGE)).is_ok());
    assert!(table.create(3 * PAGE, config(PAGE)).is_ok());
}

#[test]
fn bytes_written_through_an_entry_are_read_through_its_duplicate_in_another_table() {
    let first = table_with_shard(PAGE, PAGE);
    let mut second = OrderedMemoryCapabilityTable::new();
    assert!(second.duplicate_from(&first, &PAGE, 8 * PAGE).is_ok());
    write(&first, PAGE, 100, b"shared");
    assert_eq!(read(&second, 8 * PAGE, 100, 6), b"shared");
    write(&second, 8 * PAGE, 0, b"back");
    assert_eq!(read(&first, PAGE, 0, 4), b"back");
}

#[test]
fn bytes_written_before_set_permissions_are_read_after_it_and_a_duplicate_keeps_its_own() {
    let mut first = table_with_shard(PAGE, PAGE);
    let mut second = OrderedMemoryCapabilityTable::new();
    assert!(second.duplicate_from(&first, &PAGE, PAGE).is_ok());
    write(&first, PAGE, 10, b"kept");
    assert!(first.set_permissions(&PAGE, READ_ONLY).is_ok());
    assert_eq!(read(&first, PAGE, 10, 4), b"kept");
    assert!(same(first.get(&PAGE).unwrap().permissions(), READ_ONLY));
    assert!(same(second.get(&PAGE).unwrap().permissions(), READ_WRITE));
    assert_eq!(read(&second, PAGE, 10, 4), b"kept");
}

#[test]
fn bytes_written_before_split_are_read_through_both_halves_and_a_duplicate() {
    let mut first = table_with_shard(PAGE, 2 * PAGE);
    let mut second = OrderedMemoryCapabilityTable::new();
    assert!(second.duplicate_from(&first, &PAGE, PAGE).is_ok());
    write(&first, PAGE, PAGE - 2, b"edge");
    assert!(first.split(&PAGE, PAGE).is_ok());
    assert_eq!(first.get(&PAGE).unwrap().len(), PAGE);
    assert_eq!(first.get(&(2 * PAGE)).unwrap().len(), PAGE);
    assert_eq!(read(&first, PAGE, PAGE - 2, 2), b"ed");
    assert_eq!(read(&first, 2 * PAGE, 0, 2), b"ge");
    write(&first, 2 * PAGE, 2, b"!");
    assert_eq!(read(&second, PAGE, PAGE - 2, 5), b"edge!");
}

#[test]
fn a_split_at_zero_or_at_the_length_is_refused() {
    let mut table = table_with_shard(PAGE, 2 * PAGE);
    assert!(matches!(table.split(&PAGE, 0), Err(SplitError::OutsideShard)));
    assert!(matches!(
        table.split(&PAGE, 2 * PAGE),
        Err(SplitError::OutsideShard)
    ));
    assert_eq!(table.get(&PAGE).unwrap().len(), 2 * PAGE);
}

#[test]
fn a_dropped_entry_leaves_its_duplicate_usable() {
    let mut table = table_with_shard(PAGE, PAGE);
    assert!(table.duplicate_within(&PAGE, 4 * PAGE).is_ok());
    write(&table, PAGE, 0, b"alive");
    CapabilityTable::drop(&mut table, &PAGE);
    assert!(table.get(&PAGE).is_none());
    assert_eq!(read(&table, 4 * PAGE, 0, 5), b"alive");
    write(&table, 4 * PAGE, 0, b"still");
    assert_eq!(read(&table, 4 * PAGE, 0, 5), b"still");
}
