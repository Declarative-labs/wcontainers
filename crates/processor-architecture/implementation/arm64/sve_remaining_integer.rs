//! The SVE and SVE2 integer instructions of `sve_remaining`, element by element as Arm's pseudocode loops over them (for
//! example [SQRDMLAH (indexed)](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/SQRDMLAH--indexed---Signed-saturating-rounding-doubling-multiply-add-high-to-accumulate--indexed--)).
//!
//! SVE's saturating instructions leave FPSR.QC alone.

use core::convert::Infallible;

use crate::cryptography::{CryptographyOperation, simulate_cryptography};
use crate::encoding::Field;
use crate::planted::is as planted;
use crate::sve_remaining::{
    IndexLayout, SIZE, VECTOR_BITS, ZD, ZM, ZN, is_active, read_lane, read_predicate, read_vector,
    saturate, to_integer, to_mask, write_lane,
};
use crate::{Exception, Instruction, Processor};

/// The size bit of an encoding whose elements are words or doublewords.
pub const SZ: Field = Field::new(22, 1);
/// The rotation of CMLA, SQRDCMLAH and CDOT.
pub const ROTATION: Field = Field::new(10, 2);
/// The rotation of CADD and SQCADD.
pub const ADD_ROTATION: Field = Field::new(10, 1);
/// tszh and tszl of a shift by an immediate, which give its element size.
pub const TSZH: Field = Field::new(22, 1);
pub const TSZL: Field = Field::new(19, 2);
/// The low bits of a shift by an immediate.
pub const IMM3: Field = Field::new(16, 3);
/// ADR's shift of its offsets.
pub const MSZ: Field = Field::new(10, 2);

/// The element a multiply takes from its second source: the same element, or the one an index names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SecondElement {
    Vector,
    Indexed(IndexLayout),
}

/// What a multiply of elements of one size does with the product.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SameMultiply {
    /// MLA and MLS.
    Accumulate {
        subtract: bool,
    },
    Multiply,
    /// SQRDMLAH and SQRDMLSH: the destination above the doubled product, rounded to its high half and saturated.
    RoundingDoublingAccumulate {
        subtract: bool,
    },
    /// SQDMULH and SQRDMULH.
    DoublingHigh {
        rounding: bool,
    },
}

/// What a multiply of half-width elements into elements twice as wide does with the product.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LongMultiply {
    /// SMLALB and its forms.
    Accumulate {
        subtract: bool,
    },
    /// SMULLB and its forms.
    Multiply,
    /// SQDMLALB and its forms: the doubled product saturated, then the sum.
    SaturatingDoublingAccumulate {
        subtract: bool,
    },
    SaturatingDoubling,
}

/// How a narrowing shift or extract saturates its result.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Narrowing {
    /// The low half of the unsigned result: SHRN and RSHRN.
    Truncating,
    /// A signed source to a signed result.
    Signed,
    /// A signed source to an unsigned result.
    SignedToUnsigned,
    Unsigned,
}

/// What a bit permute does with each element.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BitPermute {
    Extract,
    Deposit,
    Group,
}

/// How ADR takes its offsets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AddressOffset {
    /// Elements of the size bit's size, the offset unsigned.
    Same,
    /// Doublewords, the offset the low word, signed or unsigned.
    LowWord { signed: bool },
}

/// One AES step of the cryptography module.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AesStep {
    Encrypt,
    Decrypt,
    MixColumns,
    InverseMixColumns,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RemainingInteger {
    /// MLA, MLS, MUL, SQRDMLAH, SQRDMLSH, SQDMULH and SQRDMULH by element, and SQRDMLAH and SQRDMLSH of vectors.
    Multiply {
        operation: SameMultiply,
        second: SecondElement,
        bits: Option<u32>,
    },
    /// The long multiplies by element: the bottom (even) or top (odd) half-width elements of the first source.
    MultiplyLong {
        operation: LongMultiply,
        layout: IndexLayout,
        narrow_bits: u32,
        signed: bool,
        top: bool,
    },
    /// SDOT, UDOT, USDOT and SUDOT: each element plus the four products of the quarter-width elements in it.
    Dot {
        first_signed: bool,
        second_signed: bool,
        second: SecondElement,
        bits: Option<u32>,
    },
    /// CDOT: each element plus the complex products of the two pairs of quarter-width elements in it.
    ComplexDot {
        second: SecondElement,
        bits: Option<u32>,
    },
    /// CMLA and SQRDCMLAH, of vectors or by element.
    ComplexMultiplyAccumulate {
        saturating: bool,
        second: SecondElement,
        bits: Option<u32>,
    },
    /// SMMLA, USMMLA and UMMLA: a 2 by 8 matrix of bytes times an 8 by 2 one, added to a 2 by 2 one of words.
    MatrixMultiply {
        first_signed: bool,
        second_signed: bool,
    },
    /// SSHLLB, SSHLLT, USHLLB and USHLLT.
    ShiftLeftLong {
        signed: bool,
        top: bool,
    },
    /// The narrowing shifts right by an immediate.
    ShiftRightNarrow {
        narrowing: Narrowing,
        rounding: bool,
        top: bool,
    },
    /// SQXTNB and its forms.
    ExtractNarrow {
        narrowing: Narrowing,
        top: bool,
    },
    Address(AddressOffset),
    /// PMULLB and PMULLT of bytes into halfwords or words into doublewords.
    PolynomialMultiplyLong {
        top: bool,
    },
    BitPermute(BitPermute),
    /// ADCLB, ADCLT, SBCLB and SBCLT.
    LongCarry {
        subtract: bool,
        top: bool,
    },
    /// CADD and SQCADD.
    ComplexAdd {
        saturating: bool,
    },
    HistogramSegment,
    HistogramCount,
    Aes(AesStep),
    /// RAX1.
    RotateAndExclusiveOr,
}

