//! HINT, MSR (immediate), SYS and SYSL at EL0 as the executor's guest configures it: SCTLR_EL1's UCI and DZE set, UMA,
//! EnRCTX and nTWI clear. Each encoding does what its own instruction does; the tables below are what a Neoverse V2
//! does with every encoding (MEASURED: each of the 128 hints, 1,024 PSTATE encodings and 16,384 encodings of SYS and of
//! SYSL run once on a Graviton4).

use core::convert::Infallible;

use wcontainers_processor_architecture::{MemoryDelta, MemorySink, ProcessorArchitecture};

use crate::behavior::{FaultAddressTag, WaitForInterruptBehavior};
use crate::branch::to_untagged_address;
use crate::encoding::{Field, RT};
use crate::load_store::{find_fault, to_fault_address};
use crate::operand::RegisterThirtyOne;
use crate::pointer_authentication::strip;
use crate::registers::read_register;
use crate::{
    AccessDirection, DocumentedInstruction, Exception, FaultStatus, Instruction, PagePermission,
    Processor, SystemAccess, WaitInstruction,
};

/// The hint number, CRm:op2.
pub const HINT_NUMBER: Field = Field::new(5, 7);
/// op1, CRn, CRm and op2 of SYS and SYSL, as one field.
pub const SYSTEM_INSTRUCTION: Field = Field::new(5, 14);
const OP1: Field = Field::new(16, 3);
const CRM: Field = Field::new(8, 4);
const OP2: Field = Field::new(5, 3);
/// WFI and XPACLRI, the hints that do something a state shows.
const WAIT_FOR_INTERRUPT_HINT: u32 = 3;
const STRIP_LINK_REGISTER_HINT: u32 = 7;
const LINK_REGISTER: u32 = 30;
/// The bytes DC ZVA and DC GZVA zero: DCZID_EL0.BS is 4 on a Neoverse V2 (MEASURED: 64 bytes written).
const ZERO_BLOCK_BYTES: u64 = 64;
/// ESR_EL1 of a data abort from EL0 by cache maintenance: EC 0x24, IL, CM and WnR, below its fault status code.
const CACHE_MAINTENANCE_ABORT: u64 = 0x9200_0140;
/// The fault status codes ESR_EL1 gives.
const TRANSLATION_FAULT_LEVEL_0: u64 = 0b00_0100;
const TRANSLATION_FAULT_LEVEL_3: u64 = 0b00_0111;
const PERMISSION_FAULT_LEVEL_3: u64 = 0b00_1111;

/// What one of these instructions does with the choices a chip makes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SystemInstructionBehavior {
    /// HINT, with what WFI does, its hint number 3.
    Hint(WaitForInterruptBehavior),
    /// MSR (immediate) as a Neoverse V2 runs it.
    PstateWrite,
    /// SYS as a Neoverse V2 runs it, with what a fault address keeps of a tag.
    System(FaultAddressTag),
    /// SYSL: undefined at EL0 on a Neoverse V2, which has no FEAT_GCS.
    SystemWithResult,
}

impl SystemInstructionBehavior {
    pub const fn to_instruction(self) -> Instruction<Infallible> {
        match self {
            Self::Hint(_) => Instruction::Hint,
            Self::PstateWrite => Instruction::MsrImmediate,
            Self::System(_) => Instruction::Sys,
            Self::SystemWithResult => Instruction::Sysl,
        }
    }
}

/// Every behavior a chip is known to show for the instruction, none for another instruction.
pub(crate) fn list_system_instruction_behaviors(
    instruction: Instruction<Infallible>,
) -> &'static [SystemInstructionBehavior] {
    use SystemInstructionBehavior as B;
    match instruction {
        Instruction::Hint => &[
            B::Hint(WaitForInterruptBehavior::Traps),
            B::Hint(WaitForInterruptBehavior::Completes),
        ],
        Instruction::MsrImmediate => &[B::PstateWrite],
        Instruction::Sys => &[
            B::System(FaultAddressTag::KeepsTopByte),
            B::System(FaultAddressTag::ClearsTopNibble),
        ],
        Instruction::Sysl => &[B::SystemWithResult],
        _ => &[],
    }
}

