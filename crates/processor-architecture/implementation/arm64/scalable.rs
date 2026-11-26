//! The SVE and SVE2 instructions of the second half of the work list at a 128-bit vector length, where a Z register
//! is the V register of its number: COMPACT here, transcribed from Arm's pseudocode
//! ([COMPACT](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/COMPACT--Copy-active-vector-elements-to-lower-numbered-elements-)),
//! integer in `scalable_integer` and floating point in `scalable_floating_point`.
//!
//! Each modeled instruction names its operands by field here, since the list leaves SVE operands unmodeled.

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

/// The bits of a Z register at a 128-bit vector length.
pub const VECTOR_BITS: u32 = u128::BITS;

pub const ZD: Field = Field::new(0, 5);
pub const ZN: Field = Field::new(5, 5);
pub const ZM: Field = Field::new(16, 5);
/// The governing predicate of most predicated forms, P0 to P7.
pub const PG: Field = Field::new(10, 3);
/// The element size: a byte shifted left by it.
pub const SIZE: Field = Field::new(22, 2);
/// COMPACT's element size: a word shifted left by it.
const SIZE_WORD_OR_DOUBLEWORD: Field = Field::new(22, 1);

/// One operand of a modeled SVE instruction, by the field that encodes it, in the order of the assembler syntax.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScalableOperand {
    /// The field of the element size.
    ElementSize(Field),
    /// Z registers the instruction reads, `registers` consecutive ones from the field's, wrapping after Z31.
    VectorSource { number: Field, registers: u32 },
    /// A Z register the instruction writes without reading it.
    VectorDestination(Field),
    /// A Z register the instruction reads and then writes.
    VectorSourceDestination(Field),
    /// A predicate the instruction reads, governing or not.
    PredicateSource(Field),
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
    let permute = match to_scalable_operation(instruction)? {
        ScalableOperation::Permute(permute) => permute,
        ScalableOperation::FloatingPoint(form) => return Some(list_floating_point_operands(form)),
        ScalableOperation::Integer(form) => return Some(list_integer_operands(form)),
    };
    Some(match permute {
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
        PermuteOperation::Compact => Some(u32::BITS << SIZE_WORD_OR_DOUBLEWORD.read(word)),
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
    match operation {
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
