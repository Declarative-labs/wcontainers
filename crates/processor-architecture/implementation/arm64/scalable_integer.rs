//! The SVE2 integer instructions of three registers at a 128-bit vector length: long, wide and narrowing arithmetic
//! of the bottom (even) or top (odd) half-width elements, multiplies, accumulates and the bitwise ternary operations, as
//! Arm's pseudocode loops over the elements (for example
//! [SADDLB](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/SADDLB--Signed-add-long--bottom--)).
//!
//! SVE's saturating instructions leave FPSR.QC alone.

use core::convert::Infallible;

use crate::operand::Signedness;
use crate::scalable::{SIZE, ScalableOperand, VECTOR_BITS, ZD, ZM, ZN, read_element, read_vector};
use crate::{Exception, Instruction, Processor};

/// What one element of the result is made of its sources, as integers wide enough to hold any of them exactly.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntegerOperation {
    Add,
    Subtract,
    AbsoluteDifference,
    Multiply,
    /// The high half of the product at the element's size.
    MultiplyHigh,
    /// SQDMULH and SQRDMULH: twice the product's high half, rounded or not, saturated.
    SaturatingDoublingMultiplyHigh {
        rounding: bool,
    },
    /// SQDMULL: twice the product, saturated at the result's size.
    SaturatingDoublingMultiply,
    /// The carry-less product.
    PolynomialMultiply,
}

/// How the sources' elements are taken and the result written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntegerShape {
    /// Every element of the size from the two sources' of the same size.
    Same,
    /// Each element from the two sources' half-width elements, each the bottom (even) or the top (odd) one.
    Long { first_top: bool, second_top: bool },
    /// Each element from the first source's of the same size and the second's half-width bottom or top one.
    Wide { top: bool },
    /// The destination's element plus or minus the long result, saturated where `saturating`.
    LongAccumulate {
        first_top: bool,
        second_top: bool,
        subtract: bool,
        saturating: bool,
    },
    /// The destination's element plus the result of the same size.
    Accumulate,
    /// The high half of each result in the bottom or top half-width element; the bottom form zeroes the top ones,
    /// the top form keeps the bottom ones.
    NarrowHigh { top: bool, rounding: bool },
    /// EORBT and EORTB: the bottom (or top) elements the first source's exclusive-or the second's other ones, the rest
    /// of the destination kept.
    InterleavingExclusiveOr { top: bool },
}

