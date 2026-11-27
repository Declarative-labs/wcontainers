//! The suite's guests on ARM64, stopped as the ARM64 architecture crate defines a stop.

use wcontainers_abi::{
    Access, SYSTEM_CALL_ARGUMENT_COUNT, SYSTEM_CALL_NUMBER_REGISTER, SystemCallRequest,
};
use wcontainers_processor_architecture_arm64::encoding::{
    Field, FixedBits, IMM12, IMM16, RD, RN, RT,
};
use wcontainers_processor_architecture_arm64::system_register::{
    SYSTEM_REGISTER, SystemRegisterEncoding,
};
use wcontainers_processor_architecture_arm64::{
    AccessDirection, Arm64, ConditionFlags, Exception, FaultStatus, INSTRUCTION_SIZE, Instruction,
    Processor, SystemAccess, User, ZERO_PROCESSOR,
};

use wcontainers_processor_architecture::ProcessorArchitecture;

use crate::{FirstTrap, Guest};

/// The architecture the guests are written for, and its instructions.
pub(crate) type Chip = Arm64<User>;
type ChipInstruction = <Chip as ProcessorArchitecture>::Instruction;

/// The first argument register of a system call and its result register, x0.
///
/// <https://man7.org/linux/man-pages/man2/syscall.2.html>
const FIRST_ARGUMENT_REGISTER: u32 = 0;
const RESULT_REGISTER: u32 = FIRST_ARGUMENT_REGISTER;
/// The register of the address a guest stores to or loads from, and of the count a guest counts down.
const ADDRESS_REGISTER: u32 = FIRST_ARGUMENT_REGISTER;
const COUNTER_REGISTER: u32 = FIRST_ARGUMENT_REGISTER;
/// The register of the value a guest stores or loads, the one after the address's.
const VALUE_REGISTER: u32 = ADDRESS_REGISTER + 1;
/// The immediate of every system call, breakpoint and undefined instruction of the guests.
const IMMEDIATE: u16 = u16::MIN;
/// What the counting guest takes from its count each time round.
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SUB--immediate---Subtract-immediate-value->
const COUNT_STEP: u32 = 1;
/// How far back the counting guest's branch goes, in instructions: to the subtraction before it.
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CBNZ--Compare-and-branch-on-nonzero->
const LOOP_BACK: i32 = -1;

/// SVC, the supervisor call, with its comment in [`IMM16`].
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SVC--Supervisor-call->
const SUPERVISOR_CALL: FixedBits = FixedBits {
    mask: 0xFFE0_001F,
    value: 0xD400_0001,
};
/// SUB (immediate) on 64 bits with no shift of its immediate, which sits in [`IMM12`].
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SUB--immediate---Subtract-immediate-value->
const SUBTRACT_IMMEDIATE: FixedBits = FixedBits {
    mask: 0xFFC0_0000,
    value: 0xD100_0000,
};
/// CBNZ on 64 bits, its offset in instructions in [`BRANCH_OFFSET`].
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CBNZ--Compare-and-branch-on-nonzero->
const COMPARE_BRANCH_NONZERO: FixedBits = FixedBits {
    mask: 0xFF00_0000,
    value: 0xB500_0000,
};
/// The signed offset of a compare and branch, imm19.
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CBNZ--Compare-and-branch-on-nonzero->
const BRANCH_OFFSET: Field = Field {
    lowest_bit: 5,
    width: 19,
};
/// The register a branch tests, the first argument's, and the link register a branch with link writes.
const TESTED_REGISTER: usize = FIRST_ARGUMENT_REGISTER as usize;
const LINK_REGISTER: usize = 30;
/// A value of the tested register whose every bit, the lowest a bit test tests, is clear, and one whose lowest is set.
const ALL_CLEAR: u64 = u64::MIN;
const LOWEST_SET: u64 = 1;

/// [CNTVCT_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CNTVCT-EL0--Counter-timer-Virtual-Count-Register),
/// the counter a guest reads the time from.
const VIRTUAL_COUNTER: SystemRegisterEncoding = SystemRegisterEncoding::new(3, 3, 14, 0, 2);
/// MRS with no register named, its L bit set: a read.
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MRS--Move-system-register-to-general-purpose-register->
const SYSTEM_REGISTER_READ: FixedBits = FixedBits {
    mask: 0xFFF0_0000,
    value: 0xD530_0000,
};
/// The register the counter reading guest reads into: neither the first nor zero, so that its number is seen.
const COUNTER_READ_REGISTER: u32 = VALUE_REGISTER;

/// UDF, permanently undefined, with its immediate in its low half.
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/UDF--Permanently-undefined->
const PERMANENTLY_UNDEFINED: FixedBits = FixedBits {
    mask: 0xFFFF_0000,
    value: 0x0000_0000,
};

