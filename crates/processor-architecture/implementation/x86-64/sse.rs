//! The legacy forms of SSE to SSE4.2, AES, PCLMULQDQ, SHA and GFNI that the lane and floating-point skeletons do not
//! cover: moves between XMM registers and memory or general registers, the fences and prefetches, MXCSR's load and
//! store, the conversions between general registers and floating point, ROUND, the variable blends, MPSADBW,
//! CRC32, the string compares and the cryptographic forms.
//!
//! A form is one row of [`SSE_FORMS`]: its operation, where its operand lies, and its iced-x86 codes. Each formula
//! follows the [Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! vol. 2's description of the instruction, written in our own words.

use alloc::vec::Vec;

use iced_x86::Code;

use crate::behavior::StoreAcrossFault;
use crate::float::{Arrangement, Precision};
use crate::forms::Instruction;
use crate::integer::OperandPlace;
use crate::planted::{Bug, is_active};
use crate::vector::{Lane, join, split};
use crate::PreciseInstruction;

/// Where a lane a form moves lies in its XMM register.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LaneSelect {
    /// At this byte.
    Fixed(u32),
    /// At the lane the immediate's low bits number, as many bits as number every lane.
    Immediate,
}

/// The aggregation a string compare's control bits 3 and 2 choose.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Aggregation {
    EqualAny,
    Ranges,
    EqualEach,
    EqualOrdered,
}

/// How a string compare finds the lengths of its strings.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StringLength {
    /// The first null element ends each.
    Implicit,
    /// EAX and EDX give them, or RAX and RDX for the forms with REX.W.
    Explicit { wide: bool },
}

/// What a string compare writes besides the flags.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StringOutput {
    /// An element's index in ECX.
    Index,
    /// A mask in XMM0.
    Mask,
}

/// The step of AES a form takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AesStep {
    Encrypt,
    EncryptLast,
    Decrypt,
    DecryptLast,
    InverseMixColumns,
    KeygenAssist,
}

/// The step of SHA-1 or SHA-256 a form takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShaStep {
    Sha1MessageFirst,
    Sha1MessageSecond,
    Sha1NextE,
    Sha1Rounds,
    Sha256MessageFirst,
    Sha256MessageSecond,
    Sha256Rounds,
}

/// The GFNI operation a form takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GaloisStep {
    Multiply,
    Affine,
    AffineInverse,
}

/// An operation of two XMM operands (and XMM0 for some) that writes the first, with the form's immediate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Combination {
    /// BLENDVPS, BLENDVPD, PBLENDVB: the source's lane where XMM0's lane has its sign bit set.
    VariableBlend(Lane),
    /// MPSADBW: eight sums of four absolute byte differences, sliding over the destination.
    MultipleSumAbsoluteDifferences,
    Aes(AesStep),
    /// PCLMULQDQ: the carry-less product of the quadwords immediate bits 0 and 4 select.
    CarryLessMultiply,
    Sha(ShaStep),
    Galois(GaloisStep),
}

/// What a form does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SseOperation {
    /// A lane of an XMM register written to a general register, zero-extended, or to memory.
    Extract { bytes: u32, select: LaneSelect, aligned: bool },
    /// A general register's or memory's low bytes into an XMM lane, the other lanes kept or zeroed.
    Insert { bytes: u32, select: LaneSelect, zeroing: bool, aligned: bool },
    /// MOVMSKPS, MOVMSKPD, PMOVMSKB: each lane's sign bit into a general register.
    MoveMask(Lane),
    /// MOVNTI: a general register into memory.
    StoreGeneral,
    LoadMxcsr,
    StoreMxcsr,
    /// LFENCE, MFENCE, SFENCE: nothing a program sees.
    Fence,
    /// PREFETCHh: nothing a program sees, and never a fault.
    Prefetch,
    Combine(Combination),
    CompareStrings { length: StringLength, output: StringOutput },
    /// CRC32 with the Castagnoli polynomial, bit-reflected.
    Crc32,
    /// CVTSI2SS, CVTSI2SD: a signed integer rounded under MXCSR into the low lane.
    IntegerToFloat(Precision),
    /// CVTSS2SI to CVTTSD2SI: the low lane rounded (or truncated) to a signed integer.
    FloatToInteger { precision: Precision, truncate: bool },
    /// ROUNDPS to ROUNDSD: each lane rounded to an integral value.
    Round { precision: Precision, arrangement: Arrangement },
    /// DPPS, DPPD: the products of the lanes immediate bits 4 and up select, summed in pairs, into the lanes its
    /// low bits select.
    DotProduct(Precision),
}

/// One form of this family.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SseForm {
    pub instruction: Instruction,
    pub operation: SseOperation,
    pub place: OperandPlace,
    /// One code, but for CRC32 one for each operand size and for a fence each ModRM byte that encodes it.
    pub codes: &'static [Code],
}

/// The bits of an XMM register.
const VECTOR_BITS: u32 = u128::BITS;
/// The bytes of an XMM register.
pub const VECTOR_BYTES: u32 = VECTOR_BITS / u8::BITS;

impl LaneSelect {
    /// The first byte of the lane of `bytes` the selection names under the immediate.
    pub fn to_offset(self, bytes: u32, immediate: u8) -> u32 {
        match self {
            Self::Fixed(offset) => offset,
            Self::Immediate if is_active(Bug::LaneSelectorUnmasked) && u32::from(immediate) >= VECTOR_BYTES / bytes => 0,
            Self::Immediate => (u32::from(immediate) & (VECTOR_BYTES / bytes - 1)) * bytes,
        }
    }
}

impl SseForm {
    /// The bytes its memory operand reaches; 0 for a form without one.
    pub fn memory_bytes(&self) -> u64 {
        if self.place == OperandPlace::Register {
            return 0;
        }
        let bytes = match self.operation {
            SseOperation::Extract { bytes, .. } | SseOperation::Insert { bytes, .. } => bytes,
            SseOperation::MoveMask(_) | SseOperation::Fence => 0,
            SseOperation::StoreGeneral | SseOperation::Crc32 | SseOperation::IntegerToFloat(_) => {
                to_memory_bits(self.codes[0]) / u8::BITS
            }
            SseOperation::LoadMxcsr | SseOperation::StoreMxcsr => u32::BITS / u8::BITS,
            SseOperation::Prefetch => 1,
            SseOperation::Combine(_) | SseOperation::CompareStrings { .. } => VECTOR_BYTES,
            SseOperation::FloatToInteger { precision, .. } => precision.to_bits() / u8::BITS,
            SseOperation::Round { arrangement: Arrangement::Packed, .. } => VECTOR_BYTES,
            SseOperation::Round { precision, .. } => precision.to_bits() / u8::BITS,
            SseOperation::DotProduct(_) => VECTOR_BYTES,
        };
        u64::from(bytes)
    }

