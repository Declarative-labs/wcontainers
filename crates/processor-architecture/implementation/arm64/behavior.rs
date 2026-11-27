//! The behaviors an instruction can have where the manual leaves a choice or chips differ.

use alloc::vec::Vec;
use core::convert::Infallible;

use crate::load_store::scalable_access::{
    AccessKind, ScalableAccess, ScalableAccessChoices, ScalableStoreAcrossFault, to_scalable_access,
};
use crate::load_store::{LoadStoreChoices, LoadStoreInstruction};
use crate::sve::{ScalableOperation, to_scalable_instruction};
use crate::system_register::{NEOVERSE_V2_SYSTEM_REGISTERS, OTHER_SYSTEM_REGISTERS};
use crate::{
    AccessDirection, DocumentedInstruction, DocumentedOperation, Instruction, PreciseInstruction,
    SmeTrapCode, to_documented_operation,
};

vstd::prelude::verus! {

/// What a load or store does when its base is the stack pointer and that is not a multiple of 16.
///
/// A chip raises the [alignment fault](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#impl-aarch64.CheckSPAlignment.0) Linux turns on for every level; QEMU leaves the check out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64)
)]
pub enum MisalignedStackPointer {
    Faults,
    Accesses,
}

/// What a store writes when part of it faults.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64)
)]
pub enum StoreAcrossFault {
    WritesNothing,
    WritesBytesBelowFault,
}

/// The [CONSTRAINED UNPREDICTABLE](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDR--immediate---Load-register--immediate--#:~:text=CONSTRAINED%20UNPREDICTABLE) choice of a post-indexed load whose target is its base.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64)
)]
pub enum LoadIntoBaseWithWriteback {
    WritebackSuppressed,
    WritebackOverwritesLoad,
    Undefined,
    NoOperation,
}

/// The [CONSTRAINED UNPREDICTABLE](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STR--immediate---Store-register--immediate--#:~:text=CONSTRAINED%20UNPREDICTABLE) choice of a store with writeback whose register is its base.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64)
)]
pub enum StoreOfBaseWithWriteback {
    StoresOriginalValue,
    Undefined,
    NoOperation,
}

/// The [CONSTRAINED UNPREDICTABLE](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDP--Load-pair-of-registers-#:~:text=CONSTRAINED%20UNPREDICTABLE) choice of a load pair that names one register twice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64)
)]
pub enum LoadPairIntoOneRegister {
    KeepsFirstValue,
    KeepsSecondValue,
    Undefined,
    NoOperation,
}

/// Whether a store exclusive that fails, as every one does whose address the exclusive monitor does not hold,
/// [checks the access](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STXR--Store-exclusive-register-#:~:text=IMPLEMENTATION%20DEFINED) as a store would.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64)
)]
pub enum FailedStoreExclusive {
    ChecksAccess,
    LeavesAccessUnchecked,
}

/// The [CONSTRAINED UNPREDICTABLE](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STXR--Store-exclusive-register-#:~:text=CONSTRAINED%20UNPREDICTABLE) choice of a store exclusive whose status register is one it stores or its base.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64)
)]
pub enum StoreExclusiveOverlap {
    Runs,
    Undefined,
    NoOperation,
}

/// Where an exclusive access that is not at a multiple of its size may lie without an alignment fault: an
/// exclusive access must be aligned to its size before [FEAT_LSE2](https://developer.arm.com/documentation/109697/2025_09/Feature-descriptions/The-Armv8-4-architecture-extension#:~:text=FEAT_LSE2), and a Neoverse V2 lets it lie anywhere inside 16
/// aligned bytes, as its ordered and atomic accesses may.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64)
)]
pub enum MisalignedExclusive {
    AlignmentFault,
    WithinSixteenBytes,
}

