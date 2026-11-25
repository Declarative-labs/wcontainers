//! The formulas of the EVEX integer operations, each over whole vectors held as four 128-bit parts, lowest first,
//! as the [Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) volume 2
//! describes them; the opmask is applied after.

use super::{read_element, write_element};
use crate::planted::{Bug, is_active};

/// Bytes of one 128-bit lane.
const LANE_BYTES: usize = 16;

/// What an integer form computes from its destination's value, its first source (EVEX.vvvv, or the only source of a
/// form without one), its second source and its immediate, element by element of the form's element size.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntegerOperation {
    /// VPERMB to VPERMQ, VPERMPS, VPERMPD: each element the second source's element the first's low bits name.
    Permute,
    /// VPERMQ, VPERMPD by an immediate: in each 256 bits, element i the one the immediate's bits 2i+1:2i name.
    PermuteQuartersByImmediate,
    /// VPERMI2B to VPERMI2PD: the index in the destination, the first table the first source, the second table the
    /// second source; the bit above the index's selects the table.
    PermuteTwoTablesIndexInDestination,
    /// VPERMT2B to VPERMT2PD: the first table in the destination, the index the first source.
    PermuteTwoTablesIndexInFirst,
    /// VPERMILPS, VPERMILPD by a vector: in each 128-bit lane, the first source's element the second's bits 1:0
    /// (doubles: bit 1) name.
    PermuteInLanes,
    /// VPERMILPS, VPSHUFD by an immediate: in each lane, element i the source's the immediate's bits 2i+1:2i name;
    /// VPERMILPD: element i the lane's element the immediate's bit i names.
    PermuteInLanesByImmediate,
    /// VPSHUFHW: in each lane, the high four words so selected, the low quadword the source's.
    ShuffleHighWords,
    /// VPSHUFLW: in each lane, the low four words so selected, the high quadword the source's.
    ShuffleLowWords,
    /// VPSHUFB: in each lane, the first source's byte the second's low four bits name, or zero where its top bit
    /// is set.
    ShuffleBytes,
    /// VSHUFPS: in each lane, the low two singles from the first source, the high two from the second, each two
    /// immediate bits naming one.
    ShuffleSingles,
    /// VSHUFPD: element i from the first source (even i) or the second (odd i), the lane's element immediate bit i
    /// names.
    ShuffleDoubles,
    /// VSHUFF32X4 to VSHUFI64X2: the low half's 128-bit blocks from the first source, the high half's from the
    /// second, each named by immediate bits (two bits a block in 512 bits, one in 256).
    ShuffleBlocks,
    /// VALIGND, VALIGNQ: the first source above the second, shifted right by the immediate's elements, modulo the
    /// element count.
    AlignElements,
    /// VPALIGNR: in each lane, the first source above the second, shifted right by the immediate's bytes; zero from
    /// 32.
    AlignBytes,
    /// VUNPCKHPS, VUNPCKHPD: in each lane, the high halves' elements interleaved, the first source's first.
    UnpackHigh,
    /// VUNPCKLPS, VUNPCKLPD: the low halves'.
    UnpackLow,
}

impl IntegerOperation {
    /// Whether the operation reads its destination: the two-table permutes do.
    pub const fn reads_destination(self) -> bool {
        matches!(self, Self::PermuteTwoTablesIndexInDestination | Self::PermuteTwoTablesIndexInFirst)
    }

    /// Whether each destination element reads the memory source's element of the same index alone.
    pub const fn is_element_wise(self) -> bool {
        false
    }