    /// Whether a memory operand not aligned to its 16 bytes raises #GP(0), as legacy SSE's do but for the unaligned
    /// moves and the string compares.
    pub fn requires_alignment(&self) -> bool {
        self.place != OperandPlace::Register
            && match self.operation {
                SseOperation::Extract { aligned, .. } | SseOperation::Insert { aligned, .. } => aligned,
                SseOperation::Combine(_) => true,
                SseOperation::Round { arrangement, .. } => arrangement == Arrangement::Packed,
                SseOperation::DotProduct(_) => true,
                _ => false,
            }
    }

    /// Whether the form writes its memory operand.
    pub fn writes_memory(&self) -> bool {
        self.place != OperandPlace::Register
            && matches!(self.operation, SseOperation::Extract { .. } | SseOperation::StoreGeneral | SseOperation::StoreMxcsr)
    }
}

/// The bits of the general register or memory operand a code names besides its XMM register.
fn to_memory_bits(code: Code) -> u32 {
    match code {
        Code::Crc32_r32_rm8 | Code::Crc32_r64_rm8 => u8::BITS,
        Code::Crc32_r32_rm16 => u16::BITS,
        Code::Movnti_m64_r64 | Code::Crc32_r64_rm64 | Code::Cvtsi2ss_xmm_rm64 | Code::Cvtsi2sd_xmm_rm64 => u64::BITS,
        _ => u32::BITS,
    }
}

/// The row of a form of this family, or `None` for any other form.
pub fn find_sse_form(instruction: Instruction) -> Option<&'static SseForm> {
    SSE_FORMS.iter().find(|form| form.instruction == instruction)
}

/// The behaviors of a form of this family: one, and for a store that can cross a page (wider than a byte and not
/// aligned to its 16), each thing it may write when the second page refuses it.
pub fn list_sse_behaviors(instruction: Instruction) -> Vec<PreciseInstruction> {
    let Some(form) = find_sse_form(instruction) else {
        return Vec::new();
    };
    let across_page = form.writes_memory() && form.memory_bytes() > 1 && !form.requires_alignment();
    let choices: &[StoreAcrossFault] = if across_page {
        &[StoreAcrossFault::WritesNothing, StoreAcrossFault::WritesBytesBelowFault]
    } else {
        &[StoreAcrossFault::WritesNothing]
    };
    choices.iter().map(|&across_fault| PreciseInstruction::Sse { instruction, across_fault }).collect()
}

impl Combination {
    /// Whether the combination reads XMM0 besides its operands.
    pub const fn reads_xmm0(self) -> bool {
        matches!(self, Self::VariableBlend(_) | Self::Sha(ShaStep::Sha256Rounds))
    }

    /// Whether the combination reads its destination: AESIMC and AESKEYGENASSIST do not.
    pub const fn reads_destination(self) -> bool {
        !matches!(self, Self::Aes(AesStep::InverseMixColumns | AesStep::KeygenAssist))
    }

    /// The destination's new value from its value, the source's, XMM0's and the immediate.
    pub fn apply(self, destination: u128, source: u128, xmm0: u128, immediate: u8) -> u128 {
        match self {
            Self::VariableBlend(lane) => blend(lane, destination, source, xmm0),
            Self::MultipleSumAbsoluteDifferences => sum_absolute_differences(destination, source, immediate),
            Self::Aes(step) => apply_aes(step, destination, source, immediate),
            Self::CarryLessMultiply => {
                let select = |value: u128, bit: u8| (value >> if immediate & bit != 0 { u64::BITS } else { 0 }) as u64;
                let source_bit = if is_active(Bug::CarryLessMultiplySourceSelectIgnored) { 0 } else { CARRY_LESS_SOURCE_SELECT };
                multiply_carry_less(select(destination, CARRY_LESS_DESTINATION_SELECT), select(source, source_bit))
            }
            Self::Sha(step) => apply_sha(step, destination, source, xmm0, immediate),
            Self::Galois(step) => apply_galois(step, destination, source, immediate),
        }
    }
}

/// PCLMULQDQ's immediate bits that choose the high quadword of the destination and of the source.
const CARRY_LESS_DESTINATION_SELECT: u8 = 1 << 0;
const CARRY_LESS_SOURCE_SELECT: u8 = 1 << 4;

fn blend(lane: Lane, destination: u128, source: u128, xmm0: u128) -> u128 {
    let bits = lane.to_bits();
    let selector_bit = if is_active(Bug::VariableBlendFromLowBit) { 0 } else { bits - 1 };
    let (first, second, selectors) = (split(lane, destination), split(lane, source), split(lane, xmm0));
    join(lane, (0..first.len()).map(|index| if selectors[index] >> selector_bit & 1 != 0 { second[index] } else { first[index] }))
}

/// MPSADBW: immediate bit 2 picks the destination's first or second four-byte block as the start, bits 1 and 0 the
/// source's block; word `i` sums the absolute differences of four destination bytes from the start plus `i` and
/// the source block's four.
fn sum_absolute_differences(destination: u128, source: u128, immediate: u8) -> u128 {
    const BLOCK_BYTES: usize = 4;
    const SOURCE_BLOCK_BITS: u8 = 3;
    const DESTINATION_BLOCK_SHIFT: u8 = 2;
    let (first, second) = (destination.to_le_bytes(), source.to_le_bytes());
    let start = usize::from(immediate >> DESTINATION_BLOCK_SHIFT & 1) * BLOCK_BYTES;
    let block = if is_active(Bug::MultipleSumSourceBlockIgnored) { 0 } else { usize::from(immediate & SOURCE_BLOCK_BITS) * BLOCK_BYTES };
    let sums = (0..u128::BITS / u16::BITS).map(|word| {
        (0..BLOCK_BYTES)
            .map(|byte| u64::from(first[start + word as usize + byte].abs_diff(second[block + byte])))
            .sum::<u64>()
    });
    join(Lane::Word, sums)
}

