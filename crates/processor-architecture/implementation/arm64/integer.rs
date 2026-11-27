//! The general-purpose integer instructions that touch no memory: one skeleton reads the operands the list gives
//! each instruction, and one small operation per family computes its result and flags.

use core::convert::Infallible;

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::power2::pow2;
use vstd::prelude::*;

use crate::condition_flags::{
    CONDITIONS, ConditionFlags, IMMEDIATE_CARRY, IMMEDIATE_NEGATIVE, IMMEDIATE_OVERFLOW,
    IMMEDIATE_ZERO, Nzcv,
};
use crate::operand::{
    Operand, OperandAccess, RegisterThirtyOne, RegisterWidth, Rotation, Signedness,
};
#[cfg(verus_keep_ghost)]
use crate::pseudocode::{
    arithmetic_immediate_value, arithmetic_or_logical, bitfield_move, bitmask_immediate_value,
    conditional_compare, conditional_select, count_leading_sign_bits, count_leading_zero_bits,
    crc32, divide, extended_register, extract, is_zero, logical_result, long_product, move_wide,
    multiply_add, multiply_add_long, multiply_high, multiply_long_value, nzcv_immediate, ones,
    reverse_bits_steps, reverse_bits_value, reverse_bytes_steps, reverse_bytes_value,
    reversed_byte, ror, set_bits, shifted_register, sint, uint, variable_shift,
    wide_immediate_position, written,
};
use crate::registers::{read_register, write_register};
use crate::{Exception, Instruction, Processor};

/// The most source operands an instruction reads: CCMP's register, second operand, flags and condition.
const MAXIMUM_SOURCES: usize = 4;
verus! {

/// The [shift amount](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--immediate---Add-immediate-value-#:~:text=sh) a set shift bit gives an arithmetic immediate.
const IMMEDIATE_SHIFT: u32 = 12;

} // verus!
verus! {

/// How many bits each step of a move wide's hw moves its immediate.
const WIDE_IMMEDIATE_STEP: u32 = u16::BITS;

/// The largest left shift of an [extended register](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--extended-register---Add-extended-and-scaled-register-#:~:text=amount); larger ones are unallocated.
const MAXIMUM_EXTEND_SHIFT: u64 = 4;

} // verus!
verus! {

/// The width of immr and imms, whose values above a 32-bit element's are unused.
const BITMASK_FIELD_BITS: u32 = 6;

/// [CRC-32](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CRC32B--CRC32H--CRC32W--CRC32X--CRC32-checksum-#:~:text=0x04C11DB7)'s polynomial.
const CRC32_POLYNOMIAL: u32 = 0x04C1_1DB7;
/// [CRC-32C](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CRC32CB--CRC32CH--CRC32CW--CRC32CX--CRC32C-checksum-#:~:text=0x1EDC6F41)'s polynomial.
const CRC32C_POLYNOMIAL: u32 = 0x1EDC_6F41;

} // verus!

verus! {

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlagSetting {
    Preserves,
    Sets,
}

/// What a conditional select gives when its condition fails: the second register, plus one, inverted or negated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SelectAlternative {
    Select,
    Increment,
    Invert,
    Negate,
}

/// What [SBFM, BFM and UBFM](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/BFM--Bitfield-move-) put around the moved field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BitfieldFill {
    SignBits,
    DestinationBits,
    Zeros,
}

/// What a conditional compare sets its flags from when its condition holds: CCMN from a sum, CCMP from a difference.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Comparison {
    Sum,
    Difference,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CrcPolynomial {
    Crc32,
    Crc32C,
}

/// What one family of integer instructions computes from its operands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntegerOperation {
    Add(FlagSetting),
    Subtract(FlagSetting),
    AddWithCarry(FlagSetting),
    SubtractWithCarry(FlagSetting),
    And(FlagSetting),
    AndNot(FlagSetting),
    Or,
    OrNot,
    ExclusiveOr,
    ExclusiveOrNot,
    MoveWideNot,
    MoveWideZero,
    MoveWideKeep,
    Bitfield(BitfieldFill),
    Extract,
    ConditionalSelect(SelectAlternative),
    ConditionalCompare(Comparison),
    ReverseBits,
    /// Reverses the bytes of each container of this many bits.
    ReverseBytes {
        container_bits: u32,
    },
    CountLeadingZeros,
    CountLeadingSignBits,
    ShiftLeftVariable,
    ShiftRightLogicalVariable,
    ShiftRightArithmeticVariable,
    RotateRightVariable,
    Divide(Signedness),
    MultiplyAdd,
    MultiplySubtract,
    MultiplyAddLong(Signedness),
    MultiplySubtractLong(Signedness),
    MultiplyHigh(Signedness),
    /// Accumulates the low `bytes` bytes of the second register into the checksum in the first.
    Crc32 {
        polynomial: CrcPolynomial,
        bytes: u32,
    },
    RotateMaskInsertFlags,
    /// SETF8 and SETF16 set N, Z and V from the low byte or halfword.
    SetFlags {
        bits: u32,
    },
    InvertCarry,
    /// [AXFLAG](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/AXFLAG--Convert-floating-point-condition-flags-from-Arm-to-external-format-).
    ConvertFlagsToExternalFormat,
    /// [XAFLAG](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/XAFLAG--Convert-floating-point-condition-flags-from-external-format-to-Arm-format-).
    ConvertFlagsFromExternalFormat,
}

} // verus!

/// What an instruction reads beyond the operands of its syntax.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ImplicitInputs {
    pub condition_flags: bool,
    /// The register it writes, part of which it keeps.
    pub destination: bool,
    /// Its own address, beyond any offset from it the syntax names: the return address a branch with link writes.
    pub program_counter: bool,
    /// FPCR.
    pub floating_point_control: bool,
}

impl IntegerOperation {
    pub const fn to_implicit_inputs(self) -> ImplicitInputs {
        let condition_flags = matches!(
            self,
            Self::AddWithCarry(_)
                | Self::SubtractWithCarry(_)
                | Self::ConditionalSelect(_)
                | Self::ConditionalCompare(_)
                | Self::RotateMaskInsertFlags
                | Self::SetFlags { .. }
                | Self::InvertCarry
                | Self::ConvertFlagsToExternalFormat
                | Self::ConvertFlagsFromExternalFormat
        );
        let destination = matches!(
            self,
            Self::MoveWideKeep | Self::Bitfield(BitfieldFill::DestinationBits)
        );
        ImplicitInputs {
            condition_flags,
            destination,
            program_counter: false,
            floating_point_control: false,
        }
    }
}

/// The family of each integer instruction that touches no memory, none for every other instruction.
pub const fn to_integer_operation(
    instruction: Instruction<Infallible>,
) -> Option<IntegerOperation> {
    use FlagSetting::{Preserves, Sets};
    use Instruction as I;
    use IntegerOperation as O;
    use Signedness::{Signed, Unsigned};
    Some(match instruction {
        I::AddImmediate32
        | I::AddImmediate64
        | I::AddShiftedRegister32
        | I::AddShiftedRegister64
        | I::AddExtendedRegister32
        | I::AddExtendedRegister64 => O::Add(Preserves),
        I::AddsImmediate32
        | I::AddsImmediate64
        | I::AddsShiftedRegister32
        | I::AddsShiftedRegister64
        | I::AddsExtendedRegister32
        | I::AddsExtendedRegister64 => O::Add(Sets),
        I::SubImmediate32
        | I::SubImmediate64
        | I::SubShiftedRegister32
        | I::SubShiftedRegister64
        | I::SubExtendedRegister32
        | I::SubExtendedRegister64 => O::Subtract(Preserves),
        I::SubsImmediate32
        | I::SubsImmediate64
        | I::SubsShiftedRegister32
        | I::SubsShiftedRegister64
        | I::SubsExtendedRegister32
        | I::SubsExtendedRegister64 => O::Subtract(Sets),
        I::Adc32 | I::Adc64 => O::AddWithCarry(Preserves),
        I::Adcs32 | I::Adcs64 => O::AddWithCarry(Sets),
        I::Sbc32 | I::Sbc64 => O::SubtractWithCarry(Preserves),
        I::Sbcs32 | I::Sbcs64 => O::SubtractWithCarry(Sets),
        I::AndImmediate32
        | I::AndImmediate64
        | I::AndShiftedRegister32
        | I::AndShiftedRegister64 => O::And(Preserves),
        I::AndsImmediate32
        | I::AndsImmediate64
        | I::AndsShiftedRegister32
        | I::AndsShiftedRegister64 => O::And(Sets),
        I::BicShiftedRegister32 | I::BicShiftedRegister64 => O::AndNot(Preserves),
        I::BicsShiftedRegister32 | I::BicsShiftedRegister64 => O::AndNot(Sets),
        I::OrrImmediate32
        | I::OrrImmediate64
        | I::OrrShiftedRegister32
        | I::OrrShiftedRegister64 => O::Or,
        I::OrnShiftedRegister32 | I::OrnShiftedRegister64 => O::OrNot,
        I::EorImmediate32
        | I::EorImmediate64
        | I::EorShiftedRegister32
        | I::EorShiftedRegister64 => O::ExclusiveOr,
        I::EonShiftedRegister32 | I::EonShiftedRegister64 => O::ExclusiveOrNot,
        I::MovnImmediate32 | I::MovnImmediate64 => O::MoveWideNot,
        I::MovzImmediate32 | I::MovzImmediate64 => O::MoveWideZero,
        I::MovkImmediate32 | I::MovkImmediate64 => O::MoveWideKeep,
        I::SbfmImmediate32 | I::SbfmImmediate64 => O::Bitfield(BitfieldFill::SignBits),
        I::BfmImmediate32 | I::BfmImmediate64 => O::Bitfield(BitfieldFill::DestinationBits),
        I::UbfmImmediate32 | I::UbfmImmediate64 => O::Bitfield(BitfieldFill::Zeros),
        I::Extr32 | I::Extr64 => O::Extract,
        I::Csel32 | I::Csel64 => O::ConditionalSelect(SelectAlternative::Select),
        I::Csinc32 | I::Csinc64 => O::ConditionalSelect(SelectAlternative::Increment),
        I::Csinv32 | I::Csinv64 => O::ConditionalSelect(SelectAlternative::Invert),
        I::Csneg32 | I::Csneg64 => O::ConditionalSelect(SelectAlternative::Negate),
        I::Ccmn32 | I::Ccmn64 | I::CcmnImmediate32 | I::CcmnImmediate64 => {
            O::ConditionalCompare(Comparison::Sum)
        }
        I::Ccmp32 | I::Ccmp64 | I::CcmpImmediate32 | I::CcmpImmediate64 => {
            O::ConditionalCompare(Comparison::Difference)
        }
        I::Rbit32 | I::Rbit64 => O::ReverseBits,
        I::Rev16Register32 | I::Rev16Register64 => O::ReverseBytes {
            container_bits: u16::BITS,
        },
        I::Rev32Register64 | I::RevRegister32 => O::ReverseBytes {
            container_bits: u32::BITS,
        },
        I::RevRegister64 => O::ReverseBytes {
            container_bits: u64::BITS,
        },
        I::Clz32 | I::Clz64 => O::CountLeadingZeros,
        I::Cls32 | I::Cls64 => O::CountLeadingSignBits,
        I::Lslv32 | I::Lslv64 => O::ShiftLeftVariable,
        I::Lsrv32 | I::Lsrv64 => O::ShiftRightLogicalVariable,
        I::Asrv32 | I::Asrv64 => O::ShiftRightArithmeticVariable,
        I::Rorv32 | I::Rorv64 => O::RotateRightVariable,
        I::Udiv32 | I::Udiv64 => O::Divide(Unsigned),
        I::Sdiv32 | I::Sdiv64 => O::Divide(Signed),
        I::Madd32 | I::Madd64 => O::MultiplyAdd,
        I::Msub32 | I::Msub64 => O::MultiplySubtract,
        I::Smaddl => O::MultiplyAddLong(Signed),
        I::Umaddl => O::MultiplyAddLong(Unsigned),
        I::Smsubl => O::MultiplySubtractLong(Signed),
        I::Umsubl => O::MultiplySubtractLong(Unsigned),
        I::Smulh64 => O::MultiplyHigh(Signed),
        I::Umulh64 => O::MultiplyHigh(Unsigned),
        I::Crc32b => crc(CrcPolynomial::Crc32, u8::BITS),
        I::Crc32h => crc(CrcPolynomial::Crc32, u16::BITS),
        I::Crc32w => crc(CrcPolynomial::Crc32, u32::BITS),
        I::Crc32x => crc(CrcPolynomial::Crc32, u64::BITS),
        I::Crc32cb => crc(CrcPolynomial::Crc32C, u8::BITS),
        I::Crc32ch => crc(CrcPolynomial::Crc32C, u16::BITS),
        I::Crc32cw => crc(CrcPolynomial::Crc32C, u32::BITS),
        I::Crc32cx => crc(CrcPolynomial::Crc32C, u64::BITS),
        I::Rmif => O::RotateMaskInsertFlags,
        I::Setf8 => O::SetFlags { bits: u8::BITS },
        I::Setf16 => O::SetFlags { bits: u16::BITS },
        I::Cfinv => O::InvertCarry,
        I::Axflag => O::ConvertFlagsToExternalFormat,
        I::Xaflag => O::ConvertFlagsFromExternalFormat,
        _ => return None,
    })
}

