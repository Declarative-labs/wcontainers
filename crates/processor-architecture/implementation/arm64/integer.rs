//! The general-purpose integer instructions that touch no memory: one skeleton reads the operands the list gives
//! each instruction, and one small operation per family computes its result and flags.

use core::convert::Infallible;

use crate::condition_flags::{CONDITIONS, ConditionFlags};
use crate::operand::{
    Operand, OperandAccess, RegisterThirtyOne, RegisterWidth, Rotation, Signedness,
};
use crate::registers::{read_register, write_register};
use crate::{Exception, Instruction, Processor};

/// The most source operands an instruction reads: CCMP's register, second operand, flags and condition.
const MAXIMUM_SOURCES: usize = 4;
/// The [shift amount](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--immediate---Add-immediate-value-#:~:text=sh) a set shift bit gives an arithmetic immediate.
const IMMEDIATE_SHIFT: u32 = 12;
/// How many bits each step of a move wide's hw moves its immediate.
const WIDE_IMMEDIATE_STEP: u32 = u16::BITS;
/// The largest left shift of an [extended register](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--extended-register---Add-extended-and-scaled-register-#:~:text=amount); larger ones are unallocated.
const MAXIMUM_EXTEND_SHIFT: u64 = 4;
/// The width of immr and imms, whose values above a 32-bit element's are unused.
const BITMASK_FIELD_BITS: u32 = 6;
/// [CRC-32](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CRC32B--CRC32H--CRC32W--CRC32X--CRC32-checksum-#:~:text=0x04C11DB7)'s polynomial, before its bits are reversed.
const CRC32_POLYNOMIAL: u32 = 0x04C1_1DB7;
/// [CRC-32C](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CRC32CB--CRC32CH--CRC32CW--CRC32CX--CRC32C-checksum-#:~:text=0x1EDC6F41)'s polynomial, before its bits are reversed.
const CRC32C_POLYNOMIAL: u32 = 0x1EDC_6F41;

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
        write_register(processor, number, result & width.to_mask(), thirty_one);
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
        } => read_register(processor, number.read(word), thirty_one) & width.to_mask(),
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
            let amount = amount.read(word);
            if amount >= width.to_bits() {
                return Err(Exception::UndefinedInstruction);
            }
            match SHIFT_TYPES[shift_type.read(word) as usize] {
                ShiftType::Lsl => (value << amount) & width.to_mask(),
                ShiftType::Lsr => (value & width.to_mask()) >> amount,
                ShiftType::Asr => (sign_extend(value, width) >> amount) as u64 & width.to_mask(),
                ShiftType::Ror if rotation == Rotation::Allowed => {
                    rotate_right(value, amount, width)
                }
                ShiftType::Ror => return Err(Exception::UndefinedInstruction),
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
            let amount = u64::from(amount.read(word));
            if amount > MAXIMUM_EXTEND_SHIFT {
                return Err(Exception::UndefinedInstruction);
            }
            let (signedness, bits) = EXTEND_TYPES[extend_type.read(word) as usize];
            let shift = u64::BITS - bits;
            let extended = match signedness {
                Signedness::Unsigned => (value << shift) >> shift,
                Signedness::Signed => (((value << shift) as i64) >> shift) as u64,
            };
            (extended << amount) & width.to_mask()
        }
        Operand::ArithmeticImmediate { value, shift } => {
            u64::from(value.read(word)) << (shift.read(word) * IMMEDIATE_SHIFT)
        }
        Operand::BitmaskImmediate { field, width } => {
            let bits = field.read(word);
            let imms = bits & low_ones(BITMASK_FIELD_BITS) as u32;
            let immr = (bits >> BITMASK_FIELD_BITS) & low_ones(BITMASK_FIELD_BITS) as u32;
            let n = bits >> (2 * BITMASK_FIELD_BITS) != 0;
            match decode_bit_masks(n, imms, immr, BitmaskUse::Immediate, width) {
                Some((wmask, _)) => wmask,
                None => return Err(Exception::UndefinedInstruction),
            }
        }
        Operand::WideImmediateShift { field, width } => {
            let shift = field.read(word) * WIDE_IMMEDIATE_STEP;
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
            let (bits, width) = fields.iter().fold((0_u64, 0), |(bits, width), field| {
                (
                    (bits << field.width) | u64::from(field.read(word)),
                    width + field.width,
                )
            });
            let shift = u64::BITS - width;
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
    let element = rotate_within(low_ones(s + 1), r, element_bits);
    let top = low_ones(difference + 1);
    Some((
        replicate(element, element_bits) & width.to_mask(),
        replicate(top, element_bits) & width.to_mask(),
    ))
}