/// Returns a guest that makes the system call and reaches a breakpoint, finished as if the call returned the value.
pub(crate) fn build_system_call(
    code: u64,
    request: SystemCallRequest,
    returned: u64,
) -> Guest<Chip> {
    let mut start = start_at(code);
    let first = FIRST_ARGUMENT_REGISTER as usize;
    start.general_purpose_registers[first..first + SYSTEM_CALL_ARGUMENT_COUNT]
        .copy_from_slice(&request.arguments);
    start.general_purpose_registers[SYSTEM_CALL_NUMBER_REGISTER] = request.number.0;
    let mut answered = start.clone();
    answered.general_purpose_registers[RESULT_REGISTER as usize] = returned;
    let call = SUPERVISOR_CALL.value | IMM16.to_bits(u32::from(IMMEDIATE));
    end_at_breakpoint(&[call], start, answered)
}

/// Returns a guest that stores the 64-bit value at the address and then reaches a breakpoint.
pub(crate) fn build_store(code: u64, address: u64, value: u64) -> Guest<Chip> {
    let mut start = start_at(code);
    start.general_purpose_registers[ADDRESS_REGISTER as usize] = address;
    start.general_purpose_registers[VALUE_REGISTER as usize] = value;
    let store = Instruction::StrImmediateUnsignedOffset64
        .to_fixed_bits()
        .value
        | RT.to_bits(VALUE_REGISTER)
        | RN.to_bits(ADDRESS_REGISTER);
    let stored = start.clone();
    end_at_breakpoint(&[store], start, stored)
}

/// Returns a guest that loads 64 bits from the address then reaches a breakpoint, finished as if it read the value.
pub(crate) fn build_load(code: u64, address: u64, value: u64) -> Guest<Chip> {
    let mut start = start_at(code);
    start.general_purpose_registers[ADDRESS_REGISTER as usize] = address;
    let mut loaded = start.clone();
    loaded.general_purpose_registers[VALUE_REGISTER as usize] = value;
    let load = Instruction::LdrImmediateUnsignedOffset64
        .to_fixed_bits()
        .value
        | RT.to_bits(VALUE_REGISTER)
        | RN.to_bits(ADDRESS_REGISTER);
    end_at_breakpoint(&[load], start, loaded)
}

/// Returns a guest whose only instruction is an undefined one.
pub(crate) fn build_undefined_instruction(code: u64) -> Guest<Chip> {
    let start = start_at(code);
    let undefined = PERMANENTLY_UNDEFINED.value | u32::from(IMMEDIATE);
    Guest {
        code: to_code(&[undefined]),
        finished: stopped_by(start.clone(), Exception::UndefinedInstruction),
        start,
    }
}

/// Returns a guest that counts a register down from the count, not zero, and then reaches a breakpoint.
pub(crate) fn build_count_down(code: u64, count: u64) -> Guest<Chip> {
    let mut start = start_at(code);
    start.general_purpose_registers[COUNTER_REGISTER as usize] = count;
    let step = SUBTRACT_IMMEDIATE.value
        | IMM12.to_bits(COUNT_STEP)
        | RN.to_bits(COUNTER_REGISTER)
        | RD.to_bits(COUNTER_REGISTER);
    let loop_back = COMPARE_BRANCH_NONZERO.value
        | BRANCH_OFFSET.to_bits(LOOP_BACK.cast_unsigned())
        | RT.to_bits(COUNTER_REGISTER);
    let mut counted = start.clone();
    counted.general_purpose_registers[COUNTER_REGISTER as usize] = u64::MIN;
    end_at_breakpoint(&[step, loop_back], start, counted)
}

/// Returns whether the guest stopped at its first instruction for the trap, every register as it started.
///
/// A page that allows nothing may be held unmapped or mapped without the access, which the interface does not tell
/// apart. A system call stops as the architecture crate takes its supervisor call: at the instruction after it.
pub(crate) fn is_stop_at_first_instruction(
    guest: &Guest<Chip>,
    trap: FirstTrap,
    stopped: &Processor<()>,
) -> bool {
    let statuses = [
        FaultStatus::TranslationFaultLevel3,
        FaultStatus::PermissionFaultLevel3,
    ];
    let expected = |exception| stopped_by(guest.start.clone(), exception) == *stopped;
    match trap {
        FirstTrap::SystemCall => {
            let mut after = guest.start.clone();
            after.program_counter = after.program_counter.wrapping_add(INSTRUCTION_SIZE);
            stopped_by(
                after,
                Exception::SupervisorCall {
                    immediate: IMMEDIATE,
                },
            ) == *stopped
        }
        FirstTrap::MemoryFault {
            address,
            access: Access::Execute,
        } => statuses
            .into_iter()
            .any(|status| expected(Exception::InstructionAbort { status, address })),
        FirstTrap::MemoryFault { address, access } => {
            let access = match access {
                Access::Write => AccessDirection::Write,
                Access::Read | Access::Execute => AccessDirection::Read,
            };
            statuses.into_iter().any(|status| {
                expected(Exception::DataAbort {
                    status,
                    access,
                    address,
                })
            })
        }
    }
}

/// Returns the processor with every register zero but the program counter, at the code.
fn start_at(code: u64) -> Processor<()> {
    Processor {
        program_counter: code,
        ..ZERO_PROCESSOR
    }
}

