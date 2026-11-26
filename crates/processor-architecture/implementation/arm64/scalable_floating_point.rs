//! The SVE floating-point instructions at a 128-bit vector length: arithmetic, compares, fused multiplies and
//! conversions, element by element as Arm's pseudocode loops (for example
//! [FADD (vectors, predicated)](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/FADD--vectors--predicated---Floating-point-add-vector--predicated--)),
//! each element computed by `floating_point::scalable_elements`.
//!
//! An inactive element of a merging form keeps the destination's, of a compare is false, and raises no flag.

use core::convert::Infallible;

use crate::encoding::Field;
use crate::floating_point::scalable_elements::{
    Conversion, ElementComparison, ElementOperation, compare_elements, compute_element,
    convert_element, multiply_add_element, to_constant,
};
use crate::floating_point::{FloatingPointOperation, Precision, Rounding};
use crate::scalable::{
    PG, SIZE, ScalableOperand, VECTOR_BITS, ZD, ZM, ZN, is_active, read_element, read_predicate,
    read_vector,
};
use crate::{Exception, Instruction, Processor};

/// The destination predicate of a compare.
pub const PD: Field = Field::new(0, 4);
/// The bit that chooses one of an immediate form's two constants.
pub const IMMEDIATE_CHOICE: Field = Field::new(5, 1);

/// What the elements of a Z register an SVE instruction reads are.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LaneKind {
    FloatingPoint(Precision),
    Integer { bits: u32 },
}

impl LaneKind {
    /// The bits of one value.
    pub const fn to_bits(self) -> u32 {
        match self {
            Self::FloatingPoint(precision) => precision.to_bits(),
            Self::Integer { bits } => bits,
        }
    }
}

/// The lanes of a Z register: what each holds in its low bits, and the container each lies in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Lanes {
    pub kind: LaneKind,
    pub container_bits: u32,
}

/// How a source's critical values place the values of their kind in its lanes: once each over a few registers, or
/// every value in the first lane with the others following it, so that two sources of the two roles pair every value
/// of one with every value of the other in some lane.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VectorRole {
    Packed,
    Rotated,
}

/// The shape of a modeled SVE floating-point instruction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FloatingPointForm {
    /// Every element of the destination from the two sources' elements.
    Unpredicated(ElementOperation),
    /// The active elements of the first source, which is the destination, from it and the second source.
    Predicated(ElementOperation),
    /// The active elements of the destination from the source's.
    Unary(ElementOperation),
    /// As `Predicated`, with one of two constants, each two to an exponent or zero where none, as the second source.
    Immediate {
        operation: ElementOperation,
        constants: [Option<i32>; 2],
    },
    /// The active elements of the destination predicate, true where the compare holds; `swapped` compares zero with
    /// the source.
    Compare {
        comparison: ElementComparison,
        with_zero: bool,
        swapped: bool,
    },
    /// FMLA and its forms, the addend the destination, or FMAD and its, the addend the register at bits 20 to 16.
    MultiplyAdd {
        negate_product: bool,
        negate_addend: bool,
        addend_is_destination: bool,
    },
    Convert(Conversion),
}