const fn crc(polynomial: CrcPolynomial, bits: u32) -> IntegerOperation {
    IntegerOperation::Crc32 {
        polynomial,
        bytes: bits / u8::BITS,
    }
}

/// The result an operation writes to its destination, and the flags it sets.
struct Outcome {
    result: Option<u64>,
    flags: Option<ConditionFlags>,
}

/// The operands of an instruction as its operation reads them.
pub(crate) struct ReadOperands {
    /// The number of the register the instruction writes, and what its number 31 names.
    pub destination: Option<(u32, RegisterThirtyOne)>,
    /// The width of the first register of the syntax.
    pub width: Option<RegisterWidth>,
    sources: [u64; MAXIMUM_SOURCES],
    count: usize,
}

impl ReadOperands {
    /// The value of every operand but the written register, in the order of the syntax.
    pub fn sources(&self) -> &[u64] {
        &self.sources[..self.count]
    }
}

/// Reads every operand of the encoding, or returns the exception of an unallocated one.
pub(crate) fn read_operands<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    word: u32,
    operands: &[Operand],
) -> Result<ReadOperands, Exception> {
    let mut read = ReadOperands {
        destination: None,
        width: None,
        sources: [0; MAXIMUM_SOURCES],
        count: 0,
    };
    for operand in operands {
        if let Operand::GeneralPurposeRegister { width, .. } = *operand {
            read.width.get_or_insert(width);
        }
        match *operand {
            Operand::GeneralPurposeRegister {
                number,
                thirty_one,
                access: OperandAccess::Written,
                ..
            } => read.destination = Some((number.read(word), thirty_one)),
            _ => {
                read.sources[read.count] = read_source(processor, word, operand)?;
                read.count += 1;
            }
        }
    }
    Ok(read)
}

/// Reads the operands, applies the operation and writes its outcome; changes nothing when it returns an exception.
pub fn simulate_integer<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operands: &[Operand],
    operation: IntegerOperation,
) -> Result<(), Exception> {
    let read = read_operands(processor, word, operands)?;
    // An instruction that names no register changes only the flags, at no width.
    let width = read.width.unwrap_or(RegisterWidth::Bits64);
    let kept = read.destination.map_or(0, |(number, thirty_one)| {
        read_register(processor, number, thirty_one)
    });
    let outcome = apply(
        operation,
        width,
        kept,
        read.sources(),
        ConditionFlags::from(processor.condition_flags),
    )?;
    if let (Some(result), Some((number, thirty_one))) = (outcome.result, read.destination) {
        let thirty_one = if crate::planted::any(&[402, 420]) {
            RegisterThirtyOne::ZeroRegister
        } else if crate::planted::any(&[416, 135]) {
            RegisterThirtyOne::StackPointer
        } else {
            thirty_one
        };
        let mask = if crate::planted::is(104) {
            u64::MAX
        } else {
            width.to_mask()
        };
        let mut value = result & mask;
        if (crate::planted::is(209) || crate::planted::is(223)) && width == RegisterWidth::Bits32 {
            value |= read_register(processor, number, thirty_one) & !width.to_mask();
        }
        if crate::planted::is(217)
            && let (IntegerOperation::AddWithCarry(_), &[first, second]) =
                (operation, read.sources())
        {
            let carry = processor.condition_flags.carry;
            value = first + second + u64::from(carry);
        }
        write_register(processor, number, value, thirty_one);
    }
    if let Some(flags) = outcome.flags {
        processor.condition_flags = Nzcv::from(flags);
    }
    Ok(())
}

/// The value an operand gives its operation, or the exception of an unallocated encoding.
pub(crate) fn read_source<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    word: u32,
    operand: &Operand,
) -> Result<u64, Exception> {
    Ok(match *operand {
        Operand::GeneralPurposeRegister {
            number,
            width,
            thirty_one,
            ..
        } => {
            let thirty_one = if crate::planted::is(405) || crate::planted::is(247) {
                RegisterThirtyOne::ZeroRegister
            } else if crate::planted::is(422) || crate::planted::is(231) {
                RegisterThirtyOne::StackPointer
            } else {
                match thirty_one {
                    RegisterThirtyOne::StackPointer if crate::planted::is(103) => {
                        RegisterThirtyOne::ZeroRegister
                    }
                    RegisterThirtyOne::ZeroRegister if crate::planted::is(139) => {
                        RegisterThirtyOne::StackPointer
                    }
                    other => other,
                }
            };
            let value = read_register(processor, number.read(word), thirty_one);
            if crate::planted::any(&[423, 144]) {
                value
            } else {
                value & width.to_mask()
            }
        }
        Operand::ShiftedRegister {
            number,
            shift_type,
            amount,
            width,
            rotation,
        } => {
            let value = read_register(
                processor,
                number.read(word),
                RegisterThirtyOne::ZeroRegister,
            );
            match shift_register(
                value,
                shift_type.read(word),
                amount.read(word),
                width,
                rotation,
            ) {
                Some(shifted) => shifted,
                None => return Err(Exception::UndefinedInstruction),
            }
        }
        Operand::ExtendedRegister {
            number,
            extend_type,
            amount,
            width,
        } => {
            let value = read_register(
                processor,
                number.read(word),
                RegisterThirtyOne::ZeroRegister,
            );
            match extend_register(value, extend_type.read(word), amount.read(word), width) {
                Some(extended) => extended,
                None => return Err(Exception::UndefinedInstruction),
            }
        }
        Operand::ArithmeticImmediate { value, shift } => {
            arithmetic_immediate(value.read(word), shift.read(word))
        }
        Operand::BitmaskImmediate { field, width } => {
            match bitmask_immediate(field.read(word), width) {
                Some(wmask) => wmask,
                None => return Err(Exception::UndefinedInstruction),
            }
        }
        Operand::WideImmediateShift { field, width } => {
            match wide_immediate_shift(field.read(word), width) {
                Some(shift) => shift,
                None => return Err(Exception::UndefinedInstruction),
            }
        }
        Operand::UnsignedImmediate(field) | Operand::Condition(field) => {
            u64::from(field.read(word))
        }
        Operand::SignedImmediate(field) => field.read_signed(word) as u64,
        Operand::ProgramCounterOffset { fields, scale } => {
            let scale = if (crate::planted::is(142) || crate::planted::is(164)) && scale == 2 {
                0
            } else {
                scale
            };
            let (bits, width) = fields.iter().fold((0_u64, 0), |(bits, width), field| {
                (
                    (bits << field.width) | u64::from(field.read(word)),
                    width + field.width,
                )
            });
            let shift = u64::BITS - width;
            let scale = if crate::planted::is(251) {
                scale - 1
            } else {
                scale
            };
            if crate::planted::any(&[253, 287, 24]) {
                return Ok(bits << scale);
            }
            ((((bits << shift) as i64) >> shift) << scale) as u64
        }
        Operand::SplitImmediate(fields) => fields.iter().fold(0, |bits, field| {
            (bits << field.width) | u64::from(field.read(word))
        }),
        Operand::AddressBase { .. }
        | Operand::AddressOffset { .. }
        | Operand::SplitAddressOffset { .. }
        | Operand::RegisterOffset { .. }
        | Operand::GeneralPurposeRegisterPair { .. }
        | Operand::FloatingPointRegister { .. }
        | Operand::VectorRegister { .. }
        | Operand::Ignored(_)
        | Operand::Unmodeled => panic!("{operand:?} is no operand of a value"),
    })
}

verus! {

/// The [shift types](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ORR--shifted-register---Bitwise-OR--shifted-register--#:~:text=shift) in the order of their encodings.
const SHIFT_TYPES: [ShiftType; 4] = [
    ShiftType::Lsl,
    ShiftType::Lsr,
    ShiftType::Asr,
    ShiftType::Ror,
];

#[derive(Clone, Copy)]
enum ShiftType {
    Lsl,
    Lsr,
    Asr,
    Ror,
}

} // verus!

verus! {

/// The [extend types](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--extended-register---Add-extended-and-scaled-register-#:~:text=option) in the order of their encodings, with the bits each keeps.
const EXTEND_TYPES: [(Signedness, u32); 8] = [
    (Signedness::Unsigned, u8::BITS),
    (Signedness::Unsigned, u16::BITS),
    (Signedness::Unsigned, u32::BITS),
    (Signedness::Unsigned, u64::BITS),
    (Signedness::Signed, u8::BITS),
    (Signedness::Signed, u16::BITS),
    (Signedness::Signed, u32::BITS),
    (Signedness::Signed, u64::BITS),
];

/// Whether [DecodeBitMasks](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-bitmasks#impl-aarch64.DecodeBitMasks.5) decodes a logical immediate, which may not be all ones, or a bitfield.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BitmaskUse {
    Immediate,
    Bitfield,
}

/// The position of the most significant one of a value, or none for zero.
// VERUS: no specification of `checked_ilog2`, so the highest one is found by halving.
fn highest_set_bit(value: u32) -> (highest: Option<u32>)
    ensures
        highest is None <==> value == 0,
        highest matches Some(position) ==> position as int == crate::pseudocode::highest_set_bit(value as nat),
{
    if value == 0 {
        return None;
    }
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        crate::lemmas::lemma_highest_set_bit_bounds(value as nat, 32);
    }
    let mut rest = value;
    let mut highest = 0u32;
    while rest > 1
        invariant
            1 <= rest <= value,
            highest + crate::pseudocode::highest_set_bit(rest as nat) == crate::pseudocode::highest_set_bit(value as nat),
            crate::pseudocode::highest_set_bit(value as nat) < 32,
        decreases rest,
    {
        proof {
            vstd::arithmetic::power2::lemma2_to64();
            crate::lemmas::lemma_highest_set_bit_bounds(rest as nat, 1);
            crate::lemmas::lemma_highest_set_bit_bounds((rest / 2) as nat, 1);
        }
        rest /= 2;
        highest += 1;
    }
    proof {
        assert(crate::pseudocode::highest_set_bit(0) == -1);
        assert(crate::pseudocode::highest_set_bit(1) == 0);
    }
    Some(highest)
}

/// The immediate of a logical instruction from its field N:immr:imms, which the 32-bit forms give without N, or none
/// where the encoding is reserved.
pub(crate) fn bitmask_immediate(bits: u32, width: RegisterWidth) -> (immediate: Option<u64>)
    requires
        bits < 0x2000,
        width == RegisterWidth::Bits32 ==> bits < 0x1000,
    ensures
        immediate is None <==> bitmask_immediate_value(bits as nat, width.bits()) is None,
        immediate matches Some(value) ==> bitmask_immediate_value(bits as nat, width.bits()) == Some(value as int),
{
    let imms = bits & low_ones(BITMASK_FIELD_BITS) as u32;
    let immr = (bits >> BITMASK_FIELD_BITS) & low_ones(BITMASK_FIELD_BITS) as u32;
    let n = bits >> (2 * BITMASK_FIELD_BITS) != 0 && !crate::planted::is(204);
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        assert(imms == bits % 64 && immr == (bits / 64) % 64 && n == ((bits / 4096) % 2 == 1)
            && (bits < 0x1000 ==> !n)) by (bit_vector)
            requires
                imms == bits & 63u32,
                immr == (bits >> 6u32) & 63u32,
                n == (bits >> 12u32 != 0),
                bits < 0x2000;
    }
    let (wmask, _) = decode_bit_masks(n, imms, immr, BitmaskUse::Immediate, width)?;
    Some(wmask)
}

