//! Kani's harnesses for the range arithmetic, over every length, start and count.

use super::{inside, part};

#[kani::proof]
fn inside_accepts_exactly_the_bytes_inside() {
    let (length, start, count): (usize, usize, usize) = kani::any();
    let fits = start as u128 + count as u128 <= length as u128;
    assert_eq!(inside(length, start, count), fits.then_some(start));
}

#[kani::proof]
fn a_part_lies_exactly_inside_its_view() {
    let (start, length, from, to): (usize, usize, u64, u64) = kani::any();
    kani::assume(start.checked_add(length).is_some());
    match part(start, length, from..to) {
        Some((at, count)) => {
            assert!(from <= to && to <= length as u64);
            assert_eq!(at, start + from as usize);
            assert_eq!(count, (to - from) as usize);
            assert!(at + count <= start + length);
        }
        None => assert!(from > to || to > length as u64),
    }
}