/// The form of each modeled SVE floating-point instruction, none for every other instruction.
pub const fn to_floating_point_form(
    instruction: Instruction<Infallible>,
) -> Option<FloatingPointForm> {
    use ElementComparison as C;
    use ElementOperation as E;
    use FloatingPointForm as F;
    use FloatingPointOperation as O;
    use Instruction as I;
    use Precision::{Double, Half, Single};
    const fn scalar(operation: FloatingPointOperation) -> ElementOperation {
        ElementOperation::Scalar(operation)
    }
    const fn round_to(rounding: Option<Rounding>, exact: bool) -> FloatingPointForm {
        FloatingPointForm::Unary(ElementOperation::Scalar(
            FloatingPointOperation::RoundToIntegral { rounding, exact },
        ))
    }
    const fn compare(comparison: ElementComparison) -> FloatingPointForm {
        FloatingPointForm::Compare {
            comparison,
            with_zero: false,
            swapped: false,
        }
    }
    const fn compare_zero(comparison: ElementComparison, swapped: bool) -> FloatingPointForm {
        FloatingPointForm::Compare {
            comparison,
            with_zero: true,
            swapped,
        }
    }
    const fn multiply_add(
        negate_product: bool,
        negate_addend: bool,
        addend_is_destination: bool,
    ) -> FloatingPointForm {
        FloatingPointForm::MultiplyAdd {
            negate_product,
            negate_addend,
            addend_is_destination,
        }
    }
    const fn precision(from: Precision, to: Precision) -> FloatingPointForm {
        FloatingPointForm::Convert(Conversion::Precision { from, to })
    }
    const fn to_integer(from: Precision, bits: u32, unsigned: bool) -> FloatingPointForm {
        FloatingPointForm::Convert(Conversion::ToInteger {
            from,
            bits,
            unsigned,
        })
    }
    const fn from_integer(bits: u32, unsigned: bool, to: Precision) -> FloatingPointForm {
        FloatingPointForm::Convert(Conversion::FromInteger { bits, unsigned, to })
    }
    const HALF_ONE: [Option<i32>; 2] = [Some(-1), Some(0)];
    const HALF_TWO: [Option<i32>; 2] = [Some(-1), Some(1)];
    const ZERO_ONE: [Option<i32>; 2] = [None, Some(0)];
    const fn immediate(
        operation: ElementOperation,
        constants: [Option<i32>; 2],
    ) -> FloatingPointForm {
        FloatingPointForm::Immediate {
            operation,
            constants,
        }
    }
    let (h, s, d) = (u16::BITS, u32::BITS, u64::BITS);
    Some(match instruction {
        I::FaddZZZ => F::Unpredicated(scalar(O::Add)),
        I::FsubZZZ => F::Unpredicated(scalar(O::Subtract)),
        I::FmulZZZ => F::Unpredicated(scalar(O::Multiply)),
        I::FrecpsZZZ => F::Unpredicated(E::ReciprocalStep),
        I::FrsqrtsZZZ => F::Unpredicated(E::ReciprocalSquareRootStep),
        I::FaddZPmZ => F::Predicated(scalar(O::Add)),
        I::FsubZPmZ => F::Predicated(scalar(O::Subtract)),
        I::FmulZPmZ => F::Predicated(scalar(O::Multiply)),
        I::FsubrZPmZ => F::Predicated(E::Reversed(O::Subtract)),
        I::FmaxnmZPmZ => F::Predicated(scalar(O::MaximumNumber)),
        I::FminnmZPmZ => F::Predicated(scalar(O::MinimumNumber)),
        I::FmaxZPmZ => F::Predicated(scalar(O::Maximum)),
        I::FminZPmZ => F::Predicated(scalar(O::Minimum)),
        I::FabdZPmZ => F::Predicated(E::AbsoluteDifference),
        I::FscaleZPmZ => F::Predicated(E::Scale),
        I::FmulxZPmZ => F::Predicated(E::MultiplyExtended),
        I::Fdivr => F::Predicated(E::Reversed(O::Divide)),
        I::FdivZPmZ => F::Predicated(scalar(O::Divide)),
        I::FaddZPmI => immediate(scalar(O::Add), HALF_ONE),
        I::FsubZPmI => immediate(scalar(O::Subtract), HALF_ONE),
        I::FmulZPmI => immediate(scalar(O::Multiply), HALF_TWO),
        I::FsubrZPmI => immediate(E::Reversed(O::Subtract), HALF_ONE),
        I::FmaxnmZPmI => immediate(scalar(O::MaximumNumber), ZERO_ONE),
        I::FminnmZPmI => immediate(scalar(O::MinimumNumber), ZERO_ONE),
        I::FmaxZPmI => immediate(scalar(O::Maximum), ZERO_ONE),
        I::FminZPmI => immediate(scalar(O::Minimum), ZERO_ONE),
        I::FrintnZPmZ => round_to(Some(Rounding::TiesToEven), false),
        I::FrintpZPmZ => round_to(Some(Rounding::TowardPlusInfinity), false),
        I::FrintmZPmZ => round_to(Some(Rounding::TowardMinusInfinity), false),
        I::FrintzZPmZ => round_to(Some(Rounding::TowardZero), false),
        I::FrintaZPmZ => round_to(Some(Rounding::TiesAway), false),
        I::FrintxZPmZ => round_to(None, true),
        I::FrintiZPmZ => round_to(None, false),
        I::FsqrtZPmZ => F::Unary(scalar(O::SquareRoot)),
        I::FrecpxZPmZ => F::Unary(E::ReciprocalExponent),
        I::FcmgePPzZZ => compare(C::GreaterOrEqual),
        I::FcmgtPPzZZ => compare(C::Greater),
        I::FcmeqPPzZZ => compare(C::Equal),
        I::FcmnePPzZZ => compare(C::NotEqual),
        I::Fcmuo => compare(C::Unordered),
        I::FacgePPzZZ => compare(C::AbsoluteGreaterOrEqual),
        I::FacgtPPzZZ => compare(C::AbsoluteGreater),
        I::FcmgePPzZ0 => compare_zero(C::GreaterOrEqual, false),
        I::FcmgtPPzZ0 => compare_zero(C::Greater, false),
        I::FcmltPPzZ0 => compare_zero(C::Greater, true),
        I::FcmlePPzZ0 => compare_zero(C::GreaterOrEqual, true),
        I::FcmeqPPzZ0 => compare_zero(C::Equal, false),
        I::FcmnePPzZ0 => compare_zero(C::NotEqual, false),
        I::FmlaZPmZZ => multiply_add(false, false, true),
        I::FmlsZPmZZ => multiply_add(true, false, true),
        I::Fnmla => multiply_add(true, true, true),
        I::Fnmls => multiply_add(false, true, true),
        I::Fmad => multiply_add(false, false, false),
        I::Fmsb => multiply_add(true, false, false),
        I::Fnmad => multiply_add(true, true, false),
        I::Fnmsb => multiply_add(false, true, false),
        I::FcvtZPmZStoH => precision(Single, Half),
        I::FcvtZPmZHtoS => precision(Half, Single),
        I::FcvtZPmZDtoH => precision(Double, Half),
        I::FcvtZPmZHtoD => precision(Half, Double),
        I::FcvtZPmZDtoS => precision(Double, Single),
        I::FcvtZPmZStoD => precision(Single, Double),
        I::ScvtfZPmZStoS => from_integer(s, false, Single),
        I::ScvtfZPmZStoD => from_integer(s, false, Double),
        I::ScvtfZPmZDtoS => from_integer(d, false, Single),
        I::ScvtfZPmZDtoD => from_integer(d, false, Double),
        I::ScvtfZPmZHtoH => from_integer(h, false, Half),
        I::ScvtfZPmZStoH => from_integer(s, false, Half),
        I::ScvtfZPmZDtoH => from_integer(d, false, Half),
        I::UcvtfZPmZStoS => from_integer(s, true, Single),
        I::UcvtfZPmZStoD => from_integer(s, true, Double),
        I::UcvtfZPmZDtoS => from_integer(d, true, Single),
        I::UcvtfZPmZDtoD => from_integer(d, true, Double),
        I::UcvtfZPmZHtoH => from_integer(h, true, Half),
        I::UcvtfZPmZStoH => from_integer(s, true, Half),
        I::UcvtfZPmZDtoH => from_integer(d, true, Half),
        I::FcvtzsZPmZStoS => to_integer(Single, s, false),
        I::FcvtzsZPmZDtoS => to_integer(Double, s, false),
        I::FcvtzsZPmZStoD => to_integer(Single, d, false),
        I::FcvtzsZPmZDtoD => to_integer(Double, d, false),
        I::FcvtzsZPmZHtoH => to_integer(Half, h, false),
        I::FcvtzsZPmZHtoS => to_integer(Half, s, false),
        I::FcvtzsZPmZHtoD => to_integer(Half, d, false),
        I::FcvtzuZPmZStoS => to_integer(Single, s, true),
        I::FcvtzuZPmZDtoS => to_integer(Double, s, true),
        I::FcvtzuZPmZStoD => to_integer(Single, d, true),
        I::FcvtzuZPmZDtoD => to_integer(Double, d, true),
        I::FcvtzuZPmZHtoH => to_integer(Half, h, true),
        I::FcvtzuZPmZHtoS => to_integer(Half, s, true),
        I::FcvtzuZPmZHtoD => to_integer(Half, d, true),
        _ => return None,
    })
}

