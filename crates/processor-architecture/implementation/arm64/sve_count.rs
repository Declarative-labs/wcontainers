//! The SVE instructions that count elements or step by them: INDEX, ADDVL and ADDPL, the counts and increments by a
//! [pattern](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/CNTB--CNTD--CNTH--CNTW--Set-scalar-to-multiple-of-predicate-constraint-element-count-), and those by a predicate's active elements.

use core::convert::Infallible;

use crate::encoding::Field;
use crate::operand::{RegisterThirtyOne, Signedness};
use crate::registers::{read_register, write_register};
use crate::sve::{
    GeneralValues, PN4, PredicateValues, RD5, RM5, RN5, SIZE, ScalableInstruction, ScalableOperand,
    ScalableOperation, VECTOR_BYTES, VectorValues, count_elements, decode_count, is_active,
    read_element, saturate, to_element_bits, to_integer, to_mask, write_element,
};
use crate::{Exception, Instruction, Processor};

const PATTERN: Field = Field::new(5, 5);
/// imm4: the count's multiplier, less one.
const MULTIPLIER: Field = Field::new(16, 4);
const INDEX_FIRST: Field = Field::new(5, 5);
const INDEX_SECOND: Field = Field::new(16, 5);
const VECTOR_LENGTH_MULTIPLE: Field = Field::new(5, 6);
/// Bytes of a predicate register at this vector length.
const PREDICATE_BYTES: u32 = VECTOR_BYTES / u8::BITS;

/// How a count changes the register it steps.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum Step {
    /// Wrapping at the register's width.
    Plain,
    /// Clamped to the width's range of the signedness.
    Saturating(Signedness),
}

/// The register a count steps, and at what width its value is read and saturated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum Stepped {
    /// A 64-bit register.
    Doubleword,
    /// The low 32 bits of the register, written back sign-extended for a signed step and zero-extended otherwise.
    Word,
    /// Each element of a Z register.
    Vector,
}

/// What a count of elements comes from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum Counted {
    /// The elements a pattern counts, times imm4 plus one.
    Pattern,
    /// The elements active in a predicate.
    Predicate,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum CountOperation {
    /// INDEX: each element its index times a step, plus a base; each from an immediate or a register.
    Index {
        base_register: bool,
        step_register: bool,
    },
    /// ADDVL and ADDPL: a register plus a multiple of the vector's or a predicate's bytes.
    AddLength { predicate: bool },
    /// CNTB and its kin.
    CountPattern,
    Step {
        counted: Counted,
        stepped: Stepped,
        step: Step,
        decrement: bool,
    },
}

const fn general(
    field: Field,
    thirty_one: RegisterThirtyOne,
    values: Option<GeneralValues>,
) -> ScalableOperand {
    ScalableOperand::General {
        field,
        thirty_one,
        values,
    }
}

const fn vector(field: Field, values: VectorValues) -> ScalableOperand {
    ScalableOperand::Vector {
        field,
        values,
        pair: false,
    }
}

use RegisterThirtyOne::{StackPointer, ZeroRegister};
use ScalableOperand::{ElementSize, SignedImmediate};

