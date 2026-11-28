//! What every implementation of a task's memory table must do, written against the traits of kernel-core alone.
//!
//! The clauses of `MemoryCapabilityTable` say nothing of a shard's bytes, so a table that loses them or stops sharing
//! them still proves; these functions check the bytes as well as every clause. Each takes new tables holding nothing.

use wcontainers_capability_table::{CreateError, DuplicateError};
use wcontainers_kernel_core::memory::{MemoryCapabilityTable, MemoryError, SetPermissionsError, SplitError};
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard, MemoryShardConfig};
use wcontainers_volatile_bytes::VolatileBytes;

/// The length of a page of the smallest translation granule both Arm64 and x86-64 have, the unit the tests hold.
///
/// https://developer.arm.com/documentation/101811/0104/Translation-granule
const PAGE: u64 = 4096;

/// The length of most shards the tests hold: three pages of the granule, so that a shard has a first, a middle and a last page.
///
/// https://developer.arm.com/documentation/101811/0104/Translation-granule
const SHARD: u64 = 12288;

/// The length of one byte, the least an overlap can be.
const BYTE_LENGTH: u64 = size_of::<u8>() as u64;

/// Where the tests hold their first shard: the lowest address Linux lets a program map by default.
///
/// https://www.kernel.org/doc/html/latest/admin-guide/sysctl/vm.html#mmap-min-addr
const BASE: u64 = 65536;

/// The second page of a shard of `SHARD` bytes at `BASE`.
const SECOND_PAGE: u64 = BASE + PAGE;

/// The third page of a shard of `SHARD` bytes at `BASE`.
const THIRD_PAGE: u64 = SECOND_PAGE + PAGE;

/// Where the tests hold a second shard, far from the first: where Linux loads a position-independent x86-64 program.
///
/// https://elixir.bootlin.com/linux/v6.12/source/arch/x86/include/asm/elf.h#L238
const FAR: u64 = 0x5555_5555_4000;

const READ_WRITE: MemoryPermissions = MemoryPermissions { read: true, write: true, execute: false };

const READ_ONLY: MemoryPermissions = MemoryPermissions { read: true, write: false, execute: false };

const READ_EXECUTE: MemoryPermissions = MemoryPermissions { read: true, write: false, execute: true };

/// Returns the config of a shard of the length held with the permissions.
fn config(length: u64, permissions: MemoryPermissions) -> MemoryShardConfig {
    MemoryShardConfig { length, permissions }
}

/// Returns whether two sets of permissions allow the same accesses.
fn permits_the_same(first: MemoryPermissions, second: MemoryPermissions) -> bool {
    (first.read, first.write, first.execute) == (second.read, second.write, second.execute)
}

/// Holds a new shard of the length at the address, which must succeed.
fn hold<S: MemoryShard, T: MemoryCapabilityTable<S>>(table: &mut T, address: u64, length: u64, permissions: MemoryPermissions) {
    assert!(table.create(address, config(length, permissions)).is_ok(), "a shard of {length} bytes fits at {address}");
}

/// Returns the shard held at the address, which must be there.
fn shard_at<S: MemoryShard, T: MemoryCapabilityTable<S>>(table: &T, address: u64) -> &S {
    table.get(&address).expect("a shard is held at the address")
}

/// Returns the length of a shard as a host length.
fn host_length<S: MemoryShard>(shard: &S) -> usize {
    usize::try_from(shard.len()).expect("the shard is no longer than the host can address")
}

/// Returns bytes counting up from one, none of them zero, each different from its neighbors.
fn number_bytes(count: usize) -> Vec<u8> {
    (1..=u8::MAX).cycle().take(count).collect()
}

/// Returns the bytes of `number_bytes` in the other order, so that no write of one is taken for a write of the other.
fn number_bytes_down(count: usize) -> Vec<u8> {
    let mut bytes = number_bytes(count);
    bytes.reverse();
    bytes
}

/// Writes the bytes the pattern gives over all of a shard and returns them.
fn fill<S: MemoryShard>(shard: &S, pattern: fn(usize) -> Vec<u8>) -> Vec<u8> {
    let bytes = pattern(host_length(shard));
    let view = shard.borrow(u64::MIN, shard.len()).expect("a shard lends all its bytes");
    assert_eq!(view.write(&bytes), bytes.len());
    bytes
}

