//! The Advanced SIMD instructions of three registers of one arrangement: element by element, pairwise or permuting,
//! as Arm's pseudocode loops over the elements (for example [ADD (vector)](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/ADD--vector---Add--vector--)).

use core::convert::Infallible;

use crate::encoding::Field;
use crate::operand::{Operand, OperandAccess, Signedness};
use crate::{Exception, Instruction, Processor};

/// Q: whether the operation uses the whole register or its low 64 bits.
const Q: Field = Field::new(30, 1);
/// size: the element is a byte shifted left by it.
const SIZE: Field = Field::new(22, 2);
const VECTOR_BITS: u32 = u128::BITS;
const HALF_VECTOR_BITS: u32 = u64::BITS;

/// Which half of the elements a permutation takes, or where it starts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::simd)
)]
pub enum Part {
    First,
    Second,
}

/// What one element of the result is made of, and how the elements are taken.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VectorOperation {
    Add,
    Subtract,
    Multiply,
    /// [PMUL](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/PMUL--Polynomial-multiply-): the low byte of the carry-less product of bytes.
    PolynomialMultiply,
    And,
    /// The first operand and the second's complement.
    BitClear,
    Or,
    OrNot,
    ExclusiveOr,
    /// All ones where the elements are equal.
    CompareEqual,
    CompareGreater(Signedness),
    CompareGreaterOrEqual(Signedness),
    /// All ones where the first element is less than the second, as signed integers.
    CompareLess,
    CompareLessOrEqual,
    /// The absolute value of the first element, as a signed integer; the most negative stays as it is.
    Absolute,
    Negate,
    /// The first element shifted left by the signed low byte of the second, right and rounded where it is negative.
    RoundingShift(Signedness),
    /// All ones where the elements share a set bit.
    TestBits,
    Maximum(Signedness),
    Minimum(Signedness),
    AbsoluteDifference(Signedness),
    HalvingAdd(Signedness),
    HalvingSubtract(Signedness),
    RoundingHalvingAdd(Signedness),
    /// The first element shifted left by the signed low byte of the second, right where it is negative.
    Shift(Signedness),
    /// The operation on two neighbouring elements of the second operand above the first.
    Pairwise(PairwiseOperation),
    /// UZP1 and UZP2: the even or odd elements of the second operand above the first.
    Unzip(Part),
    /// TRN1 and TRN2: the even or odd elements of each operand, interleaved.
    Transpose(Part),
    /// ZIP1 and ZIP2: the lower or upper half of each operand, interleaved.
    Zip(Part),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PairwiseOperation {
    Add,
    Maximum(Signedness),
    Minimum(Signedness),
}