/// What one SYS encoding does at EL0.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SystemInstructionAccess {
    Undefined,
    /// CFP RCTX and DVP RCTX, which EL1 traps with SCTLR_EL1.EnRCTX clear.
    Trapped,
    /// IC IVAU, DC CVAU and CPP RCTX, which never fault at EL0.
    NoOperation,
    /// DC ZVA, DC GVA and DC GZVA: a store to the block holding the address, zeroing it or only checking it.
    ZeroBlock {
        zeroes: bool,
    },
    /// DC CVAC, CVAP, CVADP, CIVAC and their tag forms: a data abort with CM set where the address may not be read.
    CleanOrInvalidate,
}

/// What the SYS encoding of op1, CRn, CRm and op2 does at EL0 on a Neoverse V2.
pub const fn to_system_instruction_access(field: u32) -> SystemInstructionAccess {
    use SystemInstructionAccess as A;
    let (op1, crn, crm, op2) = (
        (field >> 11) & 0b111,
        (field >> 7) & 0b1111,
        (field >> 3) & 0b1111,
        field & 0b111,
    );
    if op1 != 3 || crn != 7 {
        return A::Undefined;
    }
    match (crm, op2) {
        (3, 4 | 5) => A::Trapped,
        (4, 1 | 4) => A::ZeroBlock { zeroes: true },
        (4, 3) => A::ZeroBlock { zeroes: false },
        // CPP RCTX completes, and the hypervisor loses the single step after it (MEASURED: the next word runs).
        (5, 1) | (11, 1) | (3, 7) => A::NoOperation,
        (10 | 12 | 13 | 14, 1 | 3 | 5) => A::CleanOrInvalidate,
        _ => A::Undefined,
    }
}

/// The bytes a SYS encoding reaches from its address register's value, none for one that reaches no memory.
pub const fn to_system_instruction_bytes(field: u32) -> Option<u64> {
    match to_system_instruction_access(field) {
        SystemInstructionAccess::ZeroBlock { .. } | SystemInstructionAccess::CleanOrInvalidate => {
            Some(ZERO_BLOCK_BYTES)
        }
        SystemInstructionAccess::Undefined
        | SystemInstructionAccess::Trapped
        | SystemInstructionAccess::NoOperation => None,
    }
}