/// Returns every byte of a shard.
fn read_whole<S: MemoryShard>(shard: &S) -> Vec<u8> {
    let view = shard.borrow(u64::MIN, shard.len()).expect("a shard lends all its bytes");
    let mut read = vec![u8::MIN; host_length(shard)];
    assert_eq!(view.read(&mut read), read.len());
    read
}

/// Writes the bytes into a shard at the offset.
fn write_at<S: MemoryShard>(shard: &S, offset: u64, bytes: &[u8]) {
    let length = u64::try_from(bytes.len()).expect("a host length fits a u64");
    let view = shard.borrow(offset, length).expect("the bytes lie inside the shard");
    assert_eq!(view.write(bytes), bytes.len());
}

/// Returns the bytes of a shard from the offset, as many as asked.
fn read_at<S: MemoryShard>(shard: &S, offset: u64, count: usize) -> Vec<u8> {
    let length = u64::try_from(count).expect("a host length fits a u64");
    let view = shard.borrow(offset, length).expect("the bytes lie inside the shard");
    let mut read = vec![u8::MIN; count];
    assert_eq!(view.read(&mut read), count);
    read
}

/// Returns the length of a page as a host length.
fn page() -> usize {
    usize::try_from(PAGE).expect("a page fits a host length")
}

/// Checks that a create holds a shard of the length and permissions asked for at the address, and at no other.
pub fn creates_a_shard_as_its_config_asks<S: MemoryShard, T: MemoryCapabilityTable<S>>(mut table: T) {
    assert!(table.create(BASE, config(SHARD, READ_WRITE)).is_ok());
    assert!(table.create(FAR, config(PAGE, READ_EXECUTE)).is_ok());
    let first = shard_at(&table, BASE);
    assert_eq!(first.len(), SHARD);
    assert!(permits_the_same(first.permissions(), READ_WRITE));
    let second = shard_at(&table, FAR);
    assert_eq!(second.len(), PAGE);
    assert!(permits_the_same(second.permissions(), READ_EXECUTE));
    assert!(table.get(&SECOND_PAGE).is_none(), "an address inside a shard is not where it is held");
    assert!(table.get(&u64::MIN).is_none());
}

/// Checks that a create at a held address is refused and leaves the shard there as it was, bytes and all.
pub fn refuses_a_taken_address<S: MemoryShard, T: MemoryCapabilityTable<S>>(mut table: T) {
    hold(&mut table, BASE, PAGE, READ_WRITE);
    let bytes = fill(shard_at(&table, BASE), number_bytes);
    assert!(matches!(table.create(BASE, config(SHARD, READ_ONLY)), Err(CreateError::IdentifierTaken)));
    let shard = shard_at(&table, BASE);
    assert_eq!(shard.len(), PAGE);
    assert!(permits_the_same(shard.permissions(), READ_WRITE));
    assert_eq!(read_whole(shard), bytes);
}

/// Checks that a create sharing even one byte with a held shard is refused as an overlap and holds nothing, while one
/// beside it succeeds.
pub fn refuses_a_create_that_overlaps<S: MemoryShard, T: MemoryCapabilityTable<S>>(mut table: T) {
    hold(&mut table, SECOND_PAGE, PAGE, READ_WRITE);
    let overlapping = [
        (BASE, SHARD),
        (BASE, PAGE + BYTE_LENGTH),
        (THIRD_PAGE - BYTE_LENGTH, PAGE),
        (SECOND_PAGE + BYTE_LENGTH, BYTE_LENGTH),
    ];
    for (address, length) in overlapping {
        assert!(
            matches!(table.create(address, config(length, READ_WRITE)), Err(CreateError::Failed(MemoryError::Overlap))),
            "{length} bytes at {address} overlap the held shard"
        );
        assert!(table.get(&address).is_none(), "a refused create holds nothing");
    }
    hold(&mut table, BASE, PAGE, READ_ONLY);
    hold(&mut table, THIRD_PAGE, PAGE, READ_ONLY);
    assert_eq!(shard_at(&table, SECOND_PAGE).len(), PAGE);
}

