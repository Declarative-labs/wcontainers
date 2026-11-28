//! What every implementation of memory-shard must do, written against its traits alone.
//!
//! Each suite function takes a newly made shard and the config it was made from; the contents of a shard are not in
//! the proof, so these functions are where they are checked.

pub use wcontainers_volatile_bytes_conformance;

use wcontainers_memory_shard::{BorrowError, MemoryPermissions, MemoryShard, MemoryShardConfig};
use wcontainers_volatile_bytes::VolatileBytes;

/// The length of one byte, the step between neighboring offsets.
const BYTE: u64 = size_of::<u8>() as u64;

/// The byte the tests write where they need one value.
const WRITTEN: u8 = u8::MAX;

/// Returns the length of the shard, which the tests need to be at least two bytes that the host can address.
fn length_of<S: MemoryShard>(shard: &S) -> usize {
    let length = usize::try_from(shard.len()).expect("the shard is no longer than the host can address");
    assert!(shard.len() > BYTE, "the suite needs a shard of at least two bytes");
    length
}

/// Returns bytes that each differ from the bytes beside them, so a copy one byte off shows.
fn numbered(count: usize) -> Vec<u8> {
    (0..count).map(|index| (index % usize::from(u8::MAX)) as u8).collect()
}

/// Returns whether two sets of permissions allow the same accesses.
fn permits_the_same(first: MemoryPermissions, second: MemoryPermissions) -> bool {
    first.read == second.read && first.write == second.write && first.execute == second.execute
}

/// Returns every byte of the shard, read through one borrow of all of it.
fn read_whole<S: MemoryShard>(shard: &S) -> Vec<u8> {
    let view = shard.borrow(u64::MIN, shard.len()).expect("a shard lends all its bytes");
    let mut read = vec![WRITTEN; length_of(shard)];
    assert_eq!(view.read(&mut read), read.len());
    read
}

/// Checks that the shard has the length and the permissions its config asked for.
pub fn has_the_length_and_permissions_it_was_made_with<S: MemoryShard>(made: (&S, MemoryShardConfig)) {
    let (shard, config) = made;
    assert_eq!(shard.len(), config.length);
    assert!(permits_the_same(shard.permissions(), config.permissions));
}

/// Checks that every byte of a new shard is zero.
pub fn reads_as_zeros_when_new<S: MemoryShard>(made: (&S, MemoryShardConfig)) {
    let (shard, _) = made;
    assert!(read_whole(shard).iter().all(|&byte| byte == u8::MIN));
}

/// Checks that a borrow of any range inside the shard succeeds with the length asked for.
pub fn lends_every_range_inside<S: MemoryShard>(made: (&S, MemoryShardConfig)) {
    let (shard, _) = made;
    let length = shard.len();
    assert_eq!(shard.borrow(u64::MIN, length).expect("all of it lies inside").len(), length);
    assert_eq!(shard.borrow(u64::MIN, u64::MIN).expect("nothing at the start lies inside").len(), u64::MIN);
    assert_eq!(shard.borrow(length, u64::MIN).expect("nothing at the end lies inside").len(), u64::MIN);
    assert_eq!(shard.borrow(BYTE, length - BYTE).expect("all but the first byte lies inside").len(), length - BYTE);
    assert_eq!(shard.borrow(length - BYTE, BYTE).expect("the last byte lies inside").len(), BYTE);
    assert_eq!(shard.borrow(BYTE, BYTE).expect("the second byte lies inside").len(), BYTE);
}

/// Checks that a borrow reaching past the end is refused, also where the end of the range does not fit a `u64`.
pub fn refuses_every_range_past_the_end<S: MemoryShard>(made: (&S, MemoryShardConfig)) {
    let (shard, _) = made;
    let length = shard.len();
    let outside = [
        (u64::MIN, length + BYTE),
        (length, BYTE),
        (length + BYTE, u64::MIN),
        (BYTE, length),
        (length - BYTE, BYTE + BYTE),
        (u64::MAX, BYTE),
        (BYTE, u64::MAX),
        (u64::MAX, u64::MAX),
        (u64::MAX, u64::MIN),
        (u64::MIN, u64::MAX),
    ];
    for (offset, count) in outside {
        assert!(
            matches!(shard.borrow(offset, count), Err(BorrowError::OutsideShard)),
            "a borrow of {count} bytes at {offset} lies outside"
        );
    }
}

/// Checks that two borrows of the shard are views of the same bytes, at the offsets they were borrowed at.
pub fn shares_bytes_between_borrows<S: MemoryShard>(made: (&S, MemoryShardConfig)) {
    let (shard, _) = made;
    let length = length_of(shard);
    let bytes = numbered(length);
    let whole = shard.borrow(u64::MIN, shard.len()).expect("a shard lends all its bytes");
    assert_eq!(whole.write(&bytes), length);
    let tail = shard.borrow(BYTE, shard.len() - BYTE).expect("all but the first byte lies inside");
    let mut read = vec![WRITTEN; length - 1];
    assert_eq!(tail.read(&mut read), read.len());
    assert_eq!(read[..], bytes[1..]);
    assert_eq!(tail.write(&[WRITTEN]), size_of::<u8>());
    let mut second = [u8::MIN; 2];
    assert_eq!(whole.read(&mut second), second.len());
    assert_eq!(second, [bytes[0], WRITTEN]);
}

/// Checks that bytes written through one borrow are read back through a later borrow, once the first is gone.
pub fn keeps_bytes_between_borrows<S: MemoryShard>(made: (&S, MemoryShardConfig)) {
    let (shard, _) = made;
    let length = length_of(shard);
    let bytes = numbered(length);
    {
        let whole = shard.borrow(u64::MIN, shard.len()).expect("a shard lends all its bytes");
        assert_eq!(whole.write(&bytes), length);
    }
    assert_eq!(read_whole(shard), bytes);
    let last = shard.borrow(shard.len() - BYTE, BYTE).expect("the last byte lies inside");
    let mut read = [WRITTEN];
    assert_eq!(last.read(&mut read), size_of::<u8>());
    assert_eq!(read[0], bytes[length - 1]);
}

/// Returns a borrow of all of a shard that lives as long as the program, the value the volatile-bytes suite runs on.
pub fn whole_view_of<S: MemoryShard + 'static>(made: (&'static S, MemoryShardConfig)) -> S::ByteView<'static> {
    let (shard, _) = made;
    shard.borrow(u64::MIN, shard.len()).expect("a shard lends all its bytes")
}

/// Runs each suite function above as its own test on the shard and config an implementation gives.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn has_the_length_and_permissions_it_was_made_with() {
            $crate::has_the_length_and_permissions_it_was_made_with($value);
        }

        #[test]
        fn reads_as_zeros_when_new() {
            $crate::reads_as_zeros_when_new($value);
        }

        #[test]
        fn lends_every_range_inside() {
            $crate::lends_every_range_inside($value);
        }

        #[test]
        fn refuses_every_range_past_the_end() {
            $crate::refuses_every_range_past_the_end($value);
        }

        #[test]
        fn shares_bytes_between_borrows() {
            $crate::shares_bytes_between_borrows($value);
        }

        #[test]
        fn keeps_bytes_between_borrows() {
            $crate::keeps_bytes_between_borrows($value);
        }

        mod volatile_bytes {
            $crate::wcontainers_volatile_bytes_conformance::tests!($crate::whole_view_of($value));
        }
    };
}