/// The operation of each modeled integer instruction, none for every other instruction.
pub const fn to_remaining_integer(
    instruction: Instruction<Infallible>,
) -> Option<RemainingInteger> {
    use IndexLayout::{HighThree, One, Two};
    use Instruction as I;
    use RemainingInteger as R;
    use SecondElement::{Indexed, Vector};
    const fn same(operation: SameMultiply, layout: IndexLayout, bits: u32) -> RemainingInteger {
        RemainingInteger::Multiply {
            operation,
            second: SecondElement::Indexed(layout),
            bits: Some(bits),
        }
    }
    const fn long(
        operation: LongMultiply,
        wide: bool,
        signed: bool,
        top: bool,
    ) -> RemainingInteger {
        RemainingInteger::MultiplyLong {
            operation,
            layout: if wide {
                IndexLayout::LongTwo
            } else {
                IndexLayout::LongThree
            },
            narrow_bits: if wide { u32::BITS } else { u16::BITS },
            signed,
            top,
        }
    }
    const fn dot(
        first_signed: bool,
        second_signed: bool,
        second: SecondElement,
        bits: Option<u32>,
    ) -> RemainingInteger {
        RemainingInteger::Dot {
            first_signed,
            second_signed,
            second,
            bits,
        }
    }
    const fn shift(narrowing: Narrowing, rounding: bool, top: bool) -> RemainingInteger {
        RemainingInteger::ShiftRightNarrow {
            narrowing,
            rounding,
            top,
        }
    }
    use LongMultiply as L;
    use Narrowing as N;
    use SameMultiply as S;
    let (h, w, d) = (u16::BITS, u32::BITS, u64::BITS);
    let (b, t) = (false, true);
    let (unsigned, signed) = (false, true);
    let accumulate = S::Accumulate { subtract: false };
    let subtract = S::Accumulate { subtract: true };
    let rounding_add = S::RoundingDoublingAccumulate { subtract: false };
    let rounding_subtract = S::RoundingDoublingAccumulate { subtract: true };
    let add_long = L::Accumulate { subtract: false };
    let subtract_long = L::Accumulate { subtract: true };
    let saturating_add = L::SaturatingDoublingAccumulate { subtract: false };
    let saturating_subtract = L::SaturatingDoublingAccumulate { subtract: true };
    Some(match instruction {
        I::MlaZZZI16 => same(accumulate, HighThree, h),
        I::MlaZZZI32 => same(accumulate, Two, w),
        I::MlaZZZI64 => same(accumulate, One, d),
        I::MlsZZZI16 => same(subtract, HighThree, h),
        I::MlsZZZI32 => same(subtract, Two, w),
        I::MlsZZZI64 => same(subtract, One, d),
        I::MulZZZI16 => same(S::Multiply, HighThree, h),
        I::MulZZZI32 => same(S::Multiply, Two, w),
        I::MulZZZI64 => same(S::Multiply, One, d),
        I::SqrdmlahZZZI16 => same(rounding_add, HighThree, h),
        I::SqrdmlahZZZI => same(rounding_add, Two, w),
        I::SqrdmlahZZZI64 => same(rounding_add, One, d),
        I::SqrdmlshZZZI16 => same(rounding_subtract, HighThree, h),
        I::SqrdmlshZZZI => same(rounding_subtract, Two, w),
        I::SqrdmlshZZZI64 => same(rounding_subtract, One, d),
        I::SqdmulhZZZI16 => same(S::DoublingHigh { rounding: false }, HighThree, h),
        I::SqdmulhZZZI => same(S::DoublingHigh { rounding: false }, Two, w),
        I::SqdmulhZZZI64 => same(S::DoublingHigh { rounding: false }, One, d),
        I::SqrdmulhZZZI16 => same(S::DoublingHigh { rounding: true }, HighThree, h),
        I::SqrdmulhZZZI => same(S::DoublingHigh { rounding: true }, Two, w),
        I::SqrdmulhZZZI64 => same(S::DoublingHigh { rounding: true }, One, d),
        I::SqrdmlahZZZ => R::Multiply {
            operation: rounding_add,
            second: Vector,
            bits: None,
        },
        I::SqrdmlshZZZ => R::Multiply {
            operation: rounding_subtract,
            second: Vector,
            bits: None,
        },
        I::SmlalbZZZI => long(add_long, b, signed, b),
        I::SmlalbZZZI64 => long(add_long, t, signed, b),
        I::SmlaltZZZI => long(add_long, b, signed, t),
        I::SmlaltZZZI64 => long(add_long, t, signed, t),
        I::SmlslbZZZI => long(subtract_long, b, signed, b),
        I::SmlslbZZZI64 => long(subtract_long, t, signed, b),
        I::SmlsltZZZI => long(subtract_long, b, signed, t),
        I::SmlsltZZZI64 => long(subtract_long, t, signed, t),
        I::UmlalbZZZI32 => long(add_long, b, unsigned, b),
        I::UmlalbZZZI64 => long(add_long, t, unsigned, b),
        I::UmlaltZZZI32 => long(add_long, b, unsigned, t),
        I::UmlaltZZZI64 => long(add_long, t, unsigned, t),
        I::UmlslbZZZI32 => long(subtract_long, b, unsigned, b),
        I::UmlslbZZZI64 => long(subtract_long, t, unsigned, b),
        I::UmlsltZZZI32 => long(subtract_long, b, unsigned, t),
        I::UmlsltZZZI64 => long(subtract_long, t, unsigned, t),
        I::SmullbZZZI => long(L::Multiply, b, signed, b),
        I::SmullbZZZI64 => long(L::Multiply, t, signed, b),
        I::SmulltZZZI => long(L::Multiply, b, signed, t),
        I::SmulltZZZI64 => long(L::Multiply, t, signed, t),
        I::UmullbZZZI32 => long(L::Multiply, b, unsigned, b),
        I::UmullbZZZI64 => long(L::Multiply, t, unsigned, b),
        I::UmulltZZZI32 => long(L::Multiply, b, unsigned, t),
        I::UmulltZZZI64 => long(L::Multiply, t, unsigned, t),
        I::SqdmlalbZZZI => long(saturating_add, b, signed, b),
        I::SqdmlalbZZZI64 => long(saturating_add, t, signed, b),
        I::SqdmlaltZZZI => long(saturating_add, b, signed, t),
        I::SqdmlaltZZZI64 => long(saturating_add, t, signed, t),
        I::SqdmlslbZZZI => long(saturating_subtract, b, signed, b),
        I::SqdmlslbZZZI64 => long(saturating_subtract, t, signed, b),
        I::SqdmlsltZZZI => long(saturating_subtract, b, signed, t),
        I::SqdmlsltZZZI64 => long(saturating_subtract, t, signed, t),
        I::SqdmullbZZZI => long(L::SaturatingDoubling, b, signed, b),
        I::SqdmullbZZZI64 => long(L::SaturatingDoubling, t, signed, b),
        I::SqdmulltZZZI => long(L::SaturatingDoubling, b, signed, t),
        I::SqdmulltZZZI64 => long(L::SaturatingDoubling, t, signed, t),
        I::SdotZZZ => dot(signed, signed, Vector, None),
        I::UdotZZZ => dot(unsigned, unsigned, Vector, None),
        I::UsdotZZZ => dot(unsigned, signed, Vector, Some(w)),
        I::SdotZZZIBtoS => dot(signed, signed, Indexed(Two), Some(w)),
        I::SdotZZZIHtoD => dot(signed, signed, Indexed(One), Some(d)),
        I::UdotZZZIBtoS => dot(unsigned, unsigned, Indexed(Two), Some(w)),
        I::UdotZZZIHtoD => dot(unsigned, unsigned, Indexed(One), Some(d)),
        I::UsdotZZZI => dot(unsigned, signed, Indexed(Two), Some(w)),
        I::SudotZZZI => dot(signed, unsigned, Indexed(Two), Some(w)),
        I::CdotZZZ => R::ComplexDot {
            second: Vector,
            bits: None,
        },
        I::CdotZZZI32 => R::ComplexDot {
            second: Indexed(Two),
            bits: Some(w),
        },
        I::CdotZZZI64 => R::ComplexDot {
            second: Indexed(One),
            bits: Some(d),
        },
        I::CmlaZZZ => R::ComplexMultiplyAccumulate {
            saturating: false,
            second: Vector,
            bits: None,
        },
        I::CmlaZZZI16 => R::ComplexMultiplyAccumulate {
            saturating: false,
            second: Indexed(Two),
            bits: Some(h),
        },
        I::CmlaZZZI32 => R::ComplexMultiplyAccumulate {
            saturating: false,
            second: Indexed(One),
            bits: Some(w),
        },
        I::SqrdcmlahZZZ => R::ComplexMultiplyAccumulate {
            saturating: true,
            second: Vector,
            bits: None,
        },
        I::SqrdcmlahZZZI16 => R::ComplexMultiplyAccumulate {
            saturating: true,
            second: Indexed(Two),
            bits: Some(h),
        },
        I::SqrdcmlahZZZI => R::ComplexMultiplyAccumulate {
            saturating: true,
            second: Indexed(One),
            bits: Some(w),
        },
        I::SmmlaZZZ => R::MatrixMultiply {
            first_signed: signed,
            second_signed: signed,
        },
        I::UsmmlaZZZ => R::MatrixMultiply {
            first_signed: unsigned,
            second_signed: signed,
        },
        I::UmmlaZZZ => R::MatrixMultiply {
            first_signed: unsigned,
            second_signed: unsigned,
        },
        I::Sshllb => R::ShiftLeftLong { signed, top: b },
        I::Sshllt => R::ShiftLeftLong { signed, top: t },
        I::Ushllb => R::ShiftLeftLong {
            signed: unsigned,
            top: b,
        },
        I::Ushllt => R::ShiftLeftLong {
            signed: unsigned,
            top: t,
        },
        I::Shrnb => shift(N::Truncating, false, b),
        I::Shrnt => shift(N::Truncating, false, t),
        I::Rshrnb => shift(N::Truncating, true, b),
        I::Rshrnt => shift(N::Truncating, true, t),
        I::Sqshrnb => shift(N::Signed, false, b),
        I::Sqshrnt => shift(N::Signed, false, t),
        I::Sqrshrnb => shift(N::Signed, true, b),
        I::Sqrshrnt => shift(N::Signed, true, t),
        I::Sqshrunb => shift(N::SignedToUnsigned, false, b),
        I::Sqshrunt => shift(N::SignedToUnsigned, false, t),
        I::Sqrshrunb => shift(N::SignedToUnsigned, true, b),
        I::Sqrshrunt => shift(N::SignedToUnsigned, true, t),
        I::Uqshrnb => shift(N::Unsigned, false, b),
        I::Uqshrnt => shift(N::Unsigned, false, t),
        I::Uqrshrnb => shift(N::Unsigned, true, b),
        I::Uqrshrnt => shift(N::Unsigned, true, t),
        I::Sqxtnb => R::ExtractNarrow {
            narrowing: N::Signed,
            top: b,
        },
        I::Sqxtnt => R::ExtractNarrow {
            narrowing: N::Signed,
            top: t,
        },
        I::Sqxtunb => R::ExtractNarrow {
            narrowing: N::SignedToUnsigned,
            top: b,
        },
        I::Sqxtunt => R::ExtractNarrow {
            narrowing: N::SignedToUnsigned,
            top: t,
        },
        I::Uqxtnb => R::ExtractNarrow {
            narrowing: N::Unsigned,
            top: b,
        },
        I::Uqxtnt => R::ExtractNarrow {
            narrowing: N::Unsigned,
            top: t,
        },
        I::AdrZAzSdSameScaled => R::Address(AddressOffset::Same),
        I::AdrZAzDS32Scaled => R::Address(AddressOffset::LowWord { signed: true }),
        I::AdrZAzDU32Scaled => R::Address(AddressOffset::LowWord { signed: false }),
        I::PmullbZZZ => R::PolynomialMultiplyLong { top: b },
        I::PmulltZZZ => R::PolynomialMultiplyLong { top: t },
        I::Bext => R::BitPermute(BitPermute::Extract),
        I::Bdep => R::BitPermute(BitPermute::Deposit),
        I::Bgrp => R::BitPermute(BitPermute::Group),
        I::Adclb => R::LongCarry {
            subtract: false,
            top: b,
        },
        I::Adclt => R::LongCarry {
            subtract: false,
            top: t,
        },
        I::Sbclb => R::LongCarry {
            subtract: true,
            top: b,
        },
        I::Sbclt => R::LongCarry {
            subtract: true,
            top: t,
        },
        I::Cadd => R::ComplexAdd { saturating: false },
        I::Sqcadd => R::ComplexAdd { saturating: true },
        I::Histseg => R::HistogramSegment,
        I::Histcnt => R::HistogramCount,
        I::AeseZZZ8 => R::Aes(AesStep::Encrypt),
        I::AesdZZZ8 => R::Aes(AesStep::Decrypt),
        I::AesmcZZ8 => R::Aes(AesStep::MixColumns),
        I::AesimcZZ8 => R::Aes(AesStep::InverseMixColumns),
        I::Rax1ZZZ64 => R::RotateAndExclusiveOr,
        _ => return None,
    })
}

