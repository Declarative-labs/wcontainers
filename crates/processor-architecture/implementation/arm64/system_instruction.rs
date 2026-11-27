//! HINT, MSR (immediate), SYS and SYSL at EL0 as the executor's guest configures it: SCTLR_EL1's UCI and DZE set, UMA,
//! EnRCTX and nTWI clear, SCTLR2_EL1.EnPACM0 clear, and GCSCRE0_EL1 clear, so that a chip with FEAT_GCS has it off at
//! EL0. Each encoding does what its own instruction does with the features a chip has. With the features of a Neoverse
//! V2 the tables below are what it does with every encoding (MEASURED: each of the 128 hints, 1,024 PSTATE encodings
//! and 16,384 encodings of SYS and of SYSL run once on a Graviton4). Which encoding needs which feature is from Arm's
//! machine-readable release (AARCHMRS 2026-09: `Instructions.json` for the hints and MSR (immediate), the accessors of
//! `Registers.json` for SYS and SYSL).

use alloc::vec::Vec;
use core::convert::Infallible;

use wcontainers_processor_architecture::{MemoryDelta, MemorySink, ProcessorArchitecture};

use crate::behavior::{FaultAddressTag, WaitForInterruptBehavior};
use crate::branch::to_untagged_address;
use crate::encoding::{Field, RT};
use crate::load_store::{find_fault, to_fault_address};
use crate::operand::RegisterThirtyOne;
use crate::pointer_authentication::strip;
use crate::registers::read_register;
use crate::system_register::{HeldRegister, write_process_state_control};
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
/// SRLS, a hint of FEAT_RLCS.
const SCOPED_RELEASE_HINT: u32 = 54;
const LINK_REGISTER: u32 = 30;
/// The bytes DC ZVA and DC GZVA zero: DCZID_EL0.BS is 4 on a Neoverse V2 (MEASURED: 64 bytes written).
const ZERO_BLOCK_BYTES: u64 = 64;
/// ESR_EL1 of a data abort from EL0 by cache maintenance: EC 0x24, IL, CM and WnR, below its fault status code.
const CACHE_MAINTENANCE_ABORT: u64 = 0x9200_0140;
/// The fault status codes ESR_EL1 gives.
const TRANSLATION_FAULT_LEVEL_0: u64 = 0b00_0100;
const TRANSLATION_FAULT_LEVEL_3: u64 = 0b00_0111;
const PERMISSION_FAULT_LEVEL_3: u64 = 0b00_1111;

/// The features that change what an encoding of HINT, MSR (immediate), SYS or SYSL does at EL0, one bit each.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SystemFeatures(u16);

