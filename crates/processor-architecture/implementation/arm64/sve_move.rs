//! The SVE instructions that move elements between places: the permutes, EXT, DUP, TBL and TBX, INSR, the unpacks,
//! REV, CPY, [LASTA](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/LASTA--scalar---Extract-element-after-last-to-general-purpose-register-) and LASTB, CLASTA and CLASTB, SPLICE, SEL, and the reductions to one element.

use core::convert::Infallible;

use crate::encoding::Field;
use crate::operand::{RegisterThirtyOne, Signedness};
use crate::registers::{read_register, write_register};
use crate::simd::Part;
use crate::sve::{
    GeneralValues, PG3, PG4, PredicateValues, RD5, RM5, RN5, SIZE, ScalableInstruction,
    ScalableOperand, ScalableOperation, VECTOR_BYTES, VectorValues, count_elements, is_active,
    read_element, to_element_bits, to_integer, to_mask, to_next_register, write_element,
};
use crate::{Exception, Instruction, Processor};

/// imm8 of EXT, the most significant first.
const EXTRACT_POSITION: &[Field] = &[Field::new(16, 5), Field::new(10, 3)];
/// imm2 and tsz of DUP (indexed), the most significant first.
const DUPLICATE_INDEX: &[Field] = &[Field::new(22, 2), Field::new(16, 5)];
/// Bits of tsz in DUP (indexed).
const DUPLICATE_SIZE_BITS: u32 = 5;

/// What one element of a reduction combines, as a value of the element's bits or of 64 bits for a sum.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reduction {
    /// SADDV and UADDV, into 64 bits.
    Sum(Signedness),
    Maximum(Signedness),
    Minimum(Signedness),
    Or,
    ExclusiveOr,
    And,
}

/// Where LASTA, LASTB, CLASTA and CLASTB put the element they take.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ElementTarget {
    General,
    /// The low element of a SIMD and floating-point register, the rest cleared.
    Scalar,
    /// Every element of a Z register.
    Vector,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveOperation {
    Zip(Part),
    Unzip(Part),
    Transpose(Part),
    /// EXT, of the destination and a second register or of a pair.
    Extract {
        pair: bool,
    },
    DuplicateIndexed,
    /// TBL of one register or a pair, and TBX, which keeps the destination's element where the index is out of range.
    Table {
        pair: bool,
        keeps: bool,
    },
    DuplicateGeneral,
    /// INSR: every element up one place, and the scalar into the first.
    Insert {
        general: bool,
    },
    Unpack {
        signedness: Signedness,
        part: Part,
    },
    Reverse,
    /// CPY of a general-purpose register or a SIMD and floating-point one into active elements.
    Copy {
        general: bool,
    },
    /// LASTA where `after`, otherwise LASTB.
    Last {
        after: bool,
        target: ElementTarget,
    },
    /// CLASTA where `after`, otherwise CLASTB: the destination unchanged where no element is active.
    ConditionalLast {
        after: bool,
        target: ElementTarget,
    },
    Splice {
        pair: bool,
    },
    Select,
    Reduce(Reduction),
}

const fn vector(field: Field, values: VectorValues) -> ScalableOperand {
    ScalableOperand::Vector {
        field,
        values,
        pair: false,
    }
}

