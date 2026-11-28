//! Kani's harnesses for the range arithmetic, over every length, offset and buffer length.

use super::range;

#[kani::proof]
fn a_range_lies_exactly_inside_its_view() {
    let (length, offset, buffer): (usize, u64, usize) = kani::any();
    let (length_wide, offset_wide) = (length as u128, offset as u128);
    match range(length, offset, buffer) {
        Some((at, count)) => {
            assert_eq!(at as u128, offset_wide);
            assert_eq!(count as u128, (buffer as u128).min(length_wide - offset_wide));
            assert!(at as u128 + count as u128 <= length_wide);
        }
        None => assert!(offset_wide > length_wide),
    }
}

#[kani::proof]
fn a_range_is_refused_only_past_the_end() {
    let (length, offset, buffer): (usize, u64, usize) = kani::any();
    assert_eq!(range(length, offset, buffer).is_none(), offset as u128 > length as u128);
}
