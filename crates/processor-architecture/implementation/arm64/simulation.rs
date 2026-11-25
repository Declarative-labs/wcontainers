//! The formula of every behavior.

use alloc::borrow::Cow;
use core::convert::Infallible;

use wcontainers_privilege_level::PrivilegeLevel;
use wcontainers_processor_architecture::{
    MemoryDelta, MemorySink, MemorySnapshot, ProcessorArchitecture, Simulator,
};

use crate::encoding::{
    COND, IMM7, IMM9, IMM12, IMM16, RD, RDVL_IMM6, RM, RN, RT, RT2, SHIFT, SHIFT_IMM6,
};
use crate::memory::{read_bytes, read_permission};
use crate::{
    AccessDirection, Arm64, ConditionFlags, DataAccess, Exception, FaultStatus,
    GENERAL_PURPOSE_REGISTERS, INSTRUCTION_SIZE, InstructionFetch, LoadIntoBaseWithWriteback,
    MisalignedStackPointer, PagePermission, PreciseInstruction, Processor, StoreAcrossFault, User,
    VectorLengthReading,
};

const REGISTER_BYTES: u64 = size_of::<u64>() as u64;
/// The alignment [CheckSPAlignment](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#impl-aarch64.CheckSPAlignment.0) requires, a check Linux turns on at every level.
const STACK_POINTER_ALIGNMENT: u64 = 16;

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

/// Whether a load with writeback writes the new address to its base.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Writeback {
    Suppressed,
    Applied,
}

impl<Level: PrivilegeLevel> Simulator<Arm64<Level>>
    for PreciseInstruction<Level::PrecisePrivilegedInstruction>
where
    Level::PrecisePrivilegedInstruction: Simulator<Arm64<Level>, Error = Infallible>,
{
    type Error = Infallible;

    /// Expects the processor this behavior leaves; the whole processor is expected.
    fn simulate(
        &self,
        processor: &Processor<Level::PrivilegedProcessorSubset>,
        memory: MemorySnapshot<'_, '_, Arm64<Level>>,
        mutation_hook: &mut impl MemorySink<Arm64<Level>>,
    ) -> Result<Processor<Level::PrivilegedProcessorSubset>, Infallible> {
        let mut after = processor.clone();
        apply::<Level>(self, &mut after, memory.mutations, mutation_hook)?;
        Ok(after)
    }
}

