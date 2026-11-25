//! Kani's harnesses for the range arithmetic, over every length, count and offset.

use super::{inside, part};

#[kani::proof]
fn inside_accepts_exactly_the_bytes_inside() {
    let (length, count): (usize, usize) = kani::any();
    assert_eq!(inside(length, count), (count as u128 <= length as u128).then_some(count));
}

#[kani::proof]
fn a_part_lies_exactly_inside_its_view() {
    let (length, from, to): (usize, u64, u64) = kani::any();
    match part(length, from..to) {
        Some((at, count)) => {
            assert!(from <= to && to <= length as u64);
            assert_eq!(at as u64, from);
            assert_eq!(count as u64, to - from);
            assert!(at as u128 + count as u128 <= length as u128);
        }
        None => assert!(from > to || to > length as u64),
    }
}
