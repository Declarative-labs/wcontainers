//! The SVE and SVE2 floating-point instructions of `sve_remaining`, element by element as Arm's pseudocode loops over
//! them (for example [FADDP](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/FADDP--Floating-point-add-pairwise-)),
//! each element computed by the lane functions of `floating_point`.
//!
//! An inactive element keeps the destination's and raises no flag.

use core::convert::Infallible;

use crate::encoding::Field;
use crate::floating_point::lanes::{
    LaneOperation, compute_lane, convert_to_brain_float, dot_add_brain, multiply_add_halves,
};
use crate::floating_point::scalable_elements::{
    Conversion, convert_element, multiply_add_element, to_constant,
};
use crate::floating_point::sve_remaining_elements::compute_logarithm;
use crate::floating_point::{FloatingPointOperation, Precision};
use crate::planted::is as planted;
use crate::sve_remaining::{
    IndexLayout, SIZE, VECTOR_BITS, ZD, ZM, ZM_LOW, ZN, is_active, read_lane, read_predicate,
    read_vector, write_lane,
};
use crate::{Exception, Instruction, Processor};

/// The rotation of FCMLA (vectors).
pub const MULTIPLY_ROTATION: Field = Field::new(13, 2);
/// The rotation of FCMLA (indexed).
pub const INDEXED_ROTATION: Field = Field::new(10, 2);
/// The rotation of FCADD.
pub const ADD_ROTATION: Field = Field::new(16, 1);
/// FLOGB's element size.
pub const LOGARITHM_SIZE: Field = Field::new(17, 2);
/// FTMAD's coefficient.
pub const COEFFICIENT: Field = Field::new(16, 3);

/// The element an indexed form takes from its second source.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SecondElement {
    Vector,
    Indexed(IndexLayout),
}

/// What a reduction or a pairwise instruction applies to two elements.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PairOperation {
    Add,
    Maximum,
    Minimum,
    MaximumNumber,
    MinimumNumber,
}

impl PairOperation {
    const fn to_scalar(self) -> FloatingPointOperation {
        match self {
            Self::Add => FloatingPointOperation::Add,
            Self::Maximum => FloatingPointOperation::Maximum,
            Self::Minimum => FloatingPointOperation::Minimum,
            Self::MaximumNumber => FloatingPointOperation::MaximumNumber,
            Self::MinimumNumber => FloatingPointOperation::MinimumNumber,
        }
    }