/// Checks that a drop stops holding one entry only, does nothing where nothing is held, and frees the address.
pub fn drops_one_entry<S: MemoryShard, T: MemoryCapabilityTable<S>>(mut table: T) {
    hold(&mut table, BASE, PAGE, READ_WRITE);
    hold(&mut table, FAR, PAGE, READ_ONLY);
    let bytes = fill(shard_at(&table, FAR), number_bytes);
    table.drop(&BASE);
    assert!(table.get(&BASE).is_none());
    assert_eq!(read_whole(shard_at(&table, FAR)), bytes);
    table.drop(&BASE);
    table.drop(&(FAR + BYTE_LENGTH));
    assert_eq!(read_whole(shard_at(&table, FAR)), bytes);
    hold(&mut table, BASE, SHARD, READ_EXECUTE);
    assert!(permits_the_same(shard_at(&table, BASE).permissions(), READ_EXECUTE));
}

/// Checks that a new shard reads as zeros, also where a dropped shard of the same shape was, and over part of one.
pub fn reads_a_new_shard_as_zeros<S: MemoryShard, T: MemoryCapabilityTable<S>>(mut table: T) {
    hold(&mut table, BASE, SHARD, READ_WRITE);
    fill(shard_at(&table, BASE), number_bytes);
    table.drop(&BASE);
    hold(&mut table, BASE, SHARD, READ_WRITE);
    assert!(read_whole(shard_at(&table, BASE)).iter().all(|&byte| byte == u8::MIN));
    fill(shard_at(&table, BASE), number_bytes_down);
    table.drop(&BASE);
    hold(&mut table, SECOND_PAGE, PAGE, READ_WRITE);
    assert!(read_whole(shard_at(&table, SECOND_PAGE)).iter().all(|&byte| byte == u8::MIN));
    hold(&mut table, FAR, SHARD, READ_WRITE);
    assert!(read_whole(shard_at(&table, FAR)).iter().all(|&byte| byte == u8::MIN));
}

/// Checks that a new shard reads as zeros while a duplicate of a dropped shard still holds that shard's bytes.
pub fn keeps_bytes_while_a_duplicate_holds_them<S: MemoryShard, T: MemoryCapabilityTable<S>>(tables: (T, T)) {
    let (mut first, mut second) = tables;
    hold(&mut first, BASE, SHARD, READ_WRITE);
    let bytes = fill(shard_at(&first, BASE), number_bytes);
    assert!(second.duplicate_from(&first, &BASE, BASE).is_ok());
    first.drop(&BASE);
    assert_eq!(read_whole(shard_at(&second, BASE)), bytes);
    hold(&mut first, BASE, SHARD, READ_WRITE);
    assert!(read_whole(shard_at(&first, BASE)).iter().all(|&byte| byte == u8::MIN));
    fill(shard_at(&first, BASE), number_bytes_down);
    assert_eq!(read_whole(shard_at(&second, BASE)), bytes);
}

/// Checks a duplicate within a table: it is the same memory with the same permissions, and each refusal leaves the table.
pub fn duplicates_within_a_table<S: MemoryShard, T: MemoryCapabilityTable<S>>(mut table: T) {
    hold(&mut table, BASE, SHARD, READ_EXECUTE);
    let bytes = fill(shard_at(&table, BASE), number_bytes);
    assert!(table.duplicate_within(&BASE, FAR).is_ok());
    let duplicate = shard_at(&table, FAR);
    assert_eq!(duplicate.len(), SHARD);
    assert!(permits_the_same(duplicate.permissions(), READ_EXECUTE));
    assert_eq!(read_whole(duplicate), bytes);
    let written = number_bytes_down(page());
    write_at(shard_at(&table, FAR), PAGE, &written);
    assert_eq!(read_at(shard_at(&table, BASE), PAGE, page()), written);
    write_at(shard_at(&table, BASE), u64::MIN, &written);
    assert_eq!(read_at(shard_at(&table, FAR), u64::MIN, page()), written);
    assert!(matches!(table.duplicate_within(&SECOND_PAGE, u64::MIN), Err(DuplicateError::NothingHeld)));
    assert!(table.get(&u64::MIN).is_none());
    assert!(matches!(table.duplicate_within(&BASE, FAR), Err(DuplicateError::IdentifierTaken)));
    assert!(matches!(
        table.duplicate_within(&BASE, SECOND_PAGE),
        Err(DuplicateError::Failed(MemoryError::Overlap))
    ));
    assert!(table.get(&SECOND_PAGE).is_none());
    let after = BASE + SHARD;
    assert!(table.duplicate_within(&BASE, after).is_ok());
    assert_eq!(read_whole(shard_at(&table, after)), read_whole(shard_at(&table, BASE)));
}