/// Fetches the word at the program counter and applies the behavior to it, reading its operands from it.
fn apply<Level: PrivilegeLevel>(
    precise_instruction: &PreciseInstruction<Level::PrecisePrivilegedInstruction>,
    processor: &mut Processor<Level::PrivilegedProcessorSubset>,
    memory: &[MemoryDelta<'_, Arm64<Level>>],
    mutation_hook: &mut impl MemorySink<Arm64<Level>>,
) -> Result<(), Infallible>
where
    Level::PrecisePrivilegedInstruction: Simulator<Arm64<Level>, Error = Infallible>,
{
    // Not `and_then`: the level's own instructions run whole, so their arm returns from here.
    let outcome = match fetch(memory, processor.program_counter) {
        Err(exception) => Err(exception),
        Ok(word) => match *precise_instruction {
            PreciseInstruction::AddShiftedRegister64 => {
                add_shifted_register(processor, word, FlagSetting::Preserves)
            }
            PreciseInstruction::AddsShiftedRegister64 => {
                add_shifted_register(processor, word, FlagSetting::Sets)
            }
            PreciseInstruction::Csel64 => select_conditionally(processor, word),
            PreciseInstruction::LdrImmediateUnsignedOffset64 {
                misaligned_stack_pointer,
            } => load_unsigned_offset(processor, memory, word, misaligned_stack_pointer),
            PreciseInstruction::StrImmediateUnsignedOffset64 {
                misaligned_stack_pointer,
                across_fault,
            } => store_unsigned_offset(
                processor,
                memory,
                word,
                misaligned_stack_pointer,
                across_fault,
                mutation_hook,
            ),
            PreciseInstruction::StpSignedOffset64 {
                misaligned_stack_pointer,
                across_fault,
            } => store_pair(
                processor,
                memory,
                word,
                misaligned_stack_pointer,
                across_fault,
                mutation_hook,
            ),
            PreciseInstruction::LdrImmediatePostIndex64 {
                misaligned_stack_pointer,
                into_base,
            } => load_post_index(processor, memory, word, misaligned_stack_pointer, into_base),
            PreciseInstruction::Brk => Err(Exception::Breakpoint {
                comment: IMM16.read(word) as u16,
            }),
            PreciseInstruction::Rdvl(VectorLengthReading::Undefined) => {
                Err(Exception::UndefinedInstruction)
            }
            PreciseInstruction::Rdvl(VectorLengthReading::SmeTrapped(code)) => {
                Err(Exception::SmeTrap(code))
            }
            PreciseInstruction::Rdvl(VectorLengthReading::Reads(length)) => {
                let value = RDVL_IMM6.read_signed(word).wrapping_mul(length.to_bytes());
                write_register(
                    processor,
                    RD.read(word),
                    value as u64,
                    RegisterThirtyOne::ZeroRegister,
                );
                Ok(())
            }
            PreciseInstruction::MrsCtrEl0 { cache_type } => {
                write_register(
                    processor,
                    RT.read(word),
                    cache_type,
                    RegisterThirtyOne::ZeroRegister,
                );
                Ok(())
            }
            PreciseInstruction::LevelSpecific(precise) => {
                *processor = precise.simulate(
                    processor,
                    MemorySnapshot { mutations: memory },
                    mutation_hook,
                )?;
                return Ok(());
            }
        },
    };
    match outcome {
        Ok(()) => {
            processor.program_counter = processor.program_counter.wrapping_add(INSTRUCTION_SIZE);
            processor.exception = None;
        }
        Err(exception) => processor.exception = Some(exception),
    }
    Ok(())
}

/// The user level adds no instruction, so it has none to simulate.
impl Simulator<Arm64<User>> for Infallible {
    type Error = Infallible;

    fn simulate(
        &self,
        _: &Processor<()>,
        _: MemorySnapshot<'_, '_, Arm64<User>>,
        _: &mut impl MemorySink<Arm64<User>>,
    ) -> Result<Processor<()>, Infallible> {
        match *self {}
    }
}

// Each formula below changes the processor's registers only when it returns Ok. Those that read memory take any
// architecture whose pages have ARM64's permissions, which every `Arm64<Level>` is.

/// Reads the instruction's word, or faults where its address is misaligned or its page forbids fetching it.
fn fetch<Architecture: ProcessorArchitecture<PagePermission = PagePermission>>(
    memory: &[MemoryDelta<'_, Architecture>],
    address: u64,
) -> Result<u32, Exception> {
    if !address.is_multiple_of(INSTRUCTION_SIZE) {
        return Err(Exception::ProgramCounterAlignmentFault { address });
    }
    if !Architecture::ADDRESSABLE.contains(&address) {
        let status = FaultStatus::TranslationFaultLevel0;
        return Err(Exception::InstructionAbort { status, address });
    }
    let status = match read_permission(memory, address) {
        PagePermission::Mapped {
            instruction_fetch: InstructionFetch::Allowed,
            ..
        } => return Ok(u32::from_le_bytes(read_bytes(memory, address))),
        PagePermission::Mapped {
            instruction_fetch: InstructionFetch::Forbidden,
            ..
        } => FaultStatus::PermissionFaultLevel3,
        PagePermission::Unmapped => FaultStatus::TranslationFaultLevel3,
    };
    Err(Exception::InstructionAbort { status, address })
}

fn add_shifted_register<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    flag_setting: FlagSetting,
) -> Result<(), Exception> {
    let amount = SHIFT_IMM6.read(word);
    let operand = read_register(processor, RM.read(word), RegisterThirtyOne::ZeroRegister);
    let shifted = match SHIFT_TYPES[SHIFT.read(word) as usize] {
        ShiftType::Lsl => operand << amount,
        ShiftType::Lsr => operand >> amount,
        ShiftType::Asr => ((operand as i64) >> amount) as u64,
        ShiftType::Ror => return Err(Exception::UndefinedInstruction),
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

fn select_conditionally<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
) -> Result<(), Exception> {
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

fn load_unsigned_offset<LevelSpecific, Architecture>(
    processor: &mut Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    word: u32,
    misaligned_stack_pointer: MisalignedStackPointer,
) -> Result<(), Exception>
where
    Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
{
    let base = read_base(processor, word, misaligned_stack_pointer)?;
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

fn store_unsigned_offset<LevelSpecific, Architecture>(
    processor: &Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    word: u32,
    misaligned_stack_pointer: MisalignedStackPointer,
    across_fault: StoreAcrossFault,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> Result<(), Exception>
where
    Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
{
    let base = read_base(processor, word, misaligned_stack_pointer)?;
    let address = base.wrapping_add(u64::from(IMM12.read(word)) * REGISTER_BYTES);
    let value = read_register(processor, RT.read(word), RegisterThirtyOne::ZeroRegister);
    store(
        memory,
        address,
        &value.to_le_bytes(),
        across_fault,
        mutation_hook,
    )
}

fn store_pair<LevelSpecific, Architecture>(
    processor: &Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    word: u32,
    misaligned_stack_pointer: MisalignedStackPointer,
    across_fault: StoreAcrossFault,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> Result<(), Exception>
where
    Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
{
    let base = read_base(processor, word, misaligned_stack_pointer)?;
    let offset = IMM7.read_signed(word).wrapping_mul(REGISTER_BYTES as i64);
    let address = base.wrapping_add(offset as u64);
    let mut bytes = [0; 2 * REGISTER_BYTES as usize];
    let (low, high) = bytes.split_at_mut(REGISTER_BYTES as usize);
    for (half, field) in [(low, RT), (high, RT2)] {
        let value = read_register(processor, field.read(word), RegisterThirtyOne::ZeroRegister);
        half.copy_from_slice(&value.to_le_bytes());
    }
    store(memory, address, &bytes, across_fault, mutation_hook)
}

/// Writes the bytes at the address, or, where a byte faults, what the behavior across a fault writes.
fn store<Architecture: ProcessorArchitecture<PagePermission = PagePermission>>(
    memory: &[MemoryDelta<'_, Architecture>],
    address: u64,
    bytes: &[u8],
    across_fault: StoreAcrossFault,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> Result<(), Exception> {
    let access = AccessDirection::Write;
    let written = match find_fault(memory, address, bytes.len() as u64, access) {
        None => bytes,
        Some((fault_address, status)) => {
            let below_fault = &bytes[..fault_address.wrapping_sub(address) as usize];
            if across_fault == StoreAcrossFault::WritesBytesBelowFault && !below_fault.is_empty() {
                mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
                    start: address,
                    bytes: Cow::Borrowed(below_fault),
                });
            }
            return Err(Exception::DataAbort {
                status,
                access,
                address: fault_address,
            });
        }
    };
    mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
        start: address,
        bytes: Cow::Borrowed(written),
    });
    Ok(())
}

fn load_post_index<LevelSpecific, Architecture>(
    processor: &mut Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    word: u32,
    misaligned_stack_pointer: MisalignedStackPointer,
    into_base: LoadIntoBaseWithWriteback,
) -> Result<(), Exception>
where
    Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
{
    let target = RT.read(word);
    let base_register = RN.read(word);
    let writeback = if target == base_register && is_general_purpose(target) {
        match into_base {
            LoadIntoBaseWithWriteback::Undefined => return Err(Exception::UndefinedInstruction),
            LoadIntoBaseWithWriteback::NoOperation => return Ok(()),
            LoadIntoBaseWithWriteback::WritebackSuppressed => Writeback::Suppressed,
            LoadIntoBaseWithWriteback::WritebackOverwritesLoad => Writeback::Applied,
        }
    } else {
        Writeback::Applied
    };
    let address = read_base(processor, word, misaligned_stack_pointer)?;
    let value = load(memory, address)?;
    write_register(processor, target, value, RegisterThirtyOne::ZeroRegister);
    if writeback == Writeback::Applied {
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

/// Reads the base register of a load or store, which may fault when it is a misaligned stack pointer.
fn read_base<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    word: u32,
    misaligned_stack_pointer: MisalignedStackPointer,
) -> Result<u64, Exception> {
    let number = RN.read(word);
    let base = read_register(processor, number, RegisterThirtyOne::StackPointer);
    let misaligned = !is_general_purpose(number) && !base.is_multiple_of(STACK_POINTER_ALIGNMENT);
    if misaligned && misaligned_stack_pointer == MisalignedStackPointer::Faults {
        return Err(Exception::StackPointerAlignmentFault);
    }
    Ok(base)
}

fn load<Architecture: ProcessorArchitecture<PagePermission = PagePermission>>(
    memory: &[MemoryDelta<'_, Architecture>],
    address: u64,
) -> Result<u64, Exception> {
    let access = AccessDirection::Read;
    match find_fault(memory, address, REGISTER_BYTES, access) {
        None => Ok(u64::from_le_bytes(read_bytes(memory, address))),
        Some((fault_address, status)) => Err(Exception::DataAbort {
            status,
            access,
            address: fault_address,
        }),
    }
}

/// Finds the lowest address of the access that its page refuses, or that no page holds, and the fault's status.
fn find_fault<Architecture: ProcessorArchitecture<PagePermission = PagePermission>>(
    memory: &[MemoryDelta<'_, Architecture>],
    start: u64,
    length: u64,
    access: AccessDirection,
) -> Option<(u64, FaultStatus)> {
    (0..length)
        .map(|offset| start.wrapping_add(offset))
        .find_map(|address| {
            if !Architecture::ADDRESSABLE.contains(&address) {
                return Some((address, FaultStatus::TranslationFaultLevel0));
            }
            let status = match read_permission(memory, address) {
                PagePermission::Unmapped => FaultStatus::TranslationFaultLevel3,
                PagePermission::Mapped { data_access, .. } => match (data_access, access) {
                    (DataAccess::ReadWrite, _) | (DataAccess::ReadOnly, AccessDirection::Read) => {
                        return None;
                    }
                    (DataAccess::ReadOnly, AccessDirection::Write) | (DataAccess::Forbidden, _) => {
                        FaultStatus::PermissionFaultLevel3
                    }
                },
            };
            Some((address, status))
        })
}

fn is_general_purpose(number: u32) -> bool {
    (number as usize) < GENERAL_PURPOSE_REGISTERS
}

fn read_register<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    number: u32,
    thirty_one: RegisterThirtyOne,
) -> u64 {
    match processor.general_purpose_registers.get(number as usize) {
        Some(value) => *value,
        None => match thirty_one {
            RegisterThirtyOne::ZeroRegister => 0,
            RegisterThirtyOne::StackPointer => processor.stack_pointer,
        },
    }
}

fn write_register<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
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
