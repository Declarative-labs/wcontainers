//! The SVE and SVE2 instructions of the second half of the work list at a 128-bit vector length, where a Z register
//! is the V register of its number: permutes and table lookups here, each transcribed from Arm's pseudocode (for
//! example [ZIP1](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/ZIP1--ZIP2--vectors---Interleave-elements-from-two-half-vectors-)),
//! and floating point in `scalable_floating_point`.
//!
//! Each modeled instruction names its operands by field here, since the list leaves SVE operands unmodeled.

use alloc::vec::Vec;
use core::convert::Infallible;

use crate::encoding::Field;
use crate::scalable_floating_point::{
    FloatingPointForm, VectorRole, list_floating_point_operands, simulate_floating_point_form,
    to_floating_point_form,
};
use crate::scalable_integer::{
    IntegerForm, list_integer_operands, simulate_integer_form, to_integer_form,
};
use crate::{Exception, Instruction, PREDICATE_REGISTERS, Processor, VECTOR_REGISTERS};

/// The bits of a Z register and the bytes of one at a 128-bit vector length.
pub const VECTOR_BITS: u32 = u128::BITS;
pub const VECTOR_BYTES: u32 = VECTOR_BITS / u8::BITS;

pub const ZD: Field = Field::new(0, 5);
pub const ZN: Field = Field::new(5, 5);
pub const ZM: Field = Field::new(16, 5);
/// The governing predicate of most predicated forms, P0 to P7.
pub const PG: Field = Field::new(10, 3);
/// The element size: a byte shifted left by it.
pub const SIZE: Field = Field::new(22, 2);
/// COMPACT's element size: a word shifted left by it.
const SIZE_WORD_OR_DOUBLEWORD: Field = Field::new(22, 1);
/// EXT's byte position, imm8h:imm8l.
pub const EXTRACT_POSITION: [Field; 2] = [Field::new(16, 5), Field::new(10, 3)];
/// DUP (indexed)'s imm2:tsz, which encode the element size and the index together.
pub const DUPLICATE_INDEX: [Field; 2] = [Field::new(22, 2), Field::new(16, 5)];
/// The bits of tsz in DUP (indexed)'s imm2:tsz.
const DUPLICATE_TSZ_BITS: u32 = 5;

/// One operand of a modeled SVE instruction, by the field that encodes it, in the order of the assembler syntax.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScalableOperand {
    /// The field of the element size.
    ElementSize(Field),
    /// Z registers the instruction reads, `registers` consecutive ones from the field's, wrapping after Z31.
    VectorSource { number: Field, registers: u32 },
    /// A Z register whose elements index a table.
    TableIndices(Field),
    /// A Z register the instruction writes without reading it.
    VectorDestination(Field),
    /// A Z register the instruction reads and then writes.
    VectorSourceDestination(Field),
    /// A predicate the instruction reads, governing or not.
    PredicateSource(Field),
    /// An immediate split over fields, the most significant first, which takes every value it can hold.
    EveryValue(&'static [Field]),
    /// A byte position in a Z register, split over fields, the most significant first.
    BytePosition(&'static [Field]),
    /// A Z register of floating-point elements of the size the size field gives, read and also written where it is
    /// the destination's field.
    FloatingPointSource { number: Field, role: VectorRole },
    /// The Z register a conversion reads, its lanes as the instruction gives them.
    ConvertedSource(Field),
    /// A predicate the instruction writes without reading it.
    PredicateDestination(Field),
    /// The bit that chooses an immediate form's constant.
    ImmediateChoice(Field),
    /// FPCR, as an arithmetic instruction or a conversion between precisions reads it, and FPSR before.
    FloatingPointControl { conversion: bool },
}

/// What a modeled permute computes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PermuteOperation {
    /// ZIP1 and ZIP2: the low or high halves of the two sources, interleaved.
    Zip { high: bool },
    /// UZP1 and UZP2: the even or odd elements of the second source above the first.
    Unzip { odd: bool },
    /// TRN1 and TRN2: the even or odd elements of each source, interleaved.
    Transpose { odd: bool },
    /// TBL of one or two table registers; TBX keeps the destination's element for an index past the table.
    TableLookup {
        registers: u32,
        keeps_destination: bool,
    },
    /// EXT: the bytes of the second source above the first, from the immediate's byte.
    Extract { destructive: bool },
    /// DUP (indexed): one element of the source in every element.
    DuplicateElement,
    /// REV (vector).
    Reverse,
    /// SPLICE: the first source's elements from its first active one to its last, then the second's from its first.
    Splice { destructive: bool },
    /// SUNPKLO, SUNPKHI, UUNPKLO and UUNPKHI: the low or high half's elements, extended to twice their size.
    Unpack { high: bool, signed: bool },
    /// COMPACT: the active elements, from the first element up, and zero above them.
    Compact,
}