/// The carry-less product of two quadwords.
pub fn multiply_carry_less(first: u64, second: u64) -> u128 {
    (0..u64::BITS).filter(|bit| second >> bit & 1 != 0).fold(0, |product, bit| product ^ (u128::from(first) << bit))
}

/// AES's field polynomial, x^8 + x^4 + x^3 + x + 1, below its x^8, which GFNI's multiply shares.
const FIELD_POLYNOMIAL: u8 = 0x1B;
/// AES's S-box affine constant.
const AFFINE_CONSTANT: u8 = 0x63;
const BYTE_VALUES: usize = 1 << u8::BITS;

/// The product of two bytes in AES's field, or with another polynomial below x^8.
const fn multiply_galois(first: u8, second: u8, polynomial: u8) -> u8 {
    let (mut product, mut first, mut second) = (0, first, second);
    while second != 0 {
        if second & 1 != 0 {
            product ^= first;
        }
        let carry = first & 0x80 != 0;
        first <<= 1;
        if carry {
            first ^= polynomial;
        }
        second >>= 1;
    }
    product
}

/// The byte's inverse in AES's field, `byte^254`, and zero for zero.
const fn invert_galois(byte: u8) -> u8 {
    let (mut result, mut base, mut exponent) = (1, byte, BYTE_VALUES as u32 - 2);
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = multiply_galois(result, base, FIELD_POLYNOMIAL);
        }
        base = multiply_galois(base, base, FIELD_POLYNOMIAL);
        exponent >>= 1;
    }
    result
}

/// AES's S-box: each byte's inverse, then FIPS-197's affine map.
const SUBSTITUTION: [u8; BYTE_VALUES] = {
    let mut table = [0; BYTE_VALUES];
    let mut index = 0;
    while index < BYTE_VALUES {
        let inverse = invert_galois(index as u8);
        table[index] = inverse
            ^ inverse.rotate_left(1)
            ^ inverse.rotate_left(2)
            ^ inverse.rotate_left(3)
            ^ inverse.rotate_left(4)
            ^ AFFINE_CONSTANT;
        index += 1;
    }
    table
};

const INVERSE_SUBSTITUTION: [u8; BYTE_VALUES] = {
    let mut table = [0; BYTE_VALUES];
    let mut index = 0;
    while index < BYTE_VALUES {
        table[SUBSTITUTION[index] as usize] = index as u8;
        index += 1;
    }
    table
};

/// The rows and columns of AES's state; byte `row + 4 × column` of the register is its element.
const AES_SIDE: usize = 4;
const MIX_COLUMNS: [u8; AES_SIDE] = [2, 3, 1, 1];
const INVERSE_MIX_COLUMNS: [u8; AES_SIDE] = [14, 11, 13, 9];

/// One step of AES on the state in the destination with the round key in the source, as FIPS-197 and the SDM's
/// AESENC to AESKEYGENASSIST give it.
fn apply_aes(step: AesStep, state: u128, key: u128, immediate: u8) -> u128 {
    let bytes = state.to_le_bytes();
    let shift_rows = |bytes: [u8; 16], inverse: bool| -> [u8; 16] {
        core::array::from_fn(|index| {
            let (row, column) = (index % AES_SIDE, index / AES_SIDE);
            let from = if inverse { (column + AES_SIDE - row) % AES_SIDE } else { (column + row) % AES_SIDE };
            bytes[row + AES_SIDE * from]
        })
    };
    let substitute = |bytes: [u8; 16], table: &[u8; BYTE_VALUES]| bytes.map(|byte| table[usize::from(byte)]);
    let mix = |bytes: [u8; 16], coefficients: [u8; AES_SIDE]| -> [u8; 16] {
        core::array::from_fn(|index| {
            let (row, column) = (index % AES_SIDE, index / AES_SIDE);
            (0..AES_SIDE).fold(0, |sum, other| {
                let coefficient = coefficients[(other + AES_SIDE - row) % AES_SIDE];
                sum ^ multiply_galois(coefficient, bytes[other + AES_SIDE * column], FIELD_POLYNOMIAL)
            })
        })
    };
    let mixes = !is_active(Bug::AesEncryptSkipsMixColumns);
    let result = match step {
        AesStep::Encrypt if mixes => mix(substitute(shift_rows(bytes, false), &SUBSTITUTION), MIX_COLUMNS),
        AesStep::Encrypt | AesStep::EncryptLast => substitute(shift_rows(bytes, false), &SUBSTITUTION),
        AesStep::Decrypt => mix(substitute(shift_rows(bytes, true), &INVERSE_SUBSTITUTION), INVERSE_MIX_COLUMNS),
        AesStep::DecryptLast => substitute(shift_rows(bytes, true), &INVERSE_SUBSTITUTION),
        AesStep::InverseMixColumns => return u128::from_le_bytes(mix(key.to_le_bytes(), INVERSE_MIX_COLUMNS)),
        AesStep::KeygenAssist => return assist_key_generation(key, immediate),
    };
    u128::from_le_bytes(result) ^ key
}

/// AESKEYGENASSIST: the source's doublewords 1 and 3 through the S-box, each also rotated right by a byte and
/// XORed with the immediate.
fn assist_key_generation(source: u128, immediate: u8) -> u128 {
    let words = split(Lane::Doubleword, source);
    let substitute = |word: u64| u32::from_le_bytes((word as u32).to_le_bytes().map(|byte| SUBSTITUTION[usize::from(byte)]));
    let rotate = |word: u32| {
        let rotated = if is_active(Bug::AesKeygenRotatesLeft) { word.rotate_left(u8::BITS) } else { word.rotate_right(u8::BITS) };
        rotated ^ u32::from(immediate)
    };
    let (low, high) = (substitute(words[1]), substitute(words[3]));
    join(Lane::Doubleword, [low, rotate(low), high, rotate(high)].into_iter().map(u64::from))
}

/// SHA-1's round constants, one for each of its four functions.
const SHA1_CONSTANTS: [u32; 4] = [0x5A82_7999, 0x6ED9_EBA1, 0x8F1B_BCDC, 0xCA62_C1D6];
const SHA1_FUNCTION_BITS: u8 = 3;