impl SystemFeatures {
    pub const NONE: Self = Self(0);
    /// FEAT_PAuth: hint 7 is XPACLRI, which strips X30.
    pub const POINTER_AUTHENTICATION: Self = Self(1 << 0);
    /// FEAT_FlagM: CFINV.
    pub const FLAG_MANIPULATION: Self = Self(1 << 1);
    /// FEAT_FlagM2: XAFLAG and AXFLAG.
    pub const FLAG_MANIPULATION_2: Self = Self(1 << 2);
    /// FEAT_SSBS: MSR (immediate) of SSBS.
    pub const SPECULATIVE_STORE_BYPASS_SAFE: Self = Self(1 << 3);
    /// FEAT_DIT: MSR (immediate) of DIT.
    pub const DATA_INDEPENDENT_TIMING: Self = Self(1 << 4);
    /// FEAT_MTE: MSR (immediate) of TCO, DC GVA, DC GZVA and the tag forms of the cleans.
    pub const MEMORY_TAGGING: Self = Self(1 << 5);
    /// FEAT_DPB: DC CVAP.
    pub const PERSISTENCE: Self = Self(1 << 6);
    /// FEAT_DPB2: DC CVADP.
    pub const DEEP_PERSISTENCE: Self = Self(1 << 7);
    /// FEAT_SPECRES: CFP, DVP and CPP RCTX.
    pub const PREDICTION_RESTRICTION: Self = Self(1 << 8);
    /// FEAT_SPECRES2: COSP RCTX.
    pub const PREDICTION_RESTRICTION_2: Self = Self(1 << 9);
    /// FEAT_OCCMO: DC CVAOC, DC CIVAOC and, with FEAT_MTE, their tag forms.
    pub const OUTER_CACHE_MAINTENANCE: Self = Self(1 << 10);
    /// FEAT_GCS, off at EL0: GCSPUSHM traps, GCSSS1, GCSPOPM and GCSSS2 change nothing.
    pub const GUARDED_CONTROL_STACK: Self = Self(1 << 11);
    /// FEAT_ITE: TRCIT, which only adds to a trace.
    pub const INSTRUMENTATION_TRACE: Self = Self(1 << 12);
    /// With FEAT_MTE, MSR (immediate) of TCO completes but writes nothing, as on a chip whose hypervisor gives its
    /// guest no FEAT_MTE (MEASURED on a Graviton4 under KVM).
    pub const IGNORED_TAG_CHECK_OVERRIDE: Self = Self(1 << 13);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

/// Every feature a behavior below models, so that the cut gives memory to every encoding some chip reaches it with.
const EVERY_SYSTEM_FEATURE: SystemFeatures = SystemFeatures((1 << 14) - 1);

/// The features each instruction's encodings depend on.
const HINT_FEATURES: SystemFeatures = SystemFeatures::POINTER_AUTHENTICATION;
const PSTATE_FEATURES: SystemFeatures = SystemFeatures::FLAG_MANIPULATION
    .union(SystemFeatures::FLAG_MANIPULATION_2)
    .union(SystemFeatures::SPECULATIVE_STORE_BYPASS_SAFE)
    .union(SystemFeatures::DATA_INDEPENDENT_TIMING)
    .union(SystemFeatures::MEMORY_TAGGING)
    .union(SystemFeatures::IGNORED_TAG_CHECK_OVERRIDE);
const SYSTEM_FEATURES: SystemFeatures = SystemFeatures::MEMORY_TAGGING
    .union(SystemFeatures::PERSISTENCE)
    .union(SystemFeatures::DEEP_PERSISTENCE)
    .union(SystemFeatures::PREDICTION_RESTRICTION)
    .union(SystemFeatures::PREDICTION_RESTRICTION_2)
    .union(SystemFeatures::OUTER_CACHE_MAINTENANCE)
    .union(SystemFeatures::GUARDED_CONTROL_STACK)
    .union(SystemFeatures::INSTRUMENTATION_TRACE);
const SYSTEM_WITH_RESULT_FEATURES: SystemFeatures = SystemFeatures::GUARDED_CONTROL_STACK;

/// The features one needs, as Arm's feature list requires them: FEAT_FlagM2 needs FEAT_FlagM, FEAT_DPB2 FEAT_DPB and
/// FEAT_SPECRES2 FEAT_SPECRES; and a TCO that ignores writes is one of FEAT_MTE.
const REQUIRED_FEATURES: [(SystemFeatures, SystemFeatures); 4] = [
    (
        SystemFeatures::IGNORED_TAG_CHECK_OVERRIDE,
        SystemFeatures::MEMORY_TAGGING,
    ),
    (
        SystemFeatures::FLAG_MANIPULATION_2,
        SystemFeatures::FLAG_MANIPULATION,
    ),
    (
        SystemFeatures::DEEP_PERSISTENCE,
        SystemFeatures::PERSISTENCE,
    ),
    (
        SystemFeatures::PREDICTION_RESTRICTION_2,
        SystemFeatures::PREDICTION_RESTRICTION,
    ),
];

/// Every set of the features of the mask a chip can have.
fn list_feature_sets(mask: SystemFeatures) -> Vec<SystemFeatures> {
    let mut sets = Vec::new();
    let mut subset = 0_u16;
    loop {
        let features = SystemFeatures(subset);
        if REQUIRED_FEATURES
            .iter()
            .all(|(feature, required)| !features.contains(*feature) || features.contains(*required))
        {
            sets.push(features);
        }
        if subset == mask.0 {
            return sets;
        }
        subset = (subset.wrapping_sub(mask.0)) & mask.0;
    }
}

/// What one of these instructions does with the choices a chip makes, each with the features of its instruction the
/// chip has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SystemInstructionBehavior {
    /// HINT, with what WFI does, its hint number 3.
    Hint(WaitForInterruptBehavior, SystemFeatures),
    /// MSR (immediate).
    PstateWrite(SystemFeatures),
    /// SYS, with what a fault address keeps of a tag and what CPP RCTX does.
    System(FaultAddressTag, PredictionRestriction, SystemFeatures),
    /// SYSL: undefined at EL0 but for the stack instructions of FEAT_GCS.
    SystemWithResult(SystemFeatures),
}