const fn pair(field: Field) -> ScalableOperand {
    ScalableOperand::Vector {
        field,
        values: VectorValues::Data,
        pair: true,
    }
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

const GOVERNING: ScalableOperand = ScalableOperand::Predicate {
    field: PG3,
    values: PredicateValues::Governing,
};

use RegisterThirtyOne::{StackPointer, ZeroRegister};
use ScalableOperand::ElementSize;
use VectorValues::{Data, ShiftAmounts, Written};

const PERMUTE: &[ScalableOperand] = &[
    vector(RD5, Written),
    vector(RN5, Data),
    vector(RM5, Data),
    ElementSize(SIZE),
];
const EXTRACT: &[ScalableOperand] = &[
    vector(RD5, Data),
    vector(RN5, Data),
    ScalableOperand::SplitImmediate(EXTRACT_POSITION),
];
const EXTRACT_PAIR: &[ScalableOperand] = &[
    vector(RD5, Written),
    pair(RN5),
    ScalableOperand::SplitImmediate(EXTRACT_POSITION),
];
const DUPLICATE_INDEXED: &[ScalableOperand] = &[
    vector(RD5, Written),
    vector(RN5, Data),
    ScalableOperand::SplitImmediate(DUPLICATE_INDEX),
];
const TABLE: &[ScalableOperand] = &[
    vector(RD5, Written),
    vector(RN5, Data),
    vector(RM5, ShiftAmounts),
    ElementSize(SIZE),
];
const TABLE_PAIR: &[ScalableOperand] = &[
    vector(RD5, Written),
    pair(RN5),
    vector(RM5, ShiftAmounts),
    ElementSize(SIZE),
];
const TABLE_KEEPING: &[ScalableOperand] = &[
    vector(RD5, Data),
    vector(RN5, Data),
    vector(RM5, ShiftAmounts),
    ElementSize(SIZE),
];
const DUPLICATE_GENERAL: &[ScalableOperand] = &[
    vector(RD5, Written),
    general(RN5, StackPointer, Some(GeneralValues::Plain)),
    ElementSize(SIZE),
];
const DUPLICATE_DOUBLEWORD: &[ScalableOperand] = &[
    vector(RD5, Written),
    general(RN5, StackPointer, Some(GeneralValues::Plain)),
];
const INSERT_GENERAL: &[ScalableOperand] = &[
    vector(RD5, Data),
    general(RN5, ZeroRegister, Some(GeneralValues::Plain)),
    ElementSize(SIZE),
];
const INSERT_SCALAR: &[ScalableOperand] =
    &[vector(RD5, Data), vector(RN5, Data), ElementSize(SIZE)];
const ONE_SOURCE: &[ScalableOperand] =
    &[vector(RD5, Written), vector(RN5, Data), ElementSize(SIZE)];
const COPY_GENERAL: &[ScalableOperand] = &[
    vector(RD5, Data),
    GOVERNING,
    general(RN5, StackPointer, Some(GeneralValues::Plain)),
    ElementSize(SIZE),
];
const COPY_DOUBLEWORD: &[ScalableOperand] = &[
    vector(RD5, Data),
    GOVERNING,
    general(RN5, StackPointer, Some(GeneralValues::Plain)),
];
const COPY_SCALAR: &[ScalableOperand] = &[
    vector(RD5, Data),
    GOVERNING,
    vector(RN5, Data),
    ElementSize(SIZE),
];
const LAST_GENERAL: &[ScalableOperand] = &[
    general(RD5, ZeroRegister, None),
    GOVERNING,
    vector(RN5, Data),
    ElementSize(SIZE),
];
const LAST_SCALAR: &[ScalableOperand] = &[
    vector(RD5, Written),
    GOVERNING,
    vector(RN5, Data),
    ElementSize(SIZE),
];
const CONDITIONAL_LAST_GENERAL: &[ScalableOperand] = &[
    general(RD5, ZeroRegister, Some(GeneralValues::Plain)),
    GOVERNING,
    vector(RN5, Data),
    ElementSize(SIZE),
];
const CONDITIONAL_LAST_VECTOR: &[ScalableOperand] = &[
    vector(RD5, Data),
    GOVERNING,
    vector(RN5, Data),
    ElementSize(SIZE),
];
const SPLICE_PAIR: &[ScalableOperand] = &[
    vector(RD5, Written),
    GOVERNING,
    pair(RN5),
    ElementSize(SIZE),
];
const SELECT: &[ScalableOperand] = &[
    vector(RD5, Written),
    ScalableOperand::Predicate {
        field: PG4,
        values: PredicateValues::Governing,
    },
    vector(RN5, Data),
    vector(RM5, Data),
    ElementSize(SIZE),
];
const REDUCE: &[ScalableOperand] = &[
    vector(RD5, Written),
    GOVERNING,
    vector(RN5, Data),
    ElementSize(SIZE),
];

/// The model of each instruction of this kind, none for every other instruction.
pub(crate) fn to_move_instruction(
    instruction: Instruction<Infallible>,
) -> Option<ScalableInstruction> {
    use ElementTarget::{General, Scalar};
    use Instruction as I;
    use MoveOperation as O;
    use Signedness::{Signed, Unsigned};
    let (operation, operands) = match instruction {
        I::Zip1ZZZ => (O::Zip(Part::First), PERMUTE),
        I::Zip2ZZZ => (O::Zip(Part::Second), PERMUTE),
        I::Uzp1ZZZ => (O::Unzip(Part::First), PERMUTE),
        I::Uzp2ZZZ => (O::Unzip(Part::Second), PERMUTE),
        I::Trn1ZZZ => (O::Transpose(Part::First), PERMUTE),
        I::Trn2ZZZ => (O::Transpose(Part::Second), PERMUTE),
        I::ExtZZI => (O::Extract { pair: false }, EXTRACT),
        I::ExtZZI8 => (O::Extract { pair: true }, EXTRACT_PAIR),
        I::DupZZI => (O::DuplicateIndexed, DUPLICATE_INDEXED),
        I::TblZZZ => (
            O::Table {
                pair: false,
                keeps: false,
            },
            TABLE,
        ),
        I::TblZZZZ => (
            O::Table {
                pair: true,
                keeps: false,
            },
            TABLE_PAIR,
        ),
        I::TbxZZZ => (
            O::Table {
                pair: false,
                keeps: true,
            },
            TABLE_KEEPING,
        ),
        I::DupZR8 => (O::DuplicateGeneral, DUPLICATE_GENERAL),
        I::DupZR => (O::DuplicateGeneral, DUPLICATE_DOUBLEWORD),
        I::InsrZR => (O::Insert { general: true }, INSERT_GENERAL),
        I::InsrZV => (O::Insert { general: false }, INSERT_SCALAR),
        I::Sunpklo => (
            O::Unpack {
                signedness: Signed,
                part: Part::First,
            },
            ONE_SOURCE,
        ),
        I::Sunpkhi => (
            O::Unpack {
                signedness: Signed,
                part: Part::Second,
            },
            ONE_SOURCE,
        ),
        I::Uunpklo => (
            O::Unpack {
                signedness: Unsigned,
                part: Part::First,
            },
            ONE_SOURCE,
        ),
        I::Uunpkhi => (
            O::Unpack {
                signedness: Unsigned,
                part: Part::Second,
            },
            ONE_SOURCE,
        ),
        I::RevZZ => (O::Reverse, ONE_SOURCE),
        I::CpyZPmR => (O::Copy { general: true }, COPY_GENERAL),
        I::CpyZPmR64 => (O::Copy { general: true }, COPY_DOUBLEWORD),
        I::CpyZPmV => (O::Copy { general: false }, COPY_SCALAR),
        I::LastaRPZ => (
            O::Last {
                after: true,
                target: General,
            },
            LAST_GENERAL,
        ),
        I::LastbRPZ => (
            O::Last {
                after: false,
                target: General,
            },
            LAST_GENERAL,
        ),
        I::LastaVPZ => (
            O::Last {
                after: true,
                target: Scalar,
            },
            LAST_SCALAR,
        ),
        I::LastbVPZ => (
            O::Last {
                after: false,
                target: Scalar,
            },
            LAST_SCALAR,
        ),
        I::ClastaRPZ => (
            O::ConditionalLast {
                after: true,
                target: General,
            },
            CONDITIONAL_LAST_GENERAL,
        ),
        I::ClastbRPZ => (
            O::ConditionalLast {
                after: false,
                target: General,
            },
            CONDITIONAL_LAST_GENERAL,
        ),
        I::ClastaVPZ => (
            O::ConditionalLast {
                after: true,
                target: Scalar,
            },
            CONDITIONAL_LAST_VECTOR,
        ),
        I::ClastbVPZ => (
            O::ConditionalLast {
                after: false,
                target: Scalar,
            },
            CONDITIONAL_LAST_VECTOR,
        ),
        I::ClastaZPZ => (
            O::ConditionalLast {
                after: true,
                target: ElementTarget::Vector,
            },
            CONDITIONAL_LAST_VECTOR,
        ),
        I::ClastbZPZ => (
            O::ConditionalLast {
                after: false,
                target: ElementTarget::Vector,
            },
            CONDITIONAL_LAST_VECTOR,
        ),
        I::SpliceZPZ => (O::Splice { pair: false }, CONDITIONAL_LAST_VECTOR),
        I::SpliceZPZZ => (O::Splice { pair: true }, SPLICE_PAIR),
        I::SelZPZZ => (O::Select, SELECT),
        I::Saddv => (O::Reduce(Reduction::Sum(Signed)), REDUCE),
        I::Uaddv => (O::Reduce(Reduction::Sum(Unsigned)), REDUCE),
        I::SmaxvVPZ => (O::Reduce(Reduction::Maximum(Signed)), REDUCE),
        I::SminvVPZ => (O::Reduce(Reduction::Minimum(Signed)), REDUCE),
        I::UmaxvVPZ => (O::Reduce(Reduction::Maximum(Unsigned)), REDUCE),
        I::UminvVPZ => (O::Reduce(Reduction::Minimum(Unsigned)), REDUCE),
        I::Orv => (O::Reduce(Reduction::Or), REDUCE),
        I::Eorv => (O::Reduce(Reduction::ExclusiveOr), REDUCE),
        I::Andv => (O::Reduce(Reduction::And), REDUCE),
        _ => return None,
    };
    Some(ScalableInstruction {
        operation: ScalableOperation::Move(operation),
        operands,
    })
}

/// The last element active in the predicate.
fn find_last_active(predicate: u16, bits: u32) -> Option<u32> {
    (0..count_elements(bits))
        .rev()
        .find(|index| is_active(predicate, *index, bits))
}

/// The element LASTA or LASTB takes: after the last active, wrapping to the first, or the last active, or the last.
fn find_last_taken(predicate: u16, bits: u32, after: bool) -> u32 {
    let elements = count_elements(bits);
    match (find_last_active(predicate, bits), after) {
        (Some(last), true) => (last + 1) % elements,
        (None, true) if crate::planted::is(1046) => elements - 1,
        (None, true) => 0,
        (Some(last), false) => last,
        (None, false) => elements - 1,
    }
}

pub(crate) fn simulate_move<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operation: MoveOperation,
) -> Result<(), Exception> {
    let size = SIZE.read(word);
    let bits = to_element_bits(size);
    let elements = count_elements(bits);
    let half = elements / 2;
    let destination = RD5.read(word);
    let governing = processor.predicate_registers[PG3.read(word) as usize];
    let first_number = RN5.read(word);
    let first = processor.vector_registers[first_number as usize];
    let next = processor.vector_registers[to_next_register(first_number)];
    let second = processor.vector_registers[RM5.read(word) as usize];
    let old = processor.vector_registers[destination as usize];
    let general =
        |number: u32, thirty_one| u128::from(read_register(processor, number, thirty_one));
    let mut result = 0_u128;
    let mut put = |index: u32, value: u128| write_element(&mut result, index, bits, value);
    match operation {
        MoveOperation::Zip(part) => {
            let base = if part == Part::Second { half } else { 0 };
            for pair in 0..half {
                put(2 * pair, read_element(first, base + pair, bits));
                put(2 * pair + 1, read_element(second, base + pair, bits));
            }
        }
        MoveOperation::Unzip(part) => {
            for index in 0..elements {
                let taken = 2 * index + u32::from(part == Part::Second);
                let value = if taken < elements {
                    read_element(first, taken, bits)
                } else {
                    read_element(second, taken - elements, bits)
                };
                put(index, value);
            }
        }
        MoveOperation::Transpose(part) => {
            for pair in 0..half {
                let taken = 2 * pair + u32::from(part == Part::Second);
                put(2 * pair, read_element(first, taken, bits));
                put(2 * pair + 1, read_element(second, taken, bits));
            }
        }
        MoveOperation::Extract { pair } => {
            let encoded = EXTRACT_POSITION
                .iter()
                .fold(0, |value, field| (value << field.width) | field.read(word));
            let position = if encoded >= VECTOR_BYTES && !crate::planted::is(1049) {
                0
            } else {
                encoded
            };
            let (low, high) = if pair { (first, next) } else { (old, first) };
            let joined_low = low.checked_shr(position * u8::BITS).unwrap_or(0);
            let joined_high = high
                .checked_shl(u128::BITS - position * u8::BITS)
                .unwrap_or(0);
            result = if position == 0 {
                low
            } else {
                joined_low | joined_high
            };
        }
        MoveOperation::DuplicateIndexed => {
            let encoded = DUPLICATE_INDEX
                .iter()
                .fold(0, |value, field| (value << field.width) | field.read(word));
            let size_bits = encoded & to_mask(DUPLICATE_SIZE_BITS) as u32;
            if size_bits == 0 {
                return Err(Exception::UndefinedInstruction);
            }
            let lowest = size_bits.trailing_zeros();
            let bits = u8::BITS << lowest;
            let index = encoded >> (lowest + 1);
            let elements = count_elements(bits);
            let element = if index < elements {
                read_element(first, index, bits)
            } else if crate::planted::is(1050) {
                read_element(first, 0, bits)
            } else {
                0
            };
            for place in 0..elements {
                write_element(&mut result, place, bits, element);
            }
        }
        MoveOperation::Table { pair, keeps } => {
            let table_elements = if pair { 2 * elements } else { elements };
            for index in 0..elements {
                let taken = read_element(second, index, bits);
                let value = if taken < u128::from(table_elements) {
                    let taken = taken as u32;
                    if taken < elements {
                        read_element(first, taken, bits)
                    } else {
                        read_element(next, taken - elements, bits)
                            & if crate::planted::is(1051) {
                                0
                            } else {
                                u128::MAX
                            }
                    }
                } else if keeps && !crate::planted::is(1052) {
                    read_element(old, index, bits)
                } else {
                    0
                };
                put(index, value);
            }
        }
        MoveOperation::DuplicateGeneral => {
            let value = general(first_number, RegisterThirtyOne::StackPointer) & to_mask(bits);
            for index in 0..elements {
                put(index, value);
            }
        }
        MoveOperation::Insert {
            general: from_general,
        } => {
            let scalar = if from_general {
                general(first_number, RegisterThirtyOne::ZeroRegister)
            } else if crate::planted::is(1053) {
                first >> bits
            } else {
                first
            } & to_mask(bits);
            result = (old << bits) | scalar;
        }
        MoveOperation::Unpack { signedness, part } => {
            if size == 0 {
                return Err(Exception::UndefinedInstruction);
            }
            let narrow = bits / 2;
            let base = if part == Part::Second { elements } else { 0 };
            for index in 0..elements {
                let element = read_element(first, base + index, narrow);
                let signedness = if crate::planted::is(1054) {
                    Signedness::Unsigned
                } else {
                    signedness
                };
                put(index, to_integer(element, narrow, signedness) as u128);
            }
        }
        MoveOperation::Reverse => {
            for index in 0..elements {
                put(index, read_element(first, elements - 1 - index, bits));
            }
        }
        MoveOperation::Copy {
            general: from_general,
        } => {
            let scalar = if from_general {
                general(first_number, RegisterThirtyOne::StackPointer)
            } else {
                first
            } & to_mask(bits);
            result = old;
            for index in 0..elements {
                if is_active(governing, index, bits) {
                    write_element(&mut result, index, bits, scalar);
                }
            }
        }
        MoveOperation::Last { after, target } => {
            let element = read_element(first, find_last_taken(governing, bits, after), bits);
            return write_taken(processor, destination, target, element, bits);
        }
        MoveOperation::ConditionalLast { after, target } => {
            let Some(last) = find_last_active(governing, bits) else {
                let kept = match target {
                    ElementTarget::General => {
                        general(destination, RegisterThirtyOne::ZeroRegister) & to_mask(bits)
                    }
                    ElementTarget::Scalar | ElementTarget::Vector if crate::planted::is(1047) => {
                        old
                    }
                    ElementTarget::Scalar | ElementTarget::Vector => old & to_mask(bits),
                };
                if target == ElementTarget::Vector {
                    return Ok(());
                }
                return write_taken(processor, destination, target, kept, bits);
            };
            let taken = if after { (last + 1) % elements } else { last };
            let element = read_element(first, taken, bits);
            return write_taken(processor, destination, target, element, bits);
        }
        MoveOperation::Splice { pair } => {
            let (low, high) = if pair { (first, next) } else { (old, first) };
            let mut placed = 0;
            if let Some(last) = find_last_active(governing, bits) {
                let start = (0..=last)
                    .find(|index| is_active(governing, *index, bits) || crate::planted::is(1048))
                    .unwrap_or(last);
                for index in start..=last {
                    put(placed, read_element(low, index, bits));
                    placed += 1;
                }
            }
            for index in 0..elements - placed {
                put(placed + index, read_element(high, index, bits));
            }
        }
        MoveOperation::Select => {
            let predicate = processor.predicate_registers[PG4.read(word) as usize];
            for index in 0..elements {
                let chosen = if is_active(predicate, index, bits) != crate::planted::is(1057) {
                    first
                } else {
                    second
                };
                put(index, read_element(chosen, index, bits));
            }
        }
        MoveOperation::Reduce(reduction) => {
            result = reduce(reduction, first, governing, bits)?;
        }
    }
    processor.vector_registers[destination as usize] = result;
    Ok(())
}