/// The operation of each modeled instruction of this kind, none for every other instruction.
pub const fn to_vector_operation(instruction: Instruction<Infallible>) -> Option<VectorOperation> {
    use Instruction as I;
    use Signedness::{Signed, Unsigned};
    use VectorOperation as O;
    Some(match instruction {
        I::AddVector => O::Add,
        I::SubVector => O::Subtract,
        I::MulVector => O::Multiply,
        I::PmulVector => O::PolynomialMultiply,
        I::AndVector => O::And,
        I::BicVectorRegister8 => O::BitClear,
        I::OrrVectorRegister8 => O::Or,
        I::OrnVector => O::OrNot,
        I::EorVector => O::ExclusiveOr,
        I::CmeqVector => O::CompareEqual,
        I::CmgtVector => O::CompareGreater(Signed),
        I::CmhiVector => O::CompareGreater(Unsigned),
        I::CmgeVector => O::CompareGreaterOrEqual(Signed),
        I::CmhsVector => O::CompareGreaterOrEqual(Unsigned),
        I::CmtstVector => O::TestBits,
        I::SmaxVector => O::Maximum(Signed),
        I::UmaxVector => O::Maximum(Unsigned),
        I::SminVector => O::Minimum(Signed),
        I::UminVector => O::Minimum(Unsigned),
        I::SabdVector => O::AbsoluteDifference(Signed),
        I::UabdVector => O::AbsoluteDifference(Unsigned),
        I::ShaddVector => O::HalvingAdd(Signed),
        I::UhaddVector => O::HalvingAdd(Unsigned),
        I::ShsubVector => O::HalvingSubtract(Signed),
        I::UhsubVector => O::HalvingSubtract(Unsigned),
        I::SrhaddVector => O::RoundingHalvingAdd(Signed),
        I::UrhaddVector => O::RoundingHalvingAdd(Unsigned),
        I::SshlVector => O::Shift(Signed),
        I::UshlVector => O::Shift(Unsigned),
        I::AddpVector => O::Pairwise(PairwiseOperation::Add),
        I::SmaxpVector => O::Pairwise(PairwiseOperation::Maximum(Signed)),
        I::UmaxpVector => O::Pairwise(PairwiseOperation::Maximum(Unsigned)),
        I::SminpVector => O::Pairwise(PairwiseOperation::Minimum(Signed)),
        I::UminpVector => O::Pairwise(PairwiseOperation::Minimum(Unsigned)),
        I::Uzp1Vector => O::Unzip(Part::First),
        I::Uzp2Vector => O::Unzip(Part::Second),
        I::Trn1Vector => O::Transpose(Part::First),
        I::Trn2Vector => O::Transpose(Part::Second),
        I::Zip1Vector => O::Zip(Part::First),
        I::Zip2Vector => O::Zip(Part::Second),
        // One register, compared with zero or negated, a missing second operand reading as zero.
        I::CmgtVectorZero => O::CompareGreater(Signed),
        I::CmeqVectorZero => O::CompareEqual,
        I::CmgeVectorZero => O::CompareGreaterOrEqual(Signed),
        I::CmltVectorZero => O::CompareLess,
        I::CmleVectorZero => O::CompareLessOrEqual,
        I::AbsVector => O::Absolute,
        I::NegVector => O::Negate,
        I::SrshlVector => O::RoundingShift(Signed),
        I::UrshlVector => O::RoundingShift(Unsigned),
        I::CmgtScalarZero => O::CompareGreater(Signed),
        I::CmeqScalarZero => O::CompareEqual,
        I::CmgeScalarZero => O::CompareGreaterOrEqual(Signed),
        I::CmltScalarZero => O::CompareLess,
        I::CmleScalarZero => O::CompareLessOrEqual,
        I::AbsScalar => O::Absolute,
        I::NegScalar => O::Negate,
        I::SrshlScalar => O::RoundingShift(Signed),
        I::UrshlScalar => O::RoundingShift(Unsigned),
        // The scalar forms: one doubleword element of the low 64 bits of each register.
        I::AddScalar => O::Add,
        I::SubScalar => O::Subtract,
        I::CmeqScalar => O::CompareEqual,
        I::CmgtScalar => O::CompareGreater(Signed),
        I::CmhiScalar => O::CompareGreater(Unsigned),
        I::CmgeScalar => O::CompareGreaterOrEqual(Signed),
        I::CmhsScalar => O::CompareGreaterOrEqual(Unsigned),
        I::CmtstScalar => O::TestBits,
        I::SshlScalar => O::Shift(Signed),
        I::UshlScalar => O::Shift(Unsigned),
        _ => return None,
    })
}

/// The element of `esize` bits at the index.
fn element(vector: u128, index: u32, esize: u32) -> u128 {
    (vector >> (index * esize)) & (u128::MAX >> (u128::BITS - esize))
}

/// The element as an integer, signed or not.
fn to_integer(value: u128, esize: u32, signedness: Signedness) -> i128 {
    match signedness {
        Signedness::Unsigned => value as i128,
        Signedness::Signed => ((value << (u128::BITS - esize)) as i128) >> (u128::BITS - esize),
    }
}

/// The carry-less product's low byte.
fn polynomial_multiply(first: u128, second: u128) -> u128 {
    (0..u8::BITS)
        .filter(|bit| second >> bit & 1 != 0)
        .fold(0, |product, bit| product ^ (first << bit))
}