/// The bits of the size field's elements: a byte shifted left by it.
fn read_size_bits(word: u32) -> u32 {
    u8::BITS << SIZE.read(word)
}

/// The tsize of a shift by an immediate, tszh:tszl.
pub fn read_tsize(word: u32) -> u32 {
    (TSZH.read(word) << TSZL.width) | TSZL.read(word)
}

/// The narrower element size of a shift by an immediate in bits, eight shifted left by tsize's highest set bit; none
/// where tsize is zero.
pub fn to_shift_narrow_bits(word: u32) -> Option<u32> {
    let tsize = read_tsize(word);
    (tsize != 0).then(|| u8::BITS << (u32::BITS - 1 - tsize.leading_zeros()))
}

/// The size of the elements the operation computes in bits (the wider for a long or narrowing form); none where the
/// word is unallocated.
pub fn to_integer_bits(operation: RemainingInteger, word: u32) -> Option<u32> {
    use RemainingInteger as R;
    match operation {
        R::Multiply {
            bits: Some(bits), ..
        }
        | R::Dot {
            bits: Some(bits), ..
        }
        | R::ComplexDot {
            bits: Some(bits), ..
        }
        | R::ComplexMultiplyAccumulate {
            bits: Some(bits), ..
        } => Some(bits),
        R::Multiply { bits: None, .. } | R::ComplexMultiplyAccumulate { bits: None, .. } => {
            Some(read_size_bits(word))
        }
        // The size bit beside a size<1> of one: words or doublewords.
        R::Dot { bits: None, .. } => Some(u32::BITS << SZ.read(word)),
        // A size of 0x is unallocated.
        R::ComplexDot { bits: None, .. } | R::HistogramCount => {
            (SIZE.read(word) >= 2).then(|| read_size_bits(word))
        }
        R::MultiplyLong { narrow_bits, .. } => Some(2 * narrow_bits),
        R::MatrixMultiply { .. } => Some(u32::BITS),
        R::ShiftLeftLong { .. } | R::ShiftRightNarrow { .. } => {
            to_shift_narrow_bits(word).map(|bits| 2 * bits)
        }
        R::ExtractNarrow { .. } => match read_tsize(word) {
            0b001 => Some(u16::BITS),
            0b010 => Some(u32::BITS),
            0b100 => Some(u64::BITS),
            _ => None,
        },
        R::Address(AddressOffset::Same) | R::LongCarry { .. } => Some(u32::BITS << SZ.read(word)),
        R::Address(AddressOffset::LowWord { .. }) | R::RotateAndExclusiveOr => Some(u64::BITS),
        // Size 01 gives halfwords of bytes and 11 doublewords of words; 10 is unallocated, and 00 is PMULLB's 128-bit
        // form, an encoding of its own.
        R::PolynomialMultiplyLong { .. } => match SIZE.read(word) {
            0b01 => Some(u16::BITS),
            0b11 => Some(u64::BITS),
            _ => None,
        },
        R::BitPermute(_) | R::ComplexAdd { .. } => Some(read_size_bits(word)),
        R::HistogramSegment | R::Aes(_) => Some(u8::BITS),
    }
}

