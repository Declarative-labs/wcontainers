//! ARM64 as Linux configures it for a program that has used SVE and SME.
//!
//! Where a control could set one privilege level apart from another, it treats them alike: CTR_EL0 is readable,
//! SVE and SME are on (or hidden from every level), and the stack pointer's alignment is checked.

#![no_std]

extern crate alloc;

use alloc::vec::{self, Vec};
use core::convert::Infallible;
use core::marker::PhantomData;
use core::ops::Range;

use wcontainers_privilege_level::PrivilegeLevel;
use wcontainers_processor_architecture::ProcessorArchitecture;

pub mod advanced_simd;
#[cfg(feature = "bake")]
mod bake;
mod behavior;
pub mod branch;
mod condition_flags;
mod core_identity;
pub mod cryptography;
mod documented;
pub mod encoding;
mod exception;
mod expectation;
#[cfg(test)]
mod expectation_tests;
pub mod floating_point;
pub mod integer;
mod listing;
pub mod load_store;
pub mod memory;
pub mod memory_tagging;
pub mod operand;
pub mod planted;
pub mod pointer_authentication;
mod registers;
pub mod scalable;
pub mod scalable_floating_point;
pub mod scalable_integer;
pub mod simd;
mod simulation;
mod structure;
pub mod sve;
mod sve_count;
mod sve_element;
mod sve_move;
mod sve_predicate;
pub mod sve_remaining;
pub mod sve_remaining_floating_point;
pub mod sve_remaining_integer;
mod sve_remaining_planted;
#[cfg(test)]
mod sve_tests;
pub mod system_instruction;
pub mod system_register;
pub mod system_state_planted;
#[cfg(test)]
mod tests;

pub use behavior::{
    FailedStoreExclusive, FaultAddressTag, LoadIntoBaseWithWriteback, LoadPairIntoOneRegister,
    MisalignedExclusive, MisalignedStackPointer, StoreAcrossFault, StoreExclusiveOverlap,
    StoreOfBaseWithWriteback, VectorLength, VectorLengthReading, WaitForInterruptBehavior,
};
pub use condition_flags::{CONDITIONS, Condition, ConditionFlags};
pub use core_identity::CoreIdentity;
pub use documented::{
    DocumentedInstruction, DocumentedOperation, to_documented_operation, to_implicit_inputs,
    writes_condition_flags,
};
pub use exception::{
    AccessDirection, Exception, FaultStatus, SmeTrapCode, SystemAccess, WaitInstruction,
};
pub use expectation::{ProcessorExpectation, SuppressibleLoad, UncomparedBits};
pub use listing::{
    Instruction, InstructionClass, InstructionRecord, LISTED_INSTRUCTIONS, PrivilegeBehavior,
};
pub use load_store::{LoadStoreChoices, LoadStoreInstruction};
pub use structure::{StructureChoices, StructureInstruction};
pub use wcontainers_privilege_level_arm64_user::User;

/// The number of bits of a virtual address under a 4 KiB granule without [FEAT_LVA](https://developer.arm.com/documentation/109697/2025_09/Feature-descriptions/The-Armv8-2-architecture-extension#:~:text=FEAT_LVA%2C%20Large%20VA%20support).
pub const VIRTUAL_ADDRESS_BITS: u32 = 48;
pub const INSTRUCTION_SIZE: u64 = size_of::<u32>() as u64;
/// X0 to X30; the encoding's number after the last names the zero register or the stack pointer.
pub const GENERAL_PURPOSE_REGISTERS: usize = 31;
/// V0 to V31.
pub const VECTOR_REGISTERS: usize = 32;
/// P0 to P15, the SVE predicate registers.
pub const PREDICATE_REGISTERS: usize = 16;

/// What every privilege level has, and what `Level` adds to it.
///
/// ```compile_fail,E0277
/// let _: Option<wcontainers_processor_architecture_arm64::Arm64<String>> = None;
/// ```
pub struct Arm64<Level: PrivilegeLevel>(PhantomData<Level>);

// Written out because a derive would ask the same of the level.
impl<Level: PrivilegeLevel> Clone for Arm64<Level> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl<Level: PrivilegeLevel> Default for Arm64<Level> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<Level: PrivilegeLevel> PartialEq for Arm64<Level> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<Level: PrivilegeLevel> Eq for Arm64<Level> {}