/// Returns the guest of the instructions and a breakpoint after them, finished at it with the registers given.
fn end_at_breakpoint(
    instructions: &[u32],
    start: Processor<()>,
    mut ended: Processor<()>,
) -> Guest<Chip> {
    let breakpoint = Instruction::Brk.to_fixed_bits().value | IMM16.to_bits(u32::from(IMMEDIATE));
    let code = to_code(&[instructions, &[breakpoint]].concat());
    let code_bytes = u64::try_from(code.len()).expect("a guest's length fits in an address");
    ended.program_counter = start.program_counter + code_bytes - INSTRUCTION_SIZE;
    let finished = stopped_by(ended, Exception::Breakpoint { comment: IMMEDIATE });
    Guest {
        code,
        start,
        finished,
    }
}

fn to_code(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn stopped_by(mut processor: Processor<()>, exception: Exception) -> Processor<()> {
    processor.exception = Some(exception);
    processor
}

/// Returns each direct branch with an offset of zero: its word, and the state that takes it, every other register zero.
///
/// Each word is its instruction's fixed bits, so its offset is zero, its register the first, its condition EQ and its
/// tested bit the lowest.
pub(crate) fn build_branches_to_themselves(
    code: u64,
) -> Vec<(ChipInstruction, u32, Processor<()>)> {
    let taken_by = |instruction: ChipInstruction| -> (u64, ConditionFlags) {
        match instruction {
            Instruction::Cbnz32 | Instruction::Cbnz64 | Instruction::Tbnz => {
                (LOWEST_SET, ConditionFlags::empty())
            }
            Instruction::BCond => (ALL_CLEAR, ConditionFlags::ZERO),
            _ => (ALL_CLEAR, ConditionFlags::empty()),
        }
    };
    [
        Instruction::B,
        Instruction::Bl,
        Instruction::BCond,
        Instruction::Cbz32,
        Instruction::Cbz64,
        Instruction::Cbnz32,
        Instruction::Cbnz64,
        Instruction::Tbz,
        Instruction::Tbnz,
    ]
    .into_iter()
    .map(|instruction| {
        let (tested, flags) = taken_by(instruction);
        let mut start = start_at(code);
        start.general_purpose_registers[TESTED_REGISTER] = tested;
        start.condition_flags = flags;
        (instruction, instruction.to_fixed_bits().value, start)
    })
    .collect()
}

/// Returns the state a branch to itself from `start` ends at: the same, but for the link a branch with link writes.
pub(crate) fn after_branch_to_itself(
    instruction: ChipInstruction,
    start: &Processor<()>,
) -> Processor<()> {
    let mut after = start.clone();
    if instruction == Instruction::Bl {
        after.general_purpose_registers[LINK_REGISTER] =
            start.program_counter.wrapping_add(INSTRUCTION_SIZE);
    }
    after
}

/// Returns a guest that branches to itself for ever.
pub(crate) fn build_endless(code: u64) -> Guest<Chip> {
    let start = start_at(code);
    Guest {
        code: to_code(&[Instruction::B.to_fixed_bits().value]),
        finished: start.clone(),
        start,
    }
}

/// Returns a guest that reads the virtual counter and reaches a breakpoint, finished at the read with the trap the
/// chip takes at EL0, where the system answers the read.
pub(crate) fn build_counter_read(code: u64) -> Guest<Chip> {
    let start = start_at(code);
    let read = SYSTEM_REGISTER_READ.value
        | SYSTEM_REGISTER.to_bits(VIRTUAL_COUNTER.to_field())
        | RT.to_bits(COUNTER_READ_REGISTER);
    let breakpoint = Instruction::Brk.to_fixed_bits().value | IMM16.to_bits(u32::from(IMMEDIATE));
    let trapped = Exception::SystemAccessTrapped(SystemAccess {
        op0: VIRTUAL_COUNTER.op0 as u8,
        op1: VIRTUAL_COUNTER.op1 as u8,
        crn: VIRTUAL_COUNTER.crn as u8,
        crm: VIRTUAL_COUNTER.crm as u8,
        op2: VIRTUAL_COUNTER.op2 as u8,
        rt: COUNTER_READ_REGISTER as u8,
        direction: AccessDirection::Read,
    });
    Guest {
        code: to_code(&[read, breakpoint]),
        finished: stopped_by(start.clone(), trapped),
        start,
    }
}

/// Returns whether the guest, a load through a tagged address, stopped at the load with the chip's abort of it: the
/// address with its whole tag, or with the tag's low half alone, as a chip that keeps only those bits reports it.
pub(crate) fn is_abort_through_tag(
    guest: &Guest<Chip>,
    tagged: u64,
    stopped: &Processor<()>,
) -> bool {
    /// The tag's high half, bits 63:60 of the address.
    const TAG_HIGH: u64 = 0xF << 60;
    [tagged, tagged & !TAG_HIGH].into_iter().any(|address| {
        [
            FaultStatus::TranslationFaultLevel3,
            FaultStatus::PermissionFaultLevel3,
        ]
        .into_iter()
        .any(|status| {
            stopped_by(
                guest.start.clone(),
                Exception::DataAbort {
                    status,
                    access: AccessDirection::Read,
                    address,
                },
            ) == *stopped
        })
    })
}