/// Applies the operation to the registers the word names; changes nothing where it returns an exception.
pub fn simulate_remaining_integer<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operation: RemainingInteger,
) -> Result<(), Exception> {
    use RemainingInteger as R;
    let Some(bits) = to_integer_bits(operation, word) else {
        return Err(Exception::UndefinedInstruction);
    };
    let n = read_vector(processor, word, ZN);
    let m = read_vector(processor, word, ZM);
    let d = read_vector(processor, word, ZD);
    let lanes = VECTOR_BITS / bits;
    let result = match operation {
        R::Multiply {
            operation, second, ..
        } => {
            let (second_register, index) = read_second(processor, word, second, m);
            let mut result = 0;
            for lane in 0..lanes {
                let first = read_lane(n, lane, bits);
                let second = read_lane(second_register, index.unwrap_or(lane), bits);
                let addend = read_lane(d, lane, bits);
                write_lane(
                    &mut result,
                    lane,
                    bits,
                    multiply_same(operation, first, second, addend, bits),
                );
            }
            result
        }
        R::MultiplyLong {
            operation,
            layout,
            narrow_bits,
            signed,
            top,
        } => {
            let element = read_lane(
                layout.read_register(processor, word),
                layout.read_index(word),
                narrow_bits,
            );
            let mut result = 0;
            for lane in 0..lanes {
                let taken = 2 * lane + u32::from(top != planted(3001));
                let first = to_integer(read_lane(n, taken, narrow_bits), narrow_bits, signed);
                let second = to_integer(element, narrow_bits, signed);
                let addend = to_integer(read_lane(d, lane, bits), bits, true);
                write_lane(
                    &mut result,
                    lane,
                    bits,
                    multiply_long(operation, first, second, addend, bits),
                );
            }
            result
        }
        R::Dot {
            first_signed,
            second_signed,
            second,
            ..
        } => {
            let (second_register, index) = read_second(processor, word, second, m);
            let quarter = bits / 4;
            let mut result = 0;
            for lane in 0..lanes {
                let pair = index.unwrap_or(lane);
                let mut sum = read_lane(d, lane, bits) as i128;
                for place in 0..4 {
                    let first = to_integer(
                        read_lane(n, 4 * lane + place, quarter),
                        quarter,
                        first_signed,
                    );
                    let second_signed = second_signed != (planted(3002) && !first_signed);
                    let second = to_integer(
                        read_lane(second_register, 4 * pair + place, quarter),
                        quarter,
                        second_signed,
                    );
                    sum = sum.wrapping_add(first * second);
                }
                write_lane(&mut result, lane, bits, sum as u128);
            }
            result
        }
        R::ComplexDot { second, .. } => {
            let (second_register, index) = read_second(processor, word, second, m);
            let rotation = ROTATION.read(word);
            let first_part = rotation & 1;
            let subtracts = rotation & 1 == rotation >> 1;
            let quarter = bits / 4;
            let mut result = 0;
            for lane in 0..lanes {
                let pair = index.unwrap_or(lane);
                let mut sum = read_lane(d, lane, bits) as i128;
                for half in 0..2 {
                    let element = |vector: u128, base: u32, place: u32| {
                        to_integer(
                            read_lane(vector, 4 * base + 2 * half + place, quarter),
                            quarter,
                            true,
                        )
                    };
                    let real = element(n, lane, 0);
                    let imaginary = element(n, lane, 1);
                    let second_a = element(second_register, pair, first_part);
                    let second_b = element(second_register, pair, 1 - first_part);
                    sum = if subtracts && !planted(3003) {
                        sum.wrapping_add(real * second_a)
                            .wrapping_sub(imaginary * second_b)
                    } else {
                        sum.wrapping_add(real * second_a)
                            .wrapping_add(imaginary * second_b)
                    };
                }
                write_lane(&mut result, lane, bits, sum as u128);
            }
            result
        }
        R::ComplexMultiplyAccumulate {
            saturating, second, ..
        } => {
            let (second_register, index) = read_second(processor, word, second, m);
            let rotation = ROTATION.read(word);
            let first_part = rotation & 1;
            let subtract_real = (rotation & 1) != (rotation >> 1);
            let subtract_imaginary = rotation >> 1 == 1;
            let mut result = 0;
            for pair in 0..lanes / 2 {
                let other = index.unwrap_or(pair);
                let first = to_integer(read_lane(n, 2 * pair + first_part, bits), bits, true);
                let second_a = to_integer(
                    read_lane(second_register, 2 * other + first_part, bits),
                    bits,
                    true,
                );
                let second_b = to_integer(
                    read_lane(second_register, 2 * other + 1 - first_part, bits),
                    bits,
                    true,
                );
                let products = [first * second_a, first * second_b];
                let subtracts = [subtract_real, subtract_imaginary != planted(3004)];
                for part in 0..2 {
                    let addend = read_lane(d, 2 * pair + part, bits);
                    let product = if subtracts[part as usize] {
                        -products[part as usize]
                    } else {
                        products[part as usize]
                    };
                    let value = if saturating {
                        let addend = to_integer(addend, bits, true);
                        saturate(addend + round_high(product, bits), bits, true)
                    } else {
                        (addend as i128).wrapping_add(product) as u128
                    };
                    write_lane(&mut result, 2 * pair + part, bits, value);
                }
            }
            result
        }
        R::MatrixMultiply {
            first_signed,
            second_signed,
        } => {
            let mut result = 0;
            for row in 0..2 {
                for column in 0..2 {
                    let mut sum = read_lane(d, 2 * row + column, u32::BITS) as i128;
                    for place in 0..8 {
                        let first = to_integer(
                            read_lane(n, 8 * row + place, u8::BITS),
                            u8::BITS,
                            first_signed,
                        );
                        let second = to_integer(
                            read_lane(m, 8 * column + place, u8::BITS),
                            u8::BITS,
                            second_signed != planted(3005),
                        );
                        sum = sum.wrapping_add(first * second);
                    }
                    write_lane(&mut result, 2 * row + column, u32::BITS, sum as u128);
                }
            }
            result
        }
        R::ShiftLeftLong { signed, top } => {
            let narrow = bits / 2;
            let shift = ((read_tsize(word) << IMM3.width) | IMM3.read(word)) - narrow;
            let mut result = 0;
            for lane in 0..lanes {
                let value = to_integer(
                    read_lane(n, 2 * lane + u32::from(top), narrow),
                    narrow,
                    signed,
                );
                let shift = if planted(3006) { shift + 1 } else { shift };
                write_lane(&mut result, lane, bits, (value << shift) as u128);
            }
            result
        }
        R::ShiftRightNarrow {
            narrowing,
            rounding,
            top,
        } => {
            let narrow = bits / 2;
            let shift = 2 * narrow - ((read_tsize(word) << IMM3.width) | IMM3.read(word));
            let mut result = if top { d } else { 0 };
            for lane in 0..lanes {
                let signed = matches!(narrowing, Narrowing::Signed | Narrowing::SignedToUnsigned);
                let value = to_integer(read_lane(n, lane, bits), bits, signed);
                let rounded = if rounding && !planted(3007) {
                    value + (1 << (shift - 1))
                } else {
                    value
                };
                let shifted = rounded >> shift;
                write_lane(
                    &mut result,
                    2 * lane + u32::from(top),
                    narrow,
                    narrow_value(narrowing, shifted, narrow),
                );
            }
            result
        }
        R::ExtractNarrow { narrowing, top } => {
            let narrow = bits / 2;
            let mut result = if top { d } else { 0 };
            for lane in 0..lanes {
                let signed = matches!(narrowing, Narrowing::Signed | Narrowing::SignedToUnsigned);
                let value = to_integer(read_lane(n, lane, bits), bits, signed != planted(3008));
                write_lane(
                    &mut result,
                    2 * lane + u32::from(top),
                    narrow,
                    narrow_value(narrowing, value, narrow),
                );
            }
            result
        }
        R::Address(offset) => {
            let scale = MSZ.read(word);
            let (offset_bits, signed) = match offset {
                AddressOffset::Same => (bits, false),
                AddressOffset::LowWord { signed } => (u32::BITS, signed),
            };
            let mut result = 0;
            for lane in 0..lanes {
                let base = read_lane(n, lane, bits) as i128;
                let offset = to_integer(
                    read_lane(m, lane, bits),
                    offset_bits,
                    signed != planted(3009),
                );
                write_lane(
                    &mut result,
                    lane,
                    bits,
                    base.wrapping_add(offset << scale) as u128,
                );
            }
            result
        }
        R::PolynomialMultiplyLong { top } => {
            let narrow = bits / 2;
            let mut result = 0;
            for lane in 0..lanes {
                let taken = 2 * lane + u32::from(top);
                let first = read_lane(n, taken, narrow);
                let second = read_lane(m, taken, narrow);
                let product = (0..narrow).fold(0_u128, |product, place| {
                    if (first >> place) & 1 == 1 && !(planted(3010) && place == narrow - 1) {
                        product ^ (second << place)
                    } else {
                        product
                    }
                });
                write_lane(&mut result, lane, bits, product);
            }
            result
        }
        R::BitPermute(permute) => {
            let mut result = 0;
            for lane in 0..lanes {
                write_lane(
                    &mut result,
                    lane,
                    bits,
                    permute_bits(
                        permute,
                        read_lane(n, lane, bits),
                        read_lane(m, lane, bits),
                        bits,
                    ),
                );
            }
            result
        }
        R::LongCarry { subtract, top } => {
            let mut result = d;
            for pair in 0..lanes / 2 {
                let first = read_lane(d, 2 * pair, bits);
                let second = read_lane(n, 2 * pair + u32::from(top), bits);
                let second = if subtract {
                    !second & to_mask(bits)
                } else {
                    second
                };
                let carry_in = read_lane(m, 2 * pair + u32::from(!planted(3011)), bits) & 1;
                let sum = first + second + carry_in;
                write_lane(&mut result, 2 * pair, bits, sum);
                write_lane(&mut result, 2 * pair + 1, bits, sum >> bits);
            }
            result
        }
        R::ComplexAdd { saturating } => {
            let rotate_270 = ADD_ROTATION.read(word) == 1;
            let mut result = 0;
            for pair in 0..lanes / 2 {
                let element = |vector: u128, part: u32| {
                    to_integer(read_lane(vector, 2 * pair + part, bits), bits, true)
                };
                let (real, imaginary) = (element(d, 0), element(d, 1));
                let (second_real, second_imaginary) = (
                    element(m_low(processor, word), 0),
                    element(m_low(processor, word), 1),
                );
                let (real, imaginary) = if rotate_270 != planted(3012) {
                    (real + second_imaginary, imaginary - second_real)
                } else {
                    (real - second_imaginary, imaginary + second_real)
                };
                for (part, value) in [(0, real), (1, imaginary)] {
                    let value = if saturating {
                        saturate(value, bits, true)
                    } else {
                        value as u128
                    };
                    write_lane(&mut result, 2 * pair + part, bits, value);
                }
            }
            result
        }
        R::HistogramSegment => {
            let mut result = 0;
            for lane in 0..lanes {
                let first = read_lane(n, lane, bits);
                let count = (0..lanes)
                    .filter(|other| {
                        read_lane(m, *other, bits) == first && !(planted(3013) && *other == lane)
                    })
                    .count();
                write_lane(&mut result, lane, bits, count as u128);
            }
            result
        }
        R::HistogramCount => {
            let predicate = read_predicate(processor, word);
            let mut result = 0;
            for lane in (0..lanes).filter(|lane| is_active(predicate, *lane, bits)) {
                let first = read_lane(n, lane, bits);
                let last = if planted(3014) { lanes - 1 } else { lane };
                let count = (0..=last)
                    .filter(|other| {
                        is_active(predicate, *other, bits) && read_lane(m, *other, bits) == first
                    })
                    .count();
                write_lane(&mut result, lane, bits, count as u128);
            }
            result
        }
        R::Aes(step) => {
            // Zdn and Zm lie where the Advanced SIMD forms keep Vd and Vn; a mix of columns reads Zdn as Vn too.
            let (operation, word) = match step {
                AesStep::Encrypt => (CryptographyOperation::AesEncrypt, word),
                AesStep::Decrypt => (CryptographyOperation::AesDecrypt, word),
                AesStep::MixColumns if planted(3021) => {
                    (CryptographyOperation::AesMixColumns, word)
                }
                AesStep::MixColumns => (
                    CryptographyOperation::AesMixColumns,
                    with_source_destination(word),
                ),
                AesStep::InverseMixColumns => (
                    CryptographyOperation::AesInverseMixColumns,
                    with_source_destination(word),
                ),
            };
            return simulate_cryptography(processor, word, operation);
        }
        R::RotateAndExclusiveOr => {
            let mut result = 0;
            for lane in 0..lanes {
                let first = read_lane(n, lane, bits) as u64;
                let second = read_lane(m, lane, bits) as u64;
                let rotation = if planted(3015) { 2 } else { 1 };
                write_lane(
                    &mut result,
                    lane,
                    bits,
                    u128::from(first ^ second.rotate_left(rotation)),
                );
            }
            result
        }
    };
    processor.vector_registers[ZD.read(word) as usize] = result;
    Ok(())
}