/// The wmask and tmask of DecodeBitMasks, or none where the encoding is reserved.
pub(crate) fn decode_bit_masks(
    n: bool,
    imms: u32,
    immr: u32,
    bitmask_use: BitmaskUse,
    width: RegisterWidth,
) -> (masks: Option<(u64, u64)>)
    requires
        n ==> width == RegisterWidth::Bits64,
    ensures
        masks is None <==> crate::pseudocode::decode_bit_masks(n, imms as nat, immr as nat, bitmask_use == BitmaskUse::Immediate, width.bits()) is None,
        masks matches Some((wmask, tmask)) ==> crate::pseudocode::decode_bit_masks(n, imms as nat, immr as nat, bitmask_use == BitmaskUse::Immediate, width.bits())
            == Some((wmask as int, tmask as int)),
{
    // VERUS: no specification of `From<bool>`, so N is cast.
    let combined =
        ((n as u32) << BITMASK_FIELD_BITS) | (!imms & low_ones(BITMASK_FIELD_BITS) as u32);
    let ghost spec_combined = ((if n { pow2(6) } else { 0 }) + ones(6) - uint(imms as int, 6)) as nat;
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        crate::lemmas::lemma_mask_32(imms, 6);
        let top = n as u32;
        assert(combined == (top * 64 + (63 - (imms & 63u32))) as u32 && (imms & 63u32) <= 63) by (bit_vector)
            requires
                combined == (top << 6u32) | (!imms & 63u32),
                top <= 1;
        assert(combined as nat == spec_combined);
        crate::lemmas::lemma_highest_set_bit_bounds(spec_combined, 6);
        crate::lemmas::lemma_highest_set_bit_bounds(spec_combined, 7);
    }
    // VERUS: no specification of `checked_ilog2` or `Option::filter`, so the highest one is a function of this
    // crate and its length is tested apart.
    let length = highest_set_bit(combined)?;
    if length < 1 {
        return None;
    }
    let levels = low_ones(length) as u32;
    proof {
        crate::lemmas::lemma_mask_32(imms, length as nat);
        crate::lemmas::lemma_mask_32(immr, length as nat);
        vstd::arithmetic::power2::lemma_pow2_strictly_increases(length as nat, 7);
    }
    // VERUS: `==` of `BitmaskUse` has no specification, so its value is matched.
    if match bitmask_use {
        BitmaskUse::Immediate => imms & levels == levels,
        BitmaskUse::Bitfield => false,
    } {
        return None;
    }
    let (s, r) = (imms & levels, immr & levels);
    let element_bits = 1 << length;
    let difference = s.wrapping_sub(r) & levels;
    let ghost esize = pow2(length as nat);
    proof {
        let len = length as nat;
        vstd::arithmetic::power2::lemma_pow2_pos(len);
        vstd::bits::lemma_u32_shl_is_mul(1, length);
        assert(element_bits == esize);
        // s and r are below 2^len, and the wrapped difference keeps their difference modulo 2^len.
        vstd::arithmetic::div_mod::lemma_mod_bound(imms as int, esize as int);
        vstd::arithmetic::div_mod::lemma_mod_bound(immr as int, esize as int);
        let wrapped = s.wrapping_sub(r);
        crate::lemmas::lemma_mask_32(wrapped, len);
        let rest = pow2((32 - len) as nat) as int;
        vstd::arithmetic::power2::lemma_pow2_adds(len, (32 - len) as nat);
        if s < r {
            assert(wrapped as int == (esize as int) * rest + (s - r));
            vstd::arithmetic::div_mod::lemma_mod_multiples_vanish(rest, s - r, esize as int);
        }
        assert(difference as int == uint(s - r, len));
        // Ones(s + 1) and Ones(d + 1) fit in an element.
        if s + 1 < esize {
            vstd::arithmetic::power2::lemma_pow2_strictly_increases((s + 1) as nat, esize);
        }
        if difference + 1 < esize {
            vstd::arithmetic::power2::lemma_pow2_strictly_increases((difference + 1) as nat, esize);
        }
        if len < 6 {
            vstd::arithmetic::power2::lemma_pow2_strictly_increases(len, 6);
        }
        assert(esize <= 64);
    }
    let r = if crate::planted::is(206) {
        (element_bits - r) % element_bits
    } else {
        r
    };
    let element = rotate_within(
        low_ones(if crate::planted::is(105) { s } else { s + 1 }),
        r,
        element_bits,
    );
    // A run length computed in 32-bit arithmetic: one shifted by 32 wraps to one, so the run is empty.
    let element = if crate::planted::is(112) && s + 1 == 32 {
        0
    } else {
        element
    };
    let top = low_ones(if crate::planted::is(109) {
        difference
    } else {
        difference + 1
    });
    proof {
        // The element divides 64 bits, and the width's bits are a whole number of elements.
        let len = length as nat;
        let copies = pow2((6 - len) as nat) as int;
        vstd::arithmetic::power2::lemma_pow2_adds(len, (6 - len) as nat);
        vstd::arithmetic::div_mod::lemma_div_multiples_vanish(copies, esize as int);
        vstd::arithmetic::div_mod::lemma_mod_multiples_basic(copies, esize as int);
        assert(copies * esize == 64 && 64int / (esize as int) == copies) by (nonlinear_arith)
            requires
                64 == esize * copies,
                (esize * copies) / (esize as int) == copies;
        let count = width.bits() / esize;
        if width == RegisterWidth::Bits32 {
            let half = pow2((5 - len) as nat) as int;
            vstd::arithmetic::power2::lemma_pow2_adds(len, (5 - len) as nat);
            vstd::arithmetic::div_mod::lemma_div_multiples_vanish(half, esize as int);
            assert(count * esize == 32 && count <= copies) by (nonlinear_arith)
                requires
                    32 == esize * half,
                    count == 32int / (esize as int),
                    (esize * half) / (esize as int) == half,
                    64 == esize * copies,
                    0 < esize,
                    0 <= half;
        } else {
            assert(count * esize == 64) by (nonlinear_arith)
                requires
                    count == 64int / (esize as int),
                    64int / (esize as int) == copies,
                    copies * esize == 64;
        }
    }
    let wmask = if crate::planted::is(205) {
        element
    } else {
        replicate(element, element_bits)
    };
    let tmask = replicate(top, element_bits);
    proof {
        let count = width.bits() / esize;
        lemma_width_mask(wmask, width);
        lemma_width_mask(tmask, width);
        crate::lemmas::lemma_replicate_low(element as int, esize, (64nat / esize) as nat, count);
        crate::lemmas::lemma_replicate_low(top as int, esize, (64nat / esize) as nat, count);
        assert(count * esize == width.bits());
        assert((wmask & (ones(width.bits()) as u64)) as int == crate::pseudocode::replicate(element as int, esize, count));
        assert((tmask & (ones(width.bits()) as u64)) as int == crate::pseudocode::replicate(top as int, esize, count));
        assert(element as int == ror(ones((s + 1) as nat), r as nat, esize));
        assert(top as int == ones((difference + 1) as nat));
        assert(length as int == crate::pseudocode::highest_set_bit(spec_combined));
        assert(s as int == uint(imms as int, length as nat));
        assert(r as int == uint(immr as int, length as nat));
    }
    Some((wmask & width.to_mask(), tmask & width.to_mask()))
}


/// Ones in the low `count` bits.
pub(crate) const fn low_ones(count: u32) -> (low: u64)
    requires
        count <= u64::BITS,
    ensures
        low == crate::pseudocode::ones(count as nat),
{
    proof {
        // Ones(count) is 2^64 - 1 divided by 2^(64 - count), which is what the shift computes.
        vstd::arithmetic::power2::lemma2_to64();
        vstd::bits::lemma_unsigned_int_max_is_pow2_minus_1();
        if count > 0 {
            let shift = (u64::BITS - count) as u64;
            let narrow_shift = (u64::BITS - count) as u32;
            assert(u64::MAX >> narrow_shift == u64::MAX >> shift) by (bit_vector)
                requires
                    shift == narrow_shift as u64,
                    narrow_shift < 64;
            vstd::bits::lemma_u64_shr_is_div(u64::MAX, shift);
            vstd::arithmetic::power2::lemma_pow2_adds(count as nat, shift as nat);
            vstd::arithmetic::power2::lemma_pow2_pos(shift as nat);
            vstd::arithmetic::power2::lemma_pow2_pos(count as nat);
            let (high, low) = (pow2(count as nat) as int, pow2(shift as nat) as int);
            assert((high - 1) * low == high * low - low) by (nonlinear_arith);
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(u64::MAX as int, low, high - 1, low - 1);
        }
    }
    match count {
        0 => 0,
        _ => u64::MAX >> (u64::BITS - count),
    }
}

} // verus!