/// What a bitwise ternary instruction computes of the destination, the second source and the third.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TernaryOperation {
    ExclusiveOr,
    BitClearExclusiveOr,
    Select {
        invert_first: bool,
        invert_second: bool,
        invert_result: bool,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntegerForm {
    Elements {
        operation: IntegerOperation,
        shape: IntegerShape,
        signedness: Signedness,
    },
    /// PMUL: bytes only.
    PolynomialBytes,
    /// PMULLB and PMULLT of doublewords into one 128-bit product.
    PolynomialQuadword {
        top: bool,
    },
    Ternary(TernaryOperation),
}

/// The form of each modeled SVE2 integer instruction, none for every other instruction.
pub const fn to_integer_form(instruction: Instruction<Infallible>) -> Option<IntegerForm> {
    use Instruction as I;
    use IntegerOperation as O;
    use IntegerShape as S;
    use Signedness::{Signed, Unsigned};
    const fn form(
        operation: IntegerOperation,
        shape: IntegerShape,
        signedness: Signedness,
    ) -> IntegerForm {
        IntegerForm::Elements {
            operation,
            shape,
            signedness,
        }
    }
    const fn long(first_top: bool, second_top: bool) -> IntegerShape {
        IntegerShape::Long {
            first_top,
            second_top,
        }
    }
    const fn accumulate(top: bool, subtract: bool, saturating: bool) -> IntegerShape {
        IntegerShape::LongAccumulate {
            first_top: top,
            second_top: top,
            subtract,
            saturating,
        }
    }
    const fn narrow(top: bool, rounding: bool) -> IntegerShape {
        IntegerShape::NarrowHigh { top, rounding }
    }
    let (b, t) = (false, true);
    Some(match instruction {
        I::Saddlb => form(O::Add, long(b, b), Signed),
        I::Saddlt => form(O::Add, long(t, t), Signed),
        I::Ssublb => form(O::Subtract, long(b, b), Signed),
        I::Ssublt => form(O::Subtract, long(t, t), Signed),
        I::Sabdlb => form(O::AbsoluteDifference, long(b, b), Signed),
        I::Sabdlt => form(O::AbsoluteDifference, long(t, t), Signed),
        I::Uaddlb => form(O::Add, long(b, b), Unsigned),
        I::Uaddlt => form(O::Add, long(t, t), Unsigned),
        I::Usublb => form(O::Subtract, long(b, b), Unsigned),
        I::Usublt => form(O::Subtract, long(t, t), Unsigned),
        I::Uabdlb => form(O::AbsoluteDifference, long(b, b), Unsigned),
        I::Uabdlt => form(O::AbsoluteDifference, long(t, t), Unsigned),
        I::Saddlbt => form(O::Add, long(b, t), Signed),
        I::Ssublbt => form(O::Subtract, long(b, t), Signed),
        I::Ssubltb => form(O::Subtract, long(t, b), Signed),
        I::SmullbZZZ => form(O::Multiply, long(b, b), Signed),
        I::SmulltZZZ => form(O::Multiply, long(t, t), Signed),
        I::UmullbZZZ => form(O::Multiply, long(b, b), Unsigned),
        I::UmulltZZZ => form(O::Multiply, long(t, t), Unsigned),
        I::SqdmullbZZZ => form(O::SaturatingDoublingMultiply, long(b, b), Signed),
        I::SqdmulltZZZ => form(O::SaturatingDoublingMultiply, long(t, t), Signed),
        I::PmullbZZZ128 => IntegerForm::PolynomialQuadword { top: b },
        I::PmulltZZZ128 => IntegerForm::PolynomialQuadword { top: t },
        I::Saddwb => form(O::Add, S::Wide { top: b }, Signed),
        I::Saddwt => form(O::Add, S::Wide { top: t }, Signed),
        I::Ssubwb => form(O::Subtract, S::Wide { top: b }, Signed),
        I::Ssubwt => form(O::Subtract, S::Wide { top: t }, Signed),
        I::Uaddwb => form(O::Add, S::Wide { top: b }, Unsigned),
        I::Uaddwt => form(O::Add, S::Wide { top: t }, Unsigned),
        I::Usubwb => form(O::Subtract, S::Wide { top: b }, Unsigned),
        I::Usubwt => form(O::Subtract, S::Wide { top: t }, Unsigned),
        I::Sabalb => form(O::AbsoluteDifference, accumulate(b, false, false), Signed),
        I::Sabalt => form(O::AbsoluteDifference, accumulate(t, false, false), Signed),
        I::Uabalb => form(O::AbsoluteDifference, accumulate(b, false, false), Unsigned),
        I::Uabalt => form(O::AbsoluteDifference, accumulate(t, false, false), Unsigned),
        I::SmlalbZZZ => form(O::Multiply, accumulate(b, false, false), Signed),
        I::SmlaltZZZ => form(O::Multiply, accumulate(t, false, false), Signed),
        I::SmlslbZZZ => form(O::Multiply, accumulate(b, true, false), Signed),
        I::SmlsltZZZ => form(O::Multiply, accumulate(t, true, false), Signed),
        I::UmlalbZZZ => form(O::Multiply, accumulate(b, false, false), Unsigned),
        I::UmlaltZZZ => form(O::Multiply, accumulate(t, false, false), Unsigned),
        I::UmlslbZZZ => form(O::Multiply, accumulate(b, true, false), Unsigned),
        I::UmlsltZZZ => form(O::Multiply, accumulate(t, true, false), Unsigned),
        I::SqdmlalbZZZ => form(
            O::SaturatingDoublingMultiply,
            accumulate(b, false, true),
            Signed,
        ),
        I::SqdmlaltZZZ => form(
            O::SaturatingDoublingMultiply,
            accumulate(t, false, true),
            Signed,
        ),
        I::SqdmlslbZZZ => form(
            O::SaturatingDoublingMultiply,
            accumulate(b, true, true),
            Signed,
        ),
        I::SqdmlsltZZZ => form(
            O::SaturatingDoublingMultiply,
            accumulate(t, true, true),
            Signed,
        ),
        I::Sqdmlalbt => form(
            O::SaturatingDoublingMultiply,
            S::LongAccumulate {
                first_top: b,
                second_top: t,
                subtract: false,
                saturating: true,
            },
            Signed,
        ),
        I::Sqdmlslbt => form(
            O::SaturatingDoublingMultiply,
            S::LongAccumulate {
                first_top: b,
                second_top: t,
                subtract: true,
                saturating: true,
            },
            Signed,
        ),
        I::Addhnb => form(O::Add, narrow(b, false), Unsigned),
        I::Addhnt => form(O::Add, narrow(t, false), Unsigned),
        I::Raddhnb => form(O::Add, narrow(b, true), Unsigned),
        I::Raddhnt => form(O::Add, narrow(t, true), Unsigned),
        I::Subhnb => form(O::Subtract, narrow(b, false), Unsigned),
        I::Subhnt => form(O::Subtract, narrow(t, false), Unsigned),
        I::Rsubhnb => form(O::Subtract, narrow(b, true), Unsigned),
        I::Rsubhnt => form(O::Subtract, narrow(t, true), Unsigned),
        _ => return None,
    })
}

/// The operands of a form in the order of its syntax.
pub fn list_integer_operands(form: IntegerForm) -> &'static [ScalableOperand] {
    use ScalableOperand as O;
    const fn source(number: crate::encoding::Field) -> ScalableOperand {
        ScalableOperand::VectorSource {
            number,
            registers: 1,
        }
    }
    const THREE: &[ScalableOperand] = &[
        O::ElementSize(SIZE),
        O::VectorDestination(ZD),
        source(ZN),
        source(ZM),
    ];
    const READ_DESTINATION: &[ScalableOperand] = &[
        O::ElementSize(SIZE),
        O::VectorSourceDestination(ZD),
        source(ZN),
        source(ZM),
    ];
    const BYTES: &[ScalableOperand] = &[O::VectorDestination(ZD), source(ZN), source(ZM)];
    const TERNARY: &[ScalableOperand] = &[O::VectorSourceDestination(ZD), source(ZM), source(ZN)];
    match form {
        IntegerForm::Elements { shape, .. } => match shape {
            IntegerShape::Same | IntegerShape::Long { .. } | IntegerShape::Wide { .. } => THREE,
            IntegerShape::NarrowHigh { top: false, .. } => THREE,
            IntegerShape::LongAccumulate { .. }
            | IntegerShape::Accumulate
            | IntegerShape::NarrowHigh { top: true, .. }
            | IntegerShape::InterleavingExclusiveOr { .. } => READ_DESTINATION,
        },
        IntegerForm::PolynomialBytes | IntegerForm::PolynomialQuadword { .. } => BYTES,
        IntegerForm::Ternary(_) => TERNARY,
    }
}