/// Checks a duplicate from another table: the same memory with the same permissions, the other table left as it was.
pub fn duplicates_from_another_table<S: MemoryShard, T: MemoryCapabilityTable<S>>(tables: (T, T)) {
    let (mut first, mut second) = tables;
    hold(&mut first, BASE, SHARD, READ_ONLY);
    hold(&mut second, FAR, PAGE, READ_WRITE);
    let bytes = fill(shard_at(&first, BASE), number_bytes);
    assert!(second.duplicate_from(&first, &BASE, BASE).is_ok());
    let duplicate = shard_at(&second, BASE);
    assert_eq!(duplicate.len(), SHARD);
    assert!(permits_the_same(duplicate.permissions(), READ_ONLY));
    assert_eq!(read_whole(duplicate), bytes);
    let written = number_bytes_down(page());
    write_at(shard_at(&second, BASE), PAGE, &written);
    assert_eq!(read_at(shard_at(&first, BASE), PAGE, page()), written);
    assert!(matches!(second.duplicate_from(&first, &FAR, u64::MIN), Err(DuplicateError::NothingHeld)));
    assert!(second.get(&u64::MIN).is_none());
    assert!(matches!(second.duplicate_from(&first, &BASE, FAR), Err(DuplicateError::IdentifierTaken)));
    assert_eq!(shard_at(&second, FAR).len(), PAGE);
    let below_far = FAR - PAGE;
    assert!(matches!(
        second.duplicate_from(&first, &BASE, below_far),
        Err(DuplicateError::Failed(MemoryError::Overlap))
    ));
    assert!(second.get(&below_far).is_none());
    assert!(first.get(&FAR).is_none(), "a duplicate from another table changes only the table it is made in");
    assert_eq!(shard_at(&first, BASE).len(), SHARD);
}

/// Checks that new permissions change one entry, keep its length and bytes, and touch no other entry.
pub fn sets_the_permissions_of_one_entry<S: MemoryShard, T: MemoryCapabilityTable<S>>(mut table: T) {
    hold(&mut table, BASE, SHARD, READ_WRITE);
    hold(&mut table, FAR, PAGE, READ_WRITE);
    let bytes = fill(shard_at(&table, BASE), number_bytes);
    assert!(table.set_permissions(&BASE, READ_ONLY).is_ok());
    let shard = shard_at(&table, BASE);
    assert_eq!(shard.len(), SHARD);
    assert!(permits_the_same(shard.permissions(), READ_ONLY));
    assert_eq!(read_whole(shard), bytes);
    assert!(permits_the_same(shard_at(&table, FAR).permissions(), READ_WRITE));
    assert!(table.set_permissions(&BASE, READ_EXECUTE).is_ok());
    assert!(permits_the_same(shard_at(&table, BASE).permissions(), READ_EXECUTE));
    assert!(matches!(table.set_permissions(&SECOND_PAGE, READ_ONLY), Err(SetPermissionsError::NothingHeld)));
    assert!(table.get(&SECOND_PAGE).is_none());
    assert!(permits_the_same(shard_at(&table, BASE).permissions(), READ_EXECUTE));
}

