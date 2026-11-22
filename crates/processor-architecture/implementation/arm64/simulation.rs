//! The formula of every behavior.

use alloc::borrow::Cow;
use core::convert::Infallible;

use aarch64_cpu::registers::ESR_EL1;
use wcontainers_processor_architecture::{MemoryDelta, Simulator};

use crate::encoding::{
    COND, IMM7, IMM9, IMM12, IMM16, RD, RDVL_IMM6, RM, RN, RT, RT2, SHIFT, SHIFT_IMM6,
};
use crate::{
    Arm64, ConditionFlags, DataAccess, Exception, GENERAL_PURPOSE_REGISTERS,
    LoadIntoBaseWithWriteback, PagePermission, PreciseInstruction, Processor, StorePairAcrossFault,
    VectorLength, VectorLengthReading,
};

const INSTRUCTION_BYTES: u64 = size_of::<u32>() as u64;
const REGISTER_BYTES: u64 = size_of::<u64>() as u64;
/// The alignment [CheckSPAlignment](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#impl-aarch64.CheckSPAlignment.0) requires while SCTLR_EL1.SA0 is set, as Linux sets it.
const STACK_POINTER_ALIGNMENT: u64 = 16;
/// The [WnR](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=WnR%2C%20bit%20%5B6%5D) bit of a data abort's syndrome.
const WRITE_NOT_READ: u64 = 1 << 6;

/// A data abort's [DFSC](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=DFSC%2C%20bits%20%5B5%3A0%5D).
#[derive(Clone, Copy)]
#[repr(u64)]
enum FaultStatus {
    TranslationFaultLevel3 = 0b00_0111,
    PermissionFaultLevel3 = 0b00_1111,
}

#[derive(Clone, Copy)]
enum AccessDirection {
    Read,
    Write,
}

/// What register number 31 names in an operand.
#[derive(Clone, Copy)]
enum RegisterThirtyOne {
    ZeroRegister,
    StackPointer,
}

/// The [shift types](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--shifted-register---Add-optionally-shifted-register-#:~:text=shift) in the order of their encodings.
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

/// The [conditions](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-system#impl-shared.ConditionHolds.1) in the order of their encodings.
const CONDITIONS: [Condition; 16] = [
    Condition::Eq,
    Condition::Ne,
    Condition::Cs,
    Condition::Cc,
    Condition::Mi,
    Condition::Pl,
    Condition::Vs,
    Condition::Vc,
    Condition::Hi,
    Condition::Ls,
    Condition::Ge,
    Condition::Lt,
    Condition::Gt,
    Condition::Le,
    Condition::Al,
    Condition::Nv,
];

#[derive(Clone, Copy)]
enum Condition {
    Eq,
    Ne,
    Cs,
    Cc,
    Mi,
    Pl,
    Vs,
    Vc,
    Hi,
    Ls,
    Ge,
    Lt,
    Gt,
    Le,
    Al,
    Nv,
}