/// Writes the element LASTA, LASTB, CLASTA or CLASTB takes into its target.
fn write_taken<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    destination: u32,
    target: ElementTarget,
    element: u128,
    bits: u32,
) -> Result<(), Exception> {
    match target {
        ElementTarget::General => write_register(
            processor,
            destination,
            element as u64,
            RegisterThirtyOne::ZeroRegister,
        ),
        ElementTarget::Scalar => processor.vector_registers[destination as usize] = element,
        ElementTarget::Vector => {
            let mut result = 0;
            for index in 0..count_elements(bits) {
                write_element(&mut result, index, bits, element);
            }
            processor.vector_registers[destination as usize] = result;
        }
    }
    Ok(())
}

/// The active elements combined into one, which fills the low bits of the destination.
fn reduce(
    reduction: Reduction,
    source: u128,
    governing: u16,
    bits: u32,
) -> Result<u128, Exception> {
    let active = (0..count_elements(bits))
        .filter(|index| is_active(governing, *index, bits) || crate::planted::is(1055))
        .map(|index| read_element(source, index, bits));
    let mask = to_mask(bits);
    Ok(match reduction {
        Reduction::Sum(signedness) => {
            if signedness == Signedness::Signed && bits == u64::BITS {
                return Err(Exception::UndefinedInstruction);
            }
            let sum: i128 = active
                .map(|element| to_integer(element, bits, signedness))
                .sum();
            sum as u128 & to_mask(u64::BITS)
        }
        Reduction::Maximum(signedness) => active
            .map(|element| to_integer(element, bits, signedness))
            .max()
            .map_or(
                match signedness {
                    Signedness::Signed => 1 << (bits - 1),
                    Signedness::Unsigned => 0,
                },
                |value| value as u128 & mask,
            ),
        Reduction::Minimum(signedness) => active
            .map(|element| to_integer(element, bits, signedness))
            .min()
            .map_or(
                match signedness {
                    Signedness::Signed => mask >> 1,
                    Signedness::Unsigned if crate::planted::is(1056) => 0,
                    Signedness::Unsigned => mask,
                },
                |value| value as u128 & mask,
            ),
        Reduction::Or => active.fold(0, |value, element| value | element),
        Reduction::ExclusiveOr => active.fold(0, |value, element| value ^ element),
        Reduction::And => active.fold(mask, |value, element| value & element),
    })
}