/// One step of SHA-1 or SHA-256, as FIPS 180-4 and the SDM's SHA1MSG1 to SHA256RNDS2 give it; doubleword 3 of a
/// register is its first word.
fn apply_sha(step: ShaStep, destination: u128, source: u128, xmm0: u128, immediate: u8) -> u128 {
    let words = |value: u128| -> [u32; 4] { core::array::from_fn(|index| (value >> (u32::BITS as usize * (3 - index))) as u32) };
    let from_words = |words: [u32; 4]| words.iter().fold(0u128, |all, &word| (all << u32::BITS) | u128::from(word));
    let (first, second) = (words(destination), words(source));
    match step {
        ShaStep::Sha1MessageFirst => from_words([first[2] ^ first[0], first[3] ^ first[1], second[0] ^ first[2], second[1] ^ first[3]]),
        ShaStep::Sha1MessageSecond => {
            let w16 = (first[0] ^ second[1]).rotate_left(1);
            let w17 = (first[1] ^ second[2]).rotate_left(1);
            let w18 = (first[2] ^ second[3]).rotate_left(1);
            let w19 = (first[3] ^ w16).rotate_left(1);
            from_words([w16, w17, w18, w19])
        }
        ShaStep::Sha1NextE => from_words([second[0].wrapping_add(first[0].rotate_left(30)), second[1], second[2], second[3]]),
        ShaStep::Sha1Rounds => {
            let function = usize::from(immediate & SHA1_FUNCTION_BITS);
            let constant = SHA1_CONSTANTS[if is_active(Bug::Sha1RoundsFirstConstant) { 0 } else { function }];
            let choose = |b: u32, c: u32, d: u32| match function {
                0 => (b & c) ^ (!b & d),
                2 => (b & c) ^ (b & d) ^ (c & d),
                _ => b ^ c ^ d,
            };
            let [mut a, mut b, mut c, mut d] = first;
            let mut e = 0u32;
            for (round, &word) in second.iter().enumerate() {
                let next = choose(b, c, d).wrapping_add(a.rotate_left(5)).wrapping_add(word).wrapping_add(constant);
                let next = if round == 0 { next } else { next.wrapping_add(e) };
                (a, b, c, d, e) = (next, a, b.rotate_left(30), c, d);
            }
            from_words([a, b, c, d])
        }
        ShaStep::Sha256MessageFirst => {
            let sigma = |x: u32| x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
            from_words([
                first[0].wrapping_add(sigma(second[3])),
                first[1].wrapping_add(sigma(first[0])),
                first[2].wrapping_add(sigma(first[1])),
                first[3].wrapping_add(sigma(first[2])),
            ])
        }
        ShaStep::Sha256MessageSecond => {
            let sigma = |x: u32| x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10);
            let w16 = first[3].wrapping_add(sigma(second[1]));
            let w17 = first[2].wrapping_add(sigma(second[0]));
            let w18 = first[1].wrapping_add(sigma(w16));
            let w19 = first[0].wrapping_add(sigma(w17));
            from_words([w19, w18, w17, w16])
        }
        ShaStep::Sha256Rounds => {
            let [mut a, mut b, mut e, mut f] = second;
            let [mut c, mut d, mut g, mut h] = first;
            let schedule = words(xmm0);
            let rounds = if is_active(Bug::Sha256RoundsOneRound) { 1 } else { 2 };
            for &word in schedule[2..].iter().rev().take(rounds) {
                let big_sigma_one = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
                let choose = (e & f) ^ (!e & g);
                let first_sum = h.wrapping_add(big_sigma_one).wrapping_add(choose).wrapping_add(word);
                let big_sigma_zero = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
                let majority = (a & b) ^ (a & c) ^ (b & c);
                (h, g, f, e, d, c, b, a) =
                    (g, f, e, d.wrapping_add(first_sum), c, b, a, first_sum.wrapping_add(big_sigma_zero).wrapping_add(majority));
            }
            from_words([a, b, e, f])
        }
    }
}

/// A GFNI operation on the destination's bytes, with the source's bytes or its quadwords as 8×8 bit matrices.
fn apply_galois(step: GaloisStep, destination: u128, source: u128, immediate: u8) -> u128 {
    let (first, second) = (destination.to_le_bytes(), source.to_le_bytes());
    let polynomial = if is_active(Bug::GaloisMultiplyWrongPolynomial) { FIELD_POLYNOMIAL | 0x06 } else { FIELD_POLYNOMIAL };
    let result: [u8; 16] = core::array::from_fn(|index| match step {
        GaloisStep::Multiply => multiply_galois(first[index], second[index], polynomial),
        GaloisStep::Affine | GaloisStep::AffineInverse => {
            let byte = if step == GaloisStep::AffineInverse { invert_galois(first[index]) } else { first[index] };
            let matrix = &second[index / 8 * 8..index / 8 * 8 + 8];
            (0..u8::BITS as usize).fold(0u8, |result, bit| {
                let row = if is_active(Bug::GaloisAffineRowsReversed) { matrix[bit] } else { matrix[7 - bit] };
                let parity = (row & byte).count_ones() as u8 & 1;
                result | ((parity ^ (immediate >> bit & 1)) << bit)
            })
        }
    });
    u128::from_le_bytes(result)
}

/// The Castagnoli polynomial, bit-reflected, as CRC32 divides by it.
const CASTAGNOLI_REFLECTED: u32 = 0x82F6_3B78;
/// The IEEE 802.3 polynomial, bit-reflected, that a planted bug divides by instead.
const ETHERNET_REFLECTED: u32 = 0xEDB8_8320;

/// CRC32: the low doubleword of the destination as the remainder so far, then each of the source's bytes, lowest
/// first.
pub fn accumulate_crc32(remainder: u32, source: u64, bytes: u32) -> u32 {
    let polynomial = if is_active(Bug::Crc32EthernetPolynomial) { ETHERNET_REFLECTED } else { CASTAGNOLI_REFLECTED };
    source.to_le_bytes()[..bytes as usize].iter().fold(remainder, |remainder, &byte| {
        (0..u8::BITS).fold(remainder ^ u32::from(byte), |remainder, _| {
            (remainder >> 1) ^ if remainder & 1 != 0 { polynomial } else { 0 }
        })
    })
}

/// A string compare's control: the element format in bits 1 and 0, the aggregation in 3 and 2, the polarity in 5
/// and 4, and which index or mask in 6.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StringControl(pub u8);

impl StringControl {
    pub const fn is_words(self) -> bool {
        self.0 & 1 != 0
    }

    pub const fn is_signed(self) -> bool {
        self.0 & 2 != 0
    }