/// The bits of a form's wide elements, none where the size field is unallocated for it.
pub fn to_element_bits(form: IntegerForm, word: u32) -> Option<u32> {
    let size = SIZE.read(word);
    match form {
        IntegerForm::PolynomialBytes | IntegerForm::Ternary(_) => Some(u8::BITS),
        IntegerForm::PolynomialQuadword { .. } => Some(u128::BITS),
        IntegerForm::Elements {
            shape:
                IntegerShape::Same
                | IntegerShape::Accumulate
                | IntegerShape::InterleavingExclusiveOr { .. },
            ..
        } => Some(u8::BITS << size),
        // A long or narrowing form has no byte-sized wide element.
        IntegerForm::Elements { .. } => (size != 0).then(|| u8::BITS << size),
    }
}

/// The value of the element as an integer, signed or not.
fn to_integer(value: u128, bits: u32, signedness: Signedness) -> i128 {
    match signedness {
        Signedness::Unsigned => value as i128,
        Signedness::Signed => ((value << (u128::BITS - bits)) as i128) >> (u128::BITS - bits),
    }
}

/// The value saturated to a signed integer of the bits.
fn saturate(value: i128, bits: u32) -> i128 {
    let limit = 1_i128 << (bits - 1);
    value.clamp(-limit, limit - 1)
}