/// The second source of CADD and SQCADD, at bits 9 to 5.
fn m_low<LevelSpecific>(processor: &Processor<LevelSpecific>, word: u32) -> u128 {
    read_vector(processor, word, ZN)
}

/// The word with Rn set to its Rd, so that an Advanced SIMD model of one source reads Zdn.
const fn with_source_destination(word: u32) -> u32 {
    let destination = word & ZD.to_mask();
    (word & !ZN.to_mask()) | (destination << ZN.lowest_bit)
}

/// The register a multiply takes its second elements from, and the index of the one element it takes, if any.
fn read_second<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    word: u32,
    second: SecondElement,
    m: u128,
) -> (u128, Option<u32>) {
    match second {
        SecondElement::Vector => (m, None),
        SecondElement::Indexed(layout) => {
            let index = layout.read_index(word);
            let index = if planted(3016) && layout == IndexLayout::HighThree {
                index & 0b011
            } else {
                index
            };
            (layout.read_register(processor, word), Some(index))
        }
    }
}

/// The high half of twice the product, rounded to nearest with ties up: the shared step of SQRDMLAH and SQRDCMLAH,
/// floor((2 * product + 2 ^ (bits - 1)) / 2 ^ bits), written so that no step overflows at 64 bits.
fn round_high(product: i128, bits: u32) -> i128 {
    let rounding = if planted(3017) {
        0
    } else {
        1_i128 << (bits - 2)
    };
    (product + rounding) >> (bits - 1)
}