/// What a data abort's fault address keeps of the top byte of a tagged address: Linux documents that bits 63 to
/// 60 of a fault address are [not preserved](https://docs.kernel.org/arch/arm64/tagged-pointers.html#:~:text=bits%2063%3A60) on every chip.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64)
)]
pub enum FaultAddressTag {
    KeepsTopByte,
    ClearsTopNibble,
}

} // verus!

/// What WFI does at EL0: EL1 traps it where [SCTLR_EL1.nTWI](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/SCTLR-EL1--System-Control-Register--EL1-#:~:text=nTWI%2C%20bit%20%5B16%5D) is clear, and it waits for an interrupt or
/// completes where it is set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64)
)]
pub enum WaitForInterruptBehavior {
    Traps,
    Completes,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64)
)]
pub enum VectorLengthReading {
    Undefined,
    /// Raises the [SME exception](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=SMTC%2C%20bits%20%5B2%3A0%5D), as a chip with SME and no SVE does outside streaming mode.
    SmeTrapped(SmeTrapCode),
    Reads(VectorLength),
}

/// An SVE vector length, as [ZCR_EL1.LEN](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ZCR-EL1--SVE-Control-Register-EL1-#:~:text=LEN%2C%20bits%20%5B3%3A0%5D) permits it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64)
)]
pub enum VectorLength {
    Bits128,
    Bits256,
    Bits512,
    Bits1024,
    Bits2048,
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

impl PreciseInstruction<Infallible> {
    /// Returns the same behavior as one of a level that adds instructions of its own.
    pub(crate) const fn to_level<LevelSpecific>(self) -> PreciseInstruction<LevelSpecific> {
        match self {
            Self::Documented(documented) => PreciseInstruction::Documented(documented),
            Self::LoadStore {
                instruction,
                choices,
            } => PreciseInstruction::LoadStore {
                instruction,
                choices,
            },
            Self::Brk => PreciseInstruction::Brk,
            Self::Rdvl(reading) => PreciseInstruction::Rdvl(reading),
            Self::WaitForInterrupt(behavior) => PreciseInstruction::WaitForInterrupt(behavior),
            Self::SystemRegisterMove { direction, choices } => {
                PreciseInstruction::SystemRegisterMove { direction, choices }
            }
            Self::MrsCtrEl0 { cache_type } => PreciseInstruction::MrsCtrEl0 { cache_type },
            Self::Scalable {
                instruction,
                operation,
                trapped,
            } => PreciseInstruction::Scalable {
                instruction,
                operation,
                trapped,
            },
            Self::ScalableAccess {
                instruction,
                choices,
            } => PreciseInstruction::ScalableAccess {
                instruction,
                choices,
            },
            Self::Structure {
                instruction,
                choices,
            } => PreciseInstruction::Structure {
                instruction,
                choices,
            },
            Self::ElementAccess {
                instruction,
                choices,
            } => PreciseInstruction::ElementAccess {
                instruction,
                choices,
            },
            Self::TagAccess {
                instruction,
                choices,
            } => PreciseInstruction::TagAccess {
                instruction,
                choices,
            },
            Self::SystemInstruction(behavior) => PreciseInstruction::SystemInstruction(behavior),
            Self::Unimplemented(instruction) => PreciseInstruction::Unimplemented(instruction),
            Self::LevelSpecific(never) => match never {},
        }
    }

