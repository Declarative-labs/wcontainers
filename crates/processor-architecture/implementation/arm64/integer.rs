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
    arithmetic_immediate_value, arithmetic_or_logical, conditional_compare, extended_register,
    is_zero, logical_result, multiply_long_value, nzcv_immediate, ones, ror, shifted_register, sint,
    uint, written,
};
use crate::registers::{read_register, write_register};
use crate::{Exception, Instruction, Processor};

/// The most source operands an instruction reads: CCMP's register, second operand, flags and condition.
const MAXIMUM_SOURCES: usize = 4;
verus! {

/// The [shift amount](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--immediate---Add-immediate-value-#:~:text=sh) a set shift bit gives an arithmetic immediate.
const IMMEDIATE_SHIFT: u32 = 12;

} // verus!
/// How many bits each step of a move wide's hw moves its immediate.
const WIDE_IMMEDIATE_STEP: u32 = u16::BITS;
verus! {

/// The largest left shift of an [extended register](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--extended-register---Add-extended-and-scaled-register-#:~:text=amount); larger ones are unallocated.
const MAXIMUM_EXTEND_SHIFT: u64 = 4;

} // verus!
/// The width of immr and imms, whose values above a 32-bit element's are unused.
const BITMASK_FIELD_BITS: u32 = 6;
/// [CRC-32](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CRC32B--CRC32H--CRC32W--CRC32X--CRC32-checksum-#:~:text=0x04C11DB7)'s polynomial, before its bits are reversed.
const CRC32_POLYNOMIAL: u32 = 0x04C1_1DB7;
/// [CRC-32C](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CRC32CB--CRC32CH--CRC32CW--CRC32CX--CRC32C-checksum-#:~:text=0x1EDC6F41)'s polynomial, before its bits are reversed.
const CRC32C_POLYNOMIAL: u32 = 0x1EDC_6F41;

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
        processor.condition_flags,
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
            let carry = processor.condition_flags.contains(ConditionFlags::CARRY);
            value = first + second + u64::from(carry);
        }
        write_register(processor, number, value, thirty_one);
    }
    if let Some(flags) = outcome.flags {
        processor.condition_flags = flags;
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
            let bits = field.read(word);
            let imms = bits & low_ones(BITMASK_FIELD_BITS) as u32;
            let immr = (bits >> BITMASK_FIELD_BITS) & low_ones(BITMASK_FIELD_BITS) as u32;
            let n = bits >> (2 * BITMASK_FIELD_BITS) != 0 && !crate::planted::is(204);
            match decode_bit_masks(n, imms, immr, BitmaskUse::Immediate, width) {
                Some((wmask, _)) => wmask,
                None => return Err(Exception::UndefinedInstruction),
            }
        }
        Operand::WideImmediateShift { field, width } => {
            let shift = field.read(word)
                * if crate::planted::is(108) {
                    u8::BITS
                } else {
                    WIDE_IMMEDIATE_STEP
                };
            if shift >= width.to_bits() {
                return Err(Exception::UndefinedInstruction);
            }
            u64::from(shift)
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

} // verus!

/// Whether [DecodeBitMasks](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-bitmasks#impl-aarch64.DecodeBitMasks.5) decodes a logical immediate, which may not be all ones, or a bitfield.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BitmaskUse {
    Immediate,
    Bitfield,
}

/// The wmask and tmask of DecodeBitMasks, or none where the encoding is reserved.
fn decode_bit_masks(
    n: bool,
    imms: u32,
    immr: u32,
    bitmask_use: BitmaskUse,
    width: RegisterWidth,
) -> Option<(u64, u64)> {
    let combined =
        (u32::from(n) << BITMASK_FIELD_BITS) | (!imms & low_ones(BITMASK_FIELD_BITS) as u32);
    let length = combined.checked_ilog2().filter(|&length| length >= 1)?;
    let levels = low_ones(length) as u32;
    if bitmask_use == BitmaskUse::Immediate && imms & levels == levels {
        return None;
    }
    let (s, r) = (imms & levels, immr & levels);
    let element_bits = 1 << length;
    let difference = s.wrapping_sub(r) & levels;
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
    Some((
        if crate::planted::is(205) {
            element
        } else {
            replicate(element, element_bits)
        } & width.to_mask(),
        replicate(top, element_bits) & width.to_mask(),
    ))
}