/// One element of the result from one element of each operand.
fn apply(operation: VectorOperation, first: u128, second: u128, esize: u32) -> u128 {
    use VectorOperation as O;
    let all_ones = u128::MAX >> (u128::BITS - esize);
    let ones_if = |holds: bool| if holds { all_ones } else { 0 };
    let integers = |signedness| {
        (
            to_integer(first, esize, signedness),
            to_integer(second, esize, signedness),
        )
    };
    let value = match operation {
        O::Multiply if crate::planted::is(469) && esize == u16::BITS => {
            (first & 0xFF).wrapping_mul(second & 0xFF)
        }
        O::PolynomialMultiply if crate::planted::is(468) => {
            polynomial_multiply(first, second & 0x7F)
        }
        O::CompareGreater(_) if crate::planted::is(460) => ones_if(first > second),
        O::RoundingHalvingAdd(signedness) if crate::planted::is(470) => {
            let (first, second) = integers(signedness);
            ((first + second) >> 1) as u128
        }
        O::AbsoluteDifference(_) if crate::planted::is(471) => first.abs_diff(second),
        O::TestBits if crate::planted::is(472) => ones_if(first & second == 0),
        O::BitClear if crate::planted::is(473) => !first & second,
        O::Shift(signedness) if crate::planted::any(&[463, 464]) => {
            let shift = to_integer(second & u128::from(u8::MAX), u8::BITS, Signedness::Signed);
            let value = to_integer(first, esize, signedness);
            if shift >= 0 {
                value.checked_shl(shift as u32).unwrap_or(0) as u128
            } else {
                let right = shift.unsigned_abs().min(u128::from(i128::BITS - 1)) as u32;
                if crate::planted::is(463) {
                    // As SRSHL: adds the rounding bit below the result.
                    ((value + (1 << (right - 1))) >> right) as u128
                } else {
                    (to_integer(first, esize, Signedness::Signed) >> right) as u128
                }
            }
        }
        O::Add => first.wrapping_add(second),
        O::Subtract => first.wrapping_sub(second),
        O::Multiply => first.wrapping_mul(second),
        O::PolynomialMultiply => polynomial_multiply(first, second),
        O::And => first & second,
        O::BitClear => first & !second,
        O::Or => first | second,
        O::OrNot => first | !second,
        O::ExclusiveOr => first ^ second,
        O::CompareEqual => ones_if(first == second),
        O::CompareGreater(signedness) => {
            let (first, second) = integers(signedness);
            ones_if(first > second)
        }
        O::CompareGreaterOrEqual(signedness) => {
            let (first, second) = integers(signedness);
            ones_if(first >= second)
        }
        O::CompareLess => {
            let (first, second) = integers(Signedness::Signed);
            ones_if(first < second)
        }
        O::CompareLessOrEqual => {
            let (first, second) = integers(Signedness::Signed);
            ones_if(first <= second)
        }
        O::Absolute => to_integer(first, esize, Signedness::Signed).unsigned_abs(),
        O::Negate => first.wrapping_neg(),
        O::RoundingShift(signedness) => {
            // [SRSHL](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/SRSHL--Signed-rounding-shift-left--register--):
            // the element plus half of the last bit shifted out, shifted by the signed low byte of the second.
            let shift = to_integer(second & u128::from(u8::MAX), u8::BITS, Signedness::Signed);
            let value = to_integer(first, esize, signedness);
            if shift >= 0 {
                value.checked_shl(shift as u32).unwrap_or(0) as u128
            } else {
                let right = shift.unsigned_abs() as u32;
                // A sum below 2^65 shifted right by more than 65 is zero.
                if right > u64::BITS + 1 {
                    0
                } else {
                    ((value + (1 << (right - 1))) >> right) as u128
                }
            }
        }
        O::TestBits => ones_if(first & second != 0),
        O::Maximum(signedness) => {
            let (low, high) = integers(signedness);
            low.max(high) as u128
        }
        O::Minimum(signedness) => {
            let (low, high) = integers(signedness);
            low.min(high) as u128
        }
        O::AbsoluteDifference(signedness) => {
            let (first, second) = integers(signedness);
            first.abs_diff(second)
        }
        O::HalvingAdd(signedness) => {
            let (first, second) = integers(signedness);
            ((first + second) >> 1) as u128
        }
        O::HalvingSubtract(signedness) => {
            let (first, second) = integers(signedness);
            ((first - second) >> 1) as u128
        }
        O::RoundingHalvingAdd(signedness) => {
            let (first, second) = integers(signedness);
            ((first + second + 1) >> 1) as u128
        }
        O::Shift(signedness) => {
            // The shift is the signed low byte of the second element.
            let shift = to_integer(second & u128::from(u8::MAX), u8::BITS, Signedness::Signed);
            let value = to_integer(first, esize, signedness);
            if shift >= 0 {
                value.checked_shl(shift as u32).unwrap_or(0) as u128
            } else {
                let right = shift.unsigned_abs().min(u128::from(i128::BITS - 1)) as u32;
                (value >> right) as u128
            }
        }
        O::Pairwise(_) | O::Unzip(_) | O::Transpose(_) | O::Zip(_) => {
            unreachable!("taken whole by `simulate_vector`")
        }
    };
    value & all_ones
}