/// Checks that new permissions on an entry leave a duplicate's own permissions, and that both still share their bytes.
pub fn sets_permissions_without_touching_a_duplicate<S: MemoryShard, T: MemoryCapabilityTable<S>>(tables: (T, T)) {
    let (mut first, mut second) = tables;
    hold(&mut first, BASE, SHARD, READ_WRITE);
    fill(shard_at(&first, BASE), number_bytes);
    assert!(second.duplicate_from(&first, &BASE, BASE).is_ok());
    assert!(first.duplicate_within(&BASE, FAR).is_ok());
    assert!(first.set_permissions(&BASE, READ_ONLY).is_ok());
    assert!(permits_the_same(shard_at(&second, BASE).permissions(), READ_WRITE));
    assert!(permits_the_same(shard_at(&first, FAR).permissions(), READ_WRITE));
    assert!(permits_the_same(shard_at(&first, BASE).permissions(), READ_ONLY));
    let written = number_bytes_down(page());
    write_at(shard_at(&second, BASE), PAGE, &written);
    assert_eq!(read_at(shard_at(&first, BASE), PAGE, page()), written);
    assert_eq!(read_at(shard_at(&first, FAR), PAGE, page()), written);
    write_at(shard_at(&first, BASE), u64::MIN, &written);
    assert_eq!(read_at(shard_at(&second, BASE), u64::MIN, page()), written);
    assert!(second.set_permissions(&BASE, READ_EXECUTE).is_ok());
    assert!(permits_the_same(shard_at(&first, BASE).permissions(), READ_ONLY));
}

/// Checks that a split makes two shards with the permissions of the one, of the lengths asked, over its bytes.
pub fn splits_a_shard_in_two<S: MemoryShard, T: MemoryCapabilityTable<S>>(mut table: T) {
    hold(&mut table, BASE, SHARD, READ_EXECUTE);
    hold(&mut table, FAR, PAGE, READ_WRITE);
    let bytes = fill(shard_at(&table, BASE), number_bytes);
    assert!(table.split(&BASE, PAGE).is_ok());
    let first = shard_at(&table, BASE);
    assert_eq!(first.len(), PAGE);
    assert!(permits_the_same(first.permissions(), READ_EXECUTE));
    assert_eq!(read_whole(first), bytes[..page()]);
    let second = shard_at(&table, SECOND_PAGE);
    assert_eq!(second.len(), SHARD - PAGE);
    assert!(permits_the_same(second.permissions(), READ_EXECUTE));
    assert_eq!(read_whole(second), bytes[page()..]);
    assert_eq!(shard_at(&table, FAR).len(), PAGE);
    assert!(table.get(&THIRD_PAGE).is_none());
}

/// Checks that a split anywhere but strictly inside the shard, or where nothing is held, is refused and changes nothing.
pub fn refuses_a_split_outside_the_shard<S: MemoryShard, T: MemoryCapabilityTable<S>>(mut table: T) {
    hold(&mut table, BASE, SHARD, READ_WRITE);
    let bytes = fill(shard_at(&table, BASE), number_bytes);
    for offset in [u64::MIN, SHARD, u64::MAX] {
        assert!(matches!(table.split(&BASE, offset), Err(SplitError::OutsideShard)), "{offset} is outside");
        assert_eq!(shard_at(&table, BASE).len(), SHARD);
    }
    assert!(matches!(table.split(&SECOND_PAGE, PAGE), Err(SplitError::NothingHeld)));
    assert!(table.get(&SECOND_PAGE).is_none());
    assert!(table.get(&THIRD_PAGE).is_none());
    assert_eq!(shard_at(&table, BASE).len(), SHARD);
    assert_eq!(read_whole(shard_at(&table, BASE)), bytes);
}

/// Checks that the halves of a split and an entry duplicated before it in another table go on sharing the bytes.
pub fn shares_bytes_across_a_split<S: MemoryShard, T: MemoryCapabilityTable<S>>(tables: (T, T)) {
    let (mut first, mut second) = tables;
    hold(&mut first, BASE, SHARD, READ_WRITE);
    let bytes = fill(shard_at(&first, BASE), number_bytes);
    assert!(second.duplicate_from(&first, &BASE, BASE).is_ok());
    assert!(first.split(&BASE, PAGE).is_ok());
    let whole = shard_at(&second, BASE);
    assert_eq!(whole.len(), SHARD);
    assert!(permits_the_same(whole.permissions(), READ_WRITE));
    assert_eq!(read_whole(whole), bytes);
    let written = number_bytes_down(page());
    write_at(shard_at(&first, SECOND_PAGE), PAGE, &written);
    assert_eq!(read_at(shard_at(&second, BASE), PAGE + PAGE, page()), written);
    write_at(shard_at(&second, BASE), u64::MIN, &written);
    assert_eq!(read_whole(shard_at(&first, BASE)), written);
    write_at(shard_at(&second, BASE), PAGE, &written);
    assert_eq!(read_at(shard_at(&first, SECOND_PAGE), u64::MIN, page()), written);
}

