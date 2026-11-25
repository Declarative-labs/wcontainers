//! The formula of every behavior.

use core::convert::Infallible;

use wcontainers_privilege_level::PrivilegeLevel;
use wcontainers_processor_architecture::{
    MemoryDelta, MemorySink, MemorySnapshot, ProcessorArchitecture, Simulator,
};

use crate::branch::is_outside_lower_addresses;
use crate::encoding::{IMM16, RD, RDVL_IMM6, RT};
use crate::memory::{read_bytes, read_permission};
use crate::operand::RegisterThirtyOne;
use crate::registers::write_register;
use crate::system_register::{simulate_system_register_move, to_undecided_registers};
use crate::{
    AccessDirection, Arm64, Exception, FaultStatus, INSTRUCTION_SIZE, Instruction,
    InstructionFetch, PagePermission, PreciseInstruction, Processor, ProcessorExpectation,
    RegisterValueExpectation, User, VectorLengthReading, WaitForInterruptBehavior, WaitInstruction,
};

impl<Level: PrivilegeLevel> Simulator<Arm64<Level>>
    for PreciseInstruction<Level::PrecisePrivilegedInstruction>
where
    Level::PrecisePrivilegedInstruction: Simulator<Arm64<Level>, Error = Infallible>,
{
    type Error = Infallible;

    /// Fetches the word at the program counter and applies this behavior to it, reading its operands from it.
    fn simulate(
        &self,
        processor: &Processor<Level::PrivilegedProcessorSubset>,
        memory: MemorySnapshot<'_, '_, Arm64<Level>>,
        mutation_hook: &mut impl MemorySink<Arm64<Level>>,
    ) -> Result<ProcessorExpectation<Level::PrivilegedProcessorSubset>, Infallible> {
        if let Self::LevelSpecific(precise) = self {
            return precise.simulate(processor, memory, mutation_hook);
        }
        let mut after = processor.clone();
        let undecided = self.apply(&mut after, memory.mutations, mutation_hook);
        let mut expectation = ProcessorExpectation::new(after);
        if let Some((register, condition_flags)) = undecided {
            if let Some(expected) = expectation
                .general_purpose_registers
                .get_mut(register as usize)
            {
                *expected = RegisterValueExpectation::NonDeterministic;
            }
            if condition_flags {
                expectation.condition_flags = RegisterValueExpectation::NonDeterministic;
            }
        }
        Ok(expectation)
    }
}

impl<LevelSpecific> PreciseInstruction<LevelSpecific> {
    /// Applies this behavior to the processor in place, the level's own instructions aside; gives the register the
    /// state before does not decide, and whether NZCV is undecided too, where the instruction completes so.
    fn apply<Architecture: ProcessorArchitecture<PagePermission = PagePermission>>(
        &self,
        processor: &mut Processor<impl Sized>,
        memory: &[MemoryDelta<'_, Architecture>],
        mutation_hook: &mut impl MemorySink<Architecture>,
    ) -> Option<(u32, bool)> {
        let mut branch_target = None;
        let mut undecided = None;
        let outcome = match fetch(memory, processor.program_counter) {
            Err(exception) => Err(exception),
            Ok(word) if self.is_reserved(word) => Err(Exception::UndefinedInstruction),
            Ok(word) => match *self {
                Self::Documented(documented) => documented
                    .simulate(processor, word)
                    .map(|target| branch_target = target),
                Self::LoadStore {
                    instruction,
                    choices,
                } => instruction.simulate(choices, processor, memory, word, mutation_hook),
                Self::Brk => Err(Exception::Breakpoint {
                    comment: (IMM16.read(word) >> u32::from(crate::planted::is(31))) as u16,
                }),
                Self::WaitForInterrupt(WaitForInterruptBehavior::Traps) => {
                    Err(Exception::WaitTrapped(if crate::planted::is(256) {
                        WaitInstruction::WaitForEvent
                    } else {
                        WaitInstruction::WaitForInterrupt
                    }))
                }
                Self::WaitForInterrupt(WaitForInterruptBehavior::Completes) => Ok(()),
                Self::SystemRegisterMove { direction, choices } => {
                    simulate_system_register_move(processor, word, direction, choices).map(|()| {
                        undecided = to_undecided_registers(word, direction, choices);
                    })
                }
                Self::Rdvl(VectorLengthReading::Undefined) => Err(Exception::UndefinedInstruction),
                Self::Rdvl(VectorLengthReading::SmeTrapped(code)) => Err(Exception::SmeTrap(code)),
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
                Self::Structure {
                    instruction,
                    choices,
                } => instruction.simulate(choices, processor, memory, word, mutation_hook),
                Self::MrsCtrEl0 { cache_type } => {
                    write_register(
                        processor,
                        RT.read(word),
                        cache_type,
                        RegisterThirtyOne::ZeroRegister,
                    );
                    Ok(())
                }
                Self::Unimplemented(instruction)
                    if instruction.is_executed_as_hint_without_feature()
                        && !crate::planted::is(508) =>
                {
                    Ok(())
                }
                Self::Unimplemented(_) if crate::planted::is(512) => Ok(()),
                Self::Unimplemented(_) => Err(Exception::UndefinedInstruction),
                Self::LevelSpecific(_) => {
                    unreachable!("`simulate` runs the level's own instructions")
                }
            },
        };
        match outcome {
            Ok(()) => {
                processor.program_counter = branch_target
                    .unwrap_or(processor.program_counter.wrapping_add(INSTRUCTION_SIZE));
                processor.exception = None;
            }
            Err(exception) => {
                if exception.is_taken_after_instruction()
                    && !(crate::planted::is(30)
                        && matches!(exception, Exception::SupervisorCall { .. }))
                {
                    processor.program_counter =
                        processor.program_counter.wrapping_add(INSTRUCTION_SIZE);
                }
                processor.exception = Some(exception);
                undecided = None;
            }
        }
        undecided
    }

    /// Whether the word holds an operand value the list marks unallocated for this instruction.
    fn is_reserved(&self, word: u32) -> bool {
        let instruction = match *self {
            Self::Documented(documented) => documented.instruction(),
            Self::LoadStore { instruction, .. } => instruction.instruction(),
            Self::Brk => Instruction::Brk,
            Self::Rdvl(_) => Instruction::Rdvl,
            Self::WaitForInterrupt(_) => Instruction::Wfi,
            Self::SystemRegisterMove {
                direction: AccessDirection::Read,
                ..
            } => Instruction::Mrs,
            Self::SystemRegisterMove { .. } => Instruction::MsrRegister,
            Self::Unimplemented(instruction) => instruction,
            Self::Structure { instruction, .. } => instruction.instruction(),
            Self::MrsCtrEl0 { .. } | Self::LevelSpecific(_) => return false,
        };
        instruction.to_record().is_some_and(|record| {
            record
                .reserved
                .iter()
                .any(|pattern| pattern.is_matching(word))
        })
    }
}

/// The user level adds no instruction, so it has none to simulate.
impl Simulator<Arm64<User>> for Infallible {
    type Error = Infallible;

    fn simulate(
        &self,
        _: &Processor<()>,
        _: MemorySnapshot<'_, '_, Arm64<User>>,
        _: &mut impl MemorySink<Arm64<User>>,
    ) -> Result<ProcessorExpectation<()>, Infallible> {
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
    if is_outside_lower_addresses(address) {
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