impl SystemInstructionBehavior {
    pub const fn to_instruction(self) -> Instruction<Infallible> {
        match self {
            Self::Hint(..) => Instruction::Hint,
            Self::PstateWrite(_) => Instruction::MsrImmediate,
            Self::System(..) => Instruction::Sys,
            Self::SystemWithResult(_) => Instruction::Sysl,
        }
    }
}

/// Every behavior a chip is known to show for the instruction, none for another instruction: one for each set of
/// features, and each choice besides; what CPP RCTX does is a choice only with FEAT_SPECRES.
pub(crate) fn list_system_instruction_behaviors(
    instruction: Instruction<Infallible>,
) -> Vec<SystemInstructionBehavior> {
    use SystemInstructionBehavior as B;
    match instruction {
        Instruction::Hint => list_feature_sets(HINT_FEATURES)
            .into_iter()
            .flat_map(|features| {
                [
                    WaitForInterruptBehavior::Traps,
                    WaitForInterruptBehavior::Completes,
                ]
                .map(|wait| B::Hint(wait, features))
            })
            .collect(),
        Instruction::MsrImmediate => list_feature_sets(PSTATE_FEATURES)
            .into_iter()
            .map(B::PstateWrite)
            .collect(),
        Instruction::Sys => {
            let mut behaviors = Vec::new();
            for features in list_feature_sets(SYSTEM_FEATURES) {
                let restrictions: &[PredictionRestriction] =
                    if features.contains(SystemFeatures::PREDICTION_RESTRICTION) {
                        &[
                            PredictionRestriction::Completes,
                            PredictionRestriction::Trapped,
                        ]
                    } else {
                        &[PredictionRestriction::Completes]
                    };
                for restriction in restrictions {
                    for tag in [
                        FaultAddressTag::KeepsTopByte,
                        FaultAddressTag::ClearsTopNibble,
                    ] {
                        behaviors.push(B::System(tag, *restriction, features));
                    }
                }
            }
            behaviors
        }
        Instruction::Sysl => list_feature_sets(SYSTEM_WITH_RESULT_FEATURES)
            .into_iter()
            .map(B::SystemWithResult)
            .collect(),
        _ => Vec::new(),
    }
}

/// What CPP RCTX does at EL0 with SCTLR_EL1.EnRCTX clear.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PredictionRestriction {
    /// Completes, as a Neoverse V2 does (MEASURED: no exception and no exit to KVM).
    Completes,
    /// Traps to EL1 as CFP RCTX and DVP RCTX do, as [EnRCTX](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/SCTLR-EL1--System-Control-Register--EL1-#:~:text=EnRCTX%2C%20bit%20%5B10%5D) documents.
    Trapped,
}

/// The op2 of CPP RCTX.
const PREFETCH_PREDICTION_RESTRICTION_OP2: u32 = 7;

/// What one SYS encoding does at EL0.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SystemInstructionAccess {
    Undefined,
    /// CFP RCTX and DVP RCTX, which EL1 traps with SCTLR_EL1.EnRCTX clear.
    Trapped,
    /// IC IVAU, DC CVAU and CPP RCTX, which never fault at EL0, and with GCS off at EL0, GCSSS1 and TRCIT.
    NoOperation,
    /// DC ZVA, DC GVA and DC GZVA: a store to the block holding the address, zeroing it or only checking it.
    ZeroBlock {
        zeroes: bool,
    },
    /// DC CVAC, CVAP, CVADP, CIVAC, CVAOC, CIVAOC and their tag forms: a data abort with CM set where the address may
    /// not be read.
    CleanOrInvalidate,
}