    pub const fn to_aggregation(self) -> Aggregation {
        match self.0 >> 2 & 3 {
            0 => Aggregation::EqualAny,
            1 => Aggregation::Ranges,
            2 => Aggregation::EqualEach,
            _ => Aggregation::EqualOrdered,
        }
    }

    /// The elements in a register: 16 bytes or 8 words.
    pub const fn to_elements(self) -> usize {
        if self.is_words() { 8 } else { 16 }
    }

    const fn negates(self) -> bool {
        self.0 & 0x10 != 0
    }

    const fn masks_negation(self) -> bool {
        self.0 & 0x20 != 0
    }

    /// For an index, the most significant set bit; for a mask, one element of ones for each bit.
    pub const fn selects_most_or_expands(self) -> bool {
        self.0 & 0x40 != 0
    }
}

/// The elements of a register in the control's format, sign-extended where it is signed.
fn to_elements(value: u128, control: StringControl) -> Vec<i32> {
    let lane = if control.is_words() { Lane::Word } else { Lane::Byte };
    let bits = lane.to_bits();
    split(lane, value)
        .into_iter()
        .map(|element| if control.is_signed() { crate::vector::to_signed(element, bits) as i32 } else { element as i32 })
        .collect()
}

/// The length an implicit-length string compare finds: the index of the first null element.
pub fn find_implicit_length(value: u128, control: StringControl) -> usize {
    to_elements(value, control).iter().position(|&element| element == 0).unwrap_or(control.to_elements())
}

/// The length an explicit-length string compare takes from a register: its absolute value, at most the elements a
/// register holds.
pub fn to_explicit_length(register: u64, wide: bool, control: StringControl) -> usize {
    let magnitude = if wide && !is_active(Bug::StringLengthReadsLowHalf) {
        (register as i64).unsigned_abs()
    } else {
        u64::from((register as i32).unsigned_abs())
    };
    magnitude.min(control.to_elements() as u64) as usize
}

/// The result of a string compare's aggregation and polarity, one bit for each element of the second operand.
pub fn compare_strings(first: u128, second: u128, first_length: usize, second_length: usize, control: StringControl) -> u32 {
    let elements = control.to_elements();
    let (first, second) = (to_elements(first, control), to_elements(second, control));
    let valid = |length: usize, index: usize| index < length;
    let aggregate = |index: usize| -> bool {
        match control.to_aggregation() {
            Aggregation::EqualAny => {
                valid(second_length, index)
                    && (0..first_length).any(|needle| first[needle] == second[index])
            }
            Aggregation::Ranges => {
                valid(second_length, index)
                    && (0..first_length / 2).any(|pair| first[2 * pair] <= second[index] && second[index] <= first[2 * pair + 1])
            }
            Aggregation::EqualEach => match (valid(first_length, index), valid(second_length, index)) {
                (true, true) => first[index] == second[index],
                (false, false) => true,
                _ => false,
            },
            Aggregation::EqualOrdered => (0..elements - index).all(|offset| {
                match (valid(first_length, offset), valid(second_length, index + offset)) {
                    (false, _) => true,
                    (true, false) => false,
                    (true, true) => first[offset] == second[index + offset],
                }
            }),
        }
    };
    let all = (1u32 << elements) - 1;
    let found = (0..elements).filter(|&index| aggregate(index)).fold(0, |bits, index| bits | (1 << index));
    let second_valid = (1u32 << second_length) - 1;
    match (control.negates(), control.masks_negation()) {
        (false, _) => found,
        (true, false) => !found & all,
        (true, true) if is_active(Bug::StringMaskedNegationWhole) => !found & all,
        (true, true) => found ^ second_valid,
    }
}

use Lane::{Byte, Doubleword, Quadword};
use LaneSelect::{Fixed, Immediate as ByImmediate};
use OperandPlace::{Memory, Register};
use Precision::{Double, Single};

const fn extract(bytes: u32, select: LaneSelect, aligned: bool) -> SseOperation {
    SseOperation::Extract { bytes, select, aligned }
}

const fn insert(bytes: u32, select: LaneSelect, zeroing: bool, aligned: bool) -> SseOperation {
    SseOperation::Insert { bytes, select, zeroing, aligned }
}

const fn combine(combination: Combination) -> SseOperation {
    SseOperation::Combine(combination)
}

const fn strings(length: StringLength, output: StringOutput) -> SseOperation {
    SseOperation::CompareStrings { length, output }
}

const VECTOR: u32 = VECTOR_BYTES;
const QUADWORD: u32 = u64::BITS / u8::BITS;
const DOUBLEWORD: u32 = u32::BITS / u8::BITS;
const WORD: u32 = u16::BITS / u8::BITS;
const HIGH_QUADWORD: LaneSelect = Fixed(QUADWORD);
const LOW: LaneSelect = Fixed(0);
const EXPLICIT: StringLength = StringLength::Explicit { wide: false };
const EXPLICIT_WIDE: StringLength = StringLength::Explicit { wide: true };

/// A row's codes.
macro_rules! codes {
    ([$($code:ident),*]) => {
        &[$(Code::$code),*]
    };
}

/// The forms of each row: the operand a register, then, after a slash, memory.
macro_rules! forms {
    ($($($register:ident)? $(/ $memory:ident)?, $operation:expr, $codes:tt;)*) => {
        &[$(
            $(SseForm { instruction: Instruction::$register, operation: $operation, place: Register, codes: codes!($codes) },)?
            $(SseForm { instruction: Instruction::$memory, operation: $operation, place: Memory, codes: codes!($codes) },)?
        )*]
    };
}