/// Applies the operation to the registers the operands name and writes the destination; changes nothing when the
/// encoding is undefined, which the list's reserved values already make it.
pub fn simulate_vector<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operands: &[Operand],
    operation: VectorOperation,
) -> Result<(), Exception> {
    use VectorOperation as O;
    let mut sources = [0_u128; 2];
    let mut count = 0;
    let mut destination = None;
    // A scalar form names its registers as one floating-point precision, a vector form as vectors.
    let mut scalar = false;
    for operand in operands {
        let (number, access) = match *operand {
            Operand::VectorRegister { number, access } => (number, access),
            Operand::FloatingPointRegister { number, access, .. } => {
                scalar = true;
                (number, access)
            }
            _ => continue,
        };
        match access {
            OperandAccess::Read => {
                sources[count] = processor.vector_registers[number.read(word) as usize];
                count += 1;
            }
            OperandAccess::Written => destination = Some(number.read(word)),
        }
    }
    // The syntax names Rn before Rm.
    let [first, second] = sources;
    let datasize = if Q.read(word) != 0 && !scalar {
        VECTOR_BITS
    } else {
        HALF_VECTOR_BITS
    };
    let bitwise = matches!(
        operation,
        O::And | O::BitClear | O::Or | O::OrNot | O::ExclusiveOr | O::PolynomialMultiply
    );
    let esize = if bitwise {
        u8::BITS
    } else if scalar {
        u64::BITS
    } else {
        u8::BITS << SIZE.read(word)
    };
    let elements = datasize / esize;
    let low = |vector: u128| vector & (u128::MAX >> (u128::BITS - datasize));
    let (first, second) = (low(first), low(second));
    let mut result = 0_u128;
    let mut put = |index: u32, value: u128| result |= value << (index * esize);
    // Elements counted over the second operand above the first, as the pseudocode's concatenation has them.
    let joined = |index: u32| {
        if index < elements {
            element(first, index, esize)
        } else {
            element(second, index - elements, esize)
        }
    };
    let half = elements / 2;
    match operation {
        O::Pairwise(pairwise) => {
            for index in 0..elements {
                let (low, high) = if crate::planted::is(465) {
                    (
                        element(first, (2 * index) % elements, esize),
                        element(first, (2 * index + 1) % elements, esize),
                    )
                } else {
                    (joined(2 * index), joined(2 * index + 1))
                };
                let value = match pairwise {
                    PairwiseOperation::Maximum(_) if crate::planted::is(466) => low.max(high),
                    PairwiseOperation::Add => apply(O::Add, low, high, esize),
                    PairwiseOperation::Maximum(signedness) => {
                        apply(O::Maximum(signedness), low, high, esize)
                    }
                    PairwiseOperation::Minimum(signedness) => {
                        apply(O::Minimum(signedness), low, high, esize)
                    }
                };
                put(index, value);
            }
        }
        O::Unzip(part) => {
            for index in 0..elements {
                let taken = 2 * index + u32::from(part == Part::Second);
                if crate::planted::is(461) {
                    // The concatenation the other way round: the first operand above the second.
                    let reversed = (taken + elements) % (2 * elements);
                    put(index, joined(reversed));
                } else {
                    put(index, joined(taken));
                }
            }
        }
        O::Transpose(part) => {
            for pair in 0..half {
                let taken = 2 * pair + u32::from(part == Part::Second);
                put(2 * pair, element(first, taken, esize));
                let second_taken = if crate::planted::is(467) {
                    taken + 1
                } else {
                    taken
                };
                put(2 * pair + 1, element(second, second_taken, esize));
            }
        }
        O::Zip(part) => {
            let base = if part == Part::Second { half } else { 0 };
            let base = if crate::planted::is(474) && base > 0 {
                base - 1
            } else {
                base
            };
            for pair in 0..half {
                put(2 * pair, element(first, base + pair, esize));
                put(2 * pair + 1, element(second, base + pair, esize));
            }
        }
        _ => {
            let computed = if crate::planted::is(458) {
                elements - 1
            } else {
                elements
            };
            for index in 0..computed {
                let second_index = if crate::planted::is(459) {
                    (index + 1) % elements
                } else {
                    index
                };
                put(
                    index,
                    apply(
                        operation,
                        element(first, index, esize),
                        element(second, second_index, esize),
                        esize,
                    ),
                );
            }
        }
    }
    if let Some(number) = destination {
        let kept = if crate::planted::is(462) && datasize == HALF_VECTOR_BITS {
            processor.vector_registers[number as usize] & !(u128::MAX >> HALF_VECTOR_BITS)
        } else {
            0
        };
        processor.vector_registers[number as usize] = result | kept;
    }
    Ok(())
}