    /// The result's every element, from the destination's old value, the sources and the immediate, for a vector
    /// of `vector_bytes` and elements of `bytes`.
    pub fn compute(
        self,
        destination: &[u128; 4],
        first: &[u128; 4],
        second: &[u128; 4],
        immediate: u8,
        vector_bytes: usize,
        bytes: usize,
    ) -> [u128; 4] {
        let elements = vector_bytes / bytes;
        let per_lane = LANE_BYTES / bytes;
        let get = |vector: &[u128; 4], index: usize| read_element(vector, index, bytes);
        let immediate = u64::from(immediate);
        let mut result = [0; 4];
        for index in 0..elements {
            let lane_start = if is_active(Bug::EvexLanesFromTheFirst) { 0 } else { index / per_lane * per_lane };
            let in_lane = index % per_lane;
            let value = match self {
                Self::Permute if is_active(Bug::EvexPermuteIndexHalved) => get(second, get(first, index) as usize % elements.div_ceil(2)),
                Self::Permute => get(second, get(first, index) as usize % elements),
                Self::PermuteQuartersByImmediate => {
                    let quarter = index / 4 * 4;
                    get(first, quarter + ((immediate >> (2 * (index % 4))) & 3) as usize)
                }
                Self::PermuteTwoTablesIndexInDestination | Self::PermuteTwoTablesIndexInFirst => {
                    let (index_vector, table) = match self {
                        Self::PermuteTwoTablesIndexInDestination => (destination, first),
                        _ => (first, destination),
                    };
                    let selector = get(index_vector, index) as usize;
                    let chosen = if selector & elements != 0 && !is_active(Bug::EvexTwoTablesSelectorIgnored) { second } else { table };
                    get(chosen, selector % elements)
                }
                Self::PermuteInLanes => {
                    let control = get(second, index);
                    let within = if bytes == 8 { (control >> 1) & 1 } else { control & 3 };
                    get(first, lane_start + within as usize)
                }
                Self::PermuteInLanesByImmediate if bytes == 8 => {
                    get(first, lane_start + ((immediate >> (index % 8)) & 1) as usize)
                }
                Self::PermuteInLanesByImmediate => get(first, lane_start + ((immediate >> (2 * in_lane)) & 3) as usize),
                Self::ShuffleHighWords | Self::ShuffleLowWords => {
                    let high = self == Self::ShuffleHighWords;
                    let half = usize::from(high) * 4;
                    if (in_lane >= 4) == high {
                        get(first, lane_start + half + ((immediate >> (2 * (in_lane - half))) & 3) as usize)
                    } else {
                        get(first, index)
                    }
                }
                Self::ShuffleBytes => {
                    let control = get(second, index);
                    if control & 0x80 != 0 && !is_active(Bug::EvexShuffleBytesZeroBitIgnored) { 0 } else { get(first, lane_start + (control & 0xF) as usize) }
                }
                Self::ShuffleSingles => {
                    let source = if in_lane < 2 { first } else { second };
                    get(source, lane_start + ((immediate >> (2 * in_lane)) & 3) as usize)
                }
                Self::ShuffleDoubles => {
                    let source = if in_lane == 0 { first } else { second };
                    get(source, lane_start + ((immediate >> (index % 8)) & 1) as usize)
                }
                Self::ShuffleBlocks => {
                    let blocks = vector_bytes / LANE_BYTES;
                    let block = index / per_lane;
                    let bits = if blocks == 4 { 2 } else { 1 };
                    let source = if block < blocks / 2 { first } else { second };
                    let chosen = ((immediate >> (bits * block)) & ((1 << bits) - 1)) as usize;
                    get(source, chosen * per_lane + in_lane)
                }
                Self::AlignElements => {
                    let at = index + if is_active(Bug::EvexAlignCountUnreduced) { (immediate as usize).min(2 * elements) } else { immediate as usize % elements };
                    match at {
                        _ if at < elements => get(second, at),
                        _ if at < 2 * elements => get(first, at - elements),
                        _ => 0,
                    }
                }
                Self::AlignBytes => {
                    let at = in_lane + immediate as usize;
                    match at {
                        0..LANE_BYTES => get(second, lane_start + at),
                        LANE_BYTES..32 => get(first, lane_start + at - LANE_BYTES),
                        _ => 0,
                    }
                }
                Self::UnpackHigh | Self::UnpackLow => {
                    let base = if self == Self::UnpackHigh { per_lane / 2 } else { 0 };
                    let source = if in_lane.is_multiple_of(2) != is_active(Bug::EvexUnpackSourcesSwapped) { first } else { second };
                    get(source, lane_start + base + in_lane / 2)
                }
            };
            write_element(&mut result, index, bytes, value);
        }
        result
    }
}