fn multiply_same(
    operation: SameMultiply,
    first: u128,
    second: u128,
    addend: u128,
    bits: u32,
) -> u128 {
    let signed = |value: u128| to_integer(value, bits, true);
    match operation {
        SameMultiply::Accumulate { subtract } => {
            let product = first.wrapping_mul(second);
            if subtract {
                addend.wrapping_sub(product)
            } else {
                addend.wrapping_add(product)
            }
        }
        SameMultiply::Multiply => first.wrapping_mul(second),
        SameMultiply::RoundingDoublingAccumulate { subtract } => {
            let product = signed(first) * signed(second);
            let product = if subtract { -product } else { product };
            saturate(signed(addend) + round_high(product, bits), bits, true)
        }
        SameMultiply::DoublingHigh { rounding } => {
            let product = signed(first) * signed(second);
            let rounded = if rounding {
                round_high(product, bits)
            } else {
                product >> (bits - 1)
            };
            saturate(rounded, bits, !planted(3018))
        }
    }
}

fn multiply_long(
    operation: LongMultiply,
    first: i128,
    second: i128,
    addend: i128,
    bits: u32,
) -> u128 {
    let product = first * second;
    match operation {
        LongMultiply::Accumulate { subtract: false } => addend.wrapping_add(product) as u128,
        LongMultiply::Accumulate { subtract: true } => addend.wrapping_sub(product) as u128,
        LongMultiply::Multiply => product as u128,
        LongMultiply::SaturatingDoublingAccumulate { subtract } => {
            let doubled = to_integer(saturate(2 * product, bits, true), bits, true);
            let doubled = if planted(3019) { 2 * product } else { doubled };
            let sum = if subtract {
                addend - doubled
            } else {
                addend + doubled
            };
            saturate(sum, bits, true)
        }
        LongMultiply::SaturatingDoubling => saturate(2 * product, bits, true),
    }
}

