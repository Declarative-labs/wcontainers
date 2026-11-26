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
    /// VPDPBUSD, VPDPBUSDS: each doubleword plus the four products of the first source's unsigned bytes and the
    /// second's signed ones, wrapping or saturated signed.
    DotBytes { saturating: bool },
    /// VPDPWSSD, VPDPWSSDS: each doubleword plus the two products of the sources' signed words.
    DotWords { saturating: bool },
    /// VPMADD52LUQ, VPMADD52HUQ: each quadword plus bits 51:0 (or 103:52) of the product of the sources' low 52
    /// bits.
    MultiplyAdd52 { high: bool },
    /// VPMULTISHIFTQB: each byte the eight bits of the second source's quadword from the bit the first source's
    /// byte (its low six bits) names, wrapping round.
    MultiShiftBytes,
    /// VPSHLDW to VPSHLDQ: the first source above the second, shifted left by the immediate modulo the width, the
    /// upper half kept.
    ShiftLeftDouble,
    /// VPSHRDW to VPSHRDQ: the second source above the first, shifted right, the lower half kept.
    ShiftRightDouble,
    /// VPSHLDVW to VPSHLDVQ: the destination above the first source, shifted left by the second source's element.
    ShiftLeftDoubleVariable,
    /// VPSHRDVW to VPSHRDVQ: the first source above the destination, shifted right by the second's element.
    ShiftRightDoubleVariable,
    /// VPOPCNTB to VPOPCNTQ: the source's set bits.
    PopulationCount,
    /// VPLZCNTD, VPLZCNTQ: the source's leading zero bits, the width for zero.
    LeadingZeros,
    /// VPCONFLICTD, VPCONFLICTQ: a bit for each lower element equal to the element.
    Conflict,
    /// VGF2P8MULB: the bytes' product in GF(2^8) modulo x^8 + x^4 + x^3 + x + 1.
    GaloisMultiply,
    /// VGF2P8AFFINEQB, VGF2P8AFFINEINVQB: each byte of the first source (or its inverse in GF(2^8)) times the
    /// second source's quadword as a bit matrix, plus the immediate.
    GaloisAffine { inverse: bool },
    /// VPCLMULQDQ: in each lane, the carry-less product of the quadwords immediate bits 0 and 4 name.
    CarryLessMultiply,
    /// VAESENC to VAESDECLAST: in each lane one AES round of the first source with the second as its key.
    Aes { decrypt: bool, last: bool },
    /// VPSHUFBITQMB: for each byte of the second source, the bit of the first source's quadword its low six bits
    /// name, into an opmask register.
    ShuffleBitsToMask,
    /// VPBROADCASTMB2Q, VPBROADCASTMW2D: the opmask register's low byte or word in every element.
    BroadcastMask,
}

impl IntegerOperation {
    /// Whether the operation reads its destination: the two-table permutes do.
    pub const fn reads_destination(self) -> bool {
        matches!(
            self,
            Self::PermuteTwoTablesIndexInDestination
                | Self::PermuteTwoTablesIndexInFirst
                | Self::DotBytes { .. }
                | Self::DotWords { .. }
                | Self::MultiplyAdd52 { .. }
                | Self::ShiftLeftDoubleVariable
                | Self::ShiftRightDoubleVariable
        )
    }