    pub const fn to_instruction(self) -> Instruction<Infallible> {
        match self {
            Self::Documented(documented) => documented.instruction(),
            Self::LoadStore { instruction, .. } => instruction.instruction(),
            Self::Brk => Instruction::Brk,
            Self::Rdvl(_) => Instruction::Rdvl,
            Self::WaitForInterrupt(_) => Instruction::Wfi,
            Self::SystemRegisterMove {
                direction: AccessDirection::Read,
                ..
            } => Instruction::Mrs,
            Self::SystemRegisterMove {
                direction: AccessDirection::Write,
                ..
            } => Instruction::MsrRegister,
            Self::MrsCtrEl0 { .. } => Instruction::MrsCtrEl0,
            Self::Scalable { instruction, .. }
            | Self::ScalableAccess { instruction, .. }
            | Self::ElementAccess { instruction, .. }
            | Self::Unimplemented(instruction) => instruction,
            Self::Structure { instruction, .. } => instruction.instruction(),
            Self::TagAccess { instruction, .. } => instruction.instruction(),
            Self::SystemInstruction(behavior) => behavior.to_instruction(),
        }
    }
}

/// Returns every behavior a chip is known to show for the instruction, each cache type one that a chip was seen to read.
///
/// An optional instruction also has the behavior of a chip without its feature, an instruction that takes input from
/// outside memory and the processor has only that one, and a mandatory instruction that no behavior models yet has none.
pub(crate) fn list_known_behaviors<LevelInstruction, LevelPrecise>(
    instruction: Instruction<LevelInstruction>,
) -> Vec<PreciseInstruction<LevelPrecise>> {
    match instruction.to_every_level() {
        Some(instruction) => list_every_level_behaviors(instruction)
            .into_iter()
            .map(PreciseInstruction::to_level)
            .collect(),
        // TODO: temporary placeholder until the privilege level interface lists the behaviors of a level's instructions.
        None => Vec::new(),
    }
}

/// The behaviors of an instruction every privilege level has.
fn list_every_level_behaviors(
    instruction: Instruction<Infallible>,
) -> Vec<PreciseInstruction<Infallible>> {
    if is_forbidden(instruction) {
        return [PreciseInstruction::Unimplemented(instruction)].to_vec();
    }
    let mut behaviors = list_modeled_behaviors(instruction);
    if instruction.is_optional() && !is_modeled_as_unimplemented(instruction) {
        behaviors.push(PreciseInstruction::Unimplemented(instruction));
    }
    behaviors
}

/// Whether the instruction takes input from outside memory and the processor, so that no state decides what it does:
/// the [LS64](https://developer.arm.com/documentation/109697/2025_09/Feature-descriptions/The-Armv8-7-architecture-extension#:~:text=FEAT_LS64) family, whose
/// accesses go to a device.
const fn is_forbidden(instruction: Instruction<Infallible>) -> bool {
    use Instruction as I;
    matches!(
        instruction,
        I::Ld64b
            | I::St64b
            | I::St64bv
            | I::St64bv0
            | I::Lda64b64lMemop
            | I::Stl64b64lMemop
            | I::Stl64bv64Memop
            | I::Stl64bv064Memop
    )
}

/// Whether a modeled behavior of the instruction already does what a chip without its feature does, on every state: an
/// instruction undefined at EL0, a hint that changes nothing a state shows, or RDVL, which lists that behavior itself.
fn is_modeled_as_unimplemented(instruction: Instruction<Infallible>) -> bool {
    use crate::pointer_authentication::PointerAuthenticationOperation as P;
    match to_documented_operation(instruction) {
        Some(
            DocumentedOperation::Undefined
            | DocumentedOperation::PointerAuthentication(P::Undefined),
        ) => true,
        Some(
            DocumentedOperation::NoOperation
            | DocumentedOperation::PointerAuthentication(P::KeyDisabled),
        ) => instruction.is_executed_as_hint_without_feature(),
        _ => instruction == Instruction::Rdvl,
    }
}

/// The behaviors of the models of an instruction every privilege level has.
fn list_modeled_behaviors(
    instruction: Instruction<Infallible>,
) -> Vec<PreciseInstruction<Infallible>> {
    const LENGTHS: [VectorLength; 5] = [
        VectorLength::Bits128,
        VectorLength::Bits256,
        VectorLength::Bits512,
        VectorLength::Bits1024,
        VectorLength::Bits2048,
    ];
    const CACHE_TYPES: [u64; 3] = [
        NEOVERSE_V2_CACHE_TYPE,
        APPLE_M5_CACHE_TYPE,
        PRESENTED_CACHE_TYPE,
    ];
    if let Some(scalable) = to_scalable_instruction(instruction) {
        return list_scalable_behaviors(instruction, scalable.operation);
    }
    if let Some(documented) = DocumentedInstruction::new(instruction) {
        return [PreciseInstruction::Documented(documented)].to_vec();
    }
    if let Some(load_store) = LoadStoreInstruction::new(instruction) {
        return list_load_store_behaviors(load_store);
    }
    if let Some(access) = to_scalable_access(instruction) {
        return list_scalable_access_behaviors(instruction, access);
    }
    if let Some(access) = crate::load_store::scalable_element_access::to_element_access(instruction)
    {
        return crate::load_store::scalable_element_access::list_element_access_behaviors(
            instruction,
            access,
        );
    }
    if let Some(access) = crate::memory_tagging::TagAccessInstruction::new(instruction) {
        return crate::memory_tagging::list_tag_access_behaviors(access);
    }
    if let Some(structure) = crate::StructureInstruction::new(instruction) {
        return crate::structure::list_structure_behaviors(structure).collect();
    }
    match instruction {
        Instruction::Brk => [PreciseInstruction::Brk].to_vec(),
        Instruction::Wfi => [
            WaitForInterruptBehavior::Traps,
            WaitForInterruptBehavior::Completes,
        ]
        .map(PreciseInstruction::WaitForInterrupt)
        .to_vec(),
        Instruction::Rdvl => [
            VectorLengthReading::Undefined,
            VectorLengthReading::SmeTrapped(SmeTrapCode::Disabled),
            VectorLengthReading::SmeTrapped(SmeTrapCode::NotInStreamingMode),
        ]
        .into_iter()
        .chain(LENGTHS.map(VectorLengthReading::Reads))
        .map(PreciseInstruction::Rdvl)
        .collect(),
        Instruction::Mrs | Instruction::MsrRegister => {
            let direction = if instruction == Instruction::Mrs {
                AccessDirection::Read
            } else {
                AccessDirection::Write
            };
            [NEOVERSE_V2_SYSTEM_REGISTERS, OTHER_SYSTEM_REGISTERS]
                .map(|choices| PreciseInstruction::SystemRegisterMove { direction, choices })
                .to_vec()
        }
        Instruction::Hint | Instruction::MsrImmediate | Instruction::Sys | Instruction::Sysl => {
            crate::system_instruction::list_system_instruction_behaviors(instruction)
                .into_iter()
                .map(PreciseInstruction::SystemInstruction)
                .collect()
        }
        Instruction::MrsCtrEl0 => CACHE_TYPES
            .map(|cache_type| PreciseInstruction::MrsCtrEl0 { cache_type })
            .to_vec(),
        _ => Vec::new(),
    }
}

/// Each combination of the choices an SVE access has: a prefetch none, a load no choice across a fault.
fn list_scalable_access_behaviors(
    instruction: Instruction<Infallible>,
    access: ScalableAccess,
) -> Vec<PreciseInstruction<Infallible>> {
    if access.kind == AccessKind::Prefetch {
        return [PreciseInstruction::ScalableAccess {
            instruction,
            choices: ScalableAccessChoices {
                misaligned_stack_pointer: MisalignedStackPointer::Faults,
                across_fault: None,
                fault_address_tag: FaultAddressTag::KeepsTopByte,
            },
        }]
        .to_vec();
    }
    // One vector's length holds a store of one register, so that writing its whole vectors below a fault writes
    // nothing; a store of two to four registers can fault inside a vector, where the two differ.
    let across: &[Option<ScalableStoreAcrossFault>] = if access.kind == AccessKind::Store {
        match access.registers {
            2..=4 => &[
                Some(ScalableStoreAcrossFault::WritesNothing),
                Some(ScalableStoreAcrossFault::WritesBytesBelowFault),
                Some(ScalableStoreAcrossFault::WritesWholeVectorsBelowFault),
            ],
            _ => &[
                Some(ScalableStoreAcrossFault::WritesNothing),
                Some(ScalableStoreAcrossFault::WritesBytesBelowFault),
            ],
        }
    } else {
        &[None]
    };
    let mut behaviors = Vec::new();
    for misaligned_stack_pointer in [
        MisalignedStackPointer::Faults,
        MisalignedStackPointer::Accesses,
    ] {
        for across_fault in across {
            for fault_address_tag in [
                FaultAddressTag::KeepsTopByte,
                FaultAddressTag::ClearsTopNibble,
            ] {
                behaviors.push(PreciseInstruction::ScalableAccess {
                    instruction,
                    choices: ScalableAccessChoices {
                        misaligned_stack_pointer,
                        across_fault: *across_fault,
                        fault_address_tag,
                    },
                });
            }
        }
    }
    behaviors
}

/// The reservation granule of a Neoverse V2: CTR_EL0.ERG is 4, and a store exclusive anywhere in the 64 bytes stores
/// (MEASURED: `hidden-state-loading/arm64/RESULT.md`, question 5, and the one-sided test in the chip source).
const NEOVERSE_V2_RESERVATION_GRANULE_BYTES: u64 = 64;

/// [CTR_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CTR-EL0--Cache-Type-Register) as an Arm Neoverse V2 under KVM reads it, measured on an AWS Graviton4.
const NEOVERSE_V2_CACHE_TYPE: u64 = 0xB444_C004;

/// [CTR_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CTR-EL0--Cache-Type-Register) as an Apple M5 reads it inside a virtual machine, since macOS traps a program's read.
const APPLE_M5_CACHE_TYPE: u64 = 0x9_9444_C004;

/// [CTR_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CTR-EL0--Cache-Type-Register) as the translator presents it and unicorn's `max` model reads it.
const PRESENTED_CACHE_TYPE: u64 = 0x8444_C004;

/// An SVE instruction run at the vector length the state holds, and for one that SME gives too, the SME exception of
/// each [cause](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=SMTC%2C%20bits%20%5B2%3A0%5D) a chip with SME and without SVE raises, as for RDVL.
fn list_scalable_behaviors(
    instruction: Instruction<Infallible>,
    operation: ScalableOperation,
) -> Vec<PreciseInstruction<Infallible>> {
    let streaming = instruction
        .to_record()
        .is_some_and(|record| record.feature.contains(SME_FEATURE));
    let traps: &[SmeTrapCode] = if streaming {
        &[SmeTrapCode::Disabled, SmeTrapCode::NotInStreamingMode]
    } else {
        &[]
    };
    [None]
        .into_iter()
        .chain(traps.iter().copied().map(Some))
        .map(|trapped| PreciseInstruction::Scalable {
            instruction,
            operation,
            trapped,
        })
        .collect()
}

/// The feature that names SME in the list.
const SME_FEATURE: &str = "FEAT_SME";

/// Each combination of the documented choices of the cases the load or store has.
fn list_load_store_behaviors(
    instruction: LoadStoreInstruction,
) -> Vec<PreciseInstruction<Infallible>> {
    /// Every combination so far, once with each choice of one more case where the instruction has it.
    fn widen<Choice: Copy>(
        combinations: Vec<LoadStoreChoices>,
        open: bool,
        choices: &[Choice],
        set: impl Fn(&mut LoadStoreChoices, Choice),
    ) -> Vec<LoadStoreChoices> {
        if !open {
            return combinations;
        }
        combinations
            .iter()
            .flat_map(|combination| {
                choices.iter().map(|choice| {
                    let mut widened = *combination;
                    set(&mut widened, *choice);
                    widened
                })
            })
            .collect()
    }
    let open = instruction.to_open_cases();
    let none = LoadStoreChoices {
        misaligned_stack_pointer: None,
        across_fault: None,
        load_into_base: None,
        store_of_base: None,
        load_pair_into_one_register: None,
        failed_store_exclusive: None,
        store_exclusive_overlap: None,
        misaligned_exclusive: None,
        fault_address_tag: None,
        reservation_granule_bytes: None,
    };
    let combinations = widen(
        [none].to_vec(),
        open.misaligned_stack_pointer,
        &[
            MisalignedStackPointer::Faults,
            MisalignedStackPointer::Accesses,
        ],
        |choices, choice| choices.misaligned_stack_pointer = Some(choice),
    );
    let combinations = widen(
        combinations,
        open.across_fault,
        &[
            StoreAcrossFault::WritesNothing,
            StoreAcrossFault::WritesBytesBelowFault,
        ],
        |choices, choice| choices.across_fault = Some(choice),
    );
    let combinations = widen(
        combinations,
        open.load_into_base,
        &[
            LoadIntoBaseWithWriteback::WritebackSuppressed,
            LoadIntoBaseWithWriteback::WritebackOverwritesLoad,
            LoadIntoBaseWithWriteback::Undefined,
            LoadIntoBaseWithWriteback::NoOperation,
        ],
        |choices, choice| choices.load_into_base = Some(choice),
    );
    let combinations = widen(
        combinations,
        open.store_of_base,
        &[
            StoreOfBaseWithWriteback::StoresOriginalValue,
            StoreOfBaseWithWriteback::Undefined,
            StoreOfBaseWithWriteback::NoOperation,
        ],
        |choices, choice| choices.store_of_base = Some(choice),
    );
    let combinations = widen(
        combinations,
        open.load_pair_into_one_register,
        &[
            LoadPairIntoOneRegister::KeepsFirstValue,
            LoadPairIntoOneRegister::KeepsSecondValue,
            LoadPairIntoOneRegister::Undefined,
            LoadPairIntoOneRegister::NoOperation,
        ],
        |choices, choice| choices.load_pair_into_one_register = Some(choice),
    );
    let combinations = widen(
        combinations,
        open.failed_store_exclusive,
        &[
            FailedStoreExclusive::ChecksAccess,
            FailedStoreExclusive::LeavesAccessUnchecked,
        ],
        |choices, choice| choices.failed_store_exclusive = Some(choice),
    );
    let combinations = widen(
        combinations,
        open.store_exclusive_overlap,
        &[
            StoreExclusiveOverlap::Runs,
            StoreExclusiveOverlap::Undefined,
            StoreExclusiveOverlap::NoOperation,
        ],
        |choices, choice| choices.store_exclusive_overlap = Some(choice),
    );
    let combinations = widen(
        combinations,
        open.misaligned_exclusive,
        &[
            MisalignedExclusive::AlignmentFault,
            MisalignedExclusive::WithinSixteenBytes,
        ],
        |choices, choice| choices.misaligned_exclusive = Some(choice),
    );
    // No state single-stepping reaches holds the monitor, so the one granule a chip was measured to have is listed.
    let combinations = widen(
        combinations,
        open.reservation_granule,
        &[NEOVERSE_V2_RESERVATION_GRANULE_BYTES],
        |choices, choice| choices.reservation_granule_bytes = Some(choice),
    );
    widen(
        combinations,
        open.fault_address_tag,
        &[
            FaultAddressTag::KeepsTopByte,
            FaultAddressTag::ClearsTopNibble,
        ],
        |choices, choice| choices.fault_address_tag = Some(choice),
    )
    .into_iter()
    // A store exclusive of a byte that leaves its access unchecked never faults, so the fault address it would give is
    // no choice of its own: both tags are one behavior.
    .filter(|choices| {
        !(open.failed_store_exclusive
            && !open.misaligned_exclusive
            && choices.failed_store_exclusive == Some(FailedStoreExclusive::LeavesAccessUnchecked)
            && choices.fault_address_tag == Some(FaultAddressTag::ClearsTopNibble))
    })
    .map(|choices| PreciseInstruction::LoadStore {
        instruction,
        choices,
    })
    .collect()
}