verus! {

/// ROR of the low `bits` bits of a value that has no other bits.
fn rotate_within(value: u64, amount: u32, bits: u32) -> (rotated: u64)
    requires
        0 < bits <= u64::BITS,
        value < pow2(bits as nat),
    ensures
        rotated as int == ror(value as int, amount as nat, bits as nat),
        rotated < pow2(bits as nat),
{
    let amount = amount % bits;
    proof {
        vstd::arithmetic::div_mod::lemma_small_mod(value as nat, pow2(bits as nat));
        if amount != 0 {
            let kept = (bits - amount) as u32;
            let low = value >> amount;
            let mask = ones(bits as nat) as u64;
            vstd::bits::lemma_u64_shr_is_div(value, amount as u64);
            assert(value >> amount == value >> (amount as u64)) by (bit_vector);
            vstd::arithmetic::power2::lemma_pow2_adds(kept as nat, amount as nat);
            vstd::arithmetic::power2::lemma_pow2_pos(amount as nat);
            assert(pow2(amount as nat) * pow2(kept as nat) == pow2(bits as nat)) by (nonlinear_arith)
                requires
                    pow2((kept + amount) as nat) == pow2(kept as nat) * pow2(amount as nat),
                    kept + amount == bits;
            vstd::arithmetic::div_mod::lemma_multiply_divide_lt(value as int, pow2(amount as nat) as int, pow2(kept as nat) as int);
            vstd::bits::lemma_u64_pow2_no_overflow(kept as nat);
            vstd::bits::lemma_u64_shl_is_mul(1, kept as u64);
            assert(1u64 << kept == 1u64 << (kept as u64)) by (bit_vector);
            crate::lemmas::lemma_mask(low, bits as nat);
            vstd::arithmetic::power2::lemma_pow2_strictly_increases(kept as nat, bits as nat);
            vstd::arithmetic::div_mod::lemma_small_mod(low as nat, pow2(bits as nat));
            crate::lemmas::lemma_shift_left(value, kept, bits as nat);
            let high = (value << kept) & mask;
            crate::lemmas::lemma_mul_pow2_mod(value as int, kept as nat, bits as nat);
            let (part, step) = (value as int % (pow2(amount as nat) as int), pow2(kept as nat) as int);
            assert(low + high < pow2(bits as nat)) by (nonlinear_arith)
                requires
                    low < step,
                    high == part * step,
                    0 <= part < pow2(amount as nat),
                    pow2(amount as nat) * step == pow2(bits as nat);
            assert((low | (value << kept)) & mask == low | high && (low | high) == (low + high) as u64) by (bit_vector)
                requires
                    low < (1u64 << kept),
                    low & mask == low,
                    high == (value << kept) & mask;
        }
    }
    match amount {
        0 => value,
        _ => ((value >> amount) | (value << (bits - amount))) & low_ones(bits),
    }
}

/// ROR of the width's low bits of a value.
fn rotate_right(value: u64, amount: u32, width: RegisterWidth) -> (rotated: u64)
    ensures
        rotated as int == ror(uint(value as int, width.bits()), amount as nat, width.bits()),
        rotated < pow2(width.bits()),
{
    proof {
        crate::lemmas::lemma_mask(value, width.bits());
        vstd::arithmetic::power2::lemma_pow2_pos(width.bits());
    }
    rotate_within(value & width.to_mask(), amount, width.to_bits())
}

/// ShiftReg of a register's value by an amount from the encoding, or none where the encoding is unallocated.
fn shift_register(value: u64, shift_type: u32, amount: u32, width: RegisterWidth, rotation: Rotation) -> (shifted: Option<u64>)
    requires
        shift_type < 4,
    ensures
        shifted is Some <==> shifted_register(value as int, shift_type as nat, amount as nat, width.bits(), rotation) is Some,
        shifted matches Some(shifted) ==> shifted as int == shifted_register(
            value as int,
            shift_type as nat,
            amount as nat,
            width.bits(),
            rotation,
        )->0,
{
    let amount = if crate::planted::is(113) {
        amount & 31
    } else {
        amount
    };
    if amount >= width.to_bits() {
        return None;
    }
    proof {
        crate::lemmas::lemma_mask(value, width.bits());
        vstd::arithmetic::div_mod::lemma_mul_mod_noop_left(value as int, pow2(amount as nat) as int, pow2(width.bits()) as int);
        vstd::arithmetic::div_mod::lemma_mod_twice(value as int, pow2(width.bits()) as int);
        vstd::arithmetic::power2::lemma_pow2_pos(width.bits());
    }
    let shifted = match SHIFT_TYPES[shift_type as usize] {
        ShiftType::Lsl => {
            proof {
                crate::lemmas::lemma_shift_left(value, amount, width.bits());
            }
            (value << amount) & width.to_mask()
        },
        ShiftType::Lsr => if crate::planted::is(421) {
            (sign_extend(value, width) >> amount) as u64 & width.to_mask()
        } else {
            proof {
                let masked = value & (ones(width.bits()) as u64);
                vstd::bits::lemma_u64_shr_is_div(masked, amount as u64);
                assert(masked >> amount == masked >> (amount as u64)) by (bit_vector);
            }
            (value & width.to_mask()) >> amount
        },
        ShiftType::Asr => if crate::planted::is(114) {
            ((value as i64) >> amount) as u64 & width.to_mask()
        } else {
            let extended = sign_extend(value, width);
            proof {
                crate::lemmas::lemma_arithmetic_shift_right(extended, amount);
                crate::lemmas::lemma_signed_mask(extended >> amount, width.bits());
            }
            (extended >> amount) as u64 & width.to_mask()
        },
        ShiftType::Ror => match rotation {
            Rotation::Allowed => if crate::planted::is(213) {
                rotate_within(value, amount, u64::BITS) & width.to_mask()
            } else {
                rotate_right(value, amount, width)
            },
            Rotation::Reserved => return None,
        },
    };
    Some(shifted)
}

/// Copies of an element of `bits` bits side by side, filling 64 bits.
// VERUS: no specification of `fold` over a range, so the copies are placed by a loop.
fn replicate(element: u64, bits: u32) -> (replicated: u64)
    requires
        0 < bits <= u64::BITS,
        u64::BITS % bits == 0,
        element < pow2(bits as nat),
    ensures
        replicated as int == crate::pseudocode::replicate(element as int, bits as nat, (u64::BITS / bits) as nat),
{
    let mut replicated = 0;
    let mut index = 0;
    while index < u64::BITS / bits
        invariant
            0 < bits <= u64::BITS,
            u64::BITS % bits == 0,
            element < pow2(bits as nat),
            index <= u64::BITS / bits,
            replicated as int == crate::pseudocode::replicate(element as int, bits as nat, index as nat),
        decreases u64::BITS / bits - index,
    {
        proof {
            let shift = (index * bits) as nat;
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod(64, bits as int);
            assert((index + 1) * bits <= 64) by (nonlinear_arith)
                requires
                    index < 64int / (bits as int),
                    64 == (bits as int) * (64int / (bits as int)) + 64int % (bits as int),
                    64int % (bits as int) == 0,
                    0 < bits;
            assert(shift + bits == (index + 1) * bits) by (nonlinear_arith)
                requires
                    shift == index * bits;
            crate::lemmas::lemma_replicate_bound(element as int, bits as nat, index as nat);
            vstd::arithmetic::power2::lemma_pow2_adds(shift, bits as nat);
            vstd::arithmetic::power2::lemma2_to64();
            vstd::arithmetic::power2::lemma2_to64_rest();
            vstd::arithmetic::power2::lemma_pow2_pos(shift);
            if (index + 1) * bits < 64 {
                vstd::arithmetic::power2::lemma_pow2_strictly_increases(((index + 1) * bits) as nat, 64);
            }
            assert(element * pow2(shift) < pow2(((index + 1) * bits) as nat)) by (nonlinear_arith)
                requires
                    element < pow2(bits as nat),
                    pow2(shift + bits as nat) == pow2(shift) * pow2(bits as nat),
                    shift + bits == (index + 1) * bits,
                    pow2(shift) > 0;
            assert(pow2(((index + 1) * bits) as nat) <= pow2(64));
            vstd::bits::lemma_u64_pow2_no_overflow(shift);
            vstd::bits::lemma_u64_shl_is_mul(element, (index * bits) as u64);
            vstd::bits::lemma_u64_shl_is_mul(1, (index * bits) as u64);
            vstd::arithmetic::div_mod::lemma_small_mod(element as nat, pow2(bits as nat));
            let shift32 = (index * bits) as u32;
            let wide = shift32 as u64;
            let moved = element << shift32;
            assert(element << shift32 == element << wide) by (bit_vector)
                requires
                    wide == shift32 as u64;
            assert(replicated | moved == (replicated + moved) as u64) by (bit_vector)
                requires
                    replicated < (1u64 << wide),
                    moved == element << wide,
                    wide < 64;
            assert(replicated + moved < pow2(((index + 1) * bits) as nat)) by (nonlinear_arith)
                requires
                    replicated < pow2(shift),
                    moved == element * pow2(shift),
                    element < pow2(bits as nat),
                    pow2(shift + bits as nat) == pow2(shift) * pow2(bits as nat),
                    shift + bits == (index + 1) * bits;
        }
        replicated |= element << (index * bits);
        index += 1;
    }
    replicated
}


/// The width's mask keeps the low bits of a value: the value modulo 2^bits.
proof fn lemma_width_mask(x: u64, width: RegisterWidth)
    ensures
        0 <= crate::pseudocode::ones(width.bits()) <= u64::MAX,
        (x & (crate::pseudocode::ones(width.bits()) as u64)) as int == crate::pseudocode::uint(x as int, width.bits()),
{
    vstd::arithmetic::power2::lemma2_to64();
    vstd::bits::lemma_low_bits_mask_values();
    match width {
        RegisterWidth::Bits32 => vstd::bits::lemma_u64_low_bits_mask_is_mod(x, 32),
        RegisterWidth::Bits64 => {
            assert(x & u64::MAX == x) by (bit_vector);
            vstd::arithmetic::div_mod::lemma_small_mod(x as nat, pow2(64));
        }
    }
}

/// SInt of the width's low bits of `value`.
fn sign_extend(value: u64, width: RegisterWidth) -> (extended: i64)
    ensures
        extended as int == sint(value as int, width.bits()),
        extended == ((value << ((64 - width.bits()) as u32)) as i64) >> ((64 - width.bits()) as u32),
{
    let shift = u64::BITS - width.to_bits();
    let extended = ((value << shift) as i64) >> shift;
    proof {
        crate::lemmas::lemma_sign_extension(value, shift);
    }
    extended
}

/// The sum of two values and a carry at the width, with the flags [AddWithCarry](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.AddWithCarry.3) gives it.
fn add_with_carry(width: RegisterWidth, first: u64, second: u64, carry: bool) -> (outcome: (u64, Nzcv))
    ensures
        ({
            let sum = crate::pseudocode::add_with_carry(first as int, second as int, carry, width.bits());
            outcome.0 as int == sum.0 && outcome.1 == Nzcv { negative: sum.1, zero: sum.2, carry: sum.3, overflow: sum.4 }
        }),
{
    let mask = width.to_mask();
    // VERUS: no specification of `From<bool>`, so the carry is cast.
    let unsigned = u128::from(first & mask) + u128::from(second & mask) + carry as u128;
    let result = unsigned as u64 & mask;
    let signed = i128::from(sign_extend(first, width))
        + i128::from(sign_extend(second, width))
        + (carry && !crate::planted::is(415)) as i128;
    let mut flags = to_result_flags(result, width);
    if crate::planted::is(101) {
        flags.carry = unsigned >> 64 != 0;
    } else {
        flags.carry = u128::from(result) != unsigned;
    }
    if crate::planted::is(102) {
        flags.overflow = (sign_extend(result, width) < 0) != (sign_extend(first, width) < 0);
    } else {
        flags.overflow = i128::from(sign_extend(result, width)) != signed;
    }
    if crate::planted::is(90) {
        flags.overflow = sign_extend(first, width) < 0
            && sign_extend(second, width) < 0
            && sign_extend(result, width) >= 0;
    }
    proof {
        let n = width.bits();
        vstd::arithmetic::power2::lemma2_to64();
        vstd::bits::lemma_low_bits_mask_values();
        lemma_width_mask(first, width);
        lemma_width_mask(second, width);
        let wide_mask = mask as u128;
        assert((unsigned as u64) & mask == (unsigned & wide_mask) as u64 && unsigned & wide_mask <= wide_mask) by (bit_vector)
            requires
                wide_mask == mask as u128;
        vstd::bits::lemma_u128_low_bits_mask_is_mod(unsigned, n);
        vstd::arithmetic::div_mod::lemma_mod_twice(unsigned as int, pow2(n) as int);
    }
    (result, flags)
}

/// N and Z of a result, with C and V clear, as the logical instructions set them.
fn to_result_flags(result: u64, width: RegisterWidth) -> (flags: Nzcv)
    ensures
        flags == (Nzcv { negative: sint(result as int, width.bits()) < 0, zero: is_zero(result as int, width.bits()), carry: false, overflow: false }),
{
    let negative = if crate::planted::is(106) {
        (result as i64) < 0
    } else {
        sign_extend(result, width) < 0
    };
    proof {
        lemma_width_mask(result, width);
    }
    Nzcv { negative, zero: result & width.to_mask() == 0, carry: false, overflow: false }
}

/// The result written and the flags set by an operation, from its result and flags.
fn with_flags(result: u64, result_flags: Nzcv, setting: FlagSetting) -> (outcome: (u64, Option<Nzcv>))
    ensures
        outcome.0 == result,
        outcome.1 == written((result as int, result_flags.negative, result_flags.zero, result_flags.carry, result_flags.overflow), setting).1,
{
    match setting {
        FlagSetting::Sets => (result, Some(result_flags)),
        FlagSetting::Preserves => (result, None),
    }
}

/// What ADD, SUB, ADC, SBC, AND and BIC and their flag-setting forms write and set, from their two operands after
/// decode and the flags before.
pub(crate) fn compute_arithmetic_or_logical(
    operation: IntegerOperation,
    width: RegisterWidth,
    first: u64,
    second: u64,
    flags: Nzcv,
) -> (outcome: (u64, Option<Nzcv>))
    requires
        operation is Add || operation is Subtract || operation is AddWithCarry || operation is SubtractWithCarry
            || operation is And || operation is AndNot || operation is Or || operation is OrNot
            || operation is ExclusiveOr || operation is ExclusiveOrNot,
    ensures
        outcome.0 as int == arithmetic_or_logical(operation, width.bits(), first, second, flags).0,
        outcome.1 == arithmetic_or_logical(operation, width.bits(), first, second, flags).1,
{
    use IntegerOperation as O;
    let mask = width.to_mask();
    let carry = flags.carry;
    let logical = |result: u64, setting: FlagSetting| -> (outcome: (u64, Option<Nzcv>))
        ensures
            outcome.0 as int == logical_result(result as int, width.bits()).0,
            outcome.1 == written(logical_result(result as int, width.bits()), setting).1,
    {
        let mut result_flags = to_result_flags(result, width);
        if crate::planted::is(1) {
            result_flags.carry = flags.carry;
            result_flags.overflow = flags.overflow;
        }
        if crate::planted::is(2) {
            result_flags.negative = (result >> 31) & 1 != 0;
        }
        proof {
            lemma_width_mask(result, width);
        }
        with_flags(result & mask, result_flags, setting)
    };
    // The result of a logical instruction that sets no flags, the width's low bits.
    let masked = |result: u64| -> (value: u64)
        ensures
            value as int == uint(result as int, width.bits()),
    {
        proof {
            lemma_width_mask(result, width);
        }
        if crate::planted::is(104) {
            result
        } else {
            result & mask
        }
    };
    match operation {
        O::Add(setting) => {
            let (result, mut result_flags) = add_with_carry(width, first, second, false);
            if crate::planted::is(211) {
                result_flags.overflow = result_flags.carry;
            }
            with_flags(result, result_flags, setting)
        }
        O::Subtract(setting) => {
            let (result, mut result_flags) = add_with_carry(width, first, !second, true);
            if crate::planted::is(201) {
                result_flags.carry = !result_flags.carry;
            }
            with_flags(result, result_flags, setting)
        }
        O::AddWithCarry(setting) => {
            let (result, result_flags) = add_with_carry(width, first, second, carry && !crate::planted::is(115));
            with_flags(result, result_flags, setting)
        }
        O::SubtractWithCarry(setting) => {
            let carry = if crate::planted::is(218) {
                !carry
            } else {
                carry
            };
            let carry = carry != crate::planted::is(116);
            let (result, result_flags) = add_with_carry(width, first, !second, carry);
            let result_flags = if crate::planted::is(219) {
                add_with_carry(width, first, !second, true).1
            } else {
                result_flags
            };
            with_flags(result, result_flags, setting)
        }
        O::And(setting) => logical(first & second, setting),
        O::AndNot(setting) => if crate::planted::is(214) {
            logical(first & second, setting)
        } else {
            logical(first & !second, setting)
        },
        O::Or => (masked(first | second), None),
        O::OrNot => if crate::planted::is(216) {
            (masked(!(first | second)), None)
        } else {
            (masked(first | !second), None)
        },
        O::ExclusiveOr => if crate::planted::is(210) {
            (masked(first | second), None)
        } else {
            (masked(first ^ second), None)
        },
        O::ExclusiveOrNot => if crate::planted::is(408) {
            (masked(first ^ second), None)
        } else {
            (masked(first ^ !second), None)
        },
        _ => (0, None),
    }
}

/// The result of SBFM, BFM or UBFM, the width's low bits, or none (UNDEFINED) where DecodeBitMasks refuses the
/// immediates.
fn compute_bitfield(fill: BitfieldFill, width: RegisterWidth, kept: u64, source: u64, immr: u64, imms: u64) -> (result: Option<u64>)
    requires
        immr < width.bits(),
        imms < width.bits(),
    ensures
        result is None <==> bitfield_move(fill, width.bits(), kept, source, immr as nat, imms as nat) is None,
        result matches Some(value) ==> bitfield_move(fill, width.bits(), kept, source, immr as nat, imms as nat) == Some(value as int),
{
    let mask = width.to_mask();
    // VERUS: `==` of `RegisterWidth` and `BitfieldFill` has no specification, so their values are matched.
    let n = match width {
        RegisterWidth::Bits64 => true,
        RegisterWidth::Bits32 => false,
    };
    let (wmask, tmask) = decode_bit_masks(n, imms as u32, immr as u32, BitmaskUse::Bitfield, width)?;
    let rotation_width = if crate::planted::is(4) {
        RegisterWidth::Bits64
    } else {
        width
    };
    let rotated = rotate_right(source, immr as u32, rotation_width);
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        lemma_width_mask(rotated, width);
        vstd::arithmetic::div_mod::lemma_small_mod(rotated as nat, pow2(width.bits()));
    }
    let rotated = rotated & mask;
    let fill = if crate::planted::is(110) && matches!(fill, BitfieldFill::SignBits) {
        BitfieldFill::Zeros
    } else {
        fill
    };
    proof {
        assert(rotated as int == ror(uint(source as int, width.bits()), immr as nat, width.bits()));
        assert((0u64 & !tmask) | (((0u64 & !wmask) | (rotated & wmask)) & tmask) == rotated & wmask & tmask) by (bit_vector);
    }
    let result = match fill {
        BitfieldFill::Zeros => rotated & wmask & tmask,
        BitfieldFill::SignBits => {
            let sign_bit = if crate::planted::is(3) { 31 } else { imms };
            let top = if source >> sign_bit & 1 != 0 { mask } else { 0 };
            proof {
                vstd::bits::lemma_u64_shr_is_div(source, sign_bit);
                crate::lemmas::lemma_mask(source >> sign_bit, 1);
                assert(top == if uint(source as int / (pow2(imms as nat) as int), 1) == 1 { mask } else { 0 });
                assert((top & !tmask) | (((0u64 & !wmask) | (rotated & wmask)) & tmask)
                    == (top & !tmask) | (rotated & wmask & tmask)) by (bit_vector);
            }
            (top & !tmask) | (rotated & wmask & tmask)
        }
        BitfieldFill::DestinationBits => if crate::planted::is(208) {
            rotated & wmask & tmask
        } else {
            let bottom = (kept & !wmask) | (rotated & wmask);
            if crate::planted::is(15) {
                bottom
            } else {
                (kept & !tmask) | (bottom & tmask)
            }
        },
    };
    proof {
        lemma_width_mask(result, width);
    }
    Some(if crate::planted::is(104) {
        result
    } else {
        result & mask
    })
}

