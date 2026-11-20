//! The tests every implementation of wcontainers-system runs, written against its traits alone.

use core::mem::size_of;

use wcontainers_kernel::TaskId;
use wcontainers_system::{Clocks, RandomSource, TaskIdSource};

/// The id today's kernel gives as the one before its first.
const BEFORE_FIRST_TASK_ID: TaskId = TaskId(u32::MIN);

/// The length of each random fill, the size of a `u128`.
const RANDOM_FILL_LENGTH: usize = size_of::<u128>();

/// Checks that an id handed out and not freed is not handed out next.
pub fn allocate_task_ids_apart<S: TaskIdSource>(source: S) {
    let first = source.allocate_task_id(BEFORE_FIRST_TASK_ID);
    let second = source.allocate_task_id(first);
    assert_ne!(first, second);
}

/// Checks that freeing one id does not let the next allocation hand out another still held.
pub fn free_task_id_alone<S: TaskIdSource>(source: S) {
    let first = source.allocate_task_id(BEFORE_FIRST_TASK_ID);
    let second = source.allocate_task_id(first);
    source.free_task_id(first);
    assert_ne!(source.allocate_task_id(second), second);
}

/// Checks that the monotonic clock never goes backwards between two reads.
pub fn read_monotonic_clock_forward<C: Clocks>(clocks: C) {
    let earlier = clocks.monotonic_nanoseconds();
    assert!(clocks.monotonic_nanoseconds() >= earlier);
}

/// Checks that two fills of random bytes differ.
pub fn fill_random_bytes_differently<R: RandomSource>(source: R) {
    let mut first = [u8::MIN; RANDOM_FILL_LENGTH];
    let mut second = [u8::MIN; RANDOM_FILL_LENGTH];
    source.fill_random_bytes(&mut first);
    source.fill_random_bytes(&mut second);
    assert_ne!(first, second);
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn allocate_task_ids_apart() {
            $crate::allocate_task_ids_apart($value);
        }

        #[test]
        fn free_task_id_alone() {
            $crate::free_task_id_alone($value);
        }

        #[test]
        fn read_monotonic_clock_forward() {
            $crate::read_monotonic_clock_forward($value);
        }

        #[test]
        fn fill_random_bytes_differently() {
            $crate::fill_random_bytes_differently($value);
        }
    };
}
