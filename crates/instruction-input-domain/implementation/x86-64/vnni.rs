//! The critical values of AVX-VNNI's dot products: per doubleword lane an accumulator and the two sources' elements.
//!
//! Each element takes the edges of its byte or word (zero, one, the largest and least signed value, all ones, which
//! the unsigned bytes read as 255) in every pair, under each accumulator edge (zero, one, the signed extremes, minus
//! one); then the elements differing within a doubleword, so that each product is seen in its own place; then the
//! accumulators on both sides of each saturation threshold: the one the largest sum reaches exactly and one past it,
//! and the same for the least sum.

use alloc::vec::Vec;

use wcontainers_processor_architecture_x86_64::MXCSR_DEFAULT;
use wcontainers_processor_architecture_x86_64::vector::{Lane, join};

use crate::avx::AvxData;

/// Doubleword lanes in a 128-bit vector.
const LANES: usize = 4;

/// The accumulator, first source and second source of every lane, four lanes to a value.
pub(crate) fn generate_dot_accumulations(words: bool) -> Vec<AvxData> {
    let (lane, per_doubleword) = if words { (Lane::Word, 2) } else { (Lane::Byte, 4) };
    let elements: [u64; 5] = if words { [0, 1, 0x7FFF, 0x8000, 0xFFFF] } else { [0, 1, 0x7F, 0x80, 0xFF] };
    let accumulators: [u64; 5] = [0, 1, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF];
    let fill = |element: u64| join(lane, (0..per_doubleword).map(|_| element)) as u64;
    let mut lanes: Vec<[u64; 3]> = Vec::new();
    for &accumulator in &accumulators {
        for &first in &elements {
            for &second in &elements {
                lanes.push([accumulator, fill(first), fill(second)]);
            }
        }
    }
    let mixed_first = join(lane, (0..per_doubleword).map(|index| elements[4 - index % 4])) as u64;
    for rotation in 0..per_doubleword {
        let second = join(lane, (0..per_doubleword).map(|index| elements[(index + rotation) % 4 + 1])) as u64;
        lanes.push([0, mixed_first, second]);
    }
    let factor = |element: u64, unsigned: bool| if unsigned { element as i64 } else { to_signed(element, lane.to_bits()) };
    let sums: Vec<i64> = elements
        .iter()
        .flat_map(|&first| elements.iter().map(move |&second| (first, second)))
        .map(|(first, second)| factor(first, !words) * factor(second, false) * per_doubleword as i64)
        .collect();
    let (largest, least) = (sums.iter().copied().max().unwrap_or(0), sums.iter().copied().min().unwrap_or(0));
    let extreme = |sum: i64| elements.iter().flat_map(|&first| elements.iter().map(move |&second| (first, second))).find(|&(first, second)| factor(first, !words) * factor(second, false) * per_doubleword as i64 == sum);
    for (sum, edge, past) in [(largest, i64::from(i32::MAX), 1), (least, i64::from(i32::MIN), -1)] {
        let Some((first, second)) = extreme(sum) else { continue };
        for accumulator in [edge - sum, edge - sum + past] {
            if (i64::from(i32::MIN)..=i64::from(i32::MAX)).contains(&accumulator) {
                lanes.push([u64::from(accumulator as u32), fill(first), fill(second)]);
            }
        }
    }
    lanes
        .chunks(LANES)
        .map(|chunk| {
            let vector = |part: usize| join(Lane::Doubleword, (0..LANES).map(|index| chunk.get(index).unwrap_or(&lanes[index])[part]));
            AvxData { vectors: [vector(0), vector(1), vector(2)], mxcsr: MXCSR_DEFAULT, immediate: 0 }
        })
        .collect()
}

/// The element of `bits` bits sign-extended.
const fn to_signed(element: u64, bits: u32) -> i64 {
    ((element << (u64::BITS - bits)) as i64) >> (u64::BITS - bits)
}
