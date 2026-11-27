//! The VEX forms of AVX, AVX2, FMA, F16C and the VEX forms of AES, PCLMULQDQ and GFNI that [`crate::vector`]'s
//! VEX skeleton does not cover: one row of [`AVX_FORMS`] each, its operation, where its last source or its
//! destination lies, its iced-x86 `Code`, the bytes its memory operand reaches and whether that operand must be
//! aligned.
//!
//! A form's operands are read in the order Intel's manual lists them: the destination first, then the sources, an
//! immediate last. A VEX form never reads its destination unless its operation says so (FMA's first operand, the
//! compares that write RFLAGS); a write to an XMM register zeroes bits 128 to 255.

use alloc::vec::Vec;

use iced_x86::Code;

use iced_x86::OpCodeOperandKind;

use crate::behavior::StoreAcrossFault;
use crate::float::{Arrangement, Conversion, FloatOperation, FusedChoices, FusedNanOrder, InfiniteProductOfZero, Precision};
use crate::forms::Instruction;
use crate::integer::OperandPlace;
use crate::planted::{Bug, is_active};
use crate::vector::{Lane, LaneOperation, join, split, to_signed};
use crate::PreciseInstruction;

/// A YMM register's two halves, or the bits a source holds, lowest first.
pub type Wide = [u128; 2];