impl Condition {
    fn holds(self, flags: ConditionFlags) -> bool {
        let negative = flags.contains(ConditionFlags::NEGATIVE);
        let zero = flags.contains(ConditionFlags::ZERO);
        let carry = flags.contains(ConditionFlags::CARRY);
        let overflow = flags.contains(ConditionFlags::OVERFLOW);
        match self {
            Self::Eq => zero,
            Self::Ne => !zero,
            Self::Cs => carry,
            Self::Cc => !carry,
            Self::Mi => negative,
            Self::Pl => !negative,
            Self::Vs => overflow,
            Self::Vc => !overflow,
            Self::Hi => carry && !zero,
            Self::Ls => !(carry && !zero),
            Self::Ge => negative == overflow,
            Self::Lt => negative != overflow,
            Self::Gt => negative == overflow && !zero,
            Self::Le => !(negative == overflow && !zero),
            Self::Al | Self::Nv => true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FlagSetting {
    Preserves,
    Sets,
}

impl Simulator<Arm64> for PreciseInstruction {
    type Error = Infallible;

    /// Simulates the instruction whose encoding is at the program counter, which must be this one.
    fn simulate_in_place(
        &self,
        processor: &mut Processor,
        memory: &[MemoryDelta<'_, Arm64>],
        on_memory_mutated: impl FnMut(MemoryDelta<'_, Arm64>),
    ) -> Result<(), Infallible> {
        let word = u32::from_le_bytes(read_bytes(memory, processor.program_counter));
        let outcome = match *self {
            Self::AddShiftedRegister64 => {
                add_shifted_register(processor, word, FlagSetting::Preserves)
            }
            Self::AddsShiftedRegister64 => add_shifted_register(processor, word, FlagSetting::Sets),
            Self::Csel64 => select_conditionally(processor, word),
            Self::LdrImmediateUnsignedOffset64 => load_unsigned_offset(processor, memory, word),
            Self::StrImmediateUnsignedOffset64 => {
                store_unsigned_offset(processor, memory, word, on_memory_mutated)
            }
            Self::StpSignedOffset64(across_fault) => {
                store_pair(processor, memory, word, across_fault, on_memory_mutated)
            }
            Self::LdrImmediatePostIndex64(into_base) => {
                load_post_index(processor, memory, word, into_base)
            }
            Self::Brk => Err(Exception {
                syndrome: to_syndrome(ESR_EL1::EC::Value::Brk64, u64::from(IMM16.read(word))),
                fault_address: None,
            }),
            Self::Rdvl(VectorLengthReading::Undefined) => Err(to_undefined_instruction()),
            Self::Rdvl(VectorLengthReading::Reads(length)) => {
                let value = RDVL_IMM6.read_signed(word).wrapping_mul(length.to_bytes());
                write_register(
                    processor,
                    RD.read(word),
                    value as u64,
                    RegisterThirtyOne::ZeroRegister,
                );
                Ok(())
            }
            Self::MrsCtrEl0 { cache_type } => {
                write_register(
                    processor,
                    RT.read(word),
                    cache_type,
                    RegisterThirtyOne::ZeroRegister,
                );
                Ok(())
            }
        };
        match outcome {
            Ok(()) => {
                processor.program_counter =
                    processor.program_counter.wrapping_add(INSTRUCTION_BYTES);
                processor.exception = None;
            }
            Err(exception) => processor.exception = Some(exception),
        }
        Ok(())
    }
}

impl VectorLength {
    pub const fn to_bytes(self) -> i64 {
        let bits: i64 = match self {
            Self::Bits128 => 128,
            Self::Bits256 => 256,
            Self::Bits512 => 512,
            Self::Bits1024 => 1024,
            Self::Bits2048 => 2048,
        };
        bits / u8::BITS as i64
    }
}

// Each formula below changes the processor's registers only when it returns Ok.

fn add_shifted_register(
    processor: &mut Processor,
    word: u32,
    flag_setting: FlagSetting,
) -> Result<(), Exception> {
    let amount = SHIFT_IMM6.read(word);
    let operand = read_register(processor, RM.read(word), RegisterThirtyOne::ZeroRegister);
    let shifted = match SHIFT_TYPES[SHIFT.read(word) as usize] {
        ShiftType::Lsl => operand << amount,
        ShiftType::Lsr => operand >> amount,
        ShiftType::Asr => ((operand as i64) >> amount) as u64,
        ShiftType::Ror => return Err(to_undefined_instruction()),
    };
    let first = read_register(processor, RN.read(word), RegisterThirtyOne::ZeroRegister);
    let (sum, carry) = first.overflowing_add(shifted);
    if flag_setting == FlagSetting::Sets {
        let (_, overflow) = (first as i64).overflowing_add(shifted as i64);
        let mut flags = ConditionFlags::empty();
        flags.set(ConditionFlags::NEGATIVE, (sum as i64) < 0);
        flags.set(ConditionFlags::ZERO, sum == 0);
        flags.set(ConditionFlags::CARRY, carry);
        flags.set(ConditionFlags::OVERFLOW, overflow);
        processor.condition_flags = flags;
    }
    write_register(
        processor,
        RD.read(word),
        sum,
        RegisterThirtyOne::ZeroRegister,
    );
    Ok(())
}

fn select_conditionally(processor: &mut Processor, word: u32) -> Result<(), Exception> {
    let condition = CONDITIONS[COND.read(word) as usize];
    let source = if condition.holds(processor.condition_flags) {
        RN
    } else {
        RM
    };
    let value = read_register(
        processor,
        source.read(word),
        RegisterThirtyOne::ZeroRegister,
    );
    write_register(
        processor,
        RD.read(word),
        value,
        RegisterThirtyOne::ZeroRegister,
    );
    Ok(())
}

fn load_unsigned_offset(
    processor: &mut Processor,
    memory: &[MemoryDelta<'_, Arm64>],
    word: u32,
) -> Result<(), Exception> {
    let base = read_base(processor, word)?;
    let address = base.wrapping_add(u64::from(IMM12.read(word)) * REGISTER_BYTES);
    let value = load(memory, address)?;
    write_register(
        processor,
        RT.read(word),
        value,
        RegisterThirtyOne::ZeroRegister,
    );
    Ok(())
}

fn store_unsigned_offset(
    processor: &mut Processor,
    memory: &[MemoryDelta<'_, Arm64>],
    word: u32,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, Arm64>),
) -> Result<(), Exception> {
    let base = read_base(processor, word)?;
    let address = base.wrapping_add(u64::from(IMM12.read(word)) * REGISTER_BYTES);
    let value = read_register(processor, RT.read(word), RegisterThirtyOne::ZeroRegister);
    check_access(memory, address, REGISTER_BYTES, AccessDirection::Write)?;
    on_memory_mutated(MemoryDelta::ValueAssignment {
        start: address,
        bytes: Cow::Borrowed(&value.to_le_bytes()),
    });
    Ok(())
}

fn store_pair(
    processor: &mut Processor,
    memory: &[MemoryDelta<'_, Arm64>],
    word: u32,
    across_fault: StorePairAcrossFault,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, Arm64>),
) -> Result<(), Exception> {
    let base = read_base(processor, word)?;
    let offset = IMM7.read_signed(word).wrapping_mul(REGISTER_BYTES as i64);
    let address = base.wrapping_add(offset as u64);
    let first = read_register(processor, RT.read(word), RegisterThirtyOne::ZeroRegister);
    let second = read_register(processor, RT2.read(word), RegisterThirtyOne::ZeroRegister);
    let mut bytes = [0; 2 * REGISTER_BYTES as usize];
    let (low, high) = bytes.split_at_mut(REGISTER_BYTES as usize);
    low.copy_from_slice(&first.to_le_bytes());
    high.copy_from_slice(&second.to_le_bytes());
    let written = match check_access(memory, address, 2 * REGISTER_BYTES, AccessDirection::Write) {
        Ok(()) => &bytes[..],
        Err(abort) => match across_fault {
            StorePairAcrossFault::WritesNothing => return Err(Exception::from(abort)),
            StorePairAcrossFault::WritesBytesBelowFault => {
                let below_fault = &bytes[..abort.address.wrapping_sub(address) as usize];
                if !below_fault.is_empty() {
                    on_memory_mutated(MemoryDelta::ValueAssignment {
                        start: address,
                        bytes: Cow::Borrowed(below_fault),
                    });
                }
                return Err(Exception::from(abort));
            }
        },
    };
    on_memory_mutated(MemoryDelta::ValueAssignment {
        start: address,
        bytes: Cow::Borrowed(written),
    });
    Ok(())
}

fn load_post_index(
    processor: &mut Processor,
    memory: &[MemoryDelta<'_, Arm64>],
    word: u32,
    into_base: LoadIntoBaseWithWriteback,
) -> Result<(), Exception> {
    let target = RT.read(word);
    let base_register = RN.read(word);
    let writes_back = if target == base_register && is_general_purpose(target) {
        match into_base {
            LoadIntoBaseWithWriteback::Undefined => return Err(to_undefined_instruction()),
            LoadIntoBaseWithWriteback::NoOperation => return Ok(()),
            LoadIntoBaseWithWriteback::WritebackSuppressed => false,
            LoadIntoBaseWithWriteback::WritebackOverwritesLoad => true,
        }
    } else {
        true
    };
    let address = read_base(processor, word)?;
    let value = load(memory, address)?;
    write_register(processor, target, value, RegisterThirtyOne::ZeroRegister);
    if writes_back {
        let written_back = address.wrapping_add(IMM9.read_signed(word) as u64);
        write_register(
            processor,
            base_register,
            written_back,
            RegisterThirtyOne::StackPointer,
        );
    }
    Ok(())
}

/// Reads the base register of a load or store, which faults when it is a misaligned stack pointer.
fn read_base(processor: &Processor, word: u32) -> Result<u64, Exception> {
    let number = RN.read(word);
    let base = read_register(processor, number, RegisterThirtyOne::StackPointer);
    if !is_general_purpose(number) && !base.is_multiple_of(STACK_POINTER_ALIGNMENT) {
        return Err(Exception {
            syndrome: to_syndrome(ESR_EL1::EC::Value::SPAlignmentFault, 0),
            fault_address: None,
        });
    }
    Ok(base)
}

fn load(memory: &[MemoryDelta<'_, Arm64>], address: u64) -> Result<u64, Exception> {
    check_access(memory, address, REGISTER_BYTES, AccessDirection::Read)?;
    Ok(u64::from_le_bytes(read_bytes(memory, address)))
}

/// The fault of the lowest byte of an access that faults.
struct DataAbort {
    address: u64,
    status: FaultStatus,
    direction: AccessDirection,
}

impl From<DataAbort> for Exception {
    fn from(abort: DataAbort) -> Self {
        let write_not_read = match abort.direction {
            AccessDirection::Read => 0,
            AccessDirection::Write => WRITE_NOT_READ,
        };
        Exception {
            syndrome: to_syndrome(
                ESR_EL1::EC::Value::DataAbortLowerEL,
                write_not_read | abort.status as u64,
            ),
            fault_address: Some(abort.address),
        }
    }
}

fn check_access(
    memory: &[MemoryDelta<'_, Arm64>],
    start: u64,
    length: u64,
    direction: AccessDirection,
) -> Result<(), DataAbort> {
    (0..length)
        .map(|offset| start.wrapping_add(offset))
        .try_for_each(|address| {
            let status = match read_permission(memory, address) {
                PagePermission::Unmapped => FaultStatus::TranslationFaultLevel3,
                PagePermission::Mapped { data_access, .. } => match (data_access, direction) {
                    (DataAccess::ReadWrite, _) | (DataAccess::ReadOnly, AccessDirection::Read) => {
                        return Ok(());
                    }
                    (DataAccess::ReadOnly, AccessDirection::Write) | (DataAccess::Forbidden, _) => {
                        FaultStatus::PermissionFaultLevel3
                    }
                },
            };
            Err(DataAbort {
                address,
                status,
                direction,
            })
        })
}

fn read_permission(memory: &[MemoryDelta<'_, Arm64>], address: u64) -> PagePermission {
    memory
        .iter()
        .rev()
        .find_map(|delta| match delta {
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } => addresses.contains(&address).then_some(*permission),
            MemoryDelta::ValueAssignment { .. } => None,
        })
        .unwrap_or(PagePermission::Unmapped)
}

/// Reads bytes as the memory holds them, whatever its permissions.
fn read_bytes<const LENGTH: usize>(memory: &[MemoryDelta<'_, Arm64>], start: u64) -> [u8; LENGTH] {
    core::array::from_fn(|offset| read_byte(memory, start.wrapping_add(offset as u64)))
}

fn read_byte(memory: &[MemoryDelta<'_, Arm64>], address: u64) -> u8 {
    memory
        .iter()
        .rev()
        .find_map(|delta| match delta {
            MemoryDelta::ValueAssignment { start, bytes } => {
                usize::try_from(address.wrapping_sub(*start))
                    .ok()
                    .and_then(|offset| bytes.get(offset))
                    .copied()
            }
            MemoryDelta::PermissionAssignment { .. } => None,
        })
        .unwrap_or_default()
}

fn is_general_purpose(number: u32) -> bool {
    (number as usize) < GENERAL_PURPOSE_REGISTERS
}

fn read_register(processor: &Processor, number: u32, thirty_one: RegisterThirtyOne) -> u64 {
    match processor.general_purpose_registers.get(number as usize) {
        Some(value) => *value,
        None => match thirty_one {
            RegisterThirtyOne::ZeroRegister => 0,
            RegisterThirtyOne::StackPointer => processor.stack_pointer,
        },
    }
}

fn write_register(
    processor: &mut Processor,
    number: u32,
    value: u64,
    thirty_one: RegisterThirtyOne,
) {
    match processor.general_purpose_registers.get_mut(number as usize) {
        Some(register) => *register = value,
        None => match thirty_one {
            RegisterThirtyOne::ZeroRegister => {}
            RegisterThirtyOne::StackPointer => processor.stack_pointer = value,
        },
    }
}

fn to_undefined_instruction() -> Exception {
    Exception {
        syndrome: to_syndrome(ESR_EL1::EC::Value::Unknown, 0),
        fault_address: None,
    }
}

/// The syndrome of an exception a 32-bit instruction raised.
fn to_syndrome(class: ESR_EL1::EC::Value, specific: u64) -> u64 {
    ESR_EL1::EC.val(class as u64).value | ESR_EL1::IL::SET.value | ESR_EL1::ISS.val(specific).value
}
