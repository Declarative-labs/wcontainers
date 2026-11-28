//! What every implementation of memory-shard must do, written against its traits alone.
//!
//! Each suite function takes a newly made shard and the config it was made from; the contents of a shard are not in
//! the proof, so these functions are where they are checked.

pub use wcontainers_volatile_bytes_conformance;

use wcontainers_memory_shard::{BorrowError, MemoryPermissions, MemoryShard, MemoryShardConfig};
use wcontainers_volatile_bytes::VolatileBytes;

/// The length of one byte, the step between neighboring offsets.
const STEP: u64 = size_of::<u8>() as u64;

/// How many bytes the offset write changes, two so that a write one byte off shows at either end.
const PAIR: usize = size_of::<u16>();

/// The byte the tests write where they need one value.
const MARK: u8 = u8::MAX;

/// Returns the length of the shard, which the tests need to be at least two bytes that the host can address.
fn length_of<S: MemoryShard>(shard: &S) -> usize {
    let length =
        usize::try_from(shard.len()).expect("the shard is no longer than the host can address");
    assert!(
        shard.len() > STEP,
        "the suite needs a shard of at least two bytes"
    );
    length
}

/// Returns bytes that each differ from the bytes beside them and repeat every 255 bytes.
///
/// So a copy one byte off, or a power of two bytes off, shows.
fn number_bytes(count: usize) -> Vec<u8> {
    (1..=u8::MAX).cycle().take(count).collect()
}

/// Returns the bytes of `number_bytes` counting down, so that none is taken for the byte it replaces.
fn number_bytes_down(count: usize) -> Vec<u8> {
    (1..=u8::MAX).rev().cycle().take(count).collect()
}

/// Returns a host index as an offset into a shard.
fn offset_of(index: usize) -> u64 {
    u64::try_from(index).expect("a host index fits an offset")
}

/// Fills the shard with `number_bytes`, and returns them with a borrow from its middle and where that borrow starts.
///
/// The borrow ends one byte before the shard does, so it starts above zero and has bytes of the shard on both sides.
fn borrow_middle<S: MemoryShard>(shard: &S) -> (Vec<u8>, S::ByteView<'_>, usize) {
    let length = length_of(shard);
    let bytes = number_bytes(length);
    let whole = shard
        .borrow(u64::MIN, shard.len())
        .expect("a shard lends all its bytes");
    assert_eq!(whole.write(&bytes), length);
    let start = length / 2;
    let count = length - start - 1;
    assert!(count > 1, "the suite needs a shard of at least six bytes");
    assert_eq!(
        start + count + 1,
        length,
        "the borrow ends one byte before the shard does"
    );
    let view = shard
        .borrow(offset_of(start), offset_of(count))
        .expect("the middle of the shard lies inside");
    (bytes, view, start)
}

/// Returns whether two sets of permissions allow the same accesses.
fn permits_the_same(first: MemoryPermissions, second: MemoryPermissions) -> bool {
    (first.read, first.write, first.execute) == (second.read, second.write, second.execute)
}

/// Returns every byte of the shard, read through one borrow of all of it.
fn read_whole<S: MemoryShard>(shard: &S) -> Vec<u8> {
    let view = shard
        .borrow(u64::MIN, shard.len())
        .expect("a shard lends all its bytes");
    let mut read = vec![MARK; length_of(shard)];
    assert_eq!(view.read(&mut read), read.len());
    read
}

/// Checks that the shard has the length and the permissions its config asked for.
pub fn has_the_length_and_permissions_it_was_made_with<S: MemoryShard>(
    made: (&S, MemoryShardConfig),
) {
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
    assert_eq!(
        shard
            .borrow(u64::MIN, length)
            .expect("all of it lies inside")
            .len(),
        length
    );
    assert_eq!(
        shard
            .borrow(u64::MIN, u64::MIN)
            .expect("nothing at the start lies inside")
            .len(),
        u64::MIN
    );
    assert_eq!(
        shard
            .borrow(length, u64::MIN)
            .expect("nothing at the end lies inside")
            .len(),
        u64::MIN
    );
    assert_eq!(
        shard
            .borrow(STEP, length - STEP)
            .expect("all but the first byte lies inside")
            .len(),
        length - STEP
    );
    assert_eq!(
        shard
            .borrow(length - STEP, STEP)
            .expect("the last byte lies inside")
            .len(),
        STEP
    );
    assert_eq!(
        shard
            .borrow(STEP, STEP)
            .expect("the second byte lies inside")
            .len(),
        STEP
    );
}