impl<Level: PrivilegeLevel> ProcessorArchitecture for Arm64<Level> {
    type ProcessorState = Processor<Level::PrivilegedProcessorSubset>;
    type ProcessorStateExpectation = ProcessorExpectation<Level::PrivilegedProcessorSubset>;
    type PagePermission = PagePermission;
    type Instruction = Instruction<Level::PrivilegedInstruction>;
    type PreciseInstruction = PreciseInstruction<Level::PrecisePrivilegedInstruction>;
    type Instructions = Instructions<Level>;
    type KnownBehaviors = vec::IntoIter<Self::PreciseInstruction>;

    const ADDRESSABLE: Range<u64> = 0..1 << VIRTUAL_ADDRESS_BITS;
    const DEFAULT_PERMISSION: PagePermission = PagePermission::Unmapped;

    /// Every variant of Arm's list in its order, then MRS of CTR_EL0, then those `Level` adds.
    ///
    /// A mandatory instruction no behavior models has no known behavior, so measuring it fails as `measure_instruction`
    /// says; an optional one has at least the behavior of a chip without its feature.
    fn list_instructions() -> Instructions<Level> {
        let every_level: Vec<_> = Instruction::LISTED
            .into_iter()
            .chain([Instruction::MrsCtrEl0])
            .collect();
        Instructions {
            every_level: every_level.into_iter(),
            level_specific: Level::list_specific_instructions(),
        }
    }

    fn list_known_behaviors(
        instruction: Instruction<Level::PrivilegedInstruction>,
    ) -> vec::IntoIter<Self::PreciseInstruction> {
        behavior::list_known_behaviors(instruction).into_iter()
    }

    /// Compares every value a behavior fixes; the exclusive monitor is never compared, since no program and so no
    /// source reads it: a later store exclusive shows it.
    fn meets_expectations(
        observed: &Processor<Level::PrivilegedProcessorSubset>,
        expectation: &ProcessorExpectation<Level::PrivilegedProcessorSubset>,
    ) -> bool {
        expectation.is_met_by(observed)
    }
}

/// The local [exclusive monitor](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-exclusive#:~:text=IsExclusiveLocal), which a load exclusive sets and a store
/// exclusive or CLREX opens: state the chip keeps that no program reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExclusiveMonitor {
    Open,
    /// Marks the reservation granule holding the address a load exclusive read.
    Exclusive {
        address: u64,
    },
}

/// The instructions every privilege level has, then those `Level` adds.
pub struct Instructions<Level: PrivilegeLevel> {
    every_level: vec::IntoIter<Instruction<Level::PrivilegedInstruction>>,
    level_specific: Level::SpecificInstructions,
}

impl<Level: PrivilegeLevel> Iterator for Instructions<Level> {
    type Item = Instruction<Level::PrivilegedInstruction>;

    fn next(&mut self) -> Option<Self::Item> {
        self.every_level
            .next()
            .or_else(|| self.level_specific.next().map(Instruction::LevelSpecific))
    }
}

/// The registers every privilege level has, the exception its last instruction raised, and what one level adds.
///
/// The SVE registers are those of a 128-bit vector length: each Z register is its vector register, and each
/// predicate holds one bit for each of its bytes.
///
/// TODO: temporary placeholder until the rest of the state every level has is designed: TPIDRRO_EL0 and TPIDR2_EL0;
/// PSTATE's BTYPE; SVCR (streaming mode and ZA); Z, P and FFR beyond a 128-bit vector length; ZA and ZT0; FPMR;
/// GCSPR_EL0.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Processor<LevelSpecific> {
    pub general_purpose_registers: [u64; GENERAL_PURPOSE_REGISTERS],
    pub stack_pointer: u64,
    pub program_counter: u64,
    pub condition_flags: ConditionFlags,
    /// V0 to V31, which are Z0 to Z31 at a 128-bit vector length.
    pub vector_registers: [u128; VECTOR_REGISTERS],
    /// P0 to P15.
    pub predicate_registers: [u16; PREDICATE_REGISTERS],
    /// [FFR](https://developer.arm.com/documentation/ddi0487/latest/-Part-C-The-AArch64-Instruction-Set/-Chapter-C1-The-A64-Instruction-Set/-C1-4-Registers-in-AArch64-execution-state#:~:text=FFR), the first-fault register.
    pub first_fault_register: u16,
    /// [FPCR](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/FPCR--Floating-point-Control-Register).
    pub floating_point_control: u64,
    /// [FPSR](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/FPSR--Floating-point-Status-Register).
    pub floating_point_status: u64,
    /// [TPIDR_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/TPIDR-EL0--EL0-Read-Write-Software-Thread-ID-Register).
    pub software_thread_id: u64,
    /// PSTATE's [SSBS](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/SSBS--Speculative-Store-Bypass-Safe),
    /// [DIT](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/DIT--Data-Independent-Timing) and
    /// [TCO](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/TCO--Tag-Check-Override), each at its
    /// bit of SPSR_EL1 and of what MRS reads, every other bit zero.
    pub process_state_controls: u64,
    pub exclusive_monitor: ExclusiveMonitor,
    /// None when the last instruction completed.
    pub exception: Option<Exception>,
    pub level_specific: LevelSpecific,
}