fn polynomial_multiply(first: u128, second: u128, bits: u32) -> u128 {
    (0..bits)
        .filter(|bit| second >> bit & 1 != 0)
        .fold(0, |product, bit| product ^ (first << bit))
}

/// The operation's result on two integers of `bits` bits each, at `result_bits` bits.
fn compute(
    operation: IntegerOperation,
    first: i128,
    second: i128,
    bits: u32,
    result_bits: u32,
) -> i128 {
    match operation {
        IntegerOperation::Add => first + second,
        IntegerOperation::Subtract => first - second,
        IntegerOperation::AbsoluteDifference if crate::planted::is(2050) => first - second,
        IntegerOperation::AbsoluteDifference => (first - second).abs(),
        IntegerOperation::Multiply => first.wrapping_mul(second),
        IntegerOperation::MultiplyHigh => first.wrapping_mul(second) >> bits,
        IntegerOperation::SaturatingDoublingMultiplyHigh { rounding } => {
            // Only the most negative value times itself overflows, and saturates to the largest.
            let round = if rounding && !crate::planted::is(2049) {
                1 << (bits - 1)
            } else {
                0
            };
            match first
                .checked_mul(second)
                .and_then(|product| product.checked_mul(2))
            {
                Some(doubled) => saturate((doubled + round) >> bits, bits),
                None => (1 << (bits - 1)) - 1,
            }
        }
        IntegerOperation::SaturatingDoublingMultiply => saturate(2 * first * second, result_bits),
        IntegerOperation::PolynomialMultiply => {
            polynomial_multiply(first as u128, second as u128, bits) as i128
        }
    }
}