/// What a form does with its operands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AvxOperation {
    /// The lane operation on each 128-bit half: two sources as the legacy form's destination and source, one source
    /// as both; the upper half's immediate shifted right by `upper_shift`, as BLENDPS's and SHUFPD's bits run on.
    Halves { operation: LaneOperation, upper_shift: u8 },
    /// PSLLW to PSRAD and PSLLDQ, PSRLDQ by an immediate: the source shifted, each half alike.
    ShiftByImmediate(LaneOperation),
    /// PSLLW to PSRAD by the low quadword of an XMM register or 16 bytes of memory, each half by the same count.
    ShiftByCount(LaneOperation),
    /// VPSLLVD to VPSRAVD: each lane by the second source's lane of the same index.
    ShiftEach { lane: Lane, shift: EachShift },
    /// VPMOVSX and VPMOVZX: the source's low lanes extended, as many as fill the destination.
    Extend(LaneOperation),
    /// VPTEST: ZF where the sources share no bit, CF where the second has none the first lacks.
    Test,
    /// VTESTPS, VTESTPD: as VPTEST on each lane's sign bit alone.
    TestSigns(Lane),
    /// VMPSADBW: in each half, eight sums of four absolute byte differences, the blocks the immediate's three bits
    /// for that half select.
    MultipleSumsOfDifferences,
    /// VBLENDVPS, VBLENDVPD, VPBLENDVB: the second source's lane where the third's lane has its top bit set.
    BlendVariable(Lane),
    /// VBROADCASTSS, VBROADCASTSD, VPBROADCASTB to VPBROADCASTQ: the source's low lane in every lane.
    Broadcast(Lane),
    /// VBROADCASTF128, VBROADCASTI128: 16 bytes in both halves.
    BroadcastHalf,
    /// VINSERTF128, VINSERTI128: the first source with the half the immediate's bit 0 names replaced.
    InsertHalf,
    /// VEXTRACTF128, VEXTRACTI128: the half the immediate's bit 0 names.
    ExtractHalf,
    /// VPEXTRB to VPEXTRQ, VEXTRACTPS: the lane the immediate's low bits name, zero-extended.
    ExtractLane(Lane),
    /// VPINSRB to VPINSRQ: the first source with the lane the immediate's low bits name replaced by the second's low
    /// bits.
    InsertLane(Lane),
    /// VMOVMSKPS, VMOVMSKPD, VPMOVMSKB: each lane's sign bit, lowest first, into a general register.
    Signs(Lane),
    /// VPERMILPS, VPERMILPD with a vector control: in each half, the lane of the first source the control's lane
    /// selects (its bits 1 to 0 for singles, its bit 1 for doubles).
    PermuteVariable(Lane),
    /// VPERM2F128, VPERM2I128: each half one of the sources' four halves, or zero, as the immediate's nibble asks.
    PermuteHalves,
    /// VPERMD, VPERMPS: each doubleword the second source's doubleword the first source's lane selects by its low
    /// three bits.
    PermuteDoublewords,
    /// VPERMQ, VPERMPD: each quadword the source's quadword the immediate's two bits for it select.
    PermuteQuadwords,
    /// VMASKMOVPS, VMASKMOVPD, VPMASKMOVD, VPMASKMOVQ from memory: each lane whose mask lane has its top bit set,
    /// zero elsewhere; a lane not read never faults.
    MaskedLoad(Lane),
    /// The same to memory: only the lanes whose mask lane has its top bit set are written or checked.
    MaskedStore(Lane),
    /// VZEROUPPER: bits 128 to 255 of every vector register zeroed.
    ZeroUpper,
    /// VZEROALL: every vector register zeroed.
    ZeroAll,
    /// VLDMXCSR: MXCSR from memory, #GP(0) for a reserved bit.
    LoadMxcsr,
    /// VSTMXCSR.
    StoreMxcsr,
    /// A legacy floating-point operation on each 128-bit half, as [`crate::float::FloatForm`] computes it: two
    /// sources as the legacy destination and source, one as both.
    Float { operation: FloatOperation, precision: Precision, arrangement: Arrangement },
    /// VCMPPS to VCMPSD: the 32 predicates of the immediate's low five bits.
    Compare { precision: Precision, arrangement: Arrangement },
    /// VCVTDQ2PD, VCVTPS2PD: the source's low lanes into the wider lanes of the destination.
    ConvertWidening(Conversion),
    /// VCVTPD2DQ, VCVTTPD2DQ, VCVTPD2PS: every lane of the source into the narrower lanes of an XMM register.
    ConvertNarrowing(Conversion),
    /// VCVTSI2SS, VCVTSI2SD: a signed doubleword or quadword into the low lane, the first source's other lanes kept.
    IntegerToScalar(Precision),
    /// VCVTSS2SI to VCVTTSD2SI: the low lane into a signed doubleword or quadword, as wide as the destination.
    ScalarToInteger { precision: Precision, truncate: bool },
    /// VROUNDPS to VROUNDSD: each lane rounded to an integral value in the immediate's mode or MXCSR's.
    RoundToIntegral { precision: Precision, arrangement: Arrangement },
    /// The FMA forms: the product of two operands and the third, rounded once.
    Fused { sign: FusedSign, order: FusedOrder, precision: Precision, arrangement: Arrangement },
    /// VCVTPH2PS: each half-precision value exactly into a single.
    HalfToSingle,
    /// VCVTPS2PH: each single rounded to half precision in the immediate's mode or MXCSR's.
    SingleToHalf,
    /// VPCLMULQDQ: in each half, the carry-less product of the quadwords the immediate's bits 0 and 4 select.
    CarrylessMultiply,
    /// VAESENC to VAESKEYGENASSIST, on each half.
    Aes(AesStep),
    /// VGF2P8MULB: each byte pair's product in GF(2^8) modulo x^8 + x^4 + x^3 + x + 1.
    GaloisMultiply,
    /// VGF2P8AFFINEQB, VGF2P8AFFINEINVQB: each byte of the first source (with `inverse` its inverse in GF(2^8)
    /// first) through the bit matrix of the second source's quadword holding it, XOR the immediate.
    GaloisAffine { inverse: bool },
    /// VDPPS, VDPPD: in each half, the products of the lanes the immediate's high nibble selects (zero for the
    /// others), summed in pairs and then the pair sums, each step rounded, into the lanes its low nibble selects.
    DotProduct(Precision),
    /// VPCMPESTRI to VPCMPISTRM: the strings of the two sources compared as the immediate says, their lengths in
    /// RAX and RDX (`explicit`, as wide as `quadword_lengths` says) or up to their first null element; the result
    /// an index in ECX (`index`) or a mask in XMM0, with CF, ZF, SF and OF.
    StringCompare { explicit: bool, index: bool, quadword_lengths: bool },
    /// VMASKMOVDQU: the bytes of the first source whose byte of the second has its top bit set, stored at RDI.
    MaskedByteStore,
    /// VGATHERDPS to VPGATHERQQ: each lane of `data` bits whose mask lane (VEX.vvvv) has its top bit set loaded
    /// from the base plus its index lane (of `index` bits, sign-extended) times the scale plus the displacement,
    /// lowest first, its mask lane cleared once loaded; the whole mask zeroed at the end. A fault leaves the lanes
    /// loaded before it, and #UD comes from any two of the destination, index and mask registers being one.
    Gather { data: Lane, index: Lane },
    /// VPDPBUSD to VPDPWSSDS: each doubleword of the destination plus the products of the first source's bytes
    /// (unsigned) with the second's (signed) four at a time, or of their signed words two at a time, wrapped or, with
    /// `saturate`, saturated to a signed doubleword.
    DotAccumulate { words: bool, saturate: bool },
}