/// A shifted or extracted value as its narrower element.
fn narrow_value(narrowing: Narrowing, value: i128, narrow: u32) -> u128 {
    match narrowing {
        Narrowing::Truncating => value as u128 & to_mask(narrow),
        Narrowing::Signed => saturate(value, narrow, true),
        Narrowing::SignedToUnsigned | Narrowing::Unsigned => saturate(value, narrow, false),
    }
}

/// [BitExtract](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-sve#impl-aarch64.BitExtract.2),
/// BitDeposit and BitGroup of one element.
fn permute_bits(permute: BitPermute, data: u128, mask: u128, bits: u32) -> u128 {
    let bit = |value: u128, place: u32| (value >> place) & 1;
    let mut result = 0;
    let mut placed = 0;
    match permute {
        BitPermute::Deposit => {
            for place in 0..bits {
                if bit(mask, place) == 1 {
                    result |= bit(data, placed) << place;
                    placed += 1;
                }
            }
        }
        BitPermute::Extract | BitPermute::Group => {
            for place in (0..bits).filter(|place| bit(mask, *place) == 1) {
                result |= bit(data, place) << placed;
                placed += 1;
            }
            if permute == BitPermute::Group && !planted(3020) {
                for place in (0..bits).filter(|place| bit(mask, *place) == 0) {
                    result |= bit(data, place) << placed;
                    placed += 1;
                }
            }
        }
    }
    result
}