/// Applies the form to the registers the word names.
pub fn simulate_integer_form<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    form: IntegerForm,
) -> Result<(), Exception> {
    let Some(esize) = to_element_bits(form, word) else {
        return Err(Exception::UndefinedInstruction);
    };
    let elements = VECTOR_BITS / esize;
    let destination = read_vector(processor, word, ZD, 0);
    let first = read_vector(processor, word, ZN, 0);
    let second = read_vector(processor, word, ZM, 0);
    let mask = |bits: u32| u128::MAX >> (u128::BITS - bits);
    let mut result = 0_u128;
    let mut put =
        |index: u32, bits: u32, value: u128| result |= (value & mask(bits)) << (index * bits);
    match form {
        IntegerForm::Ternary(operation) => {
            // Zdn, Zm and Zk, which the list names Zd, Zm and Zn.
            let (zdn, zm, zk) = (destination, second, first);
            result = match operation {
                TernaryOperation::ExclusiveOr => zdn ^ zm ^ zk,
                TernaryOperation::BitClearExclusiveOr if crate::planted::is(2052) => {
                    zdn ^ (!zm & zk)
                }
                TernaryOperation::BitClearExclusiveOr => zdn ^ (zm & !zk),
                TernaryOperation::Select {
                    invert_first,
                    invert_second,
                    invert_result,
                } => {
                    let invert =
                        |value: u128, inverted: bool| if inverted { !value } else { value };
                    invert(
                        (invert(zdn, invert_first) & zk) | (invert(zm, invert_second) & !zk),
                        invert_result,
                    )
                }
            };
        }
        IntegerForm::PolynomialQuadword { top } => {
            let half = u64::BITS;
            let top = u32::from(top != crate::planted::is(2047));
            result = polynomial_multiply(
                read_element(first, top, half),
                read_element(second, top, half),
                half,
            );
        }
        IntegerForm::PolynomialBytes => {
            for index in 0..elements {
                let value = polynomial_multiply(
                    read_element(first, index, esize),
                    read_element(second, index, esize),
                    esize,
                );
                put(index, esize, value);
            }
        }
        IntegerForm::Elements {
            operation,
            shape,
            signedness,
        } => {
            let half = esize / 2;
            let narrow = |vector: u128, index: u32, top: bool| {
                let top = top != crate::planted::is(2047);
                to_integer(
                    read_element(vector, 2 * index + u32::from(top), half),
                    half,
                    signedness,
                )
            };
            let wide = |vector: u128, index: u32| {
                to_integer(read_element(vector, index, esize), esize, signedness)
            };
            match shape {
                IntegerShape::Same => {
                    for index in 0..elements {
                        let value = compute(
                            operation,
                            wide(first, index),
                            wide(second, index),
                            esize,
                            esize,
                        );
                        put(index, esize, value as u128);
                    }
                }
                IntegerShape::Accumulate => {
                    for index in 0..elements {
                        let value = compute(
                            operation,
                            wide(first, index),
                            wide(second, index),
                            esize,
                            esize,
                        );
                        put(
                            index,
                            esize,
                            (value as u128).wrapping_add(read_element(destination, index, esize)),
                        );
                    }
                }
                IntegerShape::Long {
                    first_top,
                    second_top,
                } => {
                    for index in 0..elements {
                        let value = compute(
                            operation,
                            narrow(first, index, first_top),
                            narrow(second, index, second_top),
                            half,
                            esize,
                        );
                        put(index, esize, value as u128);
                    }
                }
                IntegerShape::Wide { top } => {
                    for index in 0..elements {
                        let value = compute(
                            operation,
                            wide(first, index),
                            narrow(second, index, top),
                            esize,
                            esize,
                        );
                        put(index, esize, value as u128);
                    }
                }
                IntegerShape::LongAccumulate {
                    first_top,
                    second_top,
                    subtract,
                    saturating,
                } => {
                    for index in 0..elements {
                        let product = compute(
                            operation,
                            narrow(first, index, first_top),
                            narrow(second, index, second_top),
                            half,
                            esize,
                        );
                        let product = if subtract { -product } else { product };
                        let accumulator = read_element(destination, index, esize);
                        let value = if saturating && !crate::planted::is(2048) {
                            saturate(
                                to_integer(accumulator, esize, Signedness::Signed) + product,
                                esize,
                            ) as u128
                        } else {
                            accumulator.wrapping_add(product as u128)
                        };
                        put(index, esize, value);
                    }
                }
                IntegerShape::NarrowHigh { top, rounding } => {
                    for index in 0..elements {
                        let value = compute(
                            operation,
                            wide(first, index),
                            wide(second, index),
                            esize,
                            esize,
                        ) + if rounding { 1 << (half - 1) } else { 0 };
                        let high = ((value as u128) >> half) & mask(half);
                        let kept = read_element(destination, 2 * index, half);
                        let (low, upper) = if top {
                            (kept, high)
                        } else if crate::planted::is(2051) {
                            (high, read_element(destination, 2 * index + 1, half))
                        } else {
                            (high, 0)
                        };
                        put(index, esize, low | upper << half);
                    }
                }
                IntegerShape::InterleavingExclusiveOr { top } => {
                    for pair in 0..elements / 2 {
                        let (written, other) = if top {
                            (2 * pair + 1, 2 * pair)
                        } else {
                            (2 * pair, 2 * pair + 1)
                        };
                        let value = read_element(first, written, esize)
                            ^ read_element(second, other, esize);
                        put(written, esize, value);
                        put(other, esize, read_element(destination, other, esize));
                    }
                }
            }
        }
    }
    processor.vector_registers[ZD.read(word) as usize] = result;
    Ok(())
}