/// Which way VPSLLVD to VPSRAVD shift.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EachShift {
    Left,
    RightLogical,
    RightArithmetic,
}

/// The signs of an FMA form's product and addend: VFMADD, VFMSUB, VFNMADD, VFNMSUB, and VFMADDSUB, VFMSUBADD,
/// which subtract the addend in the even lanes and add it in the odd ones, or the reverse.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FusedSign {
    MultiplyAdd,
    MultiplySubtract,
    NegatedMultiplyAdd,
    NegatedMultiplySubtract,
    MultiplyAddSubtract,
    MultiplySubtractAdd,
}

/// Which operands, in Intel's order, are multiplied and which is added: 132 multiplies the first and the third, 213
/// the second and the first, 231 the second and the third.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FusedOrder {
    OneThreeTwo,
    TwoOneThree,
    TwoThreeOne,
}

impl FusedOrder {
    /// The operands' indices: the multiplicand, the multiplier and the addend.
    pub const fn to_roles(self) -> [usize; 3] {
        match self {
            Self::OneThreeTwo => [0, 2, 1],
            Self::TwoOneThree => [1, 0, 2],
            Self::TwoThreeOne => [1, 2, 0],
        }
    }
}

/// One AES instruction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AesStep {
    Encrypt,
    EncryptLast,
    Decrypt,
    DecryptLast,
    InverseMixColumns,
    KeyGenerationAssist,
}

/// One form of [`AVX_FORMS`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AvxForm {
    pub instruction: Instruction,
    pub operation: AvxOperation,
    /// Where the operand that may be memory lies.
    pub place: OperandPlace,
    pub code: Code,
    /// The bytes the memory operand reaches; 0 for a form without one.
    pub memory_bytes: u8,
    /// Whether a memory operand not aligned to its size raises #GP(0).
    pub aligned: bool,
    /// Whether the form works on 256 bits (VEX.L set).
    pub wide: bool,
}

impl AvxOperation {
    /// Whether the first operand is read rather than only written.
    pub const fn reads_first_operand(self) -> bool {
        matches!(
            self,
            Self::Test
                | Self::TestSigns(_)
                | Self::Fused { .. }
                | Self::DotAccumulate { .. }
                | Self::StringCompare { .. }
                | Self::Float { operation: FloatOperation::OrderedCompare { .. }, .. }
                | Self::LoadMxcsr
        )
    }

    /// Whether the operation writes RFLAGS rather than its first operand.
    pub const fn writes_flags(self) -> bool {
        matches!(
            self,
            Self::Test
                | Self::TestSigns(_)
                | Self::Float { operation: FloatOperation::OrderedCompare { .. }, .. }
                | Self::StringCompare { .. }
        )
    }

    /// Whether the operation computes under MXCSR.
    pub const fn reads_mxcsr(self) -> bool {
        matches!(
            self,
            Self::Float { .. }
                | Self::Compare { .. }
                | Self::ConvertWidening(_)
                | Self::ConvertNarrowing(_)
                | Self::IntegerToScalar(_)
                | Self::ScalarToInteger { .. }
                | Self::RoundToIntegral { .. }
                | Self::Fused { .. }
                | Self::HalfToSingle
                | Self::SingleToHalf
                | Self::StoreMxcsr
                | Self::DotProduct(_)
        )
    }
}

/// Computes an integer operation of the form on its sources, with its immediate; `None` for one that
/// [`crate::simulation`] computes itself (the floating-point ones, memory masks, MXCSR and the zeroing forms).
pub fn compute_integer(form: &AvxForm, sources: &[Wide], immediate: u8) -> Option<Wide> {
    let result = compute_lanes(form, sources, immediate)?;
    Some(crate::planted::mutate_avx(form, sources, immediate, result))
}

/// The result [`compute_integer`] gives with no planted bug on.
pub(crate) fn compute_integer_unplanted(form: &AvxForm, sources: &[Wide], immediate: u8) -> Wide {
    compute_lanes(form, sources, immediate).expect("an integer operation")
}