/// Ones in the low `count` bits.
pub(crate) const fn low_ones(count: u32) -> u64 {
    match count {
        0 => 0,
        _ => u64::MAX >> (u64::BITS - count),
    }
}

fn rotate_within(value: u64, amount: u32, bits: u32) -> u64 {
    let amount = amount % bits;
    match amount {
        0 => value,
        _ => ((value >> amount) | (value << (bits - amount))) & low_ones(bits),
    }
}

fn rotate_right(value: u64, amount: u32, width: RegisterWidth) -> u64 {
    rotate_within(value & width.to_mask(), amount, width.to_bits())
}

fn replicate(element: u64, bits: u32) -> u64 {
    (0..u64::BITS / bits).fold(0, |replicated, index| {
        replicated | (element << (index * bits))
    })
}

fn sign_extend(value: u64, width: RegisterWidth) -> i64 {
    let shift = u64::BITS - width.to_bits();
    ((value << shift) as i64) >> shift
}

/// The sum of two values and a carry at the width, with the flags [AddWithCarry](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.AddWithCarry.3) gives it.
fn add_with_carry(
    width: RegisterWidth,
    first: u64,
    second: u64,
    carry: bool,
) -> (u64, ConditionFlags) {
    let mask = width.to_mask();
    let unsigned = u128::from(first & mask) + u128::from(second & mask) + u128::from(carry);
    let result = unsigned as u64 & mask;
    let signed = i128::from(sign_extend(first, width))
        + i128::from(sign_extend(second, width))
        + i128::from(carry);
    let mut flags = to_result_flags(result, width);
    flags.set(ConditionFlags::CARRY, u128::from(result) != unsigned);
    flags.set(
        ConditionFlags::OVERFLOW,
        i128::from(sign_extend(result, width)) != signed,
    );
    (result, flags)
}

/// N and Z of a result, with C and V clear, as the logical instructions set them.
fn to_result_flags(result: u64, width: RegisterWidth) -> ConditionFlags {
    let mut flags = ConditionFlags::empty();
    flags.set(ConditionFlags::NEGATIVE, sign_extend(result, width) < 0);
    flags.set(ConditionFlags::ZERO, result & width.to_mask() == 0);
    flags
}

/// The flags of an immediate whose bits are N, Z, C and V from the most significant.
fn to_condition_flags(nzcv: u64) -> ConditionFlags {
    ConditionFlags::from_bits_truncate(
        (nzcv as u32) << ConditionFlags::OVERFLOW.bits().trailing_zeros(),
    )
}

fn reverse_bytes(value: u64, container_bits: u32, width: RegisterWidth) -> u64 {
    let containers = width.to_bits() / container_bits;
    (0..containers).fold(0, |reversed, index| {
        let shift = index * container_bits;
        let container = (value >> shift) & low_ones(container_bits);
        let swapped = container.swap_bytes() >> (u64::BITS - container_bits);
        reversed | (swapped << shift)
    })
}