const INDEX_IMMEDIATES: &[ScalableOperand] = &[
    vector(RD5, VectorValues::Written),
    ElementSize(SIZE),
    SignedImmediate(INDEX_FIRST),
    SignedImmediate(INDEX_SECOND),
];
const INDEX_REGISTER_IMMEDIATE: &[ScalableOperand] = &[
    vector(RD5, VectorValues::Written),
    ElementSize(SIZE),
    general(RN5, ZeroRegister, Some(GeneralValues::Plain)),
    SignedImmediate(INDEX_SECOND),
];
const INDEX_IMMEDIATE_REGISTER: &[ScalableOperand] = &[
    vector(RD5, VectorValues::Written),
    ElementSize(SIZE),
    SignedImmediate(INDEX_FIRST),
    general(RM5, ZeroRegister, Some(GeneralValues::Plain)),
];
const INDEX_REGISTERS: &[ScalableOperand] = &[
    vector(RD5, VectorValues::Written),
    ElementSize(SIZE),
    general(RN5, ZeroRegister, Some(GeneralValues::Plain)),
    general(RM5, ZeroRegister, Some(GeneralValues::Plain)),
];
const ADD_LENGTH: &[ScalableOperand] = &[
    general(RD5, StackPointer, None),
    general(RM5, StackPointer, Some(GeneralValues::Plain)),
    SignedImmediate(VECTOR_LENGTH_MULTIPLE),
];
const COUNT_PATTERN: &[ScalableOperand] = &[
    general(RD5, ZeroRegister, None),
    ScalableOperand::Pattern(PATTERN),
    ScalableOperand::UnsignedImmediate(MULTIPLIER),
];
const STEP_PLAIN_BY_PATTERN: &[ScalableOperand] = &[
    general(RD5, ZeroRegister, Some(GeneralValues::Plain)),
    ScalableOperand::Pattern(PATTERN),
    ScalableOperand::UnsignedImmediate(MULTIPLIER),
];
const STEP_SATURATING_BY_PATTERN: &[ScalableOperand] = &[
    general(RD5, ZeroRegister, Some(GeneralValues::Saturating)),
    ScalableOperand::Pattern(PATTERN),
    ScalableOperand::UnsignedImmediate(MULTIPLIER),
];
const STEP_VECTOR_BY_PATTERN: &[ScalableOperand] = &[
    vector(RD5, VectorValues::Data),
    ScalableOperand::Pattern(PATTERN),
    ScalableOperand::UnsignedImmediate(MULTIPLIER),
];
const COUNTED_PREDICATE: ScalableOperand = ScalableOperand::Predicate {
    field: PN4,
    values: PredicateValues::Operand,
};
const STEP_PLAIN_BY_PREDICATE: &[ScalableOperand] = &[
    general(RD5, ZeroRegister, Some(GeneralValues::Plain)),
    COUNTED_PREDICATE,
    ElementSize(SIZE),
];
const STEP_SATURATING_BY_PREDICATE: &[ScalableOperand] = &[
    general(RD5, ZeroRegister, Some(GeneralValues::Saturating)),
    COUNTED_PREDICATE,
    ElementSize(SIZE),
];
const STEP_VECTOR_BY_PREDICATE: &[ScalableOperand] = &[
    vector(RD5, VectorValues::Data),
    COUNTED_PREDICATE,
    ElementSize(SIZE),
];