fn compute_lanes(form: &AvxForm, sources: &[Wide], immediate: u8) -> Option<Wide> {
    let wide = form.wide;
    let first = sources.first().copied().unwrap_or_default();
    let second = sources.get(1).copied().unwrap_or(first);
    let both = |compute: &dyn Fn(usize) -> u128| -> Wide { [compute(0), if wide { compute(1) } else { 0 }] };
    let result = match form.operation {
        AvxOperation::Halves { operation, upper_shift } => both(&|half| {
            let shifted = if half == 0 { immediate } else { immediate >> upper_shift };
            operation.apply(first[half], second[half], shifted)
        }),
        AvxOperation::ShiftByImmediate(operation) => both(&|half| operation.apply(first[half], u128::from(immediate), 0)),
        AvxOperation::ShiftByCount(operation) => both(&|half| operation.apply(first[half], second[0], 0)),
        AvxOperation::ShiftEach { lane, shift } => both(&|half| shift_each(lane, shift, first[half], second[half])),
        AvxOperation::Extend(operation) => {
            let (from, to) = match operation {
                LaneOperation::Extend { from, to, .. } => (from.to_bits(), to.to_bits()),
                _ => panic!("{operation:?} is no extension"),
            };
            both(&|half| operation.apply(0, first[0] >> (half as u32 * u128::BITS * from / to), 0))
        }
        AvxOperation::Test => {
            let halves = if wide { 2 } else { 1 };
            let zero = (0..halves).all(|half| first[half] & second[half] == 0);
            let carry = (0..halves).all(|half| !first[half] & second[half] == 0);
            [u128::from(zero) << 6 | u128::from(carry), 0]
        }
        AvxOperation::TestSigns(lane) => {
            let signs = fill(lane, 1 << (lane.to_bits() - 1));
            let halves = if wide { 2 } else { 1 };
            let zero = (0..halves).all(|half| first[half] & second[half] & signs == 0);
            let carry = (0..halves).all(|half| !first[half] & second[half] & signs == 0);
            [u128::from(zero) << 6 | u128::from(carry), 0]
        }
        AvxOperation::MultipleSumsOfDifferences => {
            both(&|half| sum_differences(first[half], second[half], if half == 0 { immediate } else { immediate >> 3 }))
        }
        AvxOperation::BlendVariable(lane) => {
            let third = sources.get(2).copied().unwrap_or_default();
            both(&|half| {
                let (a, b, c) = (split(lane, first[half]), split(lane, second[half]), split(lane, third[half]));
                let top = 1 << (lane.to_bits() - 1);
                join(lane, (0..a.len()).map(|index| if c[index] & top != 0 { b[index] } else { a[index] }))
            })
        }
        AvxOperation::Broadcast(lane) => {
            let value = split(lane, first[0])[0];
            both(&|_| fill(lane, value))
        }
        AvxOperation::BroadcastHalf => [first[0], first[0]],
        AvxOperation::InsertHalf => {
            let mut result = first;
            result[usize::from(immediate & 1)] = second[0];
            result
        }
        AvxOperation::ExtractHalf => [first[usize::from(immediate & 1)], 0],
        AvxOperation::ExtractLane(lane) => {
            let lanes = split(lane, first[0]);
            [u128::from(lanes[usize::from(immediate) % lanes.len()]), 0]
        }
        AvxOperation::InsertLane(lane) => {
            let mut lanes = split(lane, first[0]);
            let count = lanes.len();
            lanes[usize::from(immediate) % count] = split(lane, second[0])[0];
            [join(lane, lanes.into_iter()), 0]
        }
        AvxOperation::Signs(lane) => {
            let halves = if wide { 2 } else { 1 };
            let signs = (0..halves)
                .flat_map(|half| split(lane, first[half]))
                .enumerate()
                .fold(0u128, |all, (index, value)| all | (u128::from(value >> (lane.to_bits() - 1)) << index));
            [signs, 0]
        }
        AvxOperation::PermuteVariable(lane) => both(&|half| {
            let lanes = split(lane, first[half]);
            let control = split(lane, second[half]);
            let select = |value: u64| if lane == Lane::Quadword { (value >> 1) & 1 } else { value & 3 } as usize;
            join(lane, control.iter().map(|&value| lanes[select(value)]))
        }),
        AvxOperation::PermuteHalves => {
            let choose = |nibble: u8| if nibble & 8 != 0 { 0 } else { [first[0], first[1], second[0], second[1]][usize::from(nibble & 3)] };
            [choose(immediate & 0xF), choose(immediate >> 4)]
        }
        AvxOperation::PermuteDoublewords => {
            let table: Vec<u64> = split(Lane::Doubleword, second[0]).into_iter().chain(split(Lane::Doubleword, second[1])).collect();
            let indices: Vec<u64> = split(Lane::Doubleword, first[0]).into_iter().chain(split(Lane::Doubleword, first[1])).collect();
            let picked: Vec<u64> = indices.iter().map(|&index| table[(index & 7) as usize]).collect();
            [join(Lane::Doubleword, picked[..4].iter().copied()), join(Lane::Doubleword, picked[4..].iter().copied())]
        }
        AvxOperation::PermuteQuadwords => {
            let table = [first[0] as u64, (first[0] >> 64) as u64, first[1] as u64, (first[1] >> 64) as u64];
            let picked: Vec<u64> = (0..4).map(|index| table[usize::from((immediate >> (2 * index)) & 3)]).collect();
            [join(Lane::Quadword, picked[..2].iter().copied()), join(Lane::Quadword, picked[2..].iter().copied())]
        }
        AvxOperation::CarrylessMultiply => both(&|half| {
            let pick = |value: u128, bit: u8| if immediate >> bit & 1 != 0 { (value >> 64) as u64 } else { value as u64 };
            multiply_carryless(pick(first[half], 0), pick(second[half], 4))
        }),
        AvxOperation::Aes(step) => both(&|half| compute_aes(step, first[half], second[half], immediate)),
        AvxOperation::GaloisMultiply => both(&|half| {
            let (a, b) = (split(Lane::Byte, first[half]), split(Lane::Byte, second[half]));
            join(Lane::Byte, a.iter().zip(b).map(|(&x, y)| u64::from(multiply_galois(x as u8, y as u8))))
        }),
        AvxOperation::GaloisAffine { inverse } => both(&|half| {
            let bytes = split(Lane::Byte, first[half]);
            let matrices = split(Lane::Quadword, second[half]);
            join(
                Lane::Byte,
                bytes.iter().enumerate().map(|(index, &byte)| {
                    let byte = if inverse { invert_galois(byte as u8) } else { byte as u8 };
                    u64::from(transform_affine(matrices[index / 8], byte, immediate))
                }),
            )
        }),
        AvxOperation::DotAccumulate { words, saturate } => {
            let third = sources.get(2).copied().unwrap_or_default();
            both(&|half| crate::vnni::accumulate_dots(first[half], second[half], third[half], words, saturate))
        }
        _ => return None,
    };
    Some(result)
}