/// Every form of this family modeled so far.
pub static SSE_FORMS: &[SseForm] = forms! {
    // Stores of a whole register or one lane.
    / MovapsMempsXmmps, extract(VECTOR, LOW, true), [Movaps_xmmm128_xmm];
    / MovapdMempdXmmpd, extract(VECTOR, LOW, true), [Movapd_xmmm128_xmm];
    / MovdqaMemdqXmmdq, extract(VECTOR, LOW, true), [Movdqa_xmmm128_xmm];
    / MovupsMempsXmmps, extract(VECTOR, LOW, false), [Movups_xmmm128_xmm];
    / MovupdMempdXmmpd, extract(VECTOR, LOW, false), [Movupd_xmmm128_xmm];
    / MovdquMemdqXmmdq, extract(VECTOR, LOW, false), [Movdqu_xmmm128_xmm];
    / MovntpsMemdqXmmps, extract(VECTOR, LOW, true), [Movntps_m128_xmm];
    / MovntpdMemdqXmmpd, extract(VECTOR, LOW, true), [Movntpd_m128_xmm];
    / MovntdqMemdqXmmdq, extract(VECTOR, LOW, true), [Movntdq_m128_xmm];
    / MovssMemssXmmss, extract(DOUBLEWORD, LOW, false), [Movss_xmmm32_xmm];
    / MovsdXmmMemsdXmmsd, extract(QUADWORD, LOW, false), [Movsd_xmmm64_xmm];
    / MovlpsMemqXmmq, extract(QUADWORD, LOW, false), [Movlps_m64_xmm];
    / MovlpdMemqXmmsd, extract(QUADWORD, LOW, false), [Movlpd_m64_xmm];
    / MovhpsMemqXmmps, extract(QUADWORD, HIGH_QUADWORD, false), [Movhps_m64_xmm];
    / MovhpdMemqXmmsd, extract(QUADWORD, HIGH_QUADWORD, false), [Movhpd_m64_xmm];
    / MovqMemqXmmq0fd6, extract(QUADWORD, LOW, false), [Movq_xmmm64_xmm];
    MovqGpr64Xmmq / MovqMemqXmmq0f7e, extract(QUADWORD, LOW, false), [Movq_rm64_xmm];
    MovdGpr32Xmmd / MovdMemdXmmd, extract(DOUBLEWORD, LOW, false), [Movd_rm32_xmm];
    PextrwGpr32XmmdqImmb, extract(WORD, ByImmediate, false), [Pextrw_r32_xmm_imm8];
    PextrwSse4Gpr32XmmdqImmb / PextrwSse4MemwXmmdqImmb, extract(WORD, ByImmediate, false), [Pextrw_r32m16_xmm_imm8];
    PextrbGpr32dXmmdqImmb / PextrbMembXmmdqImmb, extract(1, ByImmediate, false), [Pextrb_r32m8_xmm_imm8];
    PextrdGpr32dXmmdqImmb / PextrdMemdXmmdqImmb, extract(DOUBLEWORD, ByImmediate, false), [Pextrd_rm32_xmm_imm8];
    PextrqGpr64qXmmdqImmb / PextrqMemqXmmdqImmb, extract(QUADWORD, ByImmediate, false), [Pextrq_rm64_xmm_imm8];
    ExtractpsGpr32dXmmdqImmb / ExtractpsMemdXmmpsImmb, extract(DOUBLEWORD, ByImmediate, false), [Extractps_rm32_xmm_imm8];
    // Loads of a whole register or one lane.
    MovdXmmdqGpr32 / MovdXmmdqMemd, insert(DOUBLEWORD, LOW, true, false), [Movd_xmm_rm32];
    MovqXmmdqGpr64, insert(QUADWORD, LOW, true, false), [Movq_xmm_rm64];
    PinsrwXmmdqGpr32Immb / PinsrwXmmdqMemwImmb, insert(WORD, ByImmediate, false, false), [Pinsrw_xmm_r32m16_imm8];
    PinsrbXmmdqGpr32dImmb / PinsrbXmmdqMembImmb, insert(1, ByImmediate, false, false), [Pinsrb_xmm_r32m8_imm8];
    PinsrdXmmdqGpr32dImmb / PinsrdXmmdqMemdImmb, insert(DOUBLEWORD, ByImmediate, false, false), [Pinsrd_xmm_rm32_imm8];
    PinsrqXmmdqGpr64qImmb / PinsrqXmmdqMemqImmb, insert(QUADWORD, ByImmediate, false, false), [Pinsrq_xmm_rm64_imm8];
    / LddquXmmpdMemdq, insert(VECTOR, LOW, true, false), [Lddqu_xmm_m128];
    / MovntdqaXmmdqMemdq, insert(VECTOR, LOW, true, true), [Movntdqa_xmm_m128];
    MovmskpsGpr32Xmmps, SseOperation::MoveMask(Doubleword), [Movmskps_r32_xmm];
    MovmskpdGpr32Xmmpd, SseOperation::MoveMask(Quadword), [Movmskpd_r32_xmm];
    PmovmskbGpr32Xmmdq, SseOperation::MoveMask(Byte), [Pmovmskb_r32_xmm];
    / MovntiMemdGpr32, SseOperation::StoreGeneral, [Movnti_m32_r32];
    / MovntiMemqGpr64, SseOperation::StoreGeneral, [Movnti_m64_r64];
    / LdmxcsrMemd, SseOperation::LoadMxcsr, [Ldmxcsr_m32];
    / StmxcsrMemd, SseOperation::StoreMxcsr, [Stmxcsr_m32];
    Lfence, SseOperation::Fence, [Lfence, Lfence_E9, Lfence_EA, Lfence_EB, Lfence_EC, Lfence_ED, Lfence_EE, Lfence_EF];
    Mfence, SseOperation::Fence, [Mfence, Mfence_F1, Mfence_F2, Mfence_F3, Mfence_F4, Mfence_F5, Mfence_F6, Mfence_F7];
    Sfence, SseOperation::Fence, [Sfence, Sfence_F9, Sfence_FA, Sfence_FB, Sfence_FC, Sfence_FD, Sfence_FE, Sfence_FF];
    / PrefetchntaMemmprefetch, SseOperation::Prefetch, [Prefetchnta_m8];
    / Prefetcht0Memmprefetch, SseOperation::Prefetch, [Prefetcht0_m8];
    / Prefetcht1Memmprefetch, SseOperation::Prefetch, [Prefetcht1_m8];
    / Prefetcht2Memmprefetch, SseOperation::Prefetch, [Prefetcht2_m8];
    // Conversions between general registers and floating point.
    Cvtsi2ssXmmssGpr32d / Cvtsi2ssXmmssMemd, SseOperation::IntegerToFloat(Single), [Cvtsi2ss_xmm_rm32];
    Cvtsi2ssXmmssGpr64q / Cvtsi2ssXmmssMemq, SseOperation::IntegerToFloat(Single), [Cvtsi2ss_xmm_rm64];
    Cvtsi2sdXmmsdGpr32d / Cvtsi2sdXmmsdMemd, SseOperation::IntegerToFloat(Double), [Cvtsi2sd_xmm_rm32];
    Cvtsi2sdXmmsdGpr64q / Cvtsi2sdXmmsdMemq, SseOperation::IntegerToFloat(Double), [Cvtsi2sd_xmm_rm64];
    Cvtss2siGpr32dXmmss / Cvtss2siGpr32dMemss, SseOperation::FloatToInteger { precision: Single, truncate: false }, [Cvtss2si_r32_xmmm32];
    Cvtss2siGpr64qXmmss / Cvtss2siGpr64qMemss, SseOperation::FloatToInteger { precision: Single, truncate: false }, [Cvtss2si_r64_xmmm32];
    Cvttss2siGpr32dXmmss / Cvttss2siGpr32dMemss, SseOperation::FloatToInteger { precision: Single, truncate: true }, [Cvttss2si_r32_xmmm32];
    Cvttss2siGpr64qXmmss / Cvttss2siGpr64qMemss, SseOperation::FloatToInteger { precision: Single, truncate: true }, [Cvttss2si_r64_xmmm32];
    Cvtsd2siGpr32dXmmsd / Cvtsd2siGpr32dMemsd, SseOperation::FloatToInteger { precision: Double, truncate: false }, [Cvtsd2si_r32_xmmm64];
    Cvtsd2siGpr64qXmmsd / Cvtsd2siGpr64qMemsd, SseOperation::FloatToInteger { precision: Double, truncate: false }, [Cvtsd2si_r64_xmmm64];
    Cvttsd2siGpr32dXmmsd / Cvttsd2siGpr32dMemsd, SseOperation::FloatToInteger { precision: Double, truncate: true }, [Cvttsd2si_r32_xmmm64];
    Cvttsd2siGpr64qXmmsd / Cvttsd2siGpr64qMemsd, SseOperation::FloatToInteger { precision: Double, truncate: true }, [Cvttsd2si_r64_xmmm64];
    // SSE4.1.
    RoundpsXmmpsXmmpsImmb / RoundpsXmmpsMempsImmb, SseOperation::Round { precision: Single, arrangement: Arrangement::Packed }, [Roundps_xmm_xmmm128_imm8];
    RoundpdXmmpdXmmpdImmb / RoundpdXmmpdMempdImmb, SseOperation::Round { precision: Double, arrangement: Arrangement::Packed }, [Roundpd_xmm_xmmm128_imm8];
    RoundssXmmdXmmdImmb / RoundssXmmdMemdImmb, SseOperation::Round { precision: Single, arrangement: Arrangement::Scalar }, [Roundss_xmm_xmmm32_imm8];
    RoundsdXmmqXmmqImmb / RoundsdXmmqMemqImmb, SseOperation::Round { precision: Double, arrangement: Arrangement::Scalar }, [Roundsd_xmm_xmmm64_imm8];
    BlendvpsXmmdqXmmdq / BlendvpsXmmdqMemdq, combine(Combination::VariableBlend(Doubleword)), [Blendvps_xmm_xmmm128];
    BlendvpdXmmdqXmmdq / BlendvpdXmmdqMemdq, combine(Combination::VariableBlend(Quadword)), [Blendvpd_xmm_xmmm128];
    PblendvbXmmdqXmmdq / PblendvbXmmdqMemdq, combine(Combination::VariableBlend(Byte)), [Pblendvb_xmm_xmmm128];
    MpsadbwXmmdqXmmdqImmb / MpsadbwXmmdqMemdqImmb, combine(Combination::MultipleSumAbsoluteDifferences), [Mpsadbw_xmm_xmmm128_imm8];
    DppsXmmdqXmmdqImmb / DppsXmmdqMemdqImmb, SseOperation::DotProduct(Single), [Dpps_xmm_xmmm128_imm8];
    DppdXmmdqXmmdqImmb / DppdXmmdqMemdqImmb, SseOperation::DotProduct(Double), [Dppd_xmm_xmmm128_imm8];
    // SSE4.2.
    Crc32GpryyGpr8b / Crc32GpryyMemb, SseOperation::Crc32, [Crc32_r32_rm8, Crc32_r64_rm8];
    Crc32GpryyGprv / Crc32GpryyMemv, SseOperation::Crc32, [Crc32_r32_rm16, Crc32_r32_rm32, Crc32_r64_rm64];
    PcmpestriXmmdqXmmdqImmb / PcmpestriXmmdqMemdqImmb, strings(EXPLICIT, StringOutput::Index), [Pcmpestri_xmm_xmmm128_imm8];
    Pcmpestri64XmmdqXmmdqImmb / Pcmpestri64XmmdqMemdqImmb, strings(EXPLICIT_WIDE, StringOutput::Index), [Pcmpestri64_xmm_xmmm128_imm8];
    PcmpestrmXmmdqXmmdqImmb / PcmpestrmXmmdqMemdqImmb, strings(EXPLICIT, StringOutput::Mask), [Pcmpestrm_xmm_xmmm128_imm8];
    Pcmpestrm64XmmdqXmmdqImmb / Pcmpestrm64XmmdqMemdqImmb, strings(EXPLICIT_WIDE, StringOutput::Mask), [Pcmpestrm64_xmm_xmmm128_imm8];
    PcmpistriXmmdqXmmdqImmb / PcmpistriXmmdqMemdqImmb, strings(StringLength::Implicit, StringOutput::Index), [Pcmpistri_xmm_xmmm128_imm8];
    Pcmpistri64XmmdqXmmdqImmb / Pcmpistri64XmmdqMemdqImmb, strings(StringLength::Implicit, StringOutput::Index), [Pcmpistri_xmm_xmmm128_imm8];
    PcmpistrmXmmdqXmmdqImmb / PcmpistrmXmmdqMemdqImmb, strings(StringLength::Implicit, StringOutput::Mask), [Pcmpistrm_xmm_xmmm128_imm8];
    // AES, PCLMULQDQ, SHA and GFNI.
    AesencXmmdqXmmdq / AesencXmmdqMemdq, combine(Combination::Aes(AesStep::Encrypt)), [Aesenc_xmm_xmmm128];
    AesenclastXmmdqXmmdq / AesenclastXmmdqMemdq, combine(Combination::Aes(AesStep::EncryptLast)), [Aesenclast_xmm_xmmm128];
    AesdecXmmdqXmmdq / AesdecXmmdqMemdq, combine(Combination::Aes(AesStep::Decrypt)), [Aesdec_xmm_xmmm128];
    AesdeclastXmmdqXmmdq / AesdeclastXmmdqMemdq, combine(Combination::Aes(AesStep::DecryptLast)), [Aesdeclast_xmm_xmmm128];
    AesimcXmmdqXmmdq / AesimcXmmdqMemdq, combine(Combination::Aes(AesStep::InverseMixColumns)), [Aesimc_xmm_xmmm128];
    AeskeygenassistXmmdqXmmdqImmb / AeskeygenassistXmmdqMemdqImmb, combine(Combination::Aes(AesStep::KeygenAssist)), [Aeskeygenassist_xmm_xmmm128_imm8];
    PclmulqdqXmmdqXmmdqImmb / PclmulqdqXmmdqMemdqImmb, combine(Combination::CarryLessMultiply), [Pclmulqdq_xmm_xmmm128_imm8];
    Sha1msg1Xmmi32Xmmi32Sha / Sha1msg1Xmmi32Memi32Sha, combine(Combination::Sha(ShaStep::Sha1MessageFirst)), [Sha1msg1_xmm_xmmm128];
    Sha1msg2Xmmi32Xmmi32Sha / Sha1msg2Xmmi32Memi32Sha, combine(Combination::Sha(ShaStep::Sha1MessageSecond)), [Sha1msg2_xmm_xmmm128];
    Sha1nexteXmmi32Xmmi32Sha / Sha1nexteXmmi32Memi32Sha, combine(Combination::Sha(ShaStep::Sha1NextE)), [Sha1nexte_xmm_xmmm128];
    Sha1rnds4Xmmi32Xmmi32Imm8Sha / Sha1rnds4Xmmi32Memi32Imm8Sha, combine(Combination::Sha(ShaStep::Sha1Rounds)), [Sha1rnds4_xmm_xmmm128_imm8];
    Sha256msg1Xmmi32Xmmi32Sha / Sha256msg1Xmmi32Memi32Sha, combine(Combination::Sha(ShaStep::Sha256MessageFirst)), [Sha256msg1_xmm_xmmm128];
    Sha256msg2Xmmi32Xmmi32Sha / Sha256msg2Xmmi32Memi32Sha, combine(Combination::Sha(ShaStep::Sha256MessageSecond)), [Sha256msg2_xmm_xmmm128];
    Sha256rnds2Xmmi32Xmmi32Sha / Sha256rnds2Xmmi32Memi32Sha, combine(Combination::Sha(ShaStep::Sha256Rounds)), [Sha256rnds2_xmm_xmmm128];
    Gf2p8mulbXmmu8Xmmu8 / Gf2p8mulbXmmu8Memu8, combine(Combination::Galois(GaloisStep::Multiply)), [Gf2p8mulb_xmm_xmmm128];
    Gf2p8affineqbXmmu8Xmmu64Imm8 / Gf2p8affineqbXmmu8Memu64Imm8, combine(Combination::Galois(GaloisStep::Affine)), [Gf2p8affineqb_xmm_xmmm128_imm8];
    Gf2p8affineinvqbXmmu8Xmmu64Imm8 / Gf2p8affineinvqbXmmu8Memu64Imm8, combine(Combination::Galois(GaloisStep::AffineInverse)), [Gf2p8affineinvqb_xmm_xmmm128_imm8];
};

