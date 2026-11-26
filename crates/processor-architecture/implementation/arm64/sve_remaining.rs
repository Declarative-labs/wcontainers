//! The SVE and SVE2 instructions the two halves of the SVE work left, at a 128-bit vector length, where a Z register is
//! the V register of its number: floating point in `sve_remaining_floating_point` (pairwise, indexed, complex,
//! reductions, estimates, conversions, the trigonometric helpers, BFloat16 and the widening multiply-adds) and integers
//! in `sve_remaining_integer` (indexed multiplies, dot and matrix products, complex arithmetic, narrowing and widening
//! shifts, carries, bit permutes, histograms, addresses and cryptography), each following its page of Arm's
//! [SVE instructions](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions).

use core::convert::Infallible;

use crate::encoding::Field;
use crate::sve_remaining_floating_point::{
    RemainingFloatingPoint, simulate_remaining_floating_point, to_floating_point_bits,
    to_remaining_floating_point,
};
use crate::sve_remaining_integer::{
    RemainingInteger, simulate_remaining_integer, to_integer_bits, to_remaining_integer,
};
use crate::{Exception, Instruction, Processor};

pub use crate::sve_remaining_planted::REMAINING_PLANTED_BUGS;

/// The bits of a Z register at a 128-bit vector length.
pub const VECTOR_BITS: u32 = u128::BITS;

pub const ZD: Field = Field::new(0, 5);
pub const ZN: Field = Field::new(5, 5);
pub const ZM: Field = Field::new(16, 5);
/// The second source of a destructive form, at bits 9 to 5.
pub const ZM_LOW: Field = ZN;
/// The element size of most encodings: a byte shifted left by it.
pub const SIZE: Field = Field::new(22, 2);
/// The governing predicate, P0 to P7.
pub const PG: Field = Field::new(10, 3);

/// Where an indexed form keeps its element's index and the register it reads the element from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IndexLayout {
    /// i3h at bit 22 and i3l at bits 20 to 19, Zm at bits 18 to 16: halfwords of a same-size form.
    HighThree,
    /// i2 at bits 20 to 19, Zm at bits 18 to 16.
    Two,
    /// i1 at bit 20, Zm at bits 19 to 16.
    One,
    /// i3h at bits 20 to 19 and i3l at bit 11, Zm at bits 18 to 16: halfwords of a long form.
    LongThree,
    /// i2h at bit 20 and i2l at bit 11, Zm at bits 19 to 16: words of a long form.
    LongTwo,
}

const I3H_HIGH: Field = Field::new(22, 1);
const I_PAIR: Field = Field::new(19, 2);
const I_ONE: Field = Field::new(20, 1);
const I_LOW: Field = Field::new(11, 1);
const ZM_THREE: Field = Field::new(16, 3);
const ZM_FOUR: Field = Field::new(16, 4);

impl IndexLayout {
    /// The fields of the index, the most significant first.
    pub const fn to_index_fields(self) -> &'static [Field] {
        match self {
            Self::HighThree => &[I3H_HIGH, I_PAIR],
            Self::Two => &[I_PAIR],
            Self::One => &[I_ONE],
            Self::LongThree => &[I_PAIR, I_LOW],
            Self::LongTwo => &[I_ONE, I_LOW],
        }
    }

    /// The field of the register the element is read from.
    pub const fn to_register_field(self) -> Field {
        match self {
            Self::HighThree | Self::Two | Self::LongThree => ZM_THREE,
            Self::One | Self::LongTwo => ZM_FOUR,
        }
    }

    /// The index the word holds.
    pub fn read_index(self, word: u32) -> u32 {
        self.to_index_fields()
            .iter()
            .fold(0, |index, field| (index << field.width) | field.read(word))
    }

    /// The register the element is read from, as the word names it.
    pub fn read_register<LevelSpecific>(
        self,
        processor: &Processor<LevelSpecific>,
        word: u32,
    ) -> u128 {
        processor.vector_registers[self.to_register_field().read(word) as usize]
    }
}

/// What a modeled instruction of this module computes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RemainingOperation {
    Integer(RemainingInteger),
    FloatingPoint(RemainingFloatingPoint),
}

/// The operation of each SVE instruction this module models, none for every other instruction.
pub const fn to_remaining_operation(
    instruction: Instruction<Infallible>,
) -> Option<RemainingOperation> {
    if let Some(operation) = to_remaining_integer(instruction) {
        return Some(RemainingOperation::Integer(operation));
    }
    match to_remaining_floating_point(instruction) {
        Some(operation) => Some(RemainingOperation::FloatingPoint(operation)),
        None => None,
    }
}

/// The size of the elements the word gives the operation, in bits; none where the encoding is unallocated.
pub fn to_element_bits(operation: RemainingOperation, word: u32) -> Option<u32> {
    match operation {
        RemainingOperation::Integer(operation) => to_integer_bits(operation, word),
        RemainingOperation::FloatingPoint(operation) => to_floating_point_bits(operation, word),
    }
}

/// Applies the operation to the registers the word names; changes nothing where it returns an exception.
pub fn simulate_remaining<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operation: RemainingOperation,
) -> Result<(), Exception> {
    match operation {
        RemainingOperation::Integer(operation) => {
            simulate_remaining_integer(processor, word, operation)
        }
        RemainingOperation::FloatingPoint(operation) => {
            simulate_remaining_floating_point(processor, word, operation)
        }
    }
}

/// Ones in the lowest `bits` bits.
pub const fn to_mask(bits: u32) -> u128 {
    if bits >= u128::BITS {
        u128::MAX
    } else {
        (1 << bits) - 1
    }
}

pub const fn read_lane(vector: u128, index: u32, bits: u32) -> u128 {
    (vector >> (index * bits)) & to_mask(bits)
}

pub fn write_lane(vector: &mut u128, index: u32, bits: u32, value: u128) {
    let mask = to_mask(bits) << (index * bits);
    *vector = (*vector & !mask) | ((value << (index * bits)) & mask);
}

/// The lane's bits as a two's complement integer, or as an unsigned one.
pub const fn to_integer(value: u128, bits: u32, signed: bool) -> i128 {
    if signed {
        ((value << (u128::BITS - bits)) as i128) >> (u128::BITS - bits)
    } else {
        (value & to_mask(bits)) as i128
    }
}

/// The integer clamped to the signed or unsigned range of `bits` bits, as its bits.
pub fn saturate(value: i128, bits: u32, signed: bool) -> u128 {
    let (low, high) = if signed {
        (-(1_i128 << (bits - 1)), (1_i128 << (bits - 1)) - 1)
    } else {
        (0, (1_i128 << bits) - 1)
    };
    (value.clamp(low, high) as u128) & to_mask(bits)
}

/// Whether the predicate's element of `bits` bits is active: the lowest of its bits is set.
pub const fn is_active(predicate: u16, index: u32, bits: u32) -> bool {
    predicate >> (index * bits / u8::BITS) & 1 != 0
}

pub fn read_vector<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    word: u32,
    field: Field,
) -> u128 {
    processor.vector_registers[field.read(word) as usize]
}

pub fn read_predicate<LevelSpecific>(processor: &Processor<LevelSpecific>, word: u32) -> u16 {
    processor.predicate_registers[PG.read(word) as usize]
}