/// What PCMPxSTRx finds: the result bits (one per element), the element count, and whether each string is
/// shorter than the count.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StringMatch {
    pub result: u16,
    pub elements: u32,
    pub first_short: bool,
    pub second_short: bool,
}

/// Compares two strings as PCMPxSTRx's immediate says: bits 1 to 0 the elements (unsigned or signed bytes or
/// words), bits 3 to 2 the aggregation (equal any, ranges, equal each, equal ordered), bits 5 to 4 the polarity;
/// `lengths` the explicit lengths, or `None` for strings that end at their first null element.
pub fn compare_strings(first: u128, second: u128, lengths: Option<(i64, i64)>, immediate: u8) -> StringMatch {
    let lane = if immediate & 1 != 0 { Lane::Word } else { Lane::Byte };
    let signed = immediate & 2 != 0;
    let elements = u128::BITS / lane.to_bits();
    let read = |value: u128| -> Vec<i64> {
        split(lane, value).into_iter().map(|element| if signed { to_signed(element, lane.to_bits()) } else { element as i64 }).collect()
    };
    let (a, b) = (read(first), read(second));
    let count = elements as usize;
    let length = |explicit: Option<i64>, values: &[i64]| -> usize {
        match explicit {
            Some(length) => length.unsigned_abs().min(elements as u64) as usize,
            None => values.iter().position(|&value| value == 0).unwrap_or(count),
        }
    };
    let (first_length, second_length) = (length(lengths.map(|pair| pair.0), &a), length(lengths.map(|pair| pair.1), &b));
    let (valid_first, valid_second) = (|index: usize| index < first_length, |index: usize| index < second_length);
    let matched = |j: usize| -> bool {
        match (immediate >> 2) & 3 {
            0 => (0..count).any(|i| valid_first(i) && valid_second(j) && a[i] == b[j]),
            1 => (0..count / 2).any(|k| {
                valid_first(2 * k) && valid_first(2 * k + 1) && valid_second(j) && a[2 * k] <= b[j] && b[j] <= a[2 * k + 1]
            }),
            2 => match (valid_first(j), valid_second(j)) {
                (false, false) => true,
                (true, true) => a[j] == b[j],
                _ => false,
            },
            _ => (0..count - j).all(|k| !valid_first(k) || (valid_second(j + k) && a[k] == b[j + k])),
        }
    };
    let found = (0..count).fold(0u16, |bits, j| bits | (u16::from(matched(j)) << j));
    let all = if count == 16 { u16::MAX } else { (1 << count) - 1 };
    let valid_mask = if second_length >= 16 { u16::MAX } else { (1u16 << second_length) - 1 };
    let polarity = if is_active(Bug::StringCompareMaskedPolarityIgnored) { (immediate >> 4) & 1 } else { (immediate >> 4) & 3 };
    let result = match polarity {
        1 => !found & all,
        3 => (found ^ valid_mask) & all,
        _ => found,
    };
    StringMatch { result, elements, first_short: first_length < count, second_short: second_length < count }
}

