//! ARM64 as Linux configures it for a program that has used SVE and SME.
//!
//! Where a control could set one privilege level apart from another, it treats them alike: CTR_EL0 is readable,
//! SVE and SME are on (or hidden from every level), and the stack pointer's alignment is checked.

#![no_std]

extern crate alloc;

use alloc::vec::{self, Vec};
use core::marker::PhantomData;
use core::ops::Range;

use wcontainers_privilege_level::PrivilegeLevel;
use wcontainers_processor_architecture::ProcessorArchitecture;

mod behavior;
pub mod branch;
mod condition_flags;
mod documented;
pub mod encoding;
mod exception;
pub mod floating_point;
pub mod integer;
mod listing;
pub mod load_store;
pub mod memory;
pub mod operand;
pub mod pointer_authentication;
mod registers;
pub mod simd;
mod simulation;
pub mod system_register;
#[cfg(test)]
mod tests;

pub use behavior::{
    FailedStoreExclusive, FaultAddressTag, LoadIntoBaseWithWriteback, LoadPairIntoOneRegister,
    MisalignedExclusive, MisalignedStackPointer, StoreAcrossFault, StoreExclusiveOverlap,
    StoreOfBaseWithWriteback, VectorLength, VectorLengthReading, WaitForInterruptBehavior,
};
pub use condition_flags::{CONDITIONS, Condition, ConditionFlags};
pub use documented::{
    DocumentedInstruction, DocumentedOperation, to_documented_operation, to_implicit_inputs,
    writes_condition_flags,
};
pub use exception::{
    AccessDirection, Exception, FaultStatus, SmeTrapCode, SystemAccess, WaitInstruction,
};
pub use listing::{
    Instruction, InstructionClass, InstructionRecord, LISTED_INSTRUCTIONS, PrivilegeBehavior,
};
pub use load_store::{LoadStoreChoices, LoadStoreInstruction};
pub use wcontainers_privilege_level_arm64_user::User;

/// The number of bits of a virtual address under a 4 KiB granule without [FEAT_LVA](https://developer.arm.com/documentation/109697/2025_09/Feature-descriptions/The-Armv8-2-architecture-extension#:~:text=FEAT_LVA%2C%20Large%20VA%20support).
pub const VIRTUAL_ADDRESS_BITS: u32 = 48;
pub const INSTRUCTION_SIZE: u64 = size_of::<u32>() as u64;
/// X0 to X30; the encoding's number after the last names the zero register or the stack pointer.
pub const GENERAL_PURPOSE_REGISTERS: usize = 31;
/// V0 to V31.
pub const VECTOR_REGISTERS: usize = 32;

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
    type PagePermission = PagePermission;
    type Instruction = Instruction<Level::PrivilegedInstruction>;
    type PreciseInstruction = PreciseInstruction<Level::PrecisePrivilegedInstruction>;
    type Instructions = Instructions<Level>;
    type KnownBehaviors = vec::IntoIter<Self::PreciseInstruction>;

    const ADDRESSABLE: Range<u64> = 0..1 << VIRTUAL_ADDRESS_BITS;
    const DEFAULT_PERMISSION: PagePermission = PagePermission::Unmapped;

    /// Every variant of Arm's list in its order, then MRS of CTR_EL0, then those `Level` adds.
    ///
    /// An instruction no behavior models has no known behavior, so measuring it fails as `measure_instruction` says.
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
/// TODO: temporary placeholder until the rest of the state every level has is designed: TPIDRRO_EL0 and TPIDR2_EL0;
/// PSTATE's DIT, SSBS, TCO and BTYPE; SVCR (streaming mode and ZA); Z beyond 128 bits, P and FFR; ZA and ZT0;
/// FPMR; GCSPR_EL0; the exclusive monitor.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Processor<LevelSpecific> {
    pub general_purpose_registers: [u64; GENERAL_PURPOSE_REGISTERS],
    pub stack_pointer: u64,
    pub program_counter: u64,
    pub condition_flags: ConditionFlags,
    pub vector_registers: [u128; VECTOR_REGISTERS],
    /// [FPCR](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/FPCR--Floating-point-Control-Register).
    pub floating_point_control: u64,
    /// [FPSR](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/FPSR--Floating-point-Status-Register).
    pub floating_point_status: u64,
    /// [TPIDR_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/TPIDR-EL0--EL0-Read-Write-Software-Thread-ID-Register).
    pub software_thread_id: u64,
    /// None when the last instruction completed.
    pub exception: Option<Exception>,
    pub level_specific: LevelSpecific,
}

/// Every register zero, the condition flags clear and no exception, at a level that adds no state.
pub const ZERO_PROCESSOR: Processor<()> = Processor {
    general_purpose_registers: [0; GENERAL_PURPOSE_REGISTERS],
    stack_pointer: 0,
    program_counter: 0,
    condition_flags: ConditionFlags::empty(),
    vector_registers: [0; VECTOR_REGISTERS],
    floating_point_control: 0,
    floating_point_status: 0,
    software_thread_id: 0,
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
    LevelSpecific(LevelSpecific),
}