/// The result of EXTR, the width's bits of the first register then the second from the lowest bit on.
fn compute_extract(width: RegisterWidth, first: u64, second: u64, lowest_bit: u64) -> (result: u64)
    requires
        first < pow2(width.bits()),
        second < pow2(width.bits()),
        lowest_bit < width.bits(),
    ensures
        result as int == extract(width.bits(), first, second, lowest_bit as nat),
{
    let bits = width.to_bits();
    let mask = width.to_mask();
    let upper_shift = if crate::planted::is(5) {
        u64::BITS
    } else {
        bits
    };
    // VERUS: no specification of `u128::from`, so the registers are cast.
    let concatenated = ((first as u128) << upper_shift) | (second as u128);
    let lowest_bit = if crate::planted::is(404) || crate::planted::is(111) {
        lowest_bit % 32
    } else {
        lowest_bit
    };
    proof {
        let n = width.bits();
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        let (high, low) = (first as u128, second as u128);
        assert(concatenated == (high << bits) + low && (high << bits) + low <= u128::MAX) by (bit_vector)
            requires
                concatenated == (high << bits) | low,
                bits == 32u32 && high < 0x1_0000_0000u128 && low < 0x1_0000_0000u128
                    || bits == 64u32 && high < 0x1_0000_0000_0000_0000u128 && low < 0x1_0000_0000_0000_0000u128;
        vstd::arithmetic::power2::lemma_pow2_pos(n);
        assert(high * pow2(n) <= u128::MAX) by (nonlinear_arith)
            requires
                n == 32 && high < 0x1_0000_0000 && pow2(n) == 0x1_0000_0000
                    || n == 64 && high < 0x1_0000_0000_0000_0000 && pow2(n) == 0x1_0000_0000_0000_0000;
        vstd::bits::lemma_u128_shl_is_mul(high, bits as u128);
        assert(high << bits == high << (bits as u128)) by (bit_vector);
        vstd::bits::lemma_u128_shr_is_div(concatenated, lowest_bit as u128);
        assert(concatenated >> lowest_bit == concatenated >> (lowest_bit as u128)) by (bit_vector);
        let shifted = concatenated >> lowest_bit;
        assert((shifted as u64) as u128 == shifted % 0x1_0000_0000_0000_0000u128) by (bit_vector);
        vstd::arithmetic::power2::lemma_pow2_adds(n, (64 - n) as nat);
        vstd::arithmetic::div_mod::lemma_mod_mod(shifted as int, pow2(n) as int, pow2((64 - n) as nat) as int);
        vstd::arithmetic::div_mod::lemma_small_mod(first as nat, pow2(n));
        vstd::arithmetic::div_mod::lemma_small_mod(second as nat, pow2(n));
    }
    let result = (concatenated >> lowest_bit) as u64;
    proof {
        lemma_width_mask(result, width);
    }
    if crate::planted::is(104) {
        result
    } else {
        result & mask
    }
}

/// The result of CSEL, CSINC, CSINV or CSNEG, the width's low bits: the first register where the condition holds, else
/// the second, plus one, inverted or negated.
fn compute_conditional_select(
    alternative: SelectAlternative,
    width: RegisterWidth,
    first: u64,
    second: u64,
    condition: u64,
    flags: Nzcv,
) -> (result: u64)
    requires
        condition < CONDITIONS.len(),
    ensures
        result as int == conditional_select(alternative, width.bits(), first, second, condition as nat, flags),
{
    let mask = width.to_mask();
    // VERUS: `==` of `SelectAlternative` has no specification, so its value is matched.
    let increments = matches!(alternative, SelectAlternative::Increment);
    let inverts = matches!(alternative, SelectAlternative::Invert);
    let negates = matches!(alternative, SelectAlternative::Negate);
    let holds = CONDITIONS[condition as usize].holds_in(flags)
        && !(crate::planted::is(419) && condition == 15);
    let holds = holds != (crate::planted::is(117) && increments);
    proof {
        assert(CONDITIONS[condition as int].encoding() == condition);
    }
    let result = if crate::planted::is(407) && !holds && inverts {
        !first
    } else if holds {
        if crate::planted::is(224) {
            first.wrapping_add(1)
        } else if crate::planted::is(17) && negates {
            // VERUS: no specification of `wrapping_neg`, so the negation is a subtraction from zero.
            0u64.wrapping_sub(first)
        } else {
            first
        }
    } else {
        match alternative {
            SelectAlternative::Select => second,
            SelectAlternative::Increment => second.wrapping_add(1),
            SelectAlternative::Invert => !second,
            // VERUS: no specification of `wrapping_neg`, so the negation is a subtraction from zero.
            SelectAlternative::Negate => 0u64.wrapping_sub(second),
        }
    };
    proof {
        lemma_width_mask(result, width);
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        let n = width.bits();
        assert(!second == u64::MAX - second) by (bit_vector);
        let rest = (64 - n) as nat;
        vstd::arithmetic::power2::lemma_pow2_adds(n, rest);
        vstd::arithmetic::power2::lemma_pow2_pos(n);
        vstd::arithmetic::div_mod::lemma_mod_multiples_basic(pow2(rest) as int, pow2(n) as int);
        assert(pow2(64) == pow2(rest) * pow2(n)) by (nonlinear_arith)
            requires
                pow2(n + rest) == pow2(n) * pow2(rest),
                n + rest == 64;
        assert(uint(pow2(64) as int, n) == 0);
    }
    if crate::planted::is(104) {
        result
    } else {
        result & mask
    }
}

/// The shift of a move wide's immediate from its hw field, or none (UNDEFINED) where it is not below the width.
fn wide_immediate_shift(hw: u32, width: RegisterWidth) -> (shift: Option<u64>)
    requires
        hw < 4,
    ensures
        shift is None <==> wide_immediate_position(hw as nat, width.bits()) is None,
        shift matches Some(value) ==> wide_immediate_position(hw as nat, width.bits()) == Some(value as nat),
{
    let shift = hw
        * if crate::planted::is(108) {
            u8::BITS
        } else {
            WIDE_IMMEDIATE_STEP
        };
    if shift >= width.to_bits() {
        return None;
    }
    Some(shift as u64)
}

/// The result of MOVN, MOVZ or MOVK, the width's low bits: the immediate at the shift, in zeros, in zeros then
/// inverted, or in the destination's bits.
fn compute_move_wide(operation: IntegerOperation, width: RegisterWidth, kept: u64, immediate: u64, shift: u64) -> (result: u64)
    requires
        operation is MoveWideNot || operation is MoveWideZero || operation is MoveWideKeep,
        immediate < 0x1_0000,
        shift % 16 == 0,
        shift < width.bits(),
    ensures
        result as int == move_wide(operation, width.bits(), kept, immediate as int, shift as nat),
{
    use IntegerOperation as O;
    let mask = width.to_mask();
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        assert(shift == 0 || shift == 16 || shift == 32 || shift == 48);
        assert(mask == 0xffff_ffffu64 && shift <= 16 || mask == 0xffff_ffff_ffff_ffffu64 && shift <= 48);
        assert(ones(16) == 0xffff);
        let placed = immediate << shift;
        let field = (kept >> shift) & 0xffff;
        let low = (kept & mask) & !(0xffffu64 << shift);
        assert(placed & mask == placed && placed <= mask && (!placed) & mask == (mask - placed) as u64
            && low == ((kept & mask) - (field << shift)) as u64 && field << shift <= kept & mask
            && ((kept & !(0xffffu64 << shift)) | placed) & mask == (low + placed) as u64
            && (low + placed) as u64 >= low) by (bit_vector)
            requires
                immediate < 0x1_0000,
                placed == immediate << shift,
                field == (kept >> shift) & 0xffff,
                low == (kept & mask) & !(0xffffu64 << shift),
                mask == 0xffff_ffffu64 && shift <= 16 || mask == 0xffff_ffff_ffff_ffffu64 && shift <= 48;
        vstd::bits::lemma_u64_shl_is_mul(immediate, shift);
        vstd::bits::lemma_u64_shr_is_div(kept, shift);
        crate::lemmas::lemma_mask(kept >> shift, 16);
        vstd::bits::lemma_u64_shl_is_mul(field, shift);
        lemma_width_mask(kept, width);
    }
    let result = match operation {
        O::MoveWideNot => if crate::planted::is(19) {
            (!immediate) << shift
        } else {
            !(immediate << shift)
        },
        O::MoveWideZero => if crate::planted::is(403) {
            (kept & !(low_ones(WIDE_IMMEDIATE_STEP) << shift)) | (immediate << shift)
        } else {
            immediate << shift
        },
        _ => {
            let kept_mask = if crate::planted::is(107) {
                low_ones(WIDE_IMMEDIATE_STEP)
            } else {
                low_ones(WIDE_IMMEDIATE_STEP) << shift
            };
            (kept & !kept_mask) | (immediate << shift)
        },
    };
    if crate::planted::is(104) {
        result
    } else {
        result & mask
    }
}

} // verus!

/// The flags of an immediate whose bits are N, Z, C and V from the most significant.
fn to_condition_flags(nzcv: u64) -> ConditionFlags {
    ConditionFlags::from(to_nzcv(nzcv))
}

verus! {

/// The flags of an immediate whose bits are N, Z, C and V from the most significant.
fn to_nzcv(nzcv: u64) -> (flags: Nzcv)
    ensures
        flags == nzcv_immediate(nzcv),
{
    let nzcv = if crate::planted::is(119) {
        nzcv >> 1
    } else {
        nzcv
    };
    Nzcv {
        negative: nzcv & IMMEDIATE_NEGATIVE != 0,
        zero: nzcv & IMMEDIATE_ZERO != 0,
        carry: nzcv & IMMEDIATE_CARRY != 0,
        overflow: nzcv & IMMEDIATE_OVERFLOW != 0,
    }
}

/// The flags CCMN and CCMP set, from their two operands after decode, their NZCV immediate, their condition and the
/// flags before.
fn compute_conditional_compare(
    comparison: Comparison,
    width: RegisterWidth,
    first: u64,
    second: u64,
    nzcv: u64,
    condition: u64,
    flags: Nzcv,
) -> (set: Nzcv)
    requires
        condition < CONDITIONS.len(),
    ensures
        set == conditional_compare(comparison, width.bits(), first, second, nzcv, condition as nat, flags),
{
    let comparison = if crate::planted::is(221) {
        Comparison::Difference
    } else {
        comparison
    };
    let nzcv = if crate::planted::is(220) {
        ((nzcv & 1) << 3) | ((nzcv & 2) << 1) | ((nzcv & 4) >> 1) | ((nzcv & 8) >> 3)
    } else {
        nzcv
    };
    let second = if crate::planted::is(418) {
        (((second << 59) as i64) >> 59) as u64 & width.to_mask()
    } else {
        second
    };
    let second = if crate::planted::is(11) && second & 0x10 != 0 {
        second | !0x1F
    } else {
        second
    };
    let width = if crate::planted::is(12) {
        RegisterWidth::Bits64
    } else {
        width
    };
    let holds = CONDITIONS[condition as usize].holds_in(flags);
    proof {
        assert(CONDITIONS[condition as int].encoding() == condition);
    }
    if crate::planted::is(417) && !holds {
        let sum = match comparison {
            Comparison::Sum => add_with_carry(width, first, second, false).1,
            Comparison::Difference => add_with_carry(width, first, !second, true).1,
        };
        let immediate = to_nzcv(nzcv);
        Nzcv {
            negative: immediate.negative || sum.negative,
            zero: immediate.zero || sum.zero,
            carry: immediate.carry || sum.carry,
            overflow: immediate.overflow || sum.overflow,
        }
    } else if holds {
        match comparison {
            Comparison::Sum => if crate::planted::is(120) {
                add_with_carry(RegisterWidth::Bits64, first, second, false).1
            } else {
                add_with_carry(width, first, second, false).1
            },
            Comparison::Difference => add_with_carry(width, first, !second, true).1,
        }
    } else {
        to_nzcv(nzcv)
    }
}

/// The immediate of ADD and SUB (immediate): imm12, shifted left by 12 bits when sh is set.
fn arithmetic_immediate(imm12: u32, sh: u32) -> (immediate: u64)
    requires
        imm12 < pow2(IMMEDIATE_SHIFT as nat),
        sh <= 1,
    ensures
        immediate == arithmetic_immediate_value(imm12 as nat, sh as nat),
{
    let step = if crate::planted::is(401) {
        16
    } else {
        IMMEDIATE_SHIFT
    };
    let immediate = if crate::planted::is(203) {
        ((u64::from(imm12) << 52) as i64 >> 52) as u64
    } else {
        u64::from(imm12)
    };
    let shift = if crate::planted::is(202) {
        0
    } else {
        sh
    };
    proof {
        assert(step == 12 && shift <= 1);
        vstd::arithmetic::power2::lemma2_to64();
        let amount = (shift * step) as u32;
        assert(immediate == imm12 && immediate < pow2(12) && pow2(amount as nat) <= pow2(12));
        assert(immediate * pow2(amount as nat) <= pow2(12) * pow2(12)) by (nonlinear_arith)
            requires
                immediate < pow2(12),
                pow2(amount as nat) <= pow2(12);
        vstd::bits::lemma_u64_shl_is_mul(immediate, amount as u64);
        assert(immediate << amount == immediate << (amount as u64)) by (bit_vector);
        assert(amount == if sh == 0 { 0u32 } else { 12u32 });
    }
    immediate << (shift * step)
}

} // verus!