/// The value in every lane of 128 bits.
pub fn fill(lane: Lane, value: u64) -> u128 {
    join(lane, (0..u128::BITS / lane.to_bits()).map(|_| value))
}

fn shift_each(lane: Lane, shift: EachShift, value: u128, counts: u128) -> u128 {
    let bits = u64::from(lane.to_bits());
    let mask = if bits == 64 { u64::MAX } else { (1 << bits) - 1 };
    join(
        lane,
        split(lane, value).into_iter().zip(split(lane, counts)).map(|(value, count)| match shift {
            EachShift::Left if count >= bits => 0,
            EachShift::Left => (value << count) & mask,
            EachShift::RightLogical if count >= bits => 0,
            EachShift::RightLogical => value >> count,
            EachShift::RightArithmetic => (to_signed(value, bits as u32) >> count.min(bits - 1)) as u64 & mask,
        }),
    )
}

/// MPSADBW of one half: the first source's eleven bytes from 4 times bit 2's offset against the second's four from 4
/// times bits 1 to 0's.
fn sum_differences(first: u128, second: u128, immediate: u8) -> u128 {
    let (a, b) = (split(Lane::Byte, first), split(Lane::Byte, second));
    let (from_first, from_second) = (usize::from((immediate >> 2) & 1) * 4, usize::from(immediate & 3) * 4);
    join(
        Lane::Word,
        (0..8).map(|word| (0..4).map(|byte| a[from_first + word + byte].abs_diff(b[from_second + byte])).sum()),
    )
}

/// The carry-less product of two quadwords.
fn multiply_carryless(first: u64, second: u64) -> u128 {
    (0..u64::BITS).filter(|bit| second >> bit & 1 != 0).fold(0, |product, bit| product ^ (u128::from(first) << bit))
}

/// The product in GF(2^8) modulo x^8 + x^4 + x^3 + x + 1, AES's field and GFNI's.
pub fn multiply_galois(mut first: u8, mut second: u8) -> u8 {
    let mut product = 0;
    while second != 0 {
        if second & 1 != 0 {
            product ^= first;
        }
        first = (first << 1) ^ if first & 0x80 != 0 { 0x1B } else { 0 };
        second >>= 1;
    }
    product
}

/// The inverse in GF(2^8), zero for zero: the value to the power 254.
fn invert_galois(value: u8) -> u8 {
    let mut result = 1;
    let mut power = value;
    let mut exponent = 254u32;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = multiply_galois(result, power);
        }
        power = multiply_galois(power, power);
        exponent >>= 1;
    }
    if value == 0 { 0 } else { result }
}

/// GFNI's affine transformation: bit `bit` of the result is the parity of the matrix's byte `7 - bit` AND the
/// byte, XOR the immediate's bit.
fn transform_affine(matrix: u64, byte: u8, immediate: u8) -> u8 {
    (0..8).fold(0, |result, bit| {
        let row = if is_active(Bug::AffineMatrixRowsReversed) { (matrix >> (8 * bit)) as u8 } else { (matrix >> (8 * (7 - bit))) as u8 };
        let parity = ((row & byte).count_ones() & 1) as u8;
        result | ((parity ^ (immediate >> bit & 1)) << bit)
    })
}

/// AES's S-box, from the inverse in GF(2^8) and the affine map with constant 0x63 that FIPS 197 gives.
fn substitute(byte: u8) -> u8 {
    let inverse = invert_galois(byte);
    let rotated = |count: u32| inverse.rotate_left(count);
    inverse ^ rotated(1) ^ rotated(2) ^ rotated(3) ^ rotated(4) ^ 0x63
}