    /// Whether each destination element reads the memory source's element of the same index alone.
    pub const fn is_element_wise(self) -> bool {
        matches!(
            self,
            Self::DotBytes { .. }
                | Self::DotWords { .. }
                | Self::MultiplyAdd52 { .. }
                | Self::ShiftLeftDouble
                | Self::ShiftRightDouble
                | Self::ShiftLeftDoubleVariable
                | Self::ShiftRightDoubleVariable
                | Self::PopulationCount
                | Self::LeadingZeros
                | Self::GaloisMultiply
                | Self::ShuffleBitsToMask
        )
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
        // The carry-less product and AES work on whole lanes, one a step.
        let bytes = if matches!(self, Self::CarryLessMultiply | Self::Aes { .. }) { LANE_BYTES } else { bytes };
        let elements = vector_bytes / bytes;
        let per_lane = (LANE_BYTES / bytes).max(1);
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
                Self::DotBytes { saturating } | Self::DotWords { saturating } => {
                    let parts = if matches!(self, Self::DotBytes { .. }) { 4 } else { 2 };
                    let part_bytes = bytes / parts;
                    let sum: i64 = (0..parts)
                        .map(|part| {
                            let at = index * parts + part;
                            let a = read_element(first, at, part_bytes);
                            let b = to_signed(read_element(second, at, part_bytes), part_bytes);
                            let a = if parts == 4 { a as i64 } else { to_signed(a, part_bytes) };
                            a * b
                        })
                        .sum();
                    let total = to_signed(get(destination, index), bytes) + sum;
                    if saturating && !is_active(Bug::EvexDotProductWraps) {
                        total.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as u64
                    } else {
                        total as u64
                    }
                }
                Self::MultiplyAdd52 { high } => {
                    let low_bits = (1u128 << 52) - 1;
                    let product = (u128::from(get(first, index)) & low_bits) * (u128::from(get(second, index)) & low_bits);
                    let part = if high { product >> 52 } else { product & low_bits };
                    get(destination, index).wrapping_add(part as u64)
                }
                Self::MultiShiftBytes => {
                    let quadword = read_element(second, index / 8, 8);
                    quadword.rotate_right((get(first, index) & 63) as u32) & 0xFF
                }
                Self::ShiftLeftDouble | Self::ShiftRightDouble | Self::ShiftLeftDoubleVariable | Self::ShiftRightDoubleVariable => {
                    let bits = (bytes * 8) as u32;
                    let (high, low, count) = match self {
                        Self::ShiftLeftDouble => (get(first, index), get(second, index), u64::from(immediate as u8)),
                        Self::ShiftRightDouble => (get(second, index), get(first, index), u64::from(immediate as u8)),
                        Self::ShiftLeftDoubleVariable => (get(destination, index), get(first, index), get(second, index)),
                        _ => (get(first, index), get(destination, index), get(second, index)),
                    };
                    let count = (count % u64::from(bits)) as u32;
                    let joined = (u128::from(high) << bits) | u128::from(low);
                    let mask = if bits == 64 { u128::from(u64::MAX) } else { (1u128 << bits) - 1 };
                    let joined = if is_active(Bug::EvexShiftDoubleHalvesSwapped) { (joined >> bits) | ((joined & mask) << bits) } else { joined };
                    match self {
                        Self::ShiftLeftDouble | Self::ShiftLeftDoubleVariable => ((joined << count) >> bits & mask) as u64,
                        _ => ((joined >> count) & mask) as u64,
                    }
                }
                Self::PopulationCount => u64::from(get(first, index).count_ones()),
                Self::LeadingZeros => {
                    let value = get(first, index);
                    u64::from(value.leading_zeros()) - (64 - 8 * bytes as u64)
                }
                Self::Conflict => {
                    let value = get(first, index);
                    let upto = if is_active(Bug::EvexConflictIncludesSelf) { index + 1 } else { index };
                    (0..upto).filter(|lower| get(first, *lower) == value).fold(0, |bits, lower| bits | 1 << lower)
                }
                Self::GaloisMultiply => u64::from(multiply_galois(get(first, index) as u8, get(second, index) as u8)),
                Self::GaloisAffine { inverse } => {
                    let matrix = read_element(second, index / 8, 8);
                    let byte = get(first, index) as u8;
                    let byte = if inverse { invert_galois(byte) } else { byte };
                    u64::from(transform_affine(matrix, byte, immediate as u8))
                }
                Self::CarryLessMultiply => {
                    if in_lane == 0 {
                        let lane = index;
                        let a = read_element(first, 2 * lane + usize::from(immediate & 1 != 0), 8);
                        let high = immediate & 0x10 != 0 && !is_active(Bug::EvexCarryLessIgnoresBitFour);
                        let b = read_element(second, 2 * lane + usize::from(high), 8);
                        let product = multiply_carry_less(a, b);
                        result[lane] = product;
                    }
                    continue;
                }
                Self::Aes { decrypt, last } => {
                    if in_lane == 0 {
                        let lane = index;
                        result[lane] = round_aes(first[lane], second[lane], decrypt, last);
                    }
                    continue;
                }
                Self::ShuffleBitsToMask => {
                    let quadword = read_element(first, index / 8, 8);
                    let bit = (quadword >> (get(second, index) & 63)) & 1;
                    result[0] |= u128::from(bit) << index;
                    continue;
                }
                Self::BroadcastMask => {
                    let mask = second[0] as u64;
                    if bytes == 8 { mask & 0xFF } else { mask & 0xFFFF }
                }
            };
            write_element(&mut result, index, bytes, value);
        }
        result
    }
}