/// What the SYS encoding of op1, CRn, CRm and op2 does at EL0 on a chip with the features, by the accessors of Arm's
/// [system instructions](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Instructions) at EL0 as the
/// guest configures it.
pub fn to_system_instruction_access(
    field: u32,
    features: SystemFeatures,
) -> SystemInstructionAccess {
    use SystemFeatures as F;
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
    let tagging = features.contains(F::MEMORY_TAGGING) && !crate::planted::is(4706);
    let deep_persistence = features.contains(F::DEEP_PERSISTENCE);
    let outer = features.contains(F::OUTER_CACHE_MAINTENANCE);
    let stack = features.contains(F::GUARDED_CONTROL_STACK);
    match (crm, op2) {
        // TRCIT.
        (2, 7) if features.contains(F::INSTRUMENTATION_TRACE) => A::NoOperation,
        // CFP and DVP RCTX, and COSP RCTX.
        (3, 4 | 5) if features.contains(F::PREDICTION_RESTRICTION) => A::Trapped,
        (3, 6) if features.contains(F::PREDICTION_RESTRICTION_2) => A::Trapped,
        // CPP RCTX: a Neoverse V2 completes it and then takes no software step (MEASURED: the next word runs).
        (3, 7) if features.contains(F::PREDICTION_RESTRICTION) => A::NoOperation,
        (4, 1) => A::ZeroBlock { zeroes: true },
        (4, 4) if tagging => A::ZeroBlock { zeroes: true },
        (4, 3) if tagging => A::ZeroBlock { zeroes: false },
        (5, 1) | (11, 1) => A::NoOperation,
        // GCSPUSHM with GCSCRE0_EL1.PUSHMEn clear, and GCSSS1 with GCS off.
        (7, 0) if stack => A::Trapped,
        (7, 2) if stack => A::NoOperation,
        (10 | 14, 1) => A::CleanOrInvalidate,
        (10 | 12 | 14, 3 | 5) if tagging => A::CleanOrInvalidate,
        (12, 1) if features.contains(F::PERSISTENCE) => A::CleanOrInvalidate,
        (13, 1) if deep_persistence && crate::planted::is(4707) => A::NoOperation,
        (13, 1) if deep_persistence => A::CleanOrInvalidate,
        (13, 3 | 5) if deep_persistence && tagging => A::CleanOrInvalidate,
        (11 | 15, 0) if outer => A::CleanOrInvalidate,
        (11 | 15, 7) if outer && tagging => A::CleanOrInvalidate,
        _ => A::Undefined,
    }
}

/// The bytes a SYS encoding reaches from its address register's value on a chip with every modeled feature, none for one
/// that reaches no memory.
pub fn to_system_instruction_bytes(field: u32) -> Option<u64> {
    match to_system_instruction_access(field, EVERY_SYSTEM_FEATURE) {
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
            Self::Hint(wait, features) => match HINT_NUMBER.read(word) {
                WAIT_FOR_INTERRUPT_HINT => match wait {
                    WaitForInterruptBehavior::Traps => {
                        Err(Exception::WaitTrapped(WaitInstruction::WaitForInterrupt))
                    }
                    WaitForInterruptBehavior::Completes => Ok(()),
                },
                STRIP_LINK_REGISTER_HINT
                    if features.contains(SystemFeatures::POINTER_AUTHENTICATION)
                        && !crate::planted::is(723) =>
                {
                    let pointer = processor.general_purpose_registers[LINK_REGISTER as usize];
                    processor.general_purpose_registers[LINK_REGISTER as usize] = strip(pointer);
                    Ok(())
                }
                SCOPED_RELEASE_HINT if crate::planted::is(4705) => {
                    Err(Exception::UndefinedInstruction)
                }
                // Every other hint changes nothing a state shows: with the keys off, BTI, the other pointer
                // authentication hints, PACM with EnPACM0 clear, CHKFEAT with no feature on, the cache and ordering
                // hints, and the hints of features the chip lacks.
                _ => Ok(()),
            },
            Self::PstateWrite(features) => simulate_pstate_write(processor, word, features),
            Self::System(tag, restriction, features) => simulate_system(
                processor,
                memory,
                word,
                SystemChoices {
                    tag,
                    restriction,
                    features,
                },
                mutation_hook,
            ),
            Self::SystemWithResult(_) if crate::planted::is(726) => Ok(()),
            // GCSPOPM and GCSSS2 with GCS off leave Xt as it is.
            Self::SystemWithResult(features)
                if features.contains(SystemFeatures::GUARDED_CONTROL_STACK)
                    && is_stack_result_instruction(SYSTEM_INSTRUCTION.read(word)) =>
            {
                Ok(())
            }
            Self::SystemWithResult(_) => Err(Exception::UndefinedInstruction),
        }
    }
}

/// The PSTATE control an MSR (immediate) word writes where its feature is present: SSBS, DIT or TCO.
pub const fn to_written_process_state_control(word: u32) -> Option<HeldRegister> {
    match (OP1.read(word), OP2.read(word)) {
        (3, 1) => Some(HeldRegister::SpeculativeStoreBypassSafe),
        (3, 2) => Some(HeldRegister::DataIndependentTiming),
        (3, 4) => Some(HeldRegister::TagCheckOverride),
        _ => None,
    }
}