/// The inverse of [`substitute`].
fn substitute_inverse(byte: u8) -> u8 {
    let mixed = byte.rotate_left(1) ^ byte.rotate_left(3) ^ byte.rotate_left(6) ^ 0x05;
    invert_galois(mixed)
}

/// One AES step on a 16-byte state, the bytes in FIPS 197's order (row `index % 4`, column `index / 4`).
fn compute_aes(step: AesStep, state: u128, key: u128, immediate: u8) -> u128 {
    let bytes = state.to_le_bytes();
    let skipped = is_active(Bug::AesEncryptWithoutShiftRows);
    let shift_rows = |inverse: bool| -> [u8; 16] {
        core::array::from_fn(|index| {
            let (row, column) = (index % 4, index / 4);
            let from = if inverse { (column + 4 - row) % 4 } else if skipped { column } else { (column + row) % 4 };
            bytes[row + 4 * from]
        })
    };
    let mix = |columns: [u8; 16], factors: [u8; 4]| -> [u8; 16] {
        core::array::from_fn(|index| {
            let (row, column) = (index % 4, index / 4);
            (0..4).fold(0, |sum, other| sum ^ multiply_galois(factors[(other + 4 - row) % 4], columns[other + 4 * column]))
        })
    };
    const MIX: [u8; 4] = [2, 3, 1, 1];
    const UNMIX: [u8; 4] = [0x0E, 0x0B, 0x0D, 0x09];
    let result: [u8; 16] = match step {
        AesStep::Encrypt => mix(shift_rows(false).map(substitute), MIX),
        AesStep::EncryptLast => shift_rows(false).map(substitute),
        AesStep::Decrypt => mix(shift_rows(true).map(substitute_inverse), UNMIX),
        AesStep::DecryptLast => shift_rows(true).map(substitute_inverse),
        AesStep::InverseMixColumns => return u128::from_le_bytes(mix(bytes, UNMIX)),
        AesStep::KeyGenerationAssist => {
            let word = |index: usize| u32::from_le_bytes([bytes[4 * index], bytes[4 * index + 1], bytes[4 * index + 2], bytes[4 * index + 3]]);
            let sub_word = |value: u32| u32::from_le_bytes(value.to_le_bytes().map(substitute));
            let constant = u32::from(immediate);
            let (one, three) = (sub_word(word(1)), sub_word(word(3)));
            let words = [one, one.rotate_right(8) ^ constant, three, three.rotate_right(8) ^ constant];
            return words.iter().enumerate().fold(0, |all, (index, &value)| all | (u128::from(value) << (32 * index)));
        }
    };
    u128::from_le_bytes(result) ^ key
}

/// The choices an AVX form leaves open: what a store that faults part-way writes, and how a fused multiply-add
/// treats NaNs; `zeroed` is read by VZEROUPPER and VZEROALL alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_x86_64::avx)
)]
pub struct AvxBehavior {
    pub across_fault: StoreAcrossFault,
    pub fused: FusedChoices,
    pub zeroed: crate::xsave::ZeroedTracking,
}

const STORES: [StoreAcrossFault; 2] = [StoreAcrossFault::WritesNothing, StoreAcrossFault::WritesBytesBelowFault];
const NAN_ORDERS: [FusedNanOrder; 2] = [FusedNanOrder::OperandOrder, FusedNanOrder::FormulaOrder];
const PRODUCTS_OF_ZERO: [InfiniteProductOfZero; 2] = [InfiniteProductOfZero::Signals, InfiniteProductOfZero::Quiet];

impl AvxForm {
    /// Whether the form's memory access is checked for alignment while AC is set: every one but F16C's.
    pub const fn checks_alignment(&self) -> bool {
        !matches!(self.operation, AvxOperation::HalfToSingle | AvxOperation::SingleToHalf)
    }

    /// Whether the form stores to its memory operand.
    pub fn writes_memory(&self) -> bool {
        self.place == OperandPlace::Memory
            && self.operation != AvxOperation::LoadMxcsr
            && matches!(
                self.code.op_code().op_kind(0),
                OpCodeOperandKind::mem | OpCodeOperandKind::xmm_or_mem | OpCodeOperandKind::ymm_or_mem | OpCodeOperandKind::r32_or_mem | OpCodeOperandKind::r64_or_mem
            )
    }
}