/// Checks that a borrow reaching past the end is refused, also where the end of the range does not fit a `u64`.
pub fn refuses_every_range_past_the_end<S: MemoryShard>(made: (&S, MemoryShardConfig)) {
    let (shard, _) = made;
    let length = shard.len();
    let outside = [
        (u64::MIN, length + STEP),
        (length, STEP),
        (length + STEP, u64::MIN),
        (STEP, length),
        (u64::MAX, STEP),
        (STEP, u64::MAX),
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
    let bytes = number_bytes(length);
    let whole = shard
        .borrow(u64::MIN, shard.len())
        .expect("a shard lends all its bytes");
    assert_eq!(whole.write(&bytes), length);
    let tail = shard
        .borrow(STEP, shard.len() - STEP)
        .expect("all but the first byte lies inside");
    let mut read = vec![MARK; length - 1];
    assert_eq!(tail.read(&mut read), read.len());
    assert_eq!(read[..], bytes[1..]);
    assert_eq!(tail.write(&[MARK]), size_of::<u8>());
    let mut second = [u8::MIN; 2];
    assert_eq!(whole.read(&mut second), second.len());
    assert_eq!(second, [bytes[0], MARK]);
}

/// Checks that bytes written through one borrow are read back through a later borrow, once the first is gone.
pub fn keeps_bytes_between_borrows<S: MemoryShard>(made: (&S, MemoryShardConfig)) {
    let (shard, _) = made;
    let length = length_of(shard);
    let bytes = number_bytes(length);
    {
        let whole = shard
            .borrow(u64::MIN, shard.len())
            .expect("a shard lends all its bytes");
        assert_eq!(whole.write(&bytes), length);
    }
    assert_eq!(read_whole(shard), bytes);
    let last = shard
        .borrow(shard.len() - STEP, STEP)
        .expect("the last byte lies inside");
    let mut read = [MARK];
    assert_eq!(last.read(&mut read), size_of::<u8>());
    assert_eq!(read[0], bytes[length - 1]);
}

/// Checks that `read_at` and `read_to` on a borrow at an offset above zero read the shard's bytes from that offset on.
pub fn reads_from_where_a_borrow_starts<S: MemoryShard>(made: (&S, MemoryShardConfig)) {
    let (shard, _) = made;
    let (bytes, view, start) = borrow_middle(shard);
    let count = usize::try_from(view.len()).expect("the borrow is no longer than the shard");
    let end = start + count;
    let mut read = vec![MARK; count];
    assert_eq!(view.read_at(u64::MIN, &mut read), count);
    assert_eq!(
        read[..],
        bytes[start..end],
        "read_at 0 reads the shard's bytes from the borrow's start"
    );
    let mut read = vec![MARK; count];
    assert_eq!(view.read_at(STEP, &mut read), count - 1);
    assert_eq!(
        read[..count - 1],
        bytes[start + 1..end],
        "read_at 1 reads from one byte after the borrow's start"
    );
    assert_eq!(
        read[count - 1],
        MARK,
        "read_at leaves the rest of the buffer"
    );
    let mut sink = Vec::new();
    assert_eq!(view.read_to(&mut sink).ok(), Some(count));
    assert_eq!(
        sink[..],
        bytes[start..end],
        "read_to sends the borrow's bytes and no others"
    );
}

/// Checks that `write_at` and `write_from` on a borrow at an offset above zero change exactly the shard's bytes there.
pub fn writes_where_a_borrow_starts<S: MemoryShard>(made: (&S, MemoryShardConfig)) {
    let (shard, _) = made;
    let (mut expected, view, start) = borrow_middle(shard);
    let count = usize::try_from(view.len()).expect("the borrow is no longer than the shard");
    let end = start + count;
    let written = number_bytes_down(count);
    assert_eq!(view.write_at(STEP, &written[..PAIR]), PAIR);
    expected[start + 1..start + 1 + PAIR].copy_from_slice(&written[..PAIR]);
    assert_eq!(
        read_whole(shard),
        expected,
        "write_at 1 changes the two bytes one after the borrow's start"
    );
    let longer = number_bytes_down(count + size_of::<u64>());
    let mut source = longer.as_slice();
    assert_eq!(view.write_from(&mut source).ok(), Some(count));
    assert_eq!(
        source.len(),
        size_of::<u64>(),
        "write_from takes only the borrow's length from a longer source"
    );
    expected[start..end].copy_from_slice(&longer[..count]);
    assert_eq!(
        read_whole(shard),
        expected,
        "write_from changes the borrow's bytes and no others"
    );
    let mut tail: &[u8] = &[MARK];
    assert_eq!(view.write_from(&mut tail).ok(), Some(size_of::<u8>()));
    expected[start] = MARK;
    assert_eq!(
        read_whole(shard),
        expected,
        "a short write_from changes the borrow's first byte only"
    );
}

/// Returns a borrow of all of a shard that lives as long as the program, the value the volatile-bytes suite runs on.
pub fn whole_view_of<S: MemoryShard + 'static>(
    made: (&'static S, MemoryShardConfig),
) -> S::ByteView<'static> {
    let (shard, _) = made;
    shard
        .borrow(u64::MIN, shard.len())
        .expect("a shard lends all its bytes")
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

        #[test]
        fn reads_from_where_a_borrow_starts() {
            $crate::reads_from_where_a_borrow_starts($value);
        }

        #[test]
        fn writes_where_a_borrow_starts() {
            $crate::writes_where_a_borrow_starts($value);
        }

        mod volatile_bytes {
            $crate::wcontainers_volatile_bytes_conformance::tests!($crate::whole_view_of($value));
        }
    };
}