verus! {

/// ExtendReg of a register's value by the option and shift of the encoding, or none where the shift is unallocated.
fn extend_register(value: u64, extend_type: u32, amount: u32, width: RegisterWidth) -> (extended: Option<u64>)
    requires
        extend_type < EXTEND_TYPES.len(),
    ensures
        extended is Some <==> extended_register(value as int, extend_type as nat, amount as nat, width.bits()) is Some,
        extended matches Some(extended) ==> extended as int == extended_register(
            value as int,
            extend_type as nat,
            amount as nat,
            width.bits(),
        )->0,
{
    let amount = u64::from(amount);
    if amount > MAXIMUM_EXTEND_SHIFT {
        return None;
    }
    let (signedness, bits) = match EXTEND_TYPES[extend_type as usize] {
        (Signedness::Unsigned, 32) => if crate::planted::is(14) {
            (Signedness::Signed, 32)
        } else {
            (Signedness::Unsigned, 32)
        },
        other => other,
    };
    let shift = u64::BITS - bits;
    let signedness = if crate::planted::is(134) {
        Signedness::Unsigned
    } else {
        signedness
    };
    let extended = match signedness {
        Signedness::Unsigned => (value << shift) >> shift,
        Signedness::Signed => (((value << shift) as i64) >> shift) as u64,
    };
    if crate::planted::is(13) {
        let shifted = value << amount;
        return Some(match signedness {
            Signedness::Unsigned => ((shifted << shift) >> shift) & width.to_mask(),
            Signedness::Signed => ((((shifted << shift) as i64) >> shift) as u64) & width.to_mask(),
        });
    }
    proof {
        let (n, a, length) = (width.bits(), amount as nat, bits as nat);
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        assert(length == pow2((3 + extend_type % 4) as nat) && (signedness is Unsigned) == (extend_type < 4));
        let kept = if length <= n - a { length } else { (n - a) as nat };
        let (low, step) = (pow2((n - a) as nat) as int, pow2(a) as int);
        vstd::arithmetic::power2::lemma_pow2_pos((n - a) as nat);
        vstd::arithmetic::power2::lemma_pow2_adds((n - a) as nat, (length - (n - a)) as nat);
        vstd::arithmetic::power2::lemma_pow2_adds((n - a) as nat, (64 - (n - a)) as nat);
        crate::lemmas::lemma_shift_left(extended, amount as u32, n);
        assert(extended << amount == extended << (amount as u32)) by (bit_vector)
            requires
                amount <= 4;
        crate::lemmas::lemma_mul_pow2_mod(extended as int, a, n);
        match signedness {
            Signedness::Unsigned => {
                assert((value << shift) >> shift == value & (u64::MAX >> shift)) by (bit_vector)
                    requires
                        shift < 64;
                vstd::bits::lemma_unsigned_int_max_is_pow2_minus_1();
                crate::lemmas::lemma_ones_div(64, shift as nat);
                vstd::bits::lemma_u64_shr_is_div(u64::MAX, shift as u64);
                crate::lemmas::lemma_mask(value, length);
                if length <= n - a {
                    vstd::arithmetic::power2::lemma_pow2_adds(length, a);
                    if length + a < n {
                        vstd::arithmetic::power2::lemma_pow2_strictly_increases(length + a, n);
                    }
                    assert(0 <= extended * step < pow2(n)) by (nonlinear_arith)
                        requires
                            0 <= extended < pow2(length),
                            0 < step,
                            pow2(length + a) == pow2(length) * step,
                            pow2(length + a) <= pow2(n);
                    vstd::arithmetic::div_mod::lemma_small_mod((extended * step) as nat, pow2(n));
                } else {
                    vstd::arithmetic::div_mod::lemma_mod_mod(value as int, low, pow2((length - (n - a)) as nat) as int);
                }
            },
            Signedness::Signed => {
                let signed = ((value << shift) as i64) >> shift;
                crate::lemmas::lemma_sign_extension(value, shift);
                crate::lemmas::lemma_signed_mask(signed, 64);
                assert((signed as u64) & u64::MAX == signed as u64) by (bit_vector);
                vstd::arithmetic::div_mod::lemma_mod_mod(signed as int, low, pow2((64 - (n - a)) as nat) as int);
                crate::lemmas::lemma_sint_shift(value as int, kept, a);
                crate::lemmas::lemma_mul_pow2_mod(sint(value as int, kept), a, n);
                if kept < length {
                    crate::lemmas::lemma_sint_mod(value as int, length, kept);
                    crate::lemmas::lemma_sint_mod(value as int, kept, kept);
                }
            },
        }
    }
    Some((extended << amount) & width.to_mask())
}

} // verus!

verus! {

/// `x<lowest + width - 1:lowest> = value`, `mask` being Ones(width).
fn assign_bits(x: u64, lowest: u32, mask: u64, value: u64) -> (assigned: u64)
    requires
        lowest < u64::BITS,
    ensures
        assigned == set_bits(x, lowest, mask, value),
{
    (x & !(mask << lowest)) | ((value & mask) << lowest)
}

/// The low `bits` bits of a value in reverse order.
// VERUS: no specification of `u64::reverse_bits`, so the bits are placed one at a time, in the order of RBIT's loop.
fn reverse_low_bits(value: u64, bits: u32) -> (reversed: u64)
    requires
        0 < bits <= u64::BITS,
    ensures
        reversed == reverse_bits_value(value, bits as nat),
{
    let mut reversed = 0;
    let mut index = 0;
    while index < bits
        invariant
            0 < bits <= u64::BITS,
            index <= bits,
            reversed == reverse_bits_steps(value, bits as nat, index as nat),
        decreases bits - index,
    {
        reversed = assign_bits(reversed, bits - 1 - index, 1, value >> index);
        index += 1;
    }
    reversed
}

/// The result of RBIT: the width's low bits of the source in reverse order.
fn compute_reverse_bits(width: RegisterWidth, source: u64) -> (result: u64)
    ensures
        result == reverse_bits_value(source, width.bits()),
{
    if crate::planted::is(237) {
        u64::from((source as u32).reverse_bits()) | (u64::from(((source >> 32u32) as u32).reverse_bits()) << 32u32)
    } else if crate::planted::is(130) {
        reverse_low_bits(source, u64::BITS)
    } else {
        reverse_low_bits(source, width.to_bits())
    }
}

/// The result of REV16, REV32 and REV: the bytes of each container of `container_bits` bits in reverse order.
// VERUS: no specification of `swap_bytes` or of `fold` over a range, so the bytes are placed one at a time, in the
// order of Arm's loops.
fn reverse_bytes(value: u64, container_bits: u32, width: RegisterWidth) -> (reversed: u64)
    requires
        container_bits == 16 || container_bits == 32 || container_bits == 64,
        container_bits <= width.bits(),
    ensures
        reversed == reverse_bytes_value(value, width.bits(), container_bits as nat),
{
    let containers = if crate::planted::is(131) {
        1
    } else {
        width.to_bits() / container_bits
    };
    let elements_per_container = container_bits / u8::BITS;
    let bytes = containers * elements_per_container;
    proof {
        // Every container size and width, one at a time.
        assert(container_bits == 16 && width.bits() == 32 ==> containers == 2 && bytes == 4);
        assert(container_bits == 16 && width.bits() == 64 ==> containers == 4 && bytes == 8);
        assert(container_bits == 32 && width.bits() == 32 ==> containers == 1 && bytes == 4);
        assert(container_bits == 32 && width.bits() == 64 ==> containers == 2 && bytes == 8);
        assert(container_bits == 64 ==> containers == 1 && bytes == 8);
    }
    let mut reversed = 0;
    let mut index = 0;
    while index < bytes
        invariant
            elements_per_container == container_bits / 8,
            elements_per_container == 2 || elements_per_container == 4 || elements_per_container == 8,
            bytes == (width.bits() / (container_bits as nat)) * ((container_bits as nat) / 8),
            bytes <= 8,
            bytes % elements_per_container == 0,
            index <= bytes,
            reversed == reverse_bytes_steps(value, elements_per_container as nat, index as nat),
        decreases bytes - index,
    {
        let reversed_index = (index / elements_per_container) * elements_per_container
            + (elements_per_container - 1 - index % elements_per_container);
        proof {
            assert(reversed_index < 8) by (bit_vector)
                requires
                    elements_per_container == 2u32 || elements_per_container == 4u32 || elements_per_container == 8u32,
                    bytes <= 8u32,
                    bytes % elements_per_container == 0u32,
                    index < bytes,
                    reversed_index == (index / elements_per_container) * elements_per_container
                        + (elements_per_container - 1u32 - index % elements_per_container);
            assert(reversed_index == reversed_byte(index as nat, elements_per_container as nat));
        }
        reversed = assign_bits(reversed, u8::BITS * reversed_index, 0xff, value >> (u8::BITS * index));
        index += 1;
    }
    reversed
}

/// The result of CLZ on the width's bits of the source.
fn compute_count_leading_zeros(width: RegisterWidth, source: u64) -> (result: u64)
    requires
        source < pow2(width.bits()),
    ensures
        result as int == count_leading_zero_bits(uint(source as int, width.bits()) as nat, width.bits()),
{
    proof {
        crate::lemmas::lemma_leading_zeros(source);
        crate::lemmas::lemma_highest_set_bit_bounds(source as nat, width.bits());
        vstd::arithmetic::div_mod::lemma_small_mod(source as nat, pow2(width.bits()));
    }
    if crate::planted::is(410) && source == 0 {
        63
    } else if crate::planted::is(128) {
        u64::from(source.leading_zeros())
    } else {
        u64::from(source.leading_zeros() - (u64::BITS - width.to_bits()))
    }
}

/// The result of CLS on the width's bits of the source.
fn compute_count_leading_sign_bits(width: RegisterWidth, source: u64) -> (result: u64)
    requires
        source < pow2(width.bits()),
    ensures
        result as int == count_leading_sign_bits(source, width.bits()),
{
    let extended = sign_extend(
        source,
        if crate::planted::is(9) {
            RegisterWidth::Bits64
        } else {
            width
        },
    );
    let differing = (extended ^ (extended >> 1u32)) as u64;
    proof {
        let n = width.bits();
        let shift = (64 - n) as u32;
        let mask = ones((n - 1) as nat) as u64;
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        assert(differing == ((source >> 1u32) ^ source) & mask && differing <= mask) by (bit_vector)
            requires
                extended == ((source << shift) as i64) >> shift,
                differing == (extended ^ (extended >> 1u32)) as u64,
                shift == 32u32 && source < 0x1_0000_0000u64 && mask == 0x7fff_ffffu64
                    || shift == 0u32 && mask == 0x7fff_ffff_ffff_ffffu64;
        crate::lemmas::lemma_leading_zeros(differing);
        crate::lemmas::lemma_highest_set_bit_bounds(differing as nat, (n - 1) as nat);
    }
    // VERUS: no specification of `From<bool>`, so the bit is cast.
    u64::from(differing.leading_zeros() - (!crate::planted::is(129)) as u32 - (u64::BITS - width.to_bits()))
}

/// The result of LSLV, LSRV, ASRV and RORV, the width's low bits.
fn compute_variable_shift(operation: IntegerOperation, width: RegisterWidth, first: u64, second: u64) -> (result: u64)
    requires
        operation is ShiftLeftVariable || operation is ShiftRightLogicalVariable
            || operation is ShiftRightArithmeticVariable || operation is RotateRightVariable,
        first < pow2(width.bits()),
    ensures
        result as int == variable_shift(operation, first, second, width.bits()),
{
    use IntegerOperation as O;
    let mask = width.to_mask();
    let bits = u64::from(width.to_bits());
    let amount = second % bits;
    let ghost n = width.bits();
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        vstd::arithmetic::power2::lemma_pow2_pos(n);
        vstd::arithmetic::div_mod::lemma_small_mod(first as nat, pow2(n));
        crate::lemmas::lemma_modulo_width(second as int, n);
        assert(amount < n);
    }
    let result = match operation {
        O::ShiftLeftVariable => if crate::planted::is(227) {
            // VERUS: no specification of `min`, so the smaller amount is chosen by a comparison.
            first << if second < bits - 1 {
                second
            } else {
                bits - 1
            }
        } else if crate::planted::is(121) {
            first << (second % 64)
        } else {
            proof {
                crate::lemmas::lemma_shift_left(first, amount as u32, n);
                assert(first << amount == first << (amount as u32)) by (bit_vector)
                    requires
                        amount < 64;
            }
            first << amount
        },
        O::ShiftRightLogicalVariable => if crate::planted::is(229) {
            first >> (second % 32)
        } else {
            proof {
                vstd::bits::lemma_u64_shr_is_div(first, amount);
                vstd::arithmetic::power2::lemma_pow2_pos(amount as nat);
                vstd::arithmetic::div_mod::lemma_div_is_ordered_by_denominator(first as int, 1, pow2(amount as nat) as int);
                crate::lemmas::lemma_mask(first >> amount, n);
                vstd::arithmetic::div_mod::lemma_small_mod((first >> amount) as nat, pow2(n));
            }
            first >> amount
        },
        O::ShiftRightArithmeticVariable => if crate::planted::is(122) {
            first >> amount
        } else {
            let signed = if crate::planted::is(414) {
                first as i64
            } else {
                sign_extend(first, width)
            };
            proof {
                crate::lemmas::lemma_arithmetic_shift_right(signed, amount as u32);
                assert(signed >> amount == signed >> (amount as u32)) by (bit_vector)
                    requires
                        amount < 64;
                crate::lemmas::lemma_signed_mask(signed >> amount, n);
            }
            (signed >> amount) as u64
        },
        _ => if crate::planted::is(123) {
            rotate_right(first, amount as u32, RegisterWidth::Bits64)
        } else {
            let rotation = if crate::planted::is(10) {
                ((bits - amount) % bits) as u32
            } else {
                amount as u32
            };
            let rotated = rotate_right(first, rotation, width);
            proof {
                crate::lemmas::lemma_mask(rotated, n);
                vstd::arithmetic::div_mod::lemma_small_mod(rotated as nat, pow2(n));
            }
            rotated
        },
    };
    if crate::planted::is(104) {
        result
    } else {
        result & mask
    }
}