/// Every behavior of an AVX form: each way a store may treat a fault part-way, for a form that stores, and each
/// NaN rule of a fused multiply-add; none for any other form.
pub(crate) fn list_avx_behaviors(instruction: Instruction) -> Vec<PreciseInstruction> {
    let Some(form) = find_avx_form(instruction) else {
        return Vec::new();
    };
    // An aligned store or one of a single byte never lies across a page boundary.
    let splits = (form.writes_memory() && !form.aligned && form.memory_bytes > 1) || form.operation == AvxOperation::MaskedByteStore;
    let stores: &[StoreAcrossFault] = if splits { &STORES } else { &STORES[..1] };
    let fused = matches!(form.operation, AvxOperation::Fused { .. });
    let orders: &[FusedNanOrder] = if fused { &NAN_ORDERS } else { &NAN_ORDERS[..1] };
    let products: &[InfiniteProductOfZero] = if fused { &PRODUCTS_OF_ZERO } else { &PRODUCTS_OF_ZERO[..1] };
    let zeroes = matches!(form.operation, AvxOperation::ZeroUpper | AvxOperation::ZeroAll);
    let trackings = if zeroes { &crate::xsave::ZEROED_TRACKINGS[..] } else { &crate::xsave::ZEROED_TRACKINGS[..1] };
    let mut behaviors = Vec::new();
    for &across_fault in stores {
        for &nan_order in orders {
            for &product_of_zero in products {
                let fused = FusedChoices { nan_order, product_of_zero };
                for &zeroed in trackings {
                    behaviors.push(PreciseInstruction::Avx { instruction, behavior: AvxBehavior { across_fault, fused, zeroed } });
                }
            }
        }
    }
    behaviors
}

/// The row of a form of this module, or `None` for any other form.
pub fn find_avx_form(instruction: Instruction) -> Option<&'static AvxForm> {
    AVX_FORMS.iter().find(|form| form.instruction == instruction)
}

const fn form(
    instruction: Instruction,
    operation: AvxOperation,
    place: OperandPlace,
    code: Code,
    memory_bytes: u8,
    aligned: bool,
    wide: bool,
) -> AvxForm {
    AvxForm { instruction, operation, place, code, memory_bytes, aligned, wide }
}

use Arrangement::{Packed, Scalar};
use AvxOperation::{
    Aes, BlendVariable, Broadcast, BroadcastHalf, CarrylessMultiply, Compare, ConvertNarrowing, ConvertWidening,
    Extend, ExtractHalf, ExtractLane, Float, Fused, GaloisAffine, GaloisMultiply, HalfToSingle, Halves, InsertHalf,
    InsertLane, IntegerToScalar, LoadMxcsr, MaskedLoad, MaskedStore, MultipleSumsOfDifferences, PermuteDoublewords,
    PermuteHalves, PermuteQuadwords, PermuteVariable, RoundToIntegral, ScalarToInteger, ShiftByCount, ShiftByImmediate,
    ShiftEach, Signs, SingleToHalf, StoreMxcsr, Test, TestSigns, ZeroAll, ZeroUpper, DotProduct, StringCompare,
    MaskedByteStore, Gather, DotAccumulate,
};
use Conversion::{DoubleToSingle, FloatToInteger, IntegerToFloat, SingleToDouble};
use FloatOperation::{
    Add, AddSubtract, Convert, Divide, HorizontalAdd, HorizontalSubtract, Maximum, Minimum, Multiply, OrderedCompare, SquareRoot,
    Subtract,
};
use FusedOrder::{OneThreeTwo, TwoOneThree, TwoThreeOne};
use FusedSign::{
    MultiplyAdd, MultiplyAddSubtract, MultiplySubtract, MultiplySubtractAdd, NegatedMultiplyAdd, NegatedMultiplySubtract,
};
use Lane::{Byte, Doubleword, Quadword, Word};
use LaneOperation::{
    Absolute, AlignRight, Blend, DuplicateEvenSingles, DuplicateLowDouble, DuplicateOddSingles, InsertSingle,
    MinimumWordPosition, Move, MoveHighToLow, MoveLow, MoveLowToHigh, MoveLowZeroing, ShiftBytesLeft, ShiftBytesRight,
    ShiftLeft, ShiftRightArithmetic, ShiftRightLogical, ShuffleDoubles, ShuffleDoublewords, ShuffleHighWords,
    ShuffleLowWords, ShuffleSingles,
};
use OperandPlace::{Memory, Register};
use Precision::{Double, Single};
use crate::vector::Signedness::{Signed, Unsigned};

include!("avx_forms.rs");