/// What a modeled SVE instruction of this module computes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScalableOperation {
    Permute(PermuteOperation),
    FloatingPoint(FloatingPointForm),
    Integer(IntegerForm),
}

/// The operation of each SVE instruction this module models, none for every other instruction.
pub const fn to_scalable_operation(
    instruction: Instruction<Infallible>,
) -> Option<ScalableOperation> {
    if let Some(permute) = to_permute_operation(instruction) {
        return Some(ScalableOperation::Permute(permute));
    }
    if let Some(form) = to_integer_form(instruction) {
        return Some(ScalableOperation::Integer(form));
    }
    match to_floating_point_form(instruction) {
        Some(form) => Some(ScalableOperation::FloatingPoint(form)),
        None => None,
    }
}

/// Whether the word is unallocated for a modeled SVE instruction: its element size has no encoding.
pub fn is_unallocated(instruction: Instruction<Infallible>, word: u32) -> bool {
    to_scalable_operation(instruction)
        .is_some_and(|operation| to_element_bits(operation, word).is_none())
        || crate::load_store::scalable_access::to_scalable_access(instruction)
            .is_some_and(|access| access.to_element_bits(word).is_none())
        || crate::sve_remaining::to_remaining_operation(instruction).is_some_and(|operation| {
            crate::sve_remaining::to_element_bits(operation, word).is_none()
        })
}

const fn to_permute_operation(instruction: Instruction<Infallible>) -> Option<PermuteOperation> {
    use Instruction as I;
    use PermuteOperation as P;
    Some(match instruction {
        I::CompactZPZ64 => P::Compact,
        _ => return None,
    })
}