/// The magnitude of a signed value, as 64 bits: 2^63 for the most negative.
// VERUS: no specification of `unsigned_abs` or `wrapping_neg`, so the negation is a subtraction from zero.
fn magnitude(value: i64) -> (unsigned: u64)
    ensures
        unsigned as int == if value < 0 { -value } else { value as int },
{
    proof {
        crate::lemmas::lemma_signed_pattern(value);
        vstd::arithmetic::power2::lemma2_to64();
        if value < 0 {
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(value as int, pow2(64) as int, -1, value + pow2(64));
        } else {
            vstd::arithmetic::div_mod::lemma_small_mod(value as nat, pow2(64));
        }
    }
    if value < 0 {
        0u64.wrapping_sub(value as u64)
    } else {
        value as u64
    }
}

/// The result of UDIV and SDIV, the width's low bits.
fn compute_divide(signedness: Signedness, width: RegisterWidth, first: u64, second: u64) -> (result: u64)
    requires
        first < pow2(width.bits()),
        second < pow2(width.bits()),
    ensures
        result as int == divide(signedness, first, second, width.bits()),
{
    let mask = width.to_mask();
    let bits = width.to_bits();
    let ghost n = width.bits();
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        vstd::arithmetic::power2::lemma_pow2_pos(n);
        vstd::arithmetic::div_mod::lemma_small_mod(first as nat, pow2(n));
        vstd::arithmetic::div_mod::lemma_small_mod(second as nat, pow2(n));
    }
    // The unsigned quotient, none where the divisor is zero.
    let result = match first.checked_div(second) {
        None => if crate::planted::is(235) {
            first
        } else if crate::planted::is(124) {
            u64::MAX
        } else {
            0
        },
        Some(quotient) => match signedness {
            Signedness::Unsigned => {
                proof {
                    vstd::arithmetic::div_mod::lemma_div_is_ordered_by_denominator(first as int, 1, second as int);
                    crate::lemmas::lemma_mask(quotient, n);
                    vstd::arithmetic::div_mod::lemma_small_mod(quotient as nat, pow2(n));
                }
                quotient
            },
            Signedness::Signed => if crate::planted::is(21) && first == 1 << (bits - 1) && second == mask {
                0
            } else {
                let (dividend, divisor) = (sign_extend(first, width), sign_extend(second, width));
                if crate::planted::is(125) {
                    // VERUS: no specification of `wrapping_div_euclid`; `checked_div_euclid` fails only where it
                    // wraps, to the dividend.
                    match dividend.checked_div_euclid(divisor) {
                        Some(quotient) => quotient as u64,
                        None => dividend as u64,
                    }
                } else {
                    // VERUS: no specification of `wrapping_div`, so the quotient of the magnitudes is negated where
                    // the signs differ, which is the truncating division it computes, wrapping only for the most
                    // negative value divided by -1.
                    let quotient = magnitude(dividend) / magnitude(divisor);
                    proof {
                        let (a, b) = (dividend as int, divisor as int);
                        assert(b != 0);
                        let (x, d) = (if a < 0 { -a } else { a }, if b < 0 { -b } else { b });
                        vstd::arithmetic::div_mod::lemma_div_is_ordered_by_denominator(x, 1, d);
                        assert(quotient as int == x / d);
                        if (a < 0) != (b < 0) {
                            crate::lemmas::lemma_mask(0u64.wrapping_sub(quotient), n);
                            crate::lemmas::lemma_wrapped_mod(-(quotient as int), n);
                            vstd::arithmetic::div_mod::lemma_mod_multiples_vanish(1, -(quotient as int), pow2(64) as int);
                            if quotient == 0 {
                                vstd::arithmetic::div_mod::lemma_small_mod(0, pow2(n));
                            } else {
                                vstd::arithmetic::div_mod::lemma_small_mod((pow2(64) - quotient) as nat, pow2(64));
                            }
                        } else {
                            crate::lemmas::lemma_mask(quotient, n);
                        }
                    }
                    if (dividend < 0) != (divisor < 0) {
                        0u64.wrapping_sub(quotient)
                    } else {
                        quotient
                    }
                }
            },
        },
    };
    proof {
        crate::lemmas::lemma_mask(result, n);
        vstd::arithmetic::div_mod::lemma_small_mod(0, pow2(n));
    }
    if crate::planted::is(104) {
        result
    } else {
        result & mask
    }
}

/// The result of MADD and MSUB, the width's low bits.
fn compute_multiply_add(operation: IntegerOperation, width: RegisterWidth, first: u64, second: u64, third: u64) -> (result: u64)
    requires
        operation is MultiplyAdd || operation is MultiplySubtract,
    ensures
        result as int == multiply_add(operation is MultiplySubtract, first, second, third, width.bits()),
{
    let mask = width.to_mask();
    let product = first.wrapping_mul(second);
    let result = match operation {
        IntegerOperation::MultiplyAdd => third.wrapping_add(product),
        _ => if crate::planted::is(140) {
            product.wrapping_sub(third)
        } else {
            third.wrapping_sub(product)
        },
    };
    proof {
        let n = width.bits();
        let (m, wide) = (pow2(n) as int, pow2(64) as int);
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        vstd::arithmetic::power2::lemma_pow2_pos(n);
        let exact = first as int * second as int;
        assert(product as int == exact % wide);
        let sum = if operation is MultiplySubtract { third - product } else { third + product };
        // The wrapped sum or difference is the sum or difference modulo 2^64.
        if sum < 0 {
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(sum, wide, -1, sum + wide);
        } else if sum >= wide {
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(sum, wide, 1, sum - wide);
        } else {
            vstd::arithmetic::div_mod::lemma_small_mod(sum as nat, wide as nat);
        }
        assert(result as int == sum % wide);
        // The low n bits of the wrapped value are those of the exact sum or difference, and the product's low n bits
        // are those of the product of the operands' low n bits.
        crate::lemmas::lemma_mask(result, n);
        crate::lemmas::lemma_wrapped_mod(sum, n);
        crate::lemmas::lemma_wrapped_mod(exact, n);
        let low_product = uint(first as int, n) * uint(second as int, n);
        vstd::arithmetic::div_mod::lemma_mul_mod_noop(first as int, second as int, m);
        vstd::arithmetic::div_mod::lemma_mod_twice(third as int, m);
        if operation is MultiplySubtract {
            vstd::arithmetic::div_mod::lemma_sub_mod_noop(third as int, product as int, m);
            vstd::arithmetic::div_mod::lemma_sub_mod_noop(uint(third as int, n), low_product, m);
        } else {
            vstd::arithmetic::div_mod::lemma_add_mod_noop(third as int, product as int, m);
            vstd::arithmetic::div_mod::lemma_add_mod_noop(uint(third as int, n), low_product, m);
        }
    }
    if crate::planted::is(104) {
        result
    } else {
        result & mask
    }
}

/// The result of SMADDL, UMADDL, SMSUBL and UMSUBL.
fn compute_multiply_add_long(operation: IntegerOperation, first: u64, second: u64, third: u64) -> (result: u64)
    requires
        operation is MultiplyAddLong || operation is MultiplySubtractLong,
    ensures
        result as int == match operation {
            IntegerOperation::MultiplyAddLong(signedness) => multiply_add_long(false, first, second, third, signedness),
            IntegerOperation::MultiplySubtractLong(signedness) => multiply_add_long(true, first, second, third, signedness),
            _ => 0,
        },
{
    let result = match operation {
        IntegerOperation::MultiplyAddLong(signedness) => {
            let signedness = if crate::planted::is(7) {
                Signedness::Signed
            } else {
                signedness
            };
            let signedness = if crate::planted::is(126) {
                Signedness::Unsigned
            } else {
                signedness
            };
            third.wrapping_add(multiply_long(first, second, signedness))
        },
        IntegerOperation::MultiplySubtractLong(signedness) => if crate::planted::is(406) {
            first.wrapping_sub(multiply_long(first, second, signedness))
        } else if crate::planted::is(233) {
            third.wrapping_sub(first.wrapping_mul(second))
        } else {
            third.wrapping_sub(multiply_long(first, second, signedness))
        },
        _ => 0,
    };
    proof {
        let wide = pow2(64) as int;
        vstd::arithmetic::power2::lemma2_to64();
        let (subtract, signedness) = match operation {
            IntegerOperation::MultiplyAddLong(signedness) => (false, signedness),
            IntegerOperation::MultiplySubtractLong(signedness) => (true, signedness),
            _ => (false, Signedness::Unsigned),
        };
        let exact = long_product(first, second, signedness);
        let product = multiply_long_value(first, second, signedness);
        assert(product == exact % wide);
        let sum = if subtract { third - product } else { third + product };
        if sum < 0 {
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(sum, wide, -1, sum + wide);
        } else if sum >= wide {
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(sum, wide, 1, sum - wide);
        } else {
            vstd::arithmetic::div_mod::lemma_small_mod(sum as nat, wide as nat);
        }
        assert(result as int == sum % wide);
        if subtract {
            vstd::arithmetic::div_mod::lemma_sub_mod_noop(third as int, exact, wide);
            vstd::arithmetic::div_mod::lemma_sub_mod_noop(third as int, product, wide);
        } else {
            vstd::arithmetic::div_mod::lemma_add_mod_noop(third as int, exact, wide);
            vstd::arithmetic::div_mod::lemma_add_mod_noop(third as int, product, wide);
        }
        vstd::arithmetic::div_mod::lemma_mod_twice(exact, wide);
        vstd::arithmetic::div_mod::lemma_small_mod(third as nat, wide as nat);
    }
    result
}