/// The model of each instruction of this kind, none for every other instruction.
pub(crate) fn to_count_instruction(
    instruction: Instruction<Infallible>,
) -> Option<ScalableInstruction> {
    use CountOperation as O;
    use Instruction as I;
    use Signedness::{Signed, Unsigned};
    use Stepped::{Doubleword, Vector, Word};
    let to_step = |counted, stepped, step, decrement| {
        let operands = match (counted, stepped, step) {
            (Counted::Pattern, Vector, _) => STEP_VECTOR_BY_PATTERN,
            (Counted::Pattern, _, Step::Plain) => STEP_PLAIN_BY_PATTERN,
            (Counted::Pattern, _, Step::Saturating(_)) => STEP_SATURATING_BY_PATTERN,
            (Counted::Predicate, Vector, _) => STEP_VECTOR_BY_PREDICATE,
            (Counted::Predicate, _, Step::Plain) => STEP_PLAIN_BY_PREDICATE,
            (Counted::Predicate, _, Step::Saturating(_)) => STEP_SATURATING_BY_PREDICATE,
        };
        (
            O::Step {
                counted,
                stepped,
                step,
                decrement,
            },
            operands,
        )
    };
    let pattern = |stepped, step, decrement| to_step(Counted::Pattern, stepped, step, decrement);
    let predicate =
        |stepped, step, decrement| to_step(Counted::Predicate, stepped, step, decrement);
    let plain = Step::Plain;
    let signed = Step::Saturating(Signed);
    let unsigned = Step::Saturating(Unsigned);
    let (operation, operands) = match instruction {
        I::IndexII => (
            O::Index {
                base_register: false,
                step_register: false,
            },
            INDEX_IMMEDIATES,
        ),
        I::IndexRI => (
            O::Index {
                base_register: true,
                step_register: false,
            },
            INDEX_REGISTER_IMMEDIATE,
        ),
        I::IndexIR => (
            O::Index {
                base_register: false,
                step_register: true,
            },
            INDEX_IMMEDIATE_REGISTER,
        ),
        I::IndexRR => (
            O::Index {
                base_register: true,
                step_register: true,
            },
            INDEX_REGISTERS,
        ),
        I::Addvl => (O::AddLength { predicate: false }, ADD_LENGTH),
        I::Addpl => (O::AddLength { predicate: true }, ADD_LENGTH),
        I::Cntb | I::Cnth | I::Cntw | I::Cntd => (O::CountPattern, COUNT_PATTERN),
        I::Incb | I::InchXPiI | I::IncwXPiI | I::IncdXPiI => pattern(Doubleword, plain, false),
        I::Decb | I::DechXPiI | I::DecwXPiI | I::DecdXPiI => pattern(Doubleword, plain, true),
        I::SqincbXPiI | I::SqinchXPiI | I::SqincwXPiI | I::SqincdXPiI => {
            pattern(Doubleword, signed, false)
        }
        I::SqdecbXPiI | I::SqdechXPiI | I::SqdecwXPiI | I::SqdecdXPiI => {
            pattern(Doubleword, signed, true)
        }
        I::UqincbXPiI | I::UqinchXPiI | I::UqincwXPiI | I::UqincdXPiI => {
            pattern(Doubleword, unsigned, false)
        }
        I::UqdecbXPiI | I::UqdechXPiI | I::UqdecwXPiI | I::UqdecdXPiI => {
            pattern(Doubleword, unsigned, true)
        }
        I::SqincbXPiWdI | I::SqinchXPiWdI | I::SqincwXPiWdI | I::SqincdXPiWdI => {
            pattern(Word, signed, false)
        }
        I::SqdecbXPiWdI | I::SqdechXPiWdI | I::SqdecwXPiWdI | I::SqdecdXPiWdI => {
            pattern(Word, signed, true)
        }
        I::UqincbWPiI | I::UqinchWPiI | I::UqincwWPiI | I::UqincdWPiI => {
            pattern(Word, unsigned, false)
        }
        I::UqdecbWPiI | I::UqdechWPiI | I::UqdecwWPiI | I::UqdecdWPiI => {
            pattern(Word, unsigned, true)
        }
        I::InchZPiI | I::IncwZPiI | I::IncdZPiI => pattern(Vector, plain, false),
        I::DechZPiI | I::DecwZPiI | I::DecdZPiI => pattern(Vector, plain, true),
        I::SqinchZPiI | I::SqincwZPiI | I::SqincdZPiI => pattern(Vector, signed, false),
        I::SqdechZPiI | I::SqdecwZPiI | I::SqdecdZPiI => pattern(Vector, signed, true),
        I::UqinchZPiI | I::UqincwZPiI | I::UqincdZPiI => pattern(Vector, unsigned, false),
        I::UqdechZPiI | I::UqdecwZPiI | I::UqdecdZPiI => pattern(Vector, unsigned, true),
        I::IncpXP => predicate(Doubleword, plain, false),
        I::DecpXP => predicate(Doubleword, plain, true),
        I::SqincpXP => predicate(Doubleword, signed, false),
        I::SqdecpXP => predicate(Doubleword, signed, true),
        I::UqincpXP => predicate(Doubleword, unsigned, false),
        I::UqdecpXP => predicate(Doubleword, unsigned, true),
        I::SqincpXPWd => predicate(Word, signed, false),
        I::SqdecpXPWd => predicate(Word, signed, true),
        I::UqincpWP => predicate(Word, unsigned, false),
        I::UqdecpWP => predicate(Word, unsigned, true),
        I::IncpZP => predicate(Vector, plain, false),
        I::DecpZP => predicate(Vector, plain, true),
        I::SqincpZP => predicate(Vector, signed, false),
        I::SqdecpZP => predicate(Vector, signed, true),
        I::UqincpZP => predicate(Vector, unsigned, false),
        I::UqdecpZP => predicate(Vector, unsigned, true),
        _ => return None,
    };
    Some(ScalableInstruction {
        operation: ScalableOperation::Count(operation),
        operands,
    })
}

/// The value stepped by the count, at the width, as its bits.
fn step_value(value: u128, count: i128, bits: u32, step: Step, decrement: bool) -> u128 {
    let count = if decrement { -count } else { count };
    match step {
        Step::Plain => (value as i128).wrapping_add(count) as u128 & to_mask(bits),
        Step::Saturating(signedness) => saturate(
            to_integer(value, bits, signedness) + count,
            bits,
            signedness,
        ),
    }
}