impl SystemInstructionBehavior {
    /// Applies the behavior to the word; changes no register when it returns an exception.
    pub(crate) fn simulate<LevelSpecific, Architecture>(
        self,
        processor: &mut Processor<LevelSpecific>,
        memory: &[MemoryDelta<'_, Architecture>],
        word: u32,
        mutation_hook: &mut impl MemorySink<Architecture>,
    ) -> Result<(), Exception>
    where
        Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
    {
        match self {
            Self::Hint(wait) => match HINT_NUMBER.read(word) {
                WAIT_FOR_INTERRUPT_HINT => match wait {
                    WaitForInterruptBehavior::Traps => {
                        Err(Exception::WaitTrapped(WaitInstruction::WaitForInterrupt))
                    }
                    WaitForInterruptBehavior::Completes => Ok(()),
                },
                STRIP_LINK_REGISTER_HINT if !crate::planted::is(723) => {
                    let pointer = processor.general_purpose_registers[LINK_REGISTER as usize];
                    processor.general_purpose_registers[LINK_REGISTER as usize] = strip(pointer);
                    Ok(())
                }
                // Every other hint changes nothing a state shows: with the keys off, BTI, the other pointer
                // authentication hints, CHKFEAT with no feature on, and the hints of features the chip lacks.
                _ => Ok(()),
            },
            Self::PstateWrite => simulate_pstate_write(processor, word),
            Self::System(tag) => simulate_system(processor, memory, word, tag, mutation_hook),
            Self::SystemWithResult => Err(Exception::UndefinedInstruction),
        }
    }
}

/// [MSR (immediate)](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MSR--immediate---Move-immediate-value-to-special-register-):
/// op1 0 with op2 0 to 2 is CFINV, XAFLAG and AXFLAG whatever CRm; op1 3 writes SSBS, DIT and TCO, which no state
/// holds, and traps DAIFSet and DAIFClr; every other encoding, the PSTATE fields of EL1 and SME's SVCR among them, is
/// undefined.
fn simulate_pstate_write<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
) -> Result<(), Exception> {
    let flag_instruction = match (OP1.read(word), OP2.read(word)) {
        (0, 0) => Instruction::Cfinv,
        (0, 1) => Instruction::Xaflag,
        (0, 2) => Instruction::Axflag,
        (3, 1 | 2 | 4) => return Ok(()),
        (3, op2 @ (6 | 7)) if !crate::planted::is(724) => {
            return Err(Exception::SystemAccessTrapped(SystemAccess {
                op0: 0,
                op1: 3,
                crn: 4,
                crm: CRM.read(word) as u8,
                op2: op2 as u8,
                rt: RT.read(word) as u8,
                direction: AccessDirection::Write,
            }));
        }
        _ => return Err(Exception::UndefinedInstruction),
    };
    let Some(documented) = DocumentedInstruction::new(flag_instruction) else {
        unreachable!("CFINV, XAFLAG and AXFLAG are modeled")
    };
    documented.simulate(processor, word).map(|_| ())
}

/// SYS by what its encoding does at EL0.
fn simulate_system<LevelSpecific, Architecture>(
    processor: &mut Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    word: u32,
    tag: FaultAddressTag,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> Result<(), Exception>
where
    Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
{
    use SystemInstructionAccess as A;
    let field = SYSTEM_INSTRUCTION.read(word);
    let rt = RT.read(word);
    let address = read_register(processor, rt, RegisterThirtyOne::ZeroRegister);
    let location = to_untagged_address(address);
    match to_system_instruction_access(field) {
        A::Undefined => Err(Exception::UndefinedInstruction),
        A::Trapped => Err(Exception::SystemAccessTrapped(SystemAccess {
            op0: 1,
            op1: ((field >> 11) & 0b111) as u8,
            crn: ((field >> 7) & 0b1111) as u8,
            crm: ((field >> 3) & 0b1111) as u8,
            op2: (field & 0b111) as u8,
            rt: rt as u8,
            direction: AccessDirection::Write,
        })),
        A::NoOperation => Ok(()),
        A::ZeroBlock { zeroes } => {
            let block = location & !(ZERO_BLOCK_BYTES - 1);
            // The fault address is the register's, wherever in the block the fault lies.
            if let Some((_, status)) =
                find_fault(memory, block, ZERO_BLOCK_BYTES, AccessDirection::Write)
            {
                return Err(Exception::DataAbort {
                    status,
                    access: AccessDirection::Write,
                    address: to_fault_address(address, location, tag),
                });
            }
            if zeroes {
                mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
                    start: block,
                    bytes: alloc::borrow::Cow::Owned(alloc::vec![0; ZERO_BLOCK_BYTES as usize]),
                });
            }
            Ok(())
        }
        A::CleanOrInvalidate => match find_fault(memory, location, 1, AccessDirection::Read) {
            None => Ok(()),
            Some((_, status)) => Err(Exception::Undecoded {
                syndrome: CACHE_MAINTENANCE_ABORT
                    | match status {
                        FaultStatus::TranslationFaultLevel0 => TRANSLATION_FAULT_LEVEL_0,
                        FaultStatus::TranslationFaultLevel3 => TRANSLATION_FAULT_LEVEL_3,
                        FaultStatus::PermissionFaultLevel3 | FaultStatus::AlignmentFault => {
                            PERMISSION_FAULT_LEVEL_3
                        }
                    },
                fault_address: to_fault_address(address, location, tag),
            }),
        },
    }
}