/// The result of UMULH and SMULH: the high 64 bits of the 128-bit product.
fn compute_multiply_high(signedness: Signedness, first: u64, second: u64) -> (result: u64)
    ensures
        result as int == multiply_high(first, second, signedness),
{
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
        vstd::arithmetic::div_mod::lemma_small_mod(first as nat, pow2(64));
        vstd::arithmetic::div_mod::lemma_small_mod(second as nat, pow2(64));
        assert(0 <= first as int * second as int <= u128::MAX) by (nonlinear_arith)
            requires
                first <= u64::MAX,
                second <= u64::MAX;
    }
    // VERUS: no specification of `u128::from` or `i128::from`, so the registers are cast.
    match signedness {
        Signedness::Unsigned => if crate::planted::is(127) {
            first.wrapping_mul(second)
        } else {
            let product = (first as u128) * (second as u128);
            proof {
                vstd::bits::lemma_u128_shr_is_div(product, 64);
                let bits = u64::BITS;
                assert(product >> bits == product >> 64u128) by (bit_vector)
                    requires
                        bits == 64u32;
                assert(product as int / 0x1_0000_0000_0000_0000 <= u64::MAX) by (nonlinear_arith)
                    requires
                        product as int <= u128::MAX;
                vstd::arithmetic::div_mod::lemma_small_mod((product as int / 0x1_0000_0000_0000_0000) as nat, pow2(64));
            }
            (product >> u64::BITS) as u64
        },
        Signedness::Signed => if crate::planted::is(6) {
            (((first as u128) * (second as u128)) >> u64::BITS) as u64
        } else {
            let (low, high) = (first as i64, second as i64);
            proof {
                crate::lemmas::lemma_sign_extension(first, 0);
                crate::lemmas::lemma_sign_extension(second, 0);
                assert(((first << 0u32) as i64) >> 0u32 == first as i64 && ((second << 0u32) as i64) >> 0u32 == second as i64) by (bit_vector);
                assert(-0x4000_0000_0000_0000_0000_0000_0000_0000 <= low as int * high as int <= 0x4000_0000_0000_0000_0000_0000_0000_0000) by (nonlinear_arith)
                    requires
                        -0x8000_0000_0000_0000 <= low <= 0x7fff_ffff_ffff_ffff,
                        -0x8000_0000_0000_0000 <= high <= 0x7fff_ffff_ffff_ffff;
            }
            let product = (low as i128) * (high as i128);
            let shifted = product >> u64::BITS;
            proof {
                crate::lemmas::lemma_arithmetic_shift_right_128(product, 64);
                assert(-0x4000_0000_0000_0000 <= shifted <= 0x4000_0000_0000_0000) by (nonlinear_arith)
                    requires
                        shifted as int == product as int / 0x1_0000_0000_0000_0000,
                        -0x4000_0000_0000_0000_0000_0000_0000_0000 <= product <= 0x4000_0000_0000_0000_0000_0000_0000_0000;
                assert(shifted as u64 == (shifted as i64) as u64) by (bit_vector);
                crate::lemmas::lemma_signed_pattern(shifted as i64);
            }
            shifted as u64
        },
    }
}

} // verus!

verus! {

/// Accumulates the low `bytes` bytes of `value` into the checksum the way [CRC32](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CRC32B--CRC32H--CRC32W--CRC32X--CRC32-checksum-) does.
// VERUS: the checksum is computed as Arm's pseudocode computes it, Poly32Mod2 on the reversed bits in a `u128`, so that
// its loop is the pseudocode's; the shift register on reflected bits it replaces computed the same, unproven. Its
// planted bug 241, which took the value's bits from the most significant, is the value not reversed here, the same
// order; `min` and `planted::any` have no specification, so they are a comparison and two `is`.
fn accumulate_crc(checksum: u64, value: u64, bytes: u32, polynomial: CrcPolynomial) -> (accumulated: u64)
    requires
        bytes == 1 || bytes == 2 || bytes == 4 || bytes == 8,
    ensures
        accumulated == crc32(checksum, value, (bytes * 8) as nat, polynomial),
{
    let polynomial = if crate::planted::is(411) || crate::planted::is(132) {
        CrcPolynomial::Crc32
    } else {
        polynomial
    };
    let bytes = if crate::planted::is(133) {
        if bytes < 4 {
            bytes
        } else {
            4
        }
    } else {
        bytes
    };
    let size = bytes * u8::BITS;
    let poly = match polynomial {
        CrcPolynomial::Crc32 => CRC32_POLYNOMIAL,
        CrcPolynomial::Crc32C => CRC32C_POLYNOMIAL,
    };
    let accumulator = reverse_low_bits(checksum, u32::BITS);
    let input = if crate::planted::is(241) {
        value & low_ones(size)
    } else {
        reverse_low_bits(value, size)
    };
    let data_in = ((accumulator as u128) << size) ^ ((input as u128) << u32::BITS);
    let ghost n = (size + 32) as nat;
    let mut data = data_in;
    let mut index = size + u32::BITS - 1;
    while index >= u32::BITS
        invariant
            8 <= size <= 64,
            n == size + 32,
            31 <= index <= n - 1,
            data == crate::pseudocode::poly32_mod2_steps(data_in, poly, n, (n - 1 - index) as nat),
        decreases index,
    {
        if (data >> index) & 1 == 1 {
            proof {
                vstd::bits::lemma_u128_pow2_no_overflow(index as nat);
                vstd::bits::lemma_u128_shl_is_mul(1, index as u128);
                assert(1u128 << index == 1u128 << (index as u128)) by (bit_vector);
            }
            let mask = (1u128 << index) - 1;
            proof {
                assert(mask == ones(index as nat) as u128);
            }
            data = (data & !mask) | ((data ^ ((poly as u128) << (index - u32::BITS))) & mask);
        }
        index -= 1;
    }
    proof {
        assert(data & 0xffff_ffff <= 0xffff_ffff) by (bit_vector);
    }
    reverse_low_bits((data & 0xffff_ffff) as u64, u32::BITS)
}

} // verus!

fn apply(
    operation: IntegerOperation,
    width: RegisterWidth,
    kept: u64,
    sources: &[u64],
    flags: ConditionFlags,
) -> Result<Outcome, Exception> {
    use IntegerOperation as O;
    let mask = width.to_mask();
    let written = |result: u64| Outcome {
        result: Some(if crate::planted::is(104) {
            result
        } else {
            result & mask
        }),
        flags: None,
    };
    Ok(match (operation, sources) {
        (
            O::Add(_)
            | O::Subtract(_)
            | O::AddWithCarry(_)
            | O::SubtractWithCarry(_)
            | O::And(_)
            | O::AndNot(_)
            | O::Or
            | O::OrNot
            | O::ExclusiveOr
            | O::ExclusiveOrNot,
            &[first, second],
        ) => {
            let (result, result_flags) =
                compute_arithmetic_or_logical(operation, width, first, second, Nzcv::from(flags));
            Outcome {
                result: Some(result),
                flags: result_flags.map(ConditionFlags::from),
            }
        }
        (O::MoveWideNot | O::MoveWideZero | O::MoveWideKeep, &[immediate, shift]) => Outcome {
            result: Some(compute_move_wide(operation, width, kept, immediate, shift)),
            flags: None,
        },
        (O::Bitfield(fill), &[source, immr, imms]) => {
            match compute_bitfield(fill, width, kept, source, immr, imms) {
                Some(result) => Outcome {
                    result: Some(result),
                    flags: None,
                },
                None => return Err(Exception::UndefinedInstruction),
            }
        }
        (O::Extract, &[first, second, lowest_bit]) => Outcome {
            result: Some(compute_extract(width, first, second, lowest_bit)),
            flags: None,
        },
        (O::ConditionalSelect(alternative), &[first, second, condition]) => Outcome {
            result: Some(compute_conditional_select(
                alternative,
                width,
                first,
                second,
                condition,
                Nzcv::from(flags),
            )),
            flags: None,
        },
        (O::ConditionalCompare(comparison), &[first, second, nzcv, condition]) => Outcome {
            result: None,
            flags: Some(ConditionFlags::from(compute_conditional_compare(
                comparison,
                width,
                first,
                second,
                nzcv,
                condition,
                Nzcv::from(flags),
            ))),
        },
        (O::ReverseBits, &[source]) => written(compute_reverse_bits(width, source)),
        (O::ReverseBytes { container_bits }, &[source]) => {
            let container_bits = if crate::planted::is(239) {
                u32::BITS
            } else {
                container_bits
            };
            let container_bits = if crate::planted::is(8) {
                u16::BITS
            } else {
                container_bits
            };
            let container_bits = if crate::planted::is(409) {
                u64::BITS
            } else {
                container_bits
            };
            written(reverse_bytes(source, container_bits, width))
        }
        (O::CountLeadingZeros, &[source]) => written(compute_count_leading_zeros(width, source)),
        (O::CountLeadingSignBits, &[source]) => {
            written(compute_count_leading_sign_bits(width, source))
        }
        (
            O::ShiftLeftVariable
            | O::ShiftRightLogicalVariable
            | O::ShiftRightArithmeticVariable
            | O::RotateRightVariable,
            &[first, second],
        ) => Outcome {
            result: Some(compute_variable_shift(operation, width, first, second)),
            flags: None,
        },
        (O::Divide(signedness), &[first, second]) => Outcome {
            result: Some(compute_divide(signedness, width, first, second)),
            flags: None,
        },
        (O::MultiplyAdd | O::MultiplySubtract, &[first, second, third]) => Outcome {
            result: Some(compute_multiply_add(operation, width, first, second, third)),
            flags: None,
        },
        (O::MultiplyAddLong(_) | O::MultiplySubtractLong(_), &[first, second, third]) => {
            written(compute_multiply_add_long(operation, first, second, third))
        }
        (O::MultiplyHigh(signedness), &[first, second]) => {
            written(compute_multiply_high(signedness, first, second))
        }
        (O::Crc32 { polynomial, bytes }, &[checksum, value]) if crate::planted::is(412) => {
            written(accumulate_crc(checksum, value >> 8, bytes, polynomial))
        }
        (O::Crc32 { polynomial, bytes }, &[checksum, value]) => {
            written(accumulate_crc(checksum, value, bytes, polynomial))
        }
        (O::RotateMaskInsertFlags, &[source, rotation, selected]) => {
            let rotated = if crate::planted::is(136) {
                source.rotate_left(rotation as u32)
            } else {
                rotate_right(source, rotation as u32, width)
            };
            let selected = to_condition_flags(selected);
            Outcome {
                result: None,
                flags: Some((flags & !selected) | (to_condition_flags(rotated) & selected)),
            }
        }
        (O::SetFlags { bits }, &[source]) => {
            let mut set = flags & ConditionFlags::CARRY;
            let top = source
                >> if crate::planted::is(16) {
                    bits
                } else {
                    bits - 1
                };
            set.set(ConditionFlags::NEGATIVE, top & 1 != 0);
            set.set(ConditionFlags::ZERO, source & low_ones(bits) == 0);
            let overflow_top = if crate::planted::is(137) {
                source >> bits
            } else {
                top
            };
            set.set(
                ConditionFlags::OVERFLOW,
                (overflow_top ^ (overflow_top >> 1)) & 1 != 0,
            );
            Outcome {
                result: None,
                flags: Some(set),
            }
        }
        (O::InvertCarry, &[]) => Outcome {
            result: None,
            flags: Some(if crate::planted::is(413) {
                flags ^ ConditionFlags::CARRY ^ ConditionFlags::OVERFLOW
            } else {
                flags ^ ConditionFlags::CARRY
            }),
        },
        (O::ConvertFlagsToExternalFormat, &[]) => {
            let zero = flags.contains(ConditionFlags::ZERO);
            let carry = flags.contains(ConditionFlags::CARRY);
            let overflow = flags.contains(ConditionFlags::OVERFLOW);
            let mut set = ConditionFlags::empty();
            set.set(
                ConditionFlags::ZERO,
                zero || (overflow && !crate::planted::is(138)),
            );
            set.set(ConditionFlags::CARRY, carry && !overflow);
            Outcome {
                result: None,
                flags: Some(set),
            }
        }
        (O::ConvertFlagsFromExternalFormat, &[]) => {
            let zero = flags.contains(ConditionFlags::ZERO);
            let carry = flags.contains(ConditionFlags::CARRY);
            let mut set = ConditionFlags::empty();
            set.set(
                ConditionFlags::NEGATIVE,
                !carry && (zero == crate::planted::is(244)),
            );
            set.set(ConditionFlags::ZERO, zero && carry);
            set.set(ConditionFlags::CARRY, carry || zero);
            set.set(ConditionFlags::OVERFLOW, !carry && zero);
            Outcome {
                result: None,
                flags: Some(set),
            }
        }
        _ => panic!(
            "{operation:?} reads other operands than {} values",
            sources.len()
        ),
    })
}

verus! {

/// The 64-bit product of the low words of two values.
fn multiply_long(first: u64, second: u64, signedness: Signedness) -> (product: u64)
    ensures
        product as int == multiply_long_value(first, second, signedness),
{
    proof {
        crate::lemmas::lemma_mask(first, 32);
        crate::lemmas::lemma_mask(second, 32);
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::power2::lemma2_to64_rest();
    }
    match signedness {
        Signedness::Unsigned => {
            proof {
                let (low, high) = ((first & 0xffff_ffff) as int, (second & 0xffff_ffff) as int);
                assert(0 <= low * high < pow2(64)) by (nonlinear_arith)
                    requires
                        0 <= low < pow2(32),
                        0 <= high < pow2(32),
                        pow2(64) == pow2(32) * pow2(32);
                vstd::arithmetic::div_mod::lemma_small_mod((low * high) as nat, pow2(64));
            }
            (first & RegisterWidth::Bits32.to_mask()) * (second & RegisterWidth::Bits32.to_mask())
        },
        Signedness::Signed => {
            let (low, high) = (sign_extend(first, RegisterWidth::Bits32), sign_extend(second, RegisterWidth::Bits32));
            proof {
                assert(-0x8000_0000_0000_0000 < low * high < 0x8000_0000_0000_0000) by (nonlinear_arith)
                    requires
                        -0x8000_0000 <= low < 0x8000_0000,
                        -0x8000_0000 <= high < 0x8000_0000;
                crate::lemmas::lemma_signed_mask((low * high) as i64, 64);
                assert(((low * high) as i64 as u64) & u64::MAX == (low * high) as i64 as u64) by (bit_vector);
            }
            (low.wrapping_mul(high)) as u64
        },
    }
}

} // verus!