/// The forms of this family whose encodings carry a REX.W that their code ignores: the implicit-length string
/// compares of XED's `PCMPISTRI64`.
pub const IGNORED_WIDTH_FORMS: [Instruction; 2] = [Instruction::Pcmpistri64XmmdqXmmdqImmb, Instruction::Pcmpistri64XmmdqMemdqImmb];

#[cfg(test)]
mod tests {
    use iced_x86::{Decoder, DecoderOptions, OpKind};

    use super::{SSE_FORMS, StringControl, accumulate_crc32, apply_aes, AesStep, multiply_carry_less};
    use crate::SAMPLE_ENCODINGS;
    use crate::integer::OperandPlace;

    #[test]
    fn each_form_s_sample_decodes_to_one_of_its_codes_with_its_operand_in_place() {
        for form in SSE_FORMS {
            let decoded = Decoder::new(64, SAMPLE_ENCODINGS[form.instruction as usize], DecoderOptions::NONE).decode();
            assert!(form.codes.contains(&decoded.code()), "{:?} decodes to {:?}", form.instruction, decoded.code());
            let in_memory = (0..decoded.op_count()).any(|operand| decoded.op_kind(operand) == OpKind::Memory);
            assert_eq!(in_memory, form.place != OperandPlace::Register, "{:?}", form.instruction);
        }
    }