/// The source and destination lanes of a conversion: each in a container of the larger size.
pub const fn to_conversion_lanes(conversion: Conversion) -> (Lanes, Lanes) {
    let (source, destination) = match conversion {
        Conversion::Precision { from, to } => {
            (LaneKind::FloatingPoint(from), LaneKind::FloatingPoint(to))
        }
        Conversion::ToInteger { from, bits, .. } => {
            (LaneKind::FloatingPoint(from), LaneKind::Integer { bits })
        }
        Conversion::FromInteger { bits, to, .. } => {
            (LaneKind::Integer { bits }, LaneKind::FloatingPoint(to))
        }
    };
    let container_bits = if source.to_bits() > destination.to_bits() {
        source.to_bits()
    } else {
        destination.to_bits()
    };
    (
        Lanes {
            kind: source,
            container_bits,
        },
        Lanes {
            kind: destination,
            container_bits,
        },
    )
}

/// The operands of a form in the order of its syntax, the control registers last.
pub fn list_floating_point_operands(form: FloatingPointForm) -> &'static [ScalableOperand] {
    use ScalableOperand as O;
    use VectorRole::{Packed, Rotated};
    const fn source(number: Field, role: VectorRole) -> ScalableOperand {
        ScalableOperand::FloatingPointSource { number, role }
    }
    const CONTROL: ScalableOperand = O::FloatingPointControl { conversion: false };
    const UNPREDICATED: &[ScalableOperand] = &[
        O::ElementSize(SIZE),
        O::VectorDestination(ZD),
        source(ZN, Packed),
        source(ZM, Rotated),
        CONTROL,
    ];
    const PREDICATED: &[ScalableOperand] = &[
        O::ElementSize(SIZE),
        source(ZD, Packed),
        O::PredicateSource(PG),
        source(ZN, Rotated),
        CONTROL,
    ];
    const UNARY: &[ScalableOperand] = &[
        O::ElementSize(SIZE),
        O::VectorDestination(ZD),
        O::PredicateSource(PG),
        source(ZN, Packed),
        CONTROL,
    ];
    const IMMEDIATE: &[ScalableOperand] = &[
        O::ElementSize(SIZE),
        source(ZD, Packed),
        O::PredicateSource(PG),
        O::ImmediateChoice(IMMEDIATE_CHOICE),
        CONTROL,
    ];
    const COMPARE: &[ScalableOperand] = &[
        O::ElementSize(SIZE),
        O::PredicateDestination(PD),
        O::PredicateSource(PG),
        source(ZN, Packed),
        source(ZM, Rotated),
        CONTROL,
    ];
    const COMPARE_ZERO: &[ScalableOperand] = &[
        O::ElementSize(SIZE),
        O::PredicateDestination(PD),
        O::PredicateSource(PG),
        source(ZN, Packed),
        CONTROL,
    ];
    const MULTIPLY_ADD: &[ScalableOperand] = &[
        O::ElementSize(SIZE),
        source(ZD, Packed),
        O::PredicateSource(PG),
        source(ZN, Rotated),
        source(ZM, Rotated),
        CONTROL,
    ];
    const CONVERT_PRECISION: &[ScalableOperand] = &[
        O::VectorDestination(ZD),
        O::PredicateSource(PG),
        O::ConvertedSource(ZN),
        O::FloatingPointControl { conversion: true },
    ];
    const CONVERT_INTEGER: &[ScalableOperand] = &[
        O::VectorDestination(ZD),
        O::PredicateSource(PG),
        O::ConvertedSource(ZN),
        CONTROL,
    ];
    match form {
        FloatingPointForm::Unpredicated(_) => UNPREDICATED,
        FloatingPointForm::Predicated(_) => PREDICATED,
        FloatingPointForm::Unary(_) => UNARY,
        FloatingPointForm::Immediate { .. } => IMMEDIATE,
        FloatingPointForm::Compare {
            with_zero: false, ..
        } => COMPARE,
        FloatingPointForm::Compare {
            with_zero: true, ..
        } => COMPARE_ZERO,
        FloatingPointForm::MultiplyAdd { .. } => MULTIPLY_ADD,
        FloatingPointForm::Convert(Conversion::Precision { .. }) => CONVERT_PRECISION,
        FloatingPointForm::Convert(_) => CONVERT_INTEGER,
    }
}