    /// The value an inactive element of a reduction takes: zero, negative or positive infinity, or the default NaN.
    fn to_identity(self, precision: Precision) -> u64 {
        match self {
            Self::Add => 0,
            Self::Maximum => precision.to_infinity(true),
            Self::Minimum => precision.to_infinity(crate::planted::is(3122)),
            Self::MaximumNumber | Self::MinimumNumber => precision.to_default_nan(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RemainingFloatingPoint {
    /// FADDP and its forms: each active element from a pair of the first source's below it, or of the second's.
    Pairwise(PairOperation),
    /// FMLA and FMLS (indexed): the destination plus the product, the first source negated where `subtract`.
    IndexedMultiplyAdd {
        bits: u32,
        subtract: bool,
    },
    IndexedMultiply {
        bits: u32,
    },
    /// FCMLA, of vectors under a predicate or by element.
    ComplexMultiplyAdd {
        second: SecondElement,
        bits: Option<u32>,
    },
    ComplexAdd,
    /// FADDV and its forms: inactive elements the identity, then halves combined down to one element.
    Reduce(PairOperation),
    /// FADDA: the scalar plus each active element in turn.
    OrderedAdd,
    /// FRECPE and FRSQRTE.
    Estimate {
        square_root: bool,
    },
    /// FCVTNT, FCVTXNT and BFCVTNT: each converted element in the top half of its container.
    ConvertNarrowTop {
        from: Precision,
        to: Precision,
        odd: bool,
    },
    /// FCVTLT: the top half of each container converted to its whole.
    ConvertLongTop {
        from: Precision,
        to: Precision,
    },
    /// FCVTX: a double to a single rounding to odd, zero-extended.
    ConvertToOdd,
    /// BFCVT: each converted element in the bottom half of its container, the top half zero.
    ConvertToBrainFloat,
    Logarithm,
    TrigonometricSelect,
    ExponentialAccelerator,
    TrigonometricMultiply,
    TrigonometricMultiplyAdd,
    /// BFDOT.
    BrainFloatDot {
        second: SecondElement,
    },
    /// BFMLALB and BFMLALT.
    BrainFloatMultiplyAdd {
        second: SecondElement,
        top: bool,
    },
    BrainFloatMatrixMultiply,
    /// FMLALB, FMLALT, FMLSLB and FMLSLT.
    WideningMultiplyAdd {
        second: SecondElement,
        top: bool,
        subtract: bool,
    },
}

/// The operation of each modeled floating-point instruction, none for every other instruction.
pub const fn to_remaining_floating_point(
    instruction: Instruction<Infallible>,
) -> Option<RemainingFloatingPoint> {
    use IndexLayout::{LongThree, One, Two};
    use Instruction as I;
    use PairOperation as P;
    use Precision::{BFloat16, Double, Half, Single};
    use RemainingFloatingPoint as R;
    use SecondElement::{Indexed, Vector};
    let (h, w, d) = (u16::BITS, u32::BITS, u64::BITS);
    const fn widening(second: SecondElement, top: bool, subtract: bool) -> RemainingFloatingPoint {
        RemainingFloatingPoint::WideningMultiplyAdd {
            second,
            top,
            subtract,
        }
    }
    Some(match instruction {
        I::FaddpZPmZZ => R::Pairwise(P::Add),
        I::FmaxnmpZPmZZ => R::Pairwise(P::MaximumNumber),
        I::FminnmpZPmZZ => R::Pairwise(P::MinimumNumber),
        I::FmaxpZPmZZ => R::Pairwise(P::Maximum),
        I::FminpZPmZZ => R::Pairwise(P::Minimum),
        I::FmlaZZZI16 => R::IndexedMultiplyAdd {
            bits: h,
            subtract: false,
        },
        I::FmlaZZZI32 => R::IndexedMultiplyAdd {
            bits: w,
            subtract: false,
        },
        I::FmlaZZZI64 => R::IndexedMultiplyAdd {
            bits: d,
            subtract: false,
        },
        I::FmlsZZZI16 => R::IndexedMultiplyAdd {
            bits: h,
            subtract: true,
        },
        I::FmlsZZZI32 => R::IndexedMultiplyAdd {
            bits: w,
            subtract: true,
        },
        I::FmlsZZZI64 => R::IndexedMultiplyAdd {
            bits: d,
            subtract: true,
        },
        I::FmulZZZI16 => R::IndexedMultiply { bits: h },
        I::FmulZZZI32 => R::IndexedMultiply { bits: w },
        I::FmulZZZI64 => R::IndexedMultiply { bits: d },
        I::FcmlaZPmZZ => R::ComplexMultiplyAdd {
            second: Vector,
            bits: None,
        },
        I::FcmlaZZZI16 => R::ComplexMultiplyAdd {
            second: Indexed(Two),
            bits: Some(h),
        },
        I::FcmlaZZZI32 => R::ComplexMultiplyAdd {
            second: Indexed(One),
            bits: Some(w),
        },
        I::FcaddZPmZ => R::ComplexAdd,
        I::Faddv => R::Reduce(P::Add),
        I::FmaxnmvVPZ => R::Reduce(P::MaximumNumber),
        I::FminnmvVPZ => R::Reduce(P::MinimumNumber),
        I::FmaxvVPZ => R::Reduce(P::Maximum),
        I::FminvVPZ => R::Reduce(P::Minimum),
        I::Fadda => R::OrderedAdd,
        I::FrecpeZZ => R::Estimate { square_root: false },
        I::FrsqrteZZ => R::Estimate { square_root: true },
        I::FcvtntZPZS2h => R::ConvertNarrowTop {
            from: Single,
            to: Half,
            odd: false,
        },
        I::FcvtntZPZD2s => R::ConvertNarrowTop {
            from: Double,
            to: Single,
            odd: false,
        },
        I::FcvtxntZPmZDtoS => R::ConvertNarrowTop {
            from: Double,
            to: Single,
            odd: true,
        },
        I::FcvtltZPZH2s => R::ConvertLongTop {
            from: Half,
            to: Single,
        },
        I::FcvtltZPZS2d => R::ConvertLongTop {
            from: Single,
            to: Double,
        },
        I::FcvtxZPmZDtoS => R::ConvertToOdd,
        I::BfcvtZPmZ => R::ConvertToBrainFloat,
        I::BfcvtntZPmZ => R::ConvertNarrowTop {
            from: Single,
            to: BFloat16,
            odd: false,
        },
        I::FlogbZPmZ => R::Logarithm,
        I::Ftssel => R::TrigonometricSelect,
        I::Fexpa => R::ExponentialAccelerator,
        I::Ftsmul => R::TrigonometricMultiply,
        I::Ftmad => R::TrigonometricMultiplyAdd,
        I::BfdotZZZ => R::BrainFloatDot { second: Vector },
        I::BfdotZZI => R::BrainFloatDot {
            second: Indexed(Two),
        },
        I::BfmlalbZZZ => R::BrainFloatMultiplyAdd {
            second: Vector,
            top: false,
        },
        I::BfmlaltZZZ => R::BrainFloatMultiplyAdd {
            second: Vector,
            top: true,
        },
        I::BfmlalbZZZI => R::BrainFloatMultiplyAdd {
            second: Indexed(LongThree),
            top: false,
        },
        I::BfmlaltZZZI => R::BrainFloatMultiplyAdd {
            second: Indexed(LongThree),
            top: true,
        },
        I::BfmmlaZZZHtoS => R::BrainFloatMatrixMultiply,
        I::FmlalbZZZSHH => widening(Vector, false, false),
        I::FmlaltZZZSHH => widening(Vector, true, false),
        I::FmlslbZZZSHH => widening(Vector, false, true),
        I::FmlsltZZZSHH => widening(Vector, true, true),
        I::FmlalbZZZISHH => widening(Indexed(LongThree), false, false),
        I::FmlaltZZZISHH => widening(Indexed(LongThree), true, false),
        I::FmlslbZZZISHH => widening(Indexed(LongThree), false, true),
        I::FmlsltZZZISHH => widening(Indexed(LongThree), true, true),
        _ => return None,
    })
}

/// Where an indexed FMLA, FMLS or FMUL of elements this wide keeps its index.
pub const fn to_index_layout(bits: u32) -> IndexLayout {
    match bits {
        16 => IndexLayout::HighThree,
        32 => IndexLayout::Two,
        _ => IndexLayout::One,
    }
}

/// The precision of a size field's value: half, single and double for 01, 10 and 11; none for 00.
pub const fn to_size_precision(size: u32) -> Option<Precision> {
    match size {
        1 => Some(Precision::Half),
        2 => Some(Precision::Single),
        3 => Some(Precision::Double),
        _ => None,
    }
}

const fn to_bits_precision(bits: u32) -> Precision {
    match bits {
        16 => Precision::Half,
        32 => Precision::Single,
        _ => Precision::Double,
    }
}

/// The precision of the elements the operation computes, the source's for a conversion; none where the word is
/// unallocated.
pub fn to_element_precision(operation: RemainingFloatingPoint, word: u32) -> Option<Precision> {
    use RemainingFloatingPoint as R;
    match operation {
        R::IndexedMultiplyAdd { bits, .. }
        | R::IndexedMultiply { bits }
        | R::ComplexMultiplyAdd {
            bits: Some(bits), ..
        } => Some(to_bits_precision(bits)),
        R::ConvertNarrowTop { from, .. } | R::ConvertLongTop { from, .. } => Some(from),
        R::ConvertToOdd => Some(Precision::Double),
        R::ConvertToBrainFloat
        | R::BrainFloatDot { .. }
        | R::BrainFloatMultiplyAdd { .. }
        | R::BrainFloatMatrixMultiply
        | R::WideningMultiplyAdd { .. } => Some(Precision::Single),
        R::Logarithm => to_size_precision(LOGARITHM_SIZE.read(word)),
        _ => to_size_precision(SIZE.read(word)),
    }
}

/// The bits of the elements the operation computes, the container's for a conversion; none where it is unallocated.
pub fn to_floating_point_bits(operation: RemainingFloatingPoint, word: u32) -> Option<u32> {
    use RemainingFloatingPoint as R;
    let precision = to_element_precision(operation, word)?;
    Some(match operation {
        R::ConvertLongTop { to, .. } => to.to_bits(),
        _ => precision.to_bits(),
    })
}

/// Applies the operation to the registers the word names, adding the flags its active elements raise to FPSR; changes
/// nothing where it returns an exception.
pub fn simulate_remaining_floating_point<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operation: RemainingFloatingPoint,
) -> Result<(), Exception> {
    use RemainingFloatingPoint as R;
    let (Some(precision), Some(bits)) = (
        to_element_precision(operation, word),
        to_floating_point_bits(operation, word),
    ) else {
        return Err(Exception::UndefinedInstruction);
    };
    let fpcr = processor.floating_point_control;
    let mut flags = 0;
    let predicate = read_predicate(processor, word);
    let active = |lane: u32| is_active(predicate, lane, bits);
    let n = read_vector(processor, word, ZN);
    let m = read_vector(processor, word, ZM);
    let m_low = read_vector(processor, word, ZM_LOW);
    let d = read_vector(processor, word, ZD);
    let lanes = VECTOR_BITS / bits;
    let lane = |vector: u128, index: u32| read_lane(vector, index, bits) as u64;
    let sign = 1_u64 << (bits - 1);
    let scalar = |operation: FloatingPointOperation, sources: &[u64], flags: &mut u64| {
        compute_lane(
            LaneOperation::Scalar(operation),
            sources,
            precision,
            fpcr,
            flags,
        )
    };
    let result = match operation {
        R::Pairwise(pair) => {
            let mut result = d;
            for index in (0..lanes).filter(|index| active(*index)) {
                let (first, second) = if index % 2 == 0 {
                    (lane(d, index), lane(d, index + 1))
                } else if planted(3102) {
                    (lane(d, index - 1), lane(d, index))
                } else {
                    (lane(m_low, index - 1), lane(m_low, index))
                };
                let value = scalar(pair.to_scalar(), &[first, second], &mut flags);
                write_lane(&mut result, index, bits, u128::from(value));
            }
            result
        }
        R::IndexedMultiplyAdd { bits, subtract } => {
            let layout = to_index_layout(bits);
            let element = lane(
                layout.read_register(processor, word),
                layout.read_index(word),
            );
            let mut result = 0;
            for index in 0..lanes {
                let value = multiply_add_element(
                    [lane(n, index), element, lane(d, index)],
                    subtract != planted(3103),
                    false,
                    precision,
                    fpcr,
                    &mut flags,
                );
                write_lane(&mut result, index, bits, u128::from(value));
            }
            result
        }
        R::IndexedMultiply { bits } => {
            let layout = to_index_layout(bits);
            let index = layout.read_index(word);
            let index = if planted(3104) { 0 } else { index };
            let element = lane(layout.read_register(processor, word), index);
            let mut result = 0;
            for index in 0..lanes {
                let value = scalar(
                    FloatingPointOperation::Multiply,
                    &[lane(n, index), element],
                    &mut flags,
                );
                write_lane(&mut result, index, bits, u128::from(value));
            }
            result
        }
        R::ComplexMultiplyAdd { second, .. } => {
            let (rotation, second_register, index) = match second {
                SecondElement::Vector => (MULTIPLY_ROTATION.read(word), m, None),
                SecondElement::Indexed(layout) => (
                    INDEXED_ROTATION.read(word),
                    layout.read_register(processor, word),
                    Some(if planted(3123) {
                        0
                    } else {
                        layout.read_index(word)
                    }),
                ),
            };
            let first_part = rotation & 1;
            let negate_imaginary = rotation >> 1 == 1;
            let negate_real = (rotation & 1) != (rotation >> 1);
            let predicated = second == SecondElement::Vector;
            let mut result = 0;
            for pair in 0..lanes / 2 {
                let other = index.unwrap_or(pair);
                let first = lane(n, 2 * pair + first_part);
                let second_a = lane(second_register, 2 * other + first_part);
                let second_b = lane(second_register, 2 * other + 1 - first_part);
                let parts = [
                    (second_a, negate_real != planted(3105)),
                    (second_b, negate_imaginary),
                ];
                for (part, (second, negate)) in parts.into_iter().enumerate() {
                    let place = 2 * pair + part as u32;
                    let addend = lane(d, place);
                    let value = if predicated && !active(place) {
                        addend
                    } else {
                        let second = if negate { second ^ sign } else { second };
                        multiply_add_element(
                            [first, second, addend],
                            false,
                            false,
                            precision,
                            fpcr,
                            &mut flags,
                        )
                    };
                    write_lane(&mut result, place, bits, u128::from(value));
                }
            }
            result
        }
        R::ComplexAdd => {
            let rotate_270 = ADD_ROTATION.read(word) == 1;
            let mut result = 0;
            for pair in 0..lanes / 2 {
                let (real, imaginary) = (lane(d, 2 * pair), lane(d, 2 * pair + 1));
                let (second_real, second_imaginary) =
                    (lane(m_low, 2 * pair), lane(m_low, 2 * pair + 1));
                let real = if active(2 * pair) {
                    let addend = if rotate_270 {
                        second_imaginary
                    } else {
                        second_imaginary ^ sign
                    };
                    scalar(FloatingPointOperation::Add, &[real, addend], &mut flags)
                } else {
                    real
                };
                let imaginary = if active(2 * pair + 1) {
                    let addend = if rotate_270 != planted(3106) {
                        second_real ^ sign
                    } else {
                        second_real
                    };
                    scalar(
                        FloatingPointOperation::Add,
                        &[imaginary, addend],
                        &mut flags,
                    )
                } else {
                    imaginary
                };
                write_lane(&mut result, 2 * pair, bits, u128::from(real));
                write_lane(&mut result, 2 * pair + 1, bits, u128::from(imaginary));
            }
            result
        }
        R::Reduce(pair) => {
            let identity = pair.to_identity(precision);
            let elements: alloc::vec::Vec<u64> = (0..lanes)
                .map(|index| {
                    if active(index) {
                        lane(n, index)
                    } else {
                        identity
                    }
                })
                .collect();
            u128::from(reduce(pair, &elements, precision, fpcr, &mut flags))
        }
        R::OrderedAdd => {
            let mut sum = lane(d, 0);
            for index in (0..lanes).filter(|index| active(*index)) {
                let (first, second) = if planted(3107) {
                    (lane(m_low, index), sum)
                } else {
                    (sum, lane(m_low, index))
                };
                sum = scalar(FloatingPointOperation::Add, &[first, second], &mut flags);
            }
            u128::from(sum)
        }
        R::Estimate { square_root } => {
            let operation = if square_root || planted(3120) {
                LaneOperation::ReciprocalSquareRootEstimate
            } else {
                LaneOperation::ReciprocalEstimate
            };
            let mut result = 0;
            for index in 0..lanes {
                let value = compute_lane(operation, &[lane(n, index)], precision, fpcr, &mut flags);
                write_lane(&mut result, index, bits, u128::from(value));
            }
            result
        }
        R::ConvertNarrowTop { to, odd, .. } => {
            let mut result = d;
            let half = bits / 2;
            for index in (0..lanes).filter(|index| active(*index)) {
                let source = lane(n, index);
                let value = if odd {
                    compute_lane(
                        LaneOperation::ConvertToOdd,
                        &[source],
                        precision,
                        fpcr,
                        &mut flags,
                    )
                } else if to == Precision::BFloat16 {
                    convert_to_brain_float(source, fpcr, &mut flags)
                } else {
                    convert_element(
                        Conversion::Precision {
                            from: precision,
                            to,
                        },
                        source,
                        fpcr,
                        &mut flags,
                    )
                };
                let place = 2 * index + u32::from(!planted(3108));
                write_lane(&mut result, place, half, u128::from(value));
            }
            result
        }
        R::ConvertLongTop { from, to } => {
            let mut result = d;
            let half = bits / 2;
            for index in (0..lanes).filter(|index| active(*index)) {
                let source = read_lane(n, 2 * index + u32::from(!planted(3109)), half) as u64;
                let value =
                    convert_element(Conversion::Precision { from, to }, source, fpcr, &mut flags);
                write_lane(&mut result, index, bits, u128::from(value));
            }
            result
        }
        R::ConvertToOdd => {
            let mut result = d;
            for index in (0..lanes).filter(|index| active(*index)) {
                let value = if planted(3121) {
                    let to = Precision::Single;
                    convert_element(
                        Conversion::Precision {
                            from: precision,
                            to,
                        },
                        lane(n, index),
                        fpcr,
                        &mut flags,
                    )
                } else {
                    compute_lane(
                        LaneOperation::ConvertToOdd,
                        &[lane(n, index)],
                        precision,
                        fpcr,
                        &mut flags,
                    )
                };
                write_lane(&mut result, index, bits, u128::from(value));
            }
            result
        }
        R::ConvertToBrainFloat => {
            let mut result = d;
            let half = bits / 2;
            for index in (0..lanes).filter(|index| active(*index)) {
                let value = convert_to_brain_float(lane(n, index), fpcr, &mut flags);
                write_lane(&mut result, 2 * index, half, u128::from(value));
                if !planted(3110) {
                    write_lane(&mut result, 2 * index + 1, half, 0);
                }
            }
            result
        }
        R::Logarithm => {
            let mut result = d;
            for index in (0..lanes).filter(|index| active(*index)) {
                let value = compute_logarithm(lane(n, index), precision, fpcr, &mut flags);
                write_lane(&mut result, index, bits, u128::from(value));
            }
            result
        }
        R::TrigonometricSelect => {
            let mut result = 0;
            for index in 0..lanes {
                let (first, second) = (lane(n, index), lane(m, index));
                // FPTrigSSel: one of the sign op2<1> where op2<0> is set, else op1 negated where op2<1> is set.
                let value = if second & 1 == 1 {
                    to_constant(0, second & 2 != 0, precision)
                } else if second & 2 != 0 && !planted(3111) {
                    first ^ sign
                } else {
                    first
                };
                write_lane(&mut result, index, bits, u128::from(value));
            }
            result
        }
        R::ExponentialAccelerator => {
            let mut result = 0;
            for index in 0..lanes {
                write_lane(
                    &mut result,
                    index,
                    bits,
                    u128::from(accelerate_exponential(lane(n, index), precision)),
                );
            }
            result
        }
        R::TrigonometricMultiply => {
            let mut result = 0;
            for index in 0..lanes {
                let first = lane(n, index);
                let square = scalar(
                    FloatingPointOperation::Multiply,
                    &[first, first],
                    &mut flags,
                );
                let value = if is_nan(square, precision) {
                    square
                } else if lane(m, index) & 1 == 1 && !planted(3112) {
                    square | sign
                } else {
                    square & !sign
                };
                write_lane(&mut result, index, bits, u128::from(value));
            }
            result
        }
        R::TrigonometricMultiplyAdd => {
            let chosen = COEFFICIENT.read(word);
            let mut result = 0;
            for index in 0..lanes {
                let second = lane(m_low, index);
                let row = chosen
                    + if second & sign != 0 && !planted(3113) {
                        8
                    } else {
                        0
                    };
                let coefficient = to_trigonometric_coefficient(row, precision);
                let value = multiply_add_element(
                    [lane(d, index), second & !sign, coefficient],
                    false,
                    false,
                    precision,
                    fpcr,
                    &mut flags,
                );
                write_lane(&mut result, index, bits, u128::from(value));
            }
            result
        }
        R::BrainFloatDot { second } => {
            let (second_register, index) = read_second(processor, word, second, m);
            let mut result = 0;
            for lane_index in 0..lanes {
                let other = index.unwrap_or(lane_index);
                let half = |vector: u128, place: u32| read_lane(vector, place, u16::BITS) as u64;
                let first = [half(n, 2 * lane_index), half(n, 2 * lane_index + 1)];
                let second = [
                    half(second_register, 2 * other),
                    half(second_register, 2 * other + 1),
                ];
                let second = if planted(3114) {
                    [second[1], second[0]]
                } else {
                    second
                };
                let value = dot_add_brain(lane(d, lane_index), first, second);
                write_lane(&mut result, lane_index, bits, u128::from(value));
            }
            result
        }
        R::BrainFloatMultiplyAdd { second, top } => {
            let (second_register, index) = read_second(processor, word, second, m);
            let taken = u32::from(top);
            let mut result = 0;
            for lane_index in 0..lanes {
                let first = read_lane(n, 2 * lane_index + taken, u16::BITS) as u64;
                let other = match index {
                    Some(index) => index,
                    None => 2 * lane_index + taken,
                };
                let second = read_lane(second_register, other, u16::BITS) as u64;
                let value = multiply_add_element(
                    [first << u16::BITS, second << u16::BITS, lane(d, lane_index)],
                    false,
                    planted(3115),
                    Precision::Single,
                    fpcr,
                    &mut flags,
                );
                write_lane(&mut result, lane_index, bits, u128::from(value));
            }
            result
        }
        R::BrainFloatMatrixMultiply => {
            let half = |vector: u128, place: u32| read_lane(vector, place, u16::BITS) as u64;
            let mut result = 0;
            for row in 0..2 {
                for column in 0..2 {
                    let mut sum = lane(d, 2 * row + column);
                    for pair in 0..2 {
                        let first = [half(n, 4 * row + 2 * pair), half(n, 4 * row + 2 * pair + 1)];
                        let base = if planted(3116) {
                            2 * column + 4 * pair
                        } else {
                            4 * column + 2 * pair
                        };
                        let second = [half(m, base), half(m, base + 1)];
                        sum = dot_add_brain(sum, first, second);
                    }
                    write_lane(&mut result, 2 * row + column, bits, u128::from(sum));
                }
            }
            result
        }
        R::WideningMultiplyAdd {
            second,
            top,
            subtract,
        } => {
            let (second_register, index) = read_second(processor, word, second, m);
            let taken = u32::from(top);
            let mut result = 0;
            for lane_index in 0..lanes {
                let first = read_lane(n, 2 * lane_index + taken, u16::BITS) as u64;
                let first = if subtract {
                    first ^ (1 << (u16::BITS - 1))
                } else {
                    first
                };
                let other = match index {
                    Some(index) if !planted(3117) => index,
                    _ => 2 * lane_index + taken,
                };
                let second = read_lane(second_register, other, u16::BITS) as u64;
                let value =
                    multiply_add_halves(lane(d, lane_index), first, second, fpcr, &mut flags);
                write_lane(&mut result, lane_index, bits, u128::from(value));
            }
            result
        }
    };
    processor.vector_registers[ZD.read(word) as usize] = result;
    processor.floating_point_status |= flags;
    Ok(())
}

/// The register an indexed form takes its second elements from, and the index of the element it takes, if any.
fn read_second<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    word: u32,
    second: SecondElement,
    m: u128,
) -> (u128, Option<u32>) {
    match second {
        SecondElement::Vector => (m, None),
        SecondElement::Indexed(layout) => (
            layout.read_register(processor, word),
            Some(layout.read_index(word)),
        ),
    }
}

/// [FPReduce](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-reduceop#impl-aarch64.FPReduce.4):
/// the lower half's result combined with the upper half's, each the same of its own halves.
fn reduce(
    pair: PairOperation,
    elements: &[u64],
    precision: Precision,
    fpcr: u64,
    flags: &mut u64,
) -> u64 {
    if let [only] = elements {
        return *only;
    }
    let (low, high) = elements.split_at(elements.len() / 2);
    let high = reduce(pair, high, precision, fpcr, flags);
    let low = reduce(pair, low, precision, fpcr, flags);
    let (first, second) = if planted(3118) {
        (high, low)
    } else {
        (low, high)
    };
    compute_lane(
        LaneOperation::Scalar(pair.to_scalar()),
        &[first, second],
        precision,
        fpcr,
        flags,
    )
}

fn is_nan(bits: u64, precision: Precision) -> bool {
    let fraction_bits = precision.to_fraction_bits();
    let exponent_ones = (1 << precision.to_exponent_bits()) - 1;
    (bits >> fraction_bits) & exponent_ones == exponent_ones
        && bits & ((1 << fraction_bits) - 1) != 0
}

/// [FPExpA](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-sve#impl-aarch64.FPExpA.1):
/// the operand's bits above its table index as the exponent, the table's entry as the fraction.
fn accelerate_exponential(operand: u64, precision: Precision) -> u64 {
    let (index_bits, exponent_bits) = match precision {
        Precision::Half => (5, 5),
        Precision::Single => (6, 8),
        _ => (6, 11),
    };
    let index = operand & ((1 << index_bits) - 1);
    let index = if planted(3119) { index >> 1 } else { index };
    let exponent = (operand >> index_bits) & ((1 << exponent_bits) - 1);
    let fraction_bits = precision.to_fraction_bits();
    (exponent << fraction_bits) | to_exponential_coefficient(index as usize, precision)
}

/// FPExpCoefficient: the fraction of two to the index over the table's length.
fn to_exponential_coefficient(index: usize, precision: Precision) -> u64 {
    const HALF: [u64; 32] = [
        0x0000, 0x0016, 0x002d, 0x0045, 0x005d, 0x0075, 0x008e, 0x00a8, 0x00c2, 0x00dc, 0x00f8,
        0x0114, 0x0130, 0x014d, 0x016b, 0x0189, 0x01a8, 0x01c8, 0x01e8, 0x0209, 0x022b, 0x024e,
        0x0271, 0x0295, 0x02ba, 0x02e0, 0x0306, 0x032e, 0x0356, 0x037f, 0x03a9, 0x03d4,
    ];
    const SINGLE: [u64; 64] = [
        0x000000, 0x0164d2, 0x02cd87, 0x043a29, 0x05aac3, 0x071f62, 0x08980f, 0x0a14d5, 0x0b95c2,
        0x0d1adf, 0x0ea43a, 0x1031dc, 0x11c3d3, 0x135a2b, 0x14f4f0, 0x16942d, 0x1837f0, 0x19e046,
        0x1b8d3a, 0x1d3eda, 0x1ef532, 0x20b051, 0x227043, 0x243516, 0x25fed7, 0x27cd94, 0x29a15b,
        0x2b7a3a, 0x2d583f, 0x2f3b79, 0x3123f6, 0x3311c4, 0x3504f3, 0x36fd92, 0x38fbaf, 0x3aff5b,
        0x3d08a4, 0x3f179a, 0x412c4d, 0x4346cd, 0x45672a, 0x478d75, 0x49b9be, 0x4bec15, 0x4e248c,
        0x506334, 0x52a81e, 0x54f35b, 0x5744fd, 0x599d16, 0x5bfbb8, 0x5e60f5, 0x60ccdf, 0x633f89,
        0x65b907, 0x68396a, 0x6ac0c7, 0x6d4f30, 0x6fe4ba, 0x728177, 0x75257d, 0x77d0df, 0x7a83b3,
        0x7d3e0c,
    ];
    const DOUBLE: [u64; 64] = [
        0x0000000000000,
        0x02C9A3E778061,
        0x059B0D3158574,
        0x0874518759BC8,
        0x0B5586CF9890F,
        0x0E3EC32D3D1A2,
        0x11301D0125B51,
        0x1429AAEA92DE0,
        0x172B83C7D517B,
        0x1A35BEB6FCB75,
        0x1D4873168B9AA,
        0x2063B88628CD6,
        0x2387A6E756238,
        0x26B4565E27CDD,
        0x29E9DF51FDEE1,
        0x2D285A6E4030B,
        0x306FE0A31B715,
        0x33C08B26416FF,
        0x371A7373AA9CB,
        0x3A7DB34E59FF7,
        0x3DEA64C123422,
        0x4160A21F72E2A,
        0x44E086061892D,
        0x486A2B5C13CD0,
        0x4BFDAD5362A27,
        0x4F9B2769D2CA7,
        0x5342B569D4F82,
        0x56F4736B527DA,
        0x5AB07DD485429,
        0x5E76F15AD2148,
        0x6247EB03A5585,
        0x6623882552225,
        0x6A09E667F3BCD,
        0x6DFB23C651A2F,
        0x71F75E8EC5F74,
        0x75FEB564267C9,
        0x7A11473EB0187,
        0x7E2F336CF4E62,
        0x82589994CCE13,
        0x868D99B4492ED,
        0x8ACE5422AA0DB,
        0x8F1AE99157736,
        0x93737B0CDC5E5,
        0x97D829FDE4E50,
        0x9C49182A3F090,
        0xA0C667B5DE565,
        0xA5503B23E255D,
        0xA9E6B5579FDBF,
        0xAE89F995AD3AD,
        0xB33A2B84F15FB,
        0xB7F76F2FB5E47,
        0xBCC1E904BC1D2,
        0xC199BDD85529C,
        0xC67F12E57D14B,
        0xCB720DCEF9069,
        0xD072D4A07897C,
        0xD5818DCFBA487,
        0xDA9E603DB3285,
        0xDFC97337B9B5F,
        0xE502EE78B3FF6,
        0xEA4AFA2A490DA,
        0xEFA1BEE615A27,
        0xF50765B6E4540,
        0xFA7C1819E90D8,
    ];
    match precision {
        Precision::Half => HALF[index],
        Precision::Single => SINGLE[index],
        _ => DOUBLE[index],
    }
}

/// [FPTrigMAddCoefficient](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-sve#impl-aarch64.FPTrigMAddCoefficient.2):
/// the sine series' coefficients in rows 0 to 7 and the cosine series' in rows 8 to 15.
fn to_trigonometric_coefficient(row: u32, precision: Precision) -> u64 {
    const HALF: [u64; 16] = [
        0x3c00, 0xb155, 0x2030, 0, 0, 0, 0, 0, 0x3c00, 0xb800, 0x293a, 0, 0, 0, 0, 0,
    ];
    const SINGLE: [u64; 16] = [
        0x3f800000, 0xbe2aaaab, 0x3c088886, 0xb95008b9, 0x36369d6d, 0, 0, 0, 0x3f800000,
        0xbf000000, 0x3d2aaaa6, 0xbab60705, 0x37cd37cc, 0, 0, 0,
    ];
    const DOUBLE: [u64; 16] = [
        0x3ff0000000000000,
        0xbfc5555555555543,
        0x3f8111111110f30c,
        0xbf2a01a019b92fc6,
        0x3ec71de351f3d22b,
        0xbe5ae5e2b60f7b91,
        0x3de5d8408868552f,
        0x0000000000000000,
        0x3ff0000000000000,
        0xbfe0000000000000,
        0x3fa5555555555536,
        0xbf56c16c16c13a0b,
        0x3efa01a019b1e8d8,
        0xbe927e4f7282f468,
        0x3e21ee96d2641b13,
        0xbda8f76380fbb401,
    ];
    match precision {
        Precision::Half => HALF[row as usize],
        Precision::Single => SINGLE[row as usize],
        _ => DOUBLE[row as usize],
    }
}