/// [MSR (immediate)](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MSR--immediate---Move-immediate-value-to-special-register-):
/// op1 0 with op2 0 to 2 is CFINV, XAFLAG and AXFLAG whatever CRm; op1 3 writes CRm's bit 0 to SSBS, DIT and TCO, and
/// traps DAIFSet and DAIFClr; each of those is undefined without its feature, and every other encoding, the PSTATE
/// fields of EL1 and SME's SVCR among them, is undefined.
fn simulate_pstate_write<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    features: SystemFeatures,
) -> Result<(), Exception> {
    use SystemFeatures as F;
    let flag_instruction = match (OP1.read(word), OP2.read(word)) {
        (0, 0) if features.contains(F::FLAG_MANIPULATION) => Instruction::Cfinv,
        (0, 1) if features.contains(F::FLAG_MANIPULATION_2) => Instruction::Xaflag,
        (0, 2) if features.contains(F::FLAG_MANIPULATION_2) => Instruction::Axflag,
        (3, 1 | 2 | 4) if !crate::planted::is(728) => {
            let Some(register) = to_written_process_state_control(word) else {
                unreachable!("op1 3 with op2 1, 2 or 4 writes a PSTATE control")
            };
            let feature = match register {
                HeldRegister::SpeculativeStoreBypassSafe => F::SPECULATIVE_STORE_BYPASS_SAFE,
                HeldRegister::DataIndependentTiming => F::DATA_INDEPENDENT_TIMING,
                _ => F::MEMORY_TAGGING,
            };
            if !features.contains(feature) {
                return Err(Exception::UndefinedInstruction);
            }
            if register == HeldRegister::TagCheckOverride
                && features.contains(F::IGNORED_TAG_CHECK_OVERRIDE)
            {
                return Ok(());
            }
            let value = u64::from(CRM.read(word) & 1) != u64::from(crate::planted::is(4701));
            let bit = register.to_process_state_bit().unwrap_or(0);
            write_process_state_control(processor, register, if value { bit } else { 0 });
            if crate::planted::is(4704) && register == HeldRegister::DataIndependentTiming {
                processor.process_state_controls &= !HeldRegister::SpeculativeStoreBypassSafe
                    .to_process_state_bit()
                    .unwrap_or(0);
            }
            return Ok(());
        }
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

/// GCSPOPM and GCSSS2, the SYSL encodings of FEAT_GCS: op1 3, CRn 7, CRm 7 and op2 1 or 3.
const fn is_stack_result_instruction(field: u32) -> bool {
    field == (3 << 11 | 7 << 7 | 7 << 3 | 1) || field == (3 << 11 | 7 << 7 | 7 << 3 | 3)
}

/// The choices a chip makes for SYS.
#[derive(Clone, Copy)]
struct SystemChoices {
    tag: FaultAddressTag,
    restriction: PredictionRestriction,
    features: SystemFeatures,
}

/// SYS by what its encoding does at EL0.
fn simulate_system<LevelSpecific, Architecture>(
    processor: &mut Processor<LevelSpecific>,
    memory: &[MemoryDelta<'_, Architecture>],
    word: u32,
    choices: SystemChoices,
    mutation_hook: &mut impl MemorySink<Architecture>,
) -> Result<(), Exception>
where
    Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
{
    use SystemInstructionAccess as A;
    let SystemChoices {
        tag,
        restriction,
        features,
    } = choices;
    let field = SYSTEM_INSTRUCTION.read(word);
    let rt = RT.read(word);
    let address = read_register(processor, rt, RegisterThirtyOne::ZeroRegister);
    let location = to_untagged_address(address);
    let prediction_restriction =
        field == (3 << 11 | 7 << 7 | 3 << 3 | PREFETCH_PREDICTION_RESTRICTION_OP2);
    let access = match to_system_instruction_access(field, features.intersection(SYSTEM_FEATURES)) {
        A::NoOperation
            if prediction_restriction
                && (restriction == PredictionRestriction::Trapped || crate::planted::is(727)) =>
        {
            A::Trapped
        }
        access => access,
    };
    match access {
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
            let block = if crate::planted::is(725) {
                location
            } else {
                block
            };
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
