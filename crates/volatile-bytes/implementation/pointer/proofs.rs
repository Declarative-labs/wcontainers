//! Kani's harnesses for the range arithmetic, over every address, length, offset and buffer length.

use super::range;

#[kani::proof]
fn a_range_lies_exactly_inside_its_view() {
    let (address, length, offset, buffer): (usize, usize, u64, usize) = kani::any();
    let (start, length_wide, offset_wide) = (address as u128, length as u128, offset as u128);
    match range(address, length, offset, buffer) {
        Some((from, count)) => {
            assert!(offset_wide <= length_wide);
            assert_eq!(from as u128, start + offset_wide);
            assert_eq!(count as u128, (buffer as u128).min(length_wide - offset_wide));
            assert!(from as u128 + count as u128 <= start + length_wide);
        }
        None => assert!(offset_wide > length_wide || start + offset_wide > usize::MAX as u128),
    }
}

#[kani::proof]
fn a_range_from_bytes_that_fit_the_addresses_is_refused_only_past_the_end() {
    let (address, length, offset, buffer): (usize, usize, u64, usize) = kani::any();
    kani::assume(address.checked_add(length).is_some());
    assert_eq!(range(address, length, offset, buffer).is_none(), offset as u128 > length as u128);
}