/// Checks that new permissions on one half of a split leave the other half and a duplicate, which go on sharing the bytes.
pub fn sets_permissions_on_a_half<S: MemoryShard, T: MemoryCapabilityTable<S>>(tables: (T, T)) {
    let (mut first, mut second) = tables;
    hold(&mut first, BASE, SHARD, READ_WRITE);
    assert!(second.duplicate_from(&first, &BASE, BASE).is_ok());
    assert!(first.split(&BASE, PAGE).is_ok());
    assert!(first.split(&SECOND_PAGE, PAGE).is_ok());
    let bytes = fill(shard_at(&second, BASE), number_bytes);
    assert!(first.set_permissions(&SECOND_PAGE, READ_ONLY).is_ok());
    assert!(permits_the_same(shard_at(&first, BASE).permissions(), READ_WRITE));
    assert!(permits_the_same(shard_at(&first, SECOND_PAGE).permissions(), READ_ONLY));
    assert!(permits_the_same(shard_at(&first, THIRD_PAGE).permissions(), READ_WRITE));
    assert!(permits_the_same(shard_at(&second, BASE).permissions(), READ_WRITE));
    assert_eq!(shard_at(&first, SECOND_PAGE).len(), PAGE);
    assert_eq!(read_whole(shard_at(&first, SECOND_PAGE)), bytes[page()..page() + page()]);
    assert_eq!(read_whole(shard_at(&first, THIRD_PAGE)), bytes[page() + page()..]);
    let written = number_bytes_down(page());
    write_at(shard_at(&first, SECOND_PAGE), u64::MIN, &written);
    assert_eq!(read_at(shard_at(&second, BASE), PAGE, page()), written);
}

/// Runs each suite function above as its own test on new tables an implementation gives.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn creates_a_shard_as_its_config_asks() {
            $crate::creates_a_shard_as_its_config_asks($value);
        }

        #[test]
        fn refuses_a_taken_address() {
            $crate::refuses_a_taken_address($value);
        }

        #[test]
        fn refuses_a_create_that_overlaps() {
            $crate::refuses_a_create_that_overlaps($value);
        }

        #[test]
        fn drops_one_entry() {
            $crate::drops_one_entry($value);
        }

        #[test]
        fn reads_a_new_shard_as_zeros() {
            $crate::reads_a_new_shard_as_zeros($value);
        }

        #[test]
        fn keeps_bytes_while_a_duplicate_holds_them() {
            $crate::keeps_bytes_while_a_duplicate_holds_them(($value, $value));
        }

        #[test]
        fn duplicates_within_a_table() {
            $crate::duplicates_within_a_table($value);
        }

        #[test]
        fn duplicates_from_another_table() {
            $crate::duplicates_from_another_table(($value, $value));
        }

        #[test]
        fn sets_the_permissions_of_one_entry() {
            $crate::sets_the_permissions_of_one_entry($value);
        }

        #[test]
        fn sets_permissions_without_touching_a_duplicate() {
            $crate::sets_permissions_without_touching_a_duplicate(($value, $value));
        }

        #[test]
        fn splits_a_shard_in_two() {
            $crate::splits_a_shard_in_two($value);
        }

        #[test]
        fn refuses_a_split_outside_the_shard() {
            $crate::refuses_a_split_outside_the_shard($value);
        }

        #[test]
        fn shares_bytes_across_a_split() {
            $crate::shares_bytes_across_a_split(($value, $value));
        }

        #[test]
        fn sets_permissions_on_a_half() {
            $crate::sets_permissions_on_a_half(($value, $value));
        }
    };
}
