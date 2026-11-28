use wcontainers_memory_shard::{MemoryPermissions, MemoryShard};
use wcontainers_memory_shard_shared_allocation::{MOST_BYTES, SharedAllocationMemoryShard};
use wcontainers_volatile_bytes::VolatileBytes;

fn read_only() -> MemoryPermissions {
    MemoryPermissions { read: true, write: false, execute: false }
}

#[test]
fn a_new_shard_has_the_asked_length_and_permissions() {
    let shard = SharedAllocationMemoryShard::allocate(300, read_only()).expect("allocate");
    assert_eq!(shard.len(), 300);
    let permissions = shard.permissions();
    assert!(permissions.read && !permissions.write && !permissions.execute);
    assert!(SharedAllocationMemoryShard::allocate(MOST_BYTES + 1, read_only()).is_none());
}

#[test]
fn a_borrow_past_the_end_is_refused() {
    let shard = SharedAllocationMemoryShard::allocate(16, read_only()).expect("allocate");
    assert!(shard.borrow(8, 8).is_ok());
    assert!(shard.borrow(8, 9).is_err());
    assert!(shard.borrow(u64::MAX, 2).is_err());
}

#[test]
fn the_halves_of_a_split_and_a_duplicate_share_the_bytes() {
    let shard = SharedAllocationMemoryShard::allocate(16, read_only()).expect("allocate");
    let duplicate = shard.duplicate();
    let (first, second) = shard.split_at(4).expect("split");
    second.borrow(0, 2).expect("borrow").write(b"hi");
    let mut bytes = [0; 2];
    duplicate.borrow(4, 2).expect("borrow").read(&mut bytes);
    assert_eq!(&bytes, b"hi");
    assert_eq!(first.len(), 4);
    assert_eq!(second.len(), 12);
    assert!(shard.split_at(0).is_none());
    assert!(shard.split_at(16).is_none());
}

#[test]
fn a_read_at_the_end_copies_what_is_left() {
    let shard = SharedAllocationMemoryShard::allocate(8, read_only()).expect("allocate");
    let view = shard.borrow(2, 6).expect("borrow");
    assert_eq!(view.write_at(4, b"wxyz"), 2);
    let mut bytes = [7; 4];
    assert_eq!(view.read_at(4, &mut bytes), 2);
    assert_eq!(bytes, [b'w', b'x', 7, 7]);
    assert_eq!(view.read_at(9, &mut bytes), 0);
}