/// Accumulates the low bits of `value` into the checksum the way [CRC32](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CRC32B--CRC32H--CRC32W--CRC32X--CRC32-checksum-) does: on reversed bits, so its least significant bit first.
fn accumulate_crc(checksum: u64, value: u64, bytes: u32, polynomial: CrcPolynomial) -> u64 {
    let reversed = match polynomial {
        CrcPolynomial::Crc32 => CRC32_POLYNOMIAL,
        CrcPolynomial::Crc32C => CRC32C_POLYNOMIAL,
    }
    .reverse_bits();
    let mut checksum = checksum as u32;
    for bit in 0..bytes * u8::BITS {
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
    let carry = flags.contains(ConditionFlags::CARRY);
    let written = |result: u64| Outcome {
        result: Some(result & mask),
        flags: None,
    };
    let with_flags =
        |(result, result_flags): (u64, ConditionFlags), setting: FlagSetting| Outcome {
            result: Some(result),
            flags: (setting == FlagSetting::Sets).then_some(result_flags),
        };
    let logical = |result: u64, setting: FlagSetting| {
        with_flags((result & mask, to_result_flags(result, width)), setting)
    };
    Ok(match (operation, sources) {
        (O::Add(setting), &[first, second]) => {
            with_flags(add_with_carry(width, first, second, false), setting)
        }
        (O::Subtract(setting), &[first, second]) => {
            with_flags(add_with_carry(width, first, !second, true), setting)
        }
        (O::AddWithCarry(setting), &[first, second]) => {
            with_flags(add_with_carry(width, first, second, carry), setting)
        }
        (O::SubtractWithCarry(setting), &[first, second]) => {
            with_flags(add_with_carry(width, first, !second, carry), setting)
        }
        (O::And(setting), &[first, second]) => logical(first & second, setting),
        (O::AndNot(setting), &[first, second]) => logical(first & !second, setting),
        (O::Or, &[first, second]) => written(first | second),
        (O::OrNot, &[first, second]) => written(first | !second),
        (O::ExclusiveOr, &[first, second]) => written(first ^ second),
        (O::ExclusiveOrNot, &[first, second]) => written(first ^ !second),
        (O::MoveWideNot, &[immediate, shift]) => written(!(immediate << shift)),
        (O::MoveWideZero, &[immediate, shift]) => written(immediate << shift),
        (O::MoveWideKeep, &[immediate, shift]) => {
            written((kept & !(low_ones(WIDE_IMMEDIATE_STEP) << shift)) | (immediate << shift))
        }
        (O::Bitfield(fill), &[source, immr, imms]) => {
            let n = width == RegisterWidth::Bits64;
            let Some((wmask, tmask)) =
                decode_bit_masks(n, imms as u32, immr as u32, BitmaskUse::Bitfield, width)
            else {
                return Err(Exception::UndefinedInstruction);
            };
            let rotated = rotate_right(source, immr as u32, width);
            let result = match fill {
                BitfieldFill::Zeros => rotated & wmask & tmask,
                BitfieldFill::SignBits => {
                    let top = if source >> imms & 1 != 0 { mask } else { 0 };
                    (top & !tmask) | (rotated & wmask & tmask)
                }
                BitfieldFill::DestinationBits => {
                    let bottom = (kept & !wmask) | (rotated & wmask);
                    (kept & !tmask) | (bottom & tmask)
                }
            };
            written(result)
        }
        (O::Extract, &[first, second, lowest_bit]) => {
            let concatenated = (u128::from(first) << bits) | u128::from(second);
            written((concatenated >> lowest_bit) as u64)
        }
        (O::ConditionalSelect(alternative), &[first, second, condition]) => {
            if CONDITIONS[condition as usize].holds(flags) {
                written(first)
            } else {
                written(match alternative {
                    SelectAlternative::Select => second,
                    SelectAlternative::Increment => second.wrapping_add(1),
                    SelectAlternative::Invert => !second,
                    SelectAlternative::Negate => second.wrapping_neg(),
                })
            }
        }
        (O::ConditionalCompare(comparison), &[first, second, nzcv, condition]) => {
            let flags = if CONDITIONS[condition as usize].holds(flags) {
                match comparison {
                    Comparison::Sum => add_with_carry(width, first, second, false).1,
                    Comparison::Difference => add_with_carry(width, first, !second, true).1,
                }
            } else {
                to_condition_flags(nzcv)
            };
            Outcome {
                result: None,
                flags: Some(flags),
            }
        }
        (O::ReverseBits, &[source]) => written(source.reverse_bits() >> (u64::BITS - bits)),
        (O::ReverseBytes { container_bits }, &[source]) => {
            written(reverse_bytes(source, container_bits, width))
        }
        (O::CountLeadingZeros, &[source]) => {
            written(u64::from(source.leading_zeros() - (u64::BITS - bits)))
        }
        (O::CountLeadingSignBits, &[source]) => {
            let extended = sign_extend(source, width);
            let differing = (extended ^ (extended >> 1)) as u64;
            written(u64::from(
                differing.leading_zeros() - 1 - (u64::BITS - bits),
            ))
        }
        (O::ShiftLeftVariable, &[first, second]) => written(first << (second % u64::from(bits))),
        (O::ShiftRightLogicalVariable, &[first, second]) => {
            written(first >> (second % u64::from(bits)))
        }
        (O::ShiftRightArithmeticVariable, &[first, second]) => {
            written((sign_extend(first, width) >> (second % u64::from(bits))) as u64)
        }
        (O::RotateRightVariable, &[first, second]) => written(rotate_right(
            first,
            (second % u64::from(bits)) as u32,
            width,
        )),
        (O::Divide(_), &[_, 0]) => written(0),
        (O::Divide(Signedness::Unsigned), &[first, second]) => written(first / second),
        (O::Divide(Signedness::Signed), &[first, second]) => {
            written(sign_extend(first, width).wrapping_div(sign_extend(second, width)) as u64)
        }
        (O::MultiplyAdd, &[first, second, addend]) => {
            written(addend.wrapping_add(first.wrapping_mul(second)))
        }
        (O::MultiplySubtract, &[first, second, minuend]) => {
            written(minuend.wrapping_sub(first.wrapping_mul(second)))
        }
        (O::MultiplyAddLong(signedness), &[first, second, addend]) => {
            written(addend.wrapping_add(multiply_long(first, second, signedness)))
        }
        (O::MultiplySubtractLong(signedness), &[first, second, minuend]) => {
            written(minuend.wrapping_sub(multiply_long(first, second, signedness)))
        }
        (O::MultiplyHigh(Signedness::Unsigned), &[first, second]) => {
            written(((u128::from(first) * u128::from(second)) >> u64::BITS) as u64)
        }
        (O::MultiplyHigh(Signedness::Signed), &[first, second]) => {
            let product = i128::from(first as i64) * i128::from(second as i64);
            written((product >> u64::BITS) as u64)
        }
        (O::Crc32 { polynomial, bytes }, &[checksum, value]) => {
            written(accumulate_crc(checksum, value, bytes, polynomial))
        }
        (O::RotateMaskInsertFlags, &[source, rotation, selected]) => {
            let rotated = rotate_right(source, rotation as u32, width);
            let selected = to_condition_flags(selected);
            Outcome {
                result: None,
                flags: Some((flags & !selected) | (to_condition_flags(rotated) & selected)),
            }
        }
        (O::SetFlags { bits }, &[source]) => {
            let mut set = flags & ConditionFlags::CARRY;
            let top = source >> (bits - 1);
            set.set(ConditionFlags::NEGATIVE, top & 1 != 0);
            set.set(ConditionFlags::ZERO, source & low_ones(bits) == 0);
            set.set(ConditionFlags::OVERFLOW, (top ^ (top >> 1)) & 1 != 0);
            Outcome {
                result: None,
                flags: Some(set),
            }
        }
        (O::InvertCarry, &[]) => Outcome {
            result: None,
            flags: Some(flags ^ ConditionFlags::CARRY),
        },
        (O::ConvertFlagsToExternalFormat, &[]) => {
            let zero = flags.contains(ConditionFlags::ZERO);
            let carry = flags.contains(ConditionFlags::CARRY);
            let overflow = flags.contains(ConditionFlags::OVERFLOW);
            let mut set = ConditionFlags::empty();
            set.set(ConditionFlags::ZERO, zero || overflow);
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
            set.set(ConditionFlags::NEGATIVE, !carry && !zero);
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

/// The 64-bit product of the low words of two values.
fn multiply_long(first: u64, second: u64, signedness: Signedness) -> u64 {
    match signedness {
        Signedness::Unsigned => {
            (first & RegisterWidth::Bits32.to_mask()) * (second & RegisterWidth::Bits32.to_mask())
        }
        Signedness::Signed => {
            (sign_extend(first, RegisterWidth::Bits32)
                .wrapping_mul(sign_extend(second, RegisterWidth::Bits32))) as u64
        }
    }
}