/// The precision of a form's elements, none where the size field is unallocated: a byte has none.
pub fn to_element_precision(form: FloatingPointForm, word: u32) -> Option<Precision> {
    match form {
        FloatingPointForm::Convert(Conversion::Precision { from, .. }) => Some(from),
        FloatingPointForm::Convert(Conversion::ToInteger { from, .. }) => Some(from),
        FloatingPointForm::Convert(Conversion::FromInteger { to, .. }) => Some(to),
        _ => match SIZE.read(word) {
            1 => Some(Precision::Half),
            2 => Some(Precision::Single),
            3 => Some(Precision::Double),
            _ => None,
        },
    }
}

/// The esize of a form's elements in bits, the container's for a conversion, none where it is unallocated.
pub fn to_element_bits(form: FloatingPointForm, word: u32) -> Option<u32> {
    match form {
        FloatingPointForm::Convert(conversion) => {
            Some(to_conversion_lanes(conversion).0.container_bits)
        }
        _ => to_element_precision(form, word).map(Precision::to_bits),
    }
}

/// Applies the form to the registers the word names, adding the flags its active elements raise to FPSR.
pub fn simulate_floating_point_form<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    form: FloatingPointForm,
) -> Result<(), Exception> {
    let (Some(precision), Some(esize)) = (
        to_element_precision(form, word),
        to_element_bits(form, word),
    ) else {
        return Err(Exception::UndefinedInstruction);
    };
    let fpcr = processor.floating_point_control;
    let mut flags = 0;
    let elements = VECTOR_BITS / esize;
    let predicate = read_predicate(processor, word, PG);
    let destination = read_vector(processor, word, ZD, 0);
    let first = read_vector(processor, word, ZN, 0);
    let second = read_vector(processor, word, ZM, 0);
    let element = |vector: u128, index: u32| read_element(vector, index, esize) as u64;
    let active = |index: u32| is_active(predicate, index, esize);
    let mut result = destination;
    let mut put = |index: u32, value: u64| {
        let mask = (u128::MAX >> (u128::BITS - esize)) << (index * esize);
        result = (result & !mask) | (u128::from(value) << (index * esize));
    };
    match form {
        FloatingPointForm::Unpredicated(operation) => {
            for index in 0..elements {
                if crate::planted::is(2034) {
                    flags = 0;
                }
                let sources = [element(first, index), element(second, index)];
                put(
                    index,
                    compute_element(operation, &sources, precision, fpcr, &mut flags),
                );
            }
        }
        FloatingPointForm::Predicated(operation) => {
            for index in (0..elements).filter(|index| active(*index) || crate::planted::is(2015)) {
                let sources = [element(destination, index), element(first, index)];
                put(
                    index,
                    compute_element(operation, &sources, precision, fpcr, &mut flags),
                );
            }
        }
        FloatingPointForm::Unary(operation) => {
            for index in (0..elements).filter(|index| active(*index)) {
                let sources = [element(first, index)];
                put(
                    index,
                    compute_element(operation, &sources, precision, fpcr, &mut flags),
                );
            }
        }
        FloatingPointForm::Immediate {
            operation,
            constants,
        } => {
            let choice = IMMEDIATE_CHOICE.read(word) ^ u32::from(crate::planted::is(2031));
            let constant = match constants[choice as usize] {
                Some(exponent) => to_constant(exponent, false, precision),
                None => 0,
            };
            for index in (0..elements).filter(|index| active(*index)) {
                let sources = [element(destination, index), constant];
                put(
                    index,
                    compute_element(operation, &sources, precision, fpcr, &mut flags),
                );
            }
        }
        FloatingPointForm::Compare {
            comparison,
            with_zero,
            swapped,
        } => {
            let mut held = if crate::planted::is(2033) {
                processor.predicate_registers[PD.read(word) as usize]
                    & !(0..elements)
                        .filter(|index| active(*index))
                        .fold(0_u16, |mask, index| mask | 1 << (index * esize / u8::BITS))
            } else {
                0
            };
            for index in (0..elements).filter(|index| active(*index)) {
                let other = if with_zero { 0 } else { element(second, index) };
                let (left, right) = if swapped && !crate::planted::is(2024) {
                    (other, element(first, index))
                } else {
                    (element(first, index), other)
                };
                if compare_elements(comparison, left, right, precision, fpcr, &mut flags) {
                    held |= 1 << (index * esize / u8::BITS);
                }
            }
            processor.predicate_registers[PD.read(word) as usize] = held;
            processor.floating_point_status |= flags;
            return Ok(());
        }
        FloatingPointForm::MultiplyAdd {
            negate_product,
            negate_addend,
            addend_is_destination,
        } => {
            for index in (0..elements).filter(|index| active(*index)) {
                let sources = if addend_is_destination {
                    [
                        element(first, index),
                        element(second, index),
                        element(destination, index),
                    ]
                } else if crate::planted::is(2025) {
                    [
                        element(destination, index),
                        element(second, index),
                        element(first, index),
                    ]
                } else {
                    [
                        element(destination, index),
                        element(first, index),
                        element(second, index),
                    ]
                };
                put(
                    index,
                    multiply_add_element(
                        sources,
                        negate_product,
                        negate_addend && !crate::planted::is(2026),
                        precision,
                        fpcr,
                        &mut flags,
                    ),
                );
            }
        }
        FloatingPointForm::Convert(conversion) => {
            let (source, target) = to_conversion_lanes(conversion);
            let source_bits = source.kind.to_bits();
            let conversion = match conversion {
                Conversion::FromInteger { unsigned, to, .. } if crate::planted::is(2029) => {
                    Conversion::FromInteger {
                        bits: esize,
                        unsigned,
                        to,
                    }
                }
                other => other,
            };
            for index in (0..elements).filter(|index| active(*index)) {
                let value = if crate::planted::is(2029) {
                    element(first, index)
                } else {
                    element(first, index) & (u64::MAX >> (u64::BITS - source_bits))
                };
                let converted = convert_element(conversion, value, fpcr, &mut flags);
                // A signed integer narrower than its container is sign-extended; any other result is zero-extended.
                let converted = match (conversion, target.kind) {
                    (
                        Conversion::ToInteger {
                            unsigned: false, ..
                        },
                        LaneKind::Integer { bits },
                    ) if bits < esize && !crate::planted::is(2028) => {
                        (((converted << (u64::BITS - bits)) as i64) >> (u64::BITS - bits)) as u64
                            & (u64::MAX >> (u64::BITS - esize))
                    }
                    _ => converted,
                };
                put(index, converted);
            }
        }
    }
    processor.vector_registers[ZD.read(word) as usize] = result;
    processor.floating_point_status |= flags;
    Ok(())
}