/// The operands of a modeled SVE instruction in the order of its syntax, none for every other instruction.
pub fn list_scalable_operands(
    instruction: Instruction<Infallible>,
) -> Option<&'static [ScalableOperand]> {
    use ScalableOperand as O;
    const THREE_REGISTERS: &[ScalableOperand] = &[
        O::ElementSize(SIZE),
        O::VectorDestination(ZD),
        O::VectorSource {
            number: ZN,
            registers: 1,
        },
        O::VectorSource {
            number: ZM,
            registers: 1,
        },
    ];
    let permute = match to_scalable_operation(instruction)? {
        ScalableOperation::Permute(permute) => permute,
        ScalableOperation::FloatingPoint(form) => return Some(list_floating_point_operands(form)),
        ScalableOperation::Integer(form) => return Some(list_integer_operands(form)),
    };
    Some(match permute {
        PermuteOperation::Zip { .. }
        | PermuteOperation::Unzip { .. }
        | PermuteOperation::Transpose { .. } => THREE_REGISTERS,
        PermuteOperation::TableLookup {
            registers,
            keeps_destination,
        } => match (registers, keeps_destination) {
            (1, false) => &[
                O::ElementSize(SIZE),
                O::VectorDestination(ZD),
                O::VectorSource {
                    number: ZN,
                    registers: 1,
                },
                O::TableIndices(ZM),
            ],
            (_, false) => &[
                O::ElementSize(SIZE),
                O::VectorDestination(ZD),
                O::VectorSource {
                    number: ZN,
                    registers: 2,
                },
                O::TableIndices(ZM),
            ],
            (_, true) => &[
                O::ElementSize(SIZE),
                O::VectorSourceDestination(ZD),
                O::VectorSource {
                    number: ZN,
                    registers: 1,
                },
                O::TableIndices(ZM),
            ],
        },
        PermuteOperation::Extract { destructive: true } => &[
            O::VectorSourceDestination(ZD),
            O::VectorSource {
                number: ZN,
                registers: 1,
            },
            O::BytePosition(&EXTRACT_POSITION),
        ],
        PermuteOperation::Extract { destructive: false } => &[
            O::VectorDestination(ZD),
            O::VectorSource {
                number: ZN,
                registers: 2,
            },
            O::BytePosition(&EXTRACT_POSITION),
        ],
        PermuteOperation::DuplicateElement => &[
            O::VectorDestination(ZD),
            O::VectorSource {
                number: ZN,
                registers: 1,
            },
            O::EveryValue(&DUPLICATE_INDEX),
        ],
        PermuteOperation::Reverse | PermuteOperation::Unpack { .. } => &[
            O::ElementSize(SIZE),
            O::VectorDestination(ZD),
            O::VectorSource {
                number: ZN,
                registers: 1,
            },
        ],
        PermuteOperation::Splice { destructive: true } => &[
            O::ElementSize(SIZE),
            O::VectorSourceDestination(ZD),
            O::PredicateSource(PG),
            O::VectorSource {
                number: ZN,
                registers: 1,
            },
        ],
        PermuteOperation::Splice { destructive: false } => &[
            O::ElementSize(SIZE),
            O::VectorDestination(ZD),
            O::PredicateSource(PG),
            O::VectorSource {
                number: ZN,
                registers: 2,
            },
        ],
        PermuteOperation::Compact => &[
            O::ElementSize(SIZE_WORD_OR_DOUBLEWORD),
            O::VectorDestination(ZD),
            O::PredicateSource(PG),
            O::VectorSource {
                number: ZN,
                registers: 1,
            },
        ],
    })
}

/// The element of `esize` bits at the index.
pub fn read_element(vector: u128, index: u32, esize: u32) -> u128 {
    (vector >> (index * esize)) & (u128::MAX >> (u128::BITS - esize))
}

/// Whether the predicate's element of `esize` bits at the index is active: its lowest bit.
pub fn is_active(predicate: u16, index: u32, esize: u32) -> bool {
    predicate >> (index * esize / u8::BITS) & 1 != 0
}

/// The Z register `offset` registers after the one the field names, wrapping after Z31.
pub fn read_vector(
    processor: &Processor<impl Sized>,
    word: u32,
    field: Field,
    offset: u32,
) -> u128 {
    processor.vector_registers[((field.read(word) + offset) % VECTOR_REGISTERS as u32) as usize]
}

pub fn read_predicate(processor: &Processor<impl Sized>, word: u32, field: Field) -> u16 {
    processor.predicate_registers[field.read(word) as usize % PREDICATE_REGISTERS]
}

/// The esize of a modeled instruction's elements in bits, none where its encoding is unallocated.
pub fn to_element_bits(operation: ScalableOperation, word: u32) -> Option<u32> {
    let permute = match operation {
        ScalableOperation::Permute(permute) => permute,
        ScalableOperation::FloatingPoint(form) => {
            return crate::scalable_floating_point::to_element_bits(form, word);
        }
        ScalableOperation::Integer(form) => {
            return crate::scalable_integer::to_element_bits(form, word);
        }
    };
    match permute {
        PermuteOperation::Extract { .. } => Some(u8::BITS),
        // The lowest set bit of tsz; a tsz of zero is unallocated.
        PermuteOperation::DuplicateElement => {
            let tsz = DUPLICATE_INDEX[1].read(word);
            (tsz != 0).then(|| u8::BITS << tsz.trailing_zeros())
        }
        // The size of the result's elements, twice the source's; a size of zero is unallocated.
        PermuteOperation::Unpack { .. } => {
            let size = SIZE.read(word);
            (size != 0).then(|| u8::BITS << size)
        }
        PermuteOperation::Compact => Some(u32::BITS << SIZE_WORD_OR_DOUBLEWORD.read(word)),
        _ => Some(u8::BITS << SIZE.read(word)),
    }
}