pub(crate) fn simulate_count<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operation: CountOperation,
) -> Result<(), Exception> {
    let size = SIZE.read(word);
    let bits = to_element_bits(size);
    let destination = RD5.read(word);
    let read = |field: Field, thirty_one| {
        u128::from(read_register(processor, field.read(word), thirty_one))
    };
    match operation {
        CountOperation::Index {
            base_register,
            step_register,
        } => {
            let take = |register: bool, register_field: Field, immediate: Field| {
                if register {
                    read(register_field, RegisterThirtyOne::ZeroRegister)
                } else {
                    immediate.read_signed(word) as u128
                }
            };
            let base = take(base_register, RN5, INDEX_FIRST);
            let step = take(step_register, RM5, INDEX_SECOND);
            let base = if crate::planted::is(1042) {
                base & u128::from(u32::MAX)
            } else {
                base
            };
            let mut result = 0;
            for index in 0..count_elements(bits) {
                let element = base.wrapping_add(step.wrapping_mul(u128::from(index)));
                write_element(&mut result, index, bits, element);
            }
            processor.vector_registers[destination as usize] = result;
        }
        CountOperation::AddLength { predicate } => {
            let unit = if predicate && !crate::planted::is(1043) {
                PREDICATE_BYTES
            } else {
                VECTOR_BYTES
            };
            let multiple = VECTOR_LENGTH_MULTIPLE.read_signed(word);
            let base = read(RM5, RegisterThirtyOne::StackPointer) as u64;
            let value = base.wrapping_add_signed(multiple * i64::from(unit));
            write_register(
                processor,
                destination,
                value,
                RegisterThirtyOne::StackPointer,
            );
        }
        CountOperation::CountPattern => {
            let count = decode_count(PATTERN.read(word), bits) * (MULTIPLIER.read(word) + 1);
            write_register(
                processor,
                destination,
                u64::from(count),
                RegisterThirtyOne::ZeroRegister,
            );
        }
        CountOperation::Step {
            counted,
            stepped,
            step,
            decrement,
        } => {
            let count = match counted {
                Counted::Pattern => {
                    let counted_bits = if crate::planted::is(1045) && stepped == Stepped::Vector {
                        u8::BITS
                    } else {
                        bits
                    };
                    let count = decode_count(PATTERN.read(word), counted_bits);
                    i128::from(count * (MULTIPLIER.read(word) + 1))
                }
                Counted::Predicate => {
                    let predicate = processor.predicate_registers[PN4.read(word) as usize];
                    (0..count_elements(bits))
                        .filter(|index| is_active(predicate, *index, bits))
                        .count() as i128
                }
            };
            match stepped {
                Stepped::Doubleword | Stepped::Word => {
                    let width = if stepped == Stepped::Word || crate::planted::is(1040) {
                        u32::BITS
                    } else {
                        u64::BITS
                    };
                    let value = read(RD5, RegisterThirtyOne::ZeroRegister) & to_mask(width);
                    let stepped = step_value(value, count, width, step, decrement);
                    let written = match step {
                        Step::Saturating(Signedness::Signed) => {
                            to_integer(stepped, width, Signedness::Signed) as u64
                        }
                        Step::Saturating(Signedness::Unsigned) if crate::planted::is(1041) => {
                            to_integer(stepped, width, Signedness::Signed) as u64
                        }
                        _ => stepped as u64,
                    };
                    write_register(
                        processor,
                        destination,
                        written,
                        RegisterThirtyOne::ZeroRegister,
                    );
                }
                Stepped::Vector => {
                    if size == 0 {
                        return Err(Exception::UndefinedInstruction);
                    }
                    let old = processor.vector_registers[destination as usize];
                    let mut result = 0;
                    for index in 0..count_elements(bits) {
                        let element = read_element(old, index, bits);
                        write_element(
                            &mut result,
                            index,
                            bits,
                            step_value(element, count, bits, step, decrement),
                        );
                    }
                    processor.vector_registers[destination as usize] = result;
                }
            }
        }
    }
    Ok(())
}