verus! {

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

} // verus!

fn replicate(element: u64, bits: u32) -> u64 {
    (0..u64::BITS / bits).fold(0, |replicated, index| {
        replicated | (element << (index * bits))
    })
}

verus! {

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
            || operation is And || operation is AndNot,
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
        _ => (0, None),
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

fn reverse_bytes(value: u64, container_bits: u32, width: RegisterWidth) -> u64 {
    let containers = if crate::planted::is(131) {
        1
    } else {
        width.to_bits() / container_bits
    };
    (0..containers).fold(0, |reversed, index| {
        let shift = index * container_bits;
        let container = (value >> shift) & low_ones(container_bits);
        let swapped = container.swap_bytes() >> (u64::BITS - container_bits);
        reversed | (swapped << shift)
    })
}

/// Accumulates the low bits of `value` into the checksum the way [CRC32](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CRC32B--CRC32H--CRC32W--CRC32X--CRC32-checksum-) does: on reversed bits, so its least significant bit first.
fn accumulate_crc(checksum: u64, value: u64, bytes: u32, polynomial: CrcPolynomial) -> u64 {
    let polynomial = if crate::planted::any(&[411, 132]) {
        CrcPolynomial::Crc32
    } else {
        polynomial
    };
    let bytes = if crate::planted::is(133) {
        bytes.min(4)
    } else {
        bytes
    };
    let reversed = match polynomial {
        CrcPolynomial::Crc32 => CRC32_POLYNOMIAL,
        CrcPolynomial::Crc32C => CRC32C_POLYNOMIAL,
    }
    .reverse_bits();
    let mut checksum = checksum as u32;
    for step in 0..bytes * u8::BITS {
        let bit = if crate::planted::is(241) {
            bytes * u8::BITS - 1 - step
        } else {
            step
        };
        let feedback = (checksum ^ (value >> bit) as u32) & 1 != 0;
        checksum >>= 1;
        if feedback {
            checksum ^= reversed;
        }
    }
    u64::from(checksum)
}

fn apply(
    operation: IntegerOperation,
    width: RegisterWidth,
    kept: u64,
    sources: &[u64],
    flags: ConditionFlags,
) -> Result<Outcome, Exception> {
    use IntegerOperation as O;
    let mask = width.to_mask();
    let bits = width.to_bits();
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
            | O::AndNot(_),
            &[first, second],
        ) => {
            let (result, result_flags) =
                compute_arithmetic_or_logical(operation, width, first, second, Nzcv::from(flags));
            Outcome {
                result: Some(result),
                flags: result_flags.map(ConditionFlags::from),
            }
        }
        (O::Or, &[first, second]) => written(first | second),
        (O::OrNot, &[first, second]) if crate::planted::is(216) => written(!(first | second)),
        (O::OrNot, &[first, second]) => written(first | !second),
        (O::ExclusiveOr, &[first, second]) if crate::planted::is(210) => written(first | second),
        (O::ExclusiveOr, &[first, second]) => written(first ^ second),
        (O::ExclusiveOrNot, &[first, second]) if crate::planted::is(408) => written(first ^ second),
        (O::ExclusiveOrNot, &[first, second]) => written(first ^ !second),
        (O::MoveWideNot, &[immediate, shift]) if crate::planted::is(19) => {
            written((!immediate) << shift)
        }
        (O::MoveWideNot, &[immediate, shift]) => written(!(immediate << shift)),
        (O::MoveWideZero, &[immediate, shift]) if crate::planted::is(403) => {
            written((kept & !(low_ones(WIDE_IMMEDIATE_STEP) << shift)) | (immediate << shift))
        }
        (O::MoveWideZero, &[immediate, shift]) => written(immediate << shift),
        (O::MoveWideKeep, &[immediate, shift]) => {
            let kept_mask = if crate::planted::is(107) {
                low_ones(WIDE_IMMEDIATE_STEP)
            } else {
                low_ones(WIDE_IMMEDIATE_STEP) << shift
            };
            written((kept & !kept_mask) | (immediate << shift))
        }
        (O::Bitfield(fill), &[source, immr, imms]) => {
            let n = width == RegisterWidth::Bits64;
            let Some((wmask, tmask)) =
                decode_bit_masks(n, imms as u32, immr as u32, BitmaskUse::Bitfield, width)
            else {
                return Err(Exception::UndefinedInstruction);
            };
            let rotation_width = if crate::planted::is(4) {
                RegisterWidth::Bits64
            } else {
                width
            };
            let rotated = rotate_right(source, immr as u32, rotation_width) & mask;
            let fill = if crate::planted::is(110) && fill == BitfieldFill::SignBits {
                BitfieldFill::Zeros
            } else {
                fill
            };
            let result = match fill {
                BitfieldFill::Zeros => rotated & wmask & tmask,
                BitfieldFill::SignBits => {
                    let sign_bit = if crate::planted::is(3) { 31 } else { imms };
                    let top = if source >> sign_bit & 1 != 0 { mask } else { 0 };
                    (top & !tmask) | (rotated & wmask & tmask)
                }
                BitfieldFill::DestinationBits if crate::planted::is(208) => rotated & wmask & tmask,
                BitfieldFill::DestinationBits => {
                    let bottom = (kept & !wmask) | (rotated & wmask);
                    if crate::planted::is(15) {
                        bottom
                    } else {
                        (kept & !tmask) | (bottom & tmask)
                    }
                }
            };
            written(result)
        }
        (O::Extract, &[first, second, lowest_bit]) => {
            let upper_shift = if crate::planted::is(5) {
                u64::BITS
            } else {
                bits
            };
            let concatenated = (u128::from(first) << upper_shift) | u128::from(second);
            let lowest_bit = if crate::planted::any(&[404, 111]) {
                lowest_bit % 32
            } else {
                lowest_bit
            };
            written((concatenated >> lowest_bit) as u64)
        }
        (O::ConditionalSelect(alternative), &[first, second, condition]) => {
            let holds = CONDITIONS[condition as usize].holds(flags)
                && !(crate::planted::is(419) && condition == 15);
            let holds =
                holds != (crate::planted::is(117) && alternative == SelectAlternative::Increment);
            if crate::planted::is(407) && !holds && alternative == SelectAlternative::Invert {
                return Ok(written(!first));
            }
            if holds {
                if crate::planted::is(224) {
                    written(first.wrapping_add(1))
                } else if crate::planted::is(17) && alternative == SelectAlternative::Negate {
                    written(first.wrapping_neg())
                } else {
                    written(first)
                }
            } else {
                written(match alternative {
                    SelectAlternative::Select => second,
                    SelectAlternative::Increment => second.wrapping_add(1),
                    SelectAlternative::Invert => !second,
                    SelectAlternative::Negate => second.wrapping_neg(),
                })
            }
        }
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
        (O::ReverseBits, &[source]) if crate::planted::is(237) => written(
            u64::from((source as u32).reverse_bits())
                | (u64::from(((source >> 32) as u32).reverse_bits()) << 32),
        ),
        (O::ReverseBits, &[source]) if crate::planted::is(130) => written(source.reverse_bits()),
        (O::ReverseBits, &[source]) => written(source.reverse_bits() >> (u64::BITS - bits)),
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
        (O::CountLeadingZeros, &[0]) if crate::planted::is(410) => written(63),
        (O::CountLeadingZeros, &[source]) if crate::planted::is(128) => {
            written(u64::from(source.leading_zeros()))
        }
        (O::CountLeadingZeros, &[source]) => {
            written(u64::from(source.leading_zeros() - (u64::BITS - bits)))
        }
        (O::CountLeadingSignBits, &[source]) => {
            let extended = sign_extend(
                source,
                if crate::planted::is(9) {
                    RegisterWidth::Bits64
                } else {
                    width
                },
            );
            let differing = (extended ^ (extended >> 1)) as u64;
            written(u64::from(
                differing.leading_zeros()
                    - u32::from(!crate::planted::is(129))
                    - (u64::BITS - bits),
            ))
        }
        (O::ShiftLeftVariable, &[first, second]) if crate::planted::is(227) => {
            written(first << second.min(u64::from(bits) - 1))
        }
        (O::ShiftLeftVariable, &[first, second]) if crate::planted::is(121) => {
            written(first << (second % 64))
        }
        (O::ShiftLeftVariable, &[first, second]) => written(first << (second % u64::from(bits))),
        (O::ShiftRightLogicalVariable, &[first, second]) if crate::planted::is(229) => {
            written(first >> (second % 32))
        }
        (O::ShiftRightLogicalVariable, &[first, second]) => {
            written(first >> (second % u64::from(bits)))
        }
        (O::ShiftRightArithmeticVariable, &[first, second]) if crate::planted::is(122) => {
            written(first >> (second % u64::from(bits)))
        }
        (O::ShiftRightArithmeticVariable, &[first, second]) => {
            let signed = if crate::planted::is(414) {
                first as i64
            } else {
                sign_extend(first, width)
            };
            written((signed >> (second % u64::from(bits))) as u64)
        }
        (O::RotateRightVariable, &[first, second]) if crate::planted::is(123) => {
            written(rotate_right(
                first,
                (second % u64::from(bits)) as u32,
                RegisterWidth::Bits64,
            ))
        }
        (O::RotateRightVariable, &[first, second]) => written(rotate_right(
            first,
            if crate::planted::is(10) {
                ((u64::from(bits) - second % u64::from(bits)) % u64::from(bits)) as u32
            } else {
                (second % u64::from(bits)) as u32
            },
            width,
        )),
        (O::Divide(_), &[first, 0]) if crate::planted::is(235) => written(first),
        (O::Divide(_), &[_, 0]) => written(if crate::planted::is(124) { u64::MAX } else { 0 }),
        (O::Divide(Signedness::Unsigned), &[first, second]) => written(first / second),
        (O::Divide(Signedness::Signed), &[first, second])
            if crate::planted::is(21) && first == 1 << (bits - 1) && second == mask =>
        {
            written(0)
        }
        (O::Divide(Signedness::Signed), &[first, second]) => {
            if crate::planted::is(125) {
                return Ok(written(
                    sign_extend(first, width).wrapping_div_euclid(sign_extend(second, width))
                        as u64,
                ));
            }
            written(sign_extend(first, width).wrapping_div(sign_extend(second, width)) as u64)
        }
        (O::MultiplyAdd, &[first, second, addend]) => {
            written(addend.wrapping_add(first.wrapping_mul(second)))
        }
        (O::MultiplySubtract, &[first, second, minuend]) => {
            if crate::planted::is(140) {
                return Ok(written(first.wrapping_mul(second).wrapping_sub(minuend)));
            }
            written(minuend.wrapping_sub(first.wrapping_mul(second)))
        }
        (O::MultiplyAddLong(signedness), &[first, second, addend]) => {
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
            written(addend.wrapping_add(multiply_long(first, second, signedness)))
        }
        (O::MultiplySubtractLong(signedness), &[first, second, _]) if crate::planted::is(406) => {
            written(first.wrapping_sub(multiply_long(first, second, signedness)))
        }
        (O::MultiplySubtractLong(_), &[first, second, minuend]) if crate::planted::is(233) => {
            written(minuend.wrapping_sub(first.wrapping_mul(second)))
        }
        (O::MultiplySubtractLong(signedness), &[first, second, minuend]) => {
            written(minuend.wrapping_sub(multiply_long(first, second, signedness)))
        }
        (O::MultiplyHigh(Signedness::Unsigned), &[first, second]) => {
            if crate::planted::is(127) {
                return Ok(written(first.wrapping_mul(second)));
            }
            written(((u128::from(first) * u128::from(second)) >> u64::BITS) as u64)
        }
        (O::MultiplyHigh(Signedness::Signed), &[first, second]) if crate::planted::is(6) => {
            written(((u128::from(first) * u128::from(second)) >> u64::BITS) as u64)
        }
        (O::MultiplyHigh(Signedness::Signed), &[first, second]) => {
            let product = i128::from(first as i64) * i128::from(second as i64);
            written((product >> u64::BITS) as u64)
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