    /// FIPS-197 appendix C.1: the first round of AES-128 on its example, and the last round's input and output.
    #[test]
    fn aes_rounds_follow_fips_197() {
        let state = u128::from_be_bytes([0x00, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90, 0xa0, 0xb0, 0xc0, 0xd0, 0xe0, 0xf0]);
        let key = u128::from_be_bytes([0xd6, 0xaa, 0x74, 0xfd, 0xd2, 0xaf, 0x72, 0xfa, 0xda, 0xa6, 0x78, 0xf1, 0xd6, 0xab, 0x76, 0xfe]);
        let next = u128::from_be_bytes([0x89, 0xd8, 0x10, 0xe8, 0x85, 0x5a, 0xce, 0x68, 0x2d, 0x18, 0x43, 0xd8, 0xcb, 0x12, 0x8f, 0xe4]);
        let reverse = |value: u128| u128::from_le_bytes(value.to_be_bytes());
        assert_eq!(apply_aes(AesStep::Encrypt, reverse(state), reverse(key), 0), reverse(next));
        let last = apply_aes(AesStep::EncryptLast, reverse(state), 0, 0);
        assert_eq!(apply_aes(AesStep::DecryptLast, last, 0, 0), reverse(state));
    }

    #[test]
    fn crc32_of_the_check_string_is_castagnoli_s() {
        // CRC-32C of "123456789" is 0xE3069283, with the initial and final inversions the instruction leaves out.
        let remainder = b"123456789".iter().fold(u32::MAX, |remainder, &byte| accumulate_crc32(remainder, u64::from(byte), 1));
        assert_eq!(!remainder, 0xE306_9283);
    }

    #[test]
    fn carry_less_products_have_no_carries() {
        assert_eq!(multiply_carry_less(0b11, 0b11), 0b101);
        assert_eq!(multiply_carry_less(u64::MAX, 1), u128::from(u64::MAX));
        assert!(StringControl(0x40).selects_most_or_expands());
    }
}