/// The element of `bytes` bytes read as a signed number.
const fn to_signed(value: u64, bytes: usize) -> i64 {
    let unused = 64 - 8 * bytes as u32;
    ((value << unused) as i64) >> unused
}

/// The product of two bytes in GF(2^8) modulo x^8 + x^4 + x^3 + x + 1.
const fn multiply_galois(mut first: u8, mut second: u8) -> u8 {
    let mut product = 0;
    while second != 0 {
        if second & 1 != 0 {
            product ^= first;
        }
        let carry = first & 0x80 != 0;
        first <<= 1;
        if carry {
            first ^= 0x1B;
        }
        second >>= 1;
    }
    product
}

/// The byte's inverse in GF(2^8), zero for zero: its 254th power.
const fn invert_galois(byte: u8) -> u8 {
    let mut result = 1;
    let mut power = 0;
    while power < 254 {
        result = multiply_galois(result, byte);
        power += 1;
    }
    if byte == 0 { 0 } else { result }
}

/// The matrix's affine transform of the byte: bit i the parity of the matrix's byte 7 - i with the byte, plus the
/// immediate's bit i.
const fn transform_affine(matrix: u64, byte: u8, immediate: u8) -> u8 {
    let mut result = 0;
    let mut bit = 0;
    while bit < 8 {
        let row = (matrix >> (8 * (7 - bit))) as u8;
        let parity = (row & byte).count_ones() as u8 & 1;
        result |= (parity ^ ((immediate >> bit) & 1)) << bit;
        bit += 1;
    }
    result
}

/// The carry-less product of two quadwords.
const fn multiply_carry_less(first: u64, second: u64) -> u128 {
    let mut product = 0;
    let mut bit = 0;
    while bit < 64 {
        if (second >> bit) & 1 != 0 {
            product ^= (first as u128) << bit;
        }
        bit += 1;
    }
    product
}

/// AES's S-box, [FIPS 197](https://csrc.nist.gov/pubs/fips/197/final) 5.1.1: the inverse in GF(2^8) under the
/// standard's affine transform.
const fn substitute(byte: u8) -> u8 {
    let inverse = invert_galois(byte);
    inverse ^ inverse.rotate_left(1) ^ inverse.rotate_left(2) ^ inverse.rotate_left(3) ^ inverse.rotate_left(4) ^ 0x63
}

/// The inverse S-box, FIPS 197 5.3.2.
const fn substitute_inverse(byte: u8) -> u8 {
    invert_galois(byte.rotate_left(1) ^ byte.rotate_left(3) ^ byte.rotate_left(6) ^ 0x05)
}

/// One AES round of the state with the round key, as AESENC, AESENCLAST, AESDEC and AESDECLAST do: the state's
/// byte i lies in row i mod 4 and column i / 4.
fn round_aes(state: u128, key: u128, decrypt: bool, last: bool) -> u128 {
    let bytes = state.to_le_bytes();
    let mut shifted = [0u8; 16];
    for (at, byte) in shifted.iter_mut().enumerate() {
        let (row, column) = (at % 4, at / 4);
        let from = if decrypt { (column + 4 - row) % 4 } else { (column + row) % 4 };
        let value = bytes[row + 4 * from];
        *byte = if decrypt { substitute_inverse(value) } else { substitute(value) };
    }
    if !last && !is_active(Bug::EvexAesSkipsMixColumns) {
        let coefficients: [u8; 4] = if decrypt { [14, 11, 13, 9] } else { [2, 3, 1, 1] };
        let mut mixed = [0u8; 16];
        for column in 0..4 {
            for row in 0..4 {
                mixed[row + 4 * column] = (0..4).fold(0, |sum, other| {
                    sum ^ multiply_galois(coefficients[(other + 4 - row) % 4], shifted[other + 4 * column])
                });
            }
        }
        shifted = mixed;
    }
    u128::from_le_bytes(shifted) ^ key
}