/// Every register zero, the condition flags clear, the exclusive monitor open and no exception, at a level that adds no
/// state.
pub const ZERO_PROCESSOR: Processor<()> = Processor {
    general_purpose_registers: [0; GENERAL_PURPOSE_REGISTERS],
    stack_pointer: 0,
    program_counter: 0,
    condition_flags: ConditionFlags::empty(),
    vector_registers: [0; VECTOR_REGISTERS],
    predicate_registers: [0; PREDICATE_REGISTERS],
    first_fault_register: 0,
    floating_point_control: 0,
    floating_point_status: 0,
    software_thread_id: 0,
    process_state_controls: 0,
    exclusive_monitor: ExclusiveMonitor::Open,
    exception: None,
    level_specific: (),
};

/// What the running program may do with a page; memory that was never given a permission is unmapped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PagePermission {
    Unmapped,
    Mapped {
        data_access: DataAccess,
        instruction_fetch: InstructionFetch,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DataAccess {
    Forbidden,
    ReadOnly,
    ReadWrite,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InstructionFetch {
    Forbidden,
    Allowed,
}

/// An instruction with one behavior for every case the manual leaves open or a chip does its own way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PreciseInstruction<LevelSpecific> {
    Documented(DocumentedInstruction),
    LoadStore {
        instruction: LoadStoreInstruction,
        choices: LoadStoreChoices,
    },
    Brk,
    Rdvl(VectorLengthReading),
    WaitForInterrupt(WaitForInterruptBehavior),
    /// MRS (a read) or MSR (register) (a write), with what the chip fixes about the registers it reaches.
    SystemRegisterMove {
        direction: AccessDirection,
        choices: system_register::SystemRegisterChoices,
    },
    MrsCtrEl0 {
        cache_type: u64,
    },
    /// An SVE instruction of [`sve`] at a vector length of 128 bits, or where `trapped`, raising the SME exception as a
    /// chip with SME and without SVE does outside streaming mode.
    Scalable {
        instruction: Instruction<Infallible>,
        operation: sve::ScalableOperation,
        trapped: Option<SmeTrapCode>,
    },
    /// An SVE contiguous load, store or prefetch, with what the chip fixes about its access.
    ScalableAccess {
        instruction: Instruction<Infallible>,
        choices: load_store::scalable_access::ScalableAccessChoices,
    },
    /// HINT, MSR (immediate), SYS or SYSL.
    SystemInstruction(system_instruction::SystemInstructionBehavior),
    /// An SVE gather, scatter, prefetch of vector addresses, or first-fault or non-fault load, with what the chip fixes
    /// about its access.
    ElementAccess {
        instruction: Instruction<Infallible>,
        choices: load_store::scalable_element_access::ElementAccessChoices,
    },
    /// LDG, a tag store or STGP.
    TagAccess {
        instruction: memory_tagging::TagAccessInstruction,
        choices: memory_tagging::TagAccessChoices,
    },
    /// A load or store of Advanced SIMD structures.
    Structure {
        instruction: StructureInstruction,
        choices: StructureChoices,
    },
    /// What a chip without the instruction's feature does with its word.
    Unimplemented(Instruction<Infallible>),
    LevelSpecific(LevelSpecific),
}