/// Applies the operation to the registers the word names; changes nothing where it returns an exception.
pub fn simulate_scalable<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operation: ScalableOperation,
) -> Result<(), Exception> {
    let permute = match operation {
        ScalableOperation::Permute(permute) => permute,
        ScalableOperation::FloatingPoint(form) => {
            return simulate_floating_point_form(processor, word, form);
        }
        ScalableOperation::Integer(form) => {
            return simulate_integer_form(processor, word, form);
        }
    };
    let Some(esize) = to_element_bits(operation, word) else {
        return Err(Exception::UndefinedInstruction);
    };
    let result = permute_vectors(processor, word, permute, esize);
    processor.vector_registers[ZD.read(word) as usize] = result;
    Ok(())
}

/// The result of a permute of elements of `esize` bits.
fn permute_vectors<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    word: u32,
    operation: PermuteOperation,
    esize: u32,
) -> u128 {
    use PermuteOperation as P;
    let elements = VECTOR_BITS / esize;
    let mut result = 0_u128;
    let mut put = |index: u32, value: u128| result |= value << (index * esize);
    let first = read_vector(processor, word, ZN, 0);
    let second = read_vector(processor, word, ZM, 0);
    let destination = read_vector(processor, word, ZD, 0);
    let half = elements / 2;
    match operation {
        P::Zip { high } => {
            let base = if high {
                half - u32::from(crate::planted::is(2001))
            } else {
                0
            };
            for pair in 0..half {
                put(2 * pair, read_element(first, base + pair, esize));
                put(2 * pair + 1, read_element(second, base + pair, esize));
            }
        }
        P::Unzip { odd } => {
            for index in 0..elements {
                let taken = 2 * index + u32::from(odd);
                let taken = if crate::planted::is(2002) {
                    (taken + elements) % (2 * elements)
                } else {
                    taken
                };
                let value = if taken < elements {
                    read_element(first, taken, esize)
                } else {
                    read_element(second, taken - elements, esize)
                };
                put(index, value);
            }
        }
        P::Transpose { odd } => {
            for pair in 0..half {
                let taken = 2 * pair + u32::from(odd);
                put(2 * pair, read_element(first, taken, esize));
                let second_taken = taken + u32::from(crate::planted::is(2003) && odd);
                put(
                    2 * pair + 1,
                    read_element(second, second_taken % elements, esize),
                );
            }
        }
        P::TableLookup {
            registers,
            keeps_destination,
        } => {
            // [TBL](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/TBL--Programmable-table-lookup-in-one-or-two-vector-table-):
            // an index past the table's elements gives zero, or for TBX the destination's element.
            for index in 0..elements {
                let table_index = read_element(second, index, esize);
                let table_elements = if crate::planted::is(2004) {
                    registers * elements + 1
                } else if crate::planted::is(2005) {
                    elements
                } else {
                    registers * elements
                };
                let value = if table_index < u128::from(table_elements) {
                    let table_index = (table_index as u32) % (registers * elements);
                    let offset = if crate::planted::is(2006) && table_index >= elements {
                        // The second table register without the wrap after Z31.
                        u32::from(ZN.read(word) != VECTOR_REGISTERS as u32 - 1)
                    } else {
                        table_index / elements
                    };
                    let table = read_vector(processor, word, ZN, offset);
                    read_element(table, table_index % elements, esize)
                } else if keeps_destination && !crate::planted::is(2007) {
                    read_element(destination, index, esize)
                } else {
                    0
                };
                put(index, value);
            }
        }
        P::Extract { destructive } => {
            // [EXT](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/EXT--Extract-vector-from-pair-of-vectors-):
            // a position past the vector's bytes is zero.
            let position = EXTRACT_POSITION[0].read(word) << EXTRACT_POSITION[1].width
                | EXTRACT_POSITION[1].read(word);
            let position = if position >= VECTOR_BYTES && !crate::planted::is(2008) {
                0
            } else {
                position % VECTOR_BYTES
            };
            let (low, high) = if destructive {
                (destination, first)
            } else {
                (first, read_vector(processor, word, ZN, 1))
            };
            let shift = position * u8::BITS;
            result = if shift == 0 {
                low
            } else {
                (low >> shift) | (high << (VECTOR_BITS - shift))
            };
        }
        P::DuplicateElement => {
            // [DUP (indexed)](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/DUP--indexed---Broadcast-vector-element-):
            // the index is imm2:tsz above tsz's lowest set bit; past the vector's elements it gives zero.
            let encoded =
                DUPLICATE_INDEX[0].read(word) << DUPLICATE_TSZ_BITS | DUPLICATE_INDEX[1].read(word);
            let element_index = encoded >> (esize / u8::BITS).trailing_zeros() >> 1;
            let element_index = if crate::planted::is(2009) {
                element_index % elements
            } else {
                element_index
            };
            let value = if element_index < elements {
                read_element(first, element_index, esize)
            } else {
                0
            };
            for index in 0..elements {
                put(index, value);
            }
        }
        P::Reverse => {
            for index in 0..elements {
                let reversed = if crate::planted::is(2010) && esize == u64::BITS {
                    index
                } else {
                    elements - 1 - index
                };
                put(index, read_element(first, reversed, esize));
            }
        }
        P::Splice { destructive } => {
            // [SPLICE](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/SPLICE--Splice-two-vectors-under-predicate-control-).
            let predicate = read_predicate(processor, word, PG);
            let (spliced, rest) = if destructive {
                (destination, first)
            } else {
                (first, read_vector(processor, word, ZN, 1))
            };
            let active: Vec<u32> = (0..elements)
                .filter(|index| {
                    if crate::planted::is(2012) {
                        predicate >> index & 1 != 0
                    } else {
                        is_active(predicate, *index, esize)
                    }
                })
                .collect();
            let mut place = 0;
            if let (Some(&from), Some(&to)) = (active.first(), active.last()) {
                for index in from..=to {
                    put(place, read_element(spliced, index, esize));
                    place += 1;
                }
            }
            let rest_elements = if crate::planted::is(2011) && place == 0 {
                0
            } else {
                elements - place
            };
            for index in 0..rest_elements {
                put(place + index, read_element(rest, index, esize));
            }
        }
        P::Unpack { high, signed } => {
            let source_bits = esize / 2;
            let base = if high && !(crate::planted::is(2013) && esize == u64::BITS) {
                elements
            } else {
                0
            };
            for index in 0..elements {
                let value = read_element(first, base + index, source_bits);
                let value = if signed {
                    (((value << (u128::BITS - source_bits)) as i128) >> (u128::BITS - source_bits))
                        as u128
                        & (u128::MAX >> (u128::BITS - esize))
                } else {
                    value
                };
                put(index, value);
            }
        }
        P::Compact => {
            let predicate = read_predicate(processor, word, PG);
            let mut place = 0;
            for index in 0..elements {
                if is_active(predicate, index, esize) {
                    put(place, read_element(first, index, esize));
                    place += 1;
                }
            }
            if crate::planted::is(2014) {
                for index in place..elements {
                    put(index, read_element(first, index, esize));
                }
            }
        }
    }
    result
}
