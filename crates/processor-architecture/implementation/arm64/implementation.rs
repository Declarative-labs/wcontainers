//! ARM64 as a program at EL0 sees it, under an EL1 that configures the processor as Linux does.

#![no_std]

extern crate alloc;

use core::ops::Range;

use wcontainers_processor_architecture::ProcessorArchitecture;

pub mod encoding;
pub mod memory;
mod simulation;
pub mod syndrome;
#[cfg(test)]
mod tests;

/// The number of bits of a virtual address under a 4 KiB granule without [FEAT_LVA](https://developer.arm.com/documentation/109697/2025_09/Feature-descriptions/The-Armv8-2-architecture-extension#:~:text=FEAT_LVA%2C%20Large%20VA%20support).
pub const VIRTUAL_ADDRESS_BITS: u32 = 48;
/// X0 to X30; the encoding's number after the last names the zero register or the stack pointer.
pub const GENERAL_PURPOSE_REGISTERS: usize = 31;
/// V0 to V31.
pub const VECTOR_REGISTERS: usize = 32;
pub const INSTRUCTION_BYTES: u64 = size_of::<u32>() as u64;

/// Every register zero, the condition flags clear and no exception.
pub const ZERO_PROCESSOR: Processor = Processor {
    general_purpose_registers: [0; GENERAL_PURPOSE_REGISTERS],
    stack_pointer: 0,
    program_counter: 0,
    condition_flags: ConditionFlags::empty(),
    vector_registers: [0; VECTOR_REGISTERS],
    floating_point_control: 0,
    floating_point_status: 0,
    software_thread_id: 0,
    exception: None,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Arm64;

impl ProcessorArchitecture for Arm64 {
    type ProcessorState = Processor;
    type PagePermission = PagePermission;
    type Instruction = Instruction;
    type PreciseInstruction = PreciseInstruction;

    const ADDRESSABLE: Range<u64> = 0..1 << VIRTUAL_ADDRESS_BITS;

    fn list_instructions() -> impl Iterator<Item = Instruction> {
        [
            Instruction::AddShiftedRegister64,
            Instruction::AddsShiftedRegister64,
            Instruction::Csel64,
            Instruction::LdrImmediateUnsignedOffset64,
            Instruction::StrImmediateUnsignedOffset64,
            Instruction::StpSignedOffset64,
            Instruction::LdrImmediatePostIndex64,
            Instruction::Brk,
            Instruction::Rdvl,
            Instruction::MrsCtrEl0,
        ]
        .into_iter()
    }
}

/// The registers a program at EL0 can see or change, and the exception its last instruction raised.
///
/// TODO: temporary placeholder until the rest of the EL0 state is designed: TPIDRRO_EL0 and TPIDR2_EL0; PSTATE's
/// DIT, SSBS, TCO and BTYPE; SVCR (streaming mode and ZA); Z beyond 128 bits, P and FFR; ZA and ZT0; FPMR;
/// GCSPR_EL0; the exclusive monitor.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Processor {
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
}

bitflags::bitflags! {
    /// PSTATE's [NZCV](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/NZCV--Condition-Flags), at its bit positions in that register.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct ConditionFlags: u32 {
        const NEGATIVE = 1 << 31;
        const ZERO = 1 << 30;
        const CARRY = 1 << 29;
        const OVERFLOW = 1 << 28;
    }
}

/// A synchronous exception taken to EL1, with the instruction that raised it still at the program counter.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Exception {
    /// [ESR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-).
    pub syndrome: u64,
    /// [FAR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/FAR-EL1--Fault-Address-Register--EL1-), where the syndrome makes it valid.
    pub fault_address: Option<u64>,
}

/// What a program at EL0 may do with a page; memory that was never given a permission is unmapped.
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

/// An A64 [instruction variant](https://developer.arm.com/documentation/ddi0487/mc/-Part-C-The-AArch64-Instruction-Set/-Chapter-C1-The-A64-Instruction-Set/-C1-1-About-the-A64-instruction-set#:~:text=each%20syntax%20is%20an%20instruction%20variant).
///
/// TODO: temporary placeholder until every variant is listed: these are the ten of the first slice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Instruction {
    AddShiftedRegister64,
    AddsShiftedRegister64,
    Csel64,
    LdrImmediateUnsignedOffset64,
    StrImmediateUnsignedOffset64,
    StpSignedOffset64,
    LdrImmediatePostIndex64,
    Brk,
    Rdvl,
    MrsCtrEl0,
}

/// An instruction with one behavior for every case the manual leaves open or a chip does its own way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PreciseInstruction {
    AddShiftedRegister64,
    AddsShiftedRegister64,
    Csel64,
    LdrImmediateUnsignedOffset64 {
        misaligned_stack_pointer: MisalignedStackPointer,
    },
    StrImmediateUnsignedOffset64 {
        misaligned_stack_pointer: MisalignedStackPointer,
        across_fault: StoreAcrossFault,
    },
    StpSignedOffset64 {
        misaligned_stack_pointer: MisalignedStackPointer,
        across_fault: StoreAcrossFault,
    },
    LdrImmediatePostIndex64 {
        misaligned_stack_pointer: MisalignedStackPointer,
        into_base: LoadIntoBaseWithWriteback,
    },
    Brk,
    Rdvl(VectorLengthReading),
    MrsCtrEl0 {
        cache_type: u64,
    },
}

impl PreciseInstruction {
    pub const fn to_instruction(self) -> Instruction {
        match self {
            Self::AddShiftedRegister64 => Instruction::AddShiftedRegister64,
            Self::AddsShiftedRegister64 => Instruction::AddsShiftedRegister64,
            Self::Csel64 => Instruction::Csel64,
            Self::LdrImmediateUnsignedOffset64 { .. } => Instruction::LdrImmediateUnsignedOffset64,
            Self::StrImmediateUnsignedOffset64 { .. } => Instruction::StrImmediateUnsignedOffset64,
            Self::StpSignedOffset64 { .. } => Instruction::StpSignedOffset64,
            Self::LdrImmediatePostIndex64 { .. } => Instruction::LdrImmediatePostIndex64,
            Self::Brk => Instruction::Brk,
            Self::Rdvl(_) => Instruction::Rdvl,
            Self::MrsCtrEl0 { .. } => Instruction::MrsCtrEl0,
        }
    }
}

/// What a load or store does when its base is the stack pointer and that is not a multiple of 16.
///
/// A chip raises the [alignment fault](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#impl-aarch64.CheckSPAlignment.0) SCTLR_EL1.SA0 asks for, as Linux sets it; QEMU leaves the check out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MisalignedStackPointer {
    Faults,
    Accesses,
}

/// What a store writes when part of it faults.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StoreAcrossFault {
    WritesNothing,
    WritesBytesBelowFault,
}

/// The [CONSTRAINED UNPREDICTABLE](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDR--immediate---Load-register--immediate--#:~:text=CONSTRAINED%20UNPREDICTABLE) choice of a post-indexed load whose target is its base.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LoadIntoBaseWithWriteback {
    WritebackSuppressed,
    WritebackOverwritesLoad,
    Undefined,
    NoOperation,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VectorLengthReading {
    Undefined,
    /// Raises the [SME exception](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=SMTC%2C%20bits%20%5B2%3A0%5D), as a chip with SME and no SVE does outside streaming mode.
    SmeTrapped(SmeTrapCode),
    Reads(VectorLength),
}

/// The SMTC field of the syndrome of a trapped SME access.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u64)]
pub enum SmeTrapCode {
    /// CPACR_EL1.SMEN traps it, as it stays where EL1 sees no SME.
    DisabledByCpacr = 0b000,
    /// The instruction is legal only in streaming mode.
    NotInStreamingMode = 0b010,
}

/// An SVE vector length, as [ZCR_EL1.LEN](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ZCR-EL1--SVE-Control-Register-EL1-#:~:text=LEN%2C%20bits%20%5B3%3A0%5D) permits it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VectorLength {
    Bits128,
    Bits256,
    Bits512,
    Bits1024,
    Bits2048,
}

/// Every behavior this crate defines for the instruction, with three cache types that differ in their lowest and
/// highest bits.
///
/// TODO: temporary placeholder until the interface lists an instruction's behaviors.
pub fn list_behaviors(instruction: Instruction) -> impl Iterator<Item = PreciseInstruction> {
    use alloc::vec::Vec;

    const MISALIGNED: [MisalignedStackPointer; 2] = [
        MisalignedStackPointer::Faults,
        MisalignedStackPointer::Accesses,
    ];
    const ACROSS_FAULT: [StoreAcrossFault; 2] = [
        StoreAcrossFault::WritesNothing,
        StoreAcrossFault::WritesBytesBelowFault,
    ];
    const INTO_BASE: [LoadIntoBaseWithWriteback; 4] = [
        LoadIntoBaseWithWriteback::WritebackSuppressed,
        LoadIntoBaseWithWriteback::WritebackOverwritesLoad,
        LoadIntoBaseWithWriteback::Undefined,
        LoadIntoBaseWithWriteback::NoOperation,
    ];
    const LENGTHS: [VectorLength; 5] = [
        VectorLength::Bits128,
        VectorLength::Bits256,
        VectorLength::Bits512,
        VectorLength::Bits1024,
        VectorLength::Bits2048,
    ];
    let behaviors: Vec<PreciseInstruction> = match instruction {
        Instruction::AddShiftedRegister64 => [PreciseInstruction::AddShiftedRegister64].to_vec(),
        Instruction::AddsShiftedRegister64 => [PreciseInstruction::AddsShiftedRegister64].to_vec(),
        Instruction::Csel64 => [PreciseInstruction::Csel64].to_vec(),
        Instruction::LdrImmediateUnsignedOffset64 => MISALIGNED
            .map(
                |misaligned_stack_pointer| PreciseInstruction::LdrImmediateUnsignedOffset64 {
                    misaligned_stack_pointer,
                },
            )
            .to_vec(),
        Instruction::StrImmediateUnsignedOffset64 => MISALIGNED
            .iter()
            .flat_map(|&misaligned_stack_pointer| {
                ACROSS_FAULT.map(
                    |across_fault| PreciseInstruction::StrImmediateUnsignedOffset64 {
                        misaligned_stack_pointer,
                        across_fault,
                    },
                )
            })
            .collect(),
        Instruction::StpSignedOffset64 => MISALIGNED
            .iter()
            .flat_map(|&misaligned_stack_pointer| {
                ACROSS_FAULT.map(|across_fault| PreciseInstruction::StpSignedOffset64 {
                    misaligned_stack_pointer,
                    across_fault,
                })
            })
            .collect(),
        Instruction::LdrImmediatePostIndex64 => MISALIGNED
            .iter()
            .flat_map(|&misaligned_stack_pointer| {
                INTO_BASE.map(|into_base| PreciseInstruction::LdrImmediatePostIndex64 {
                    misaligned_stack_pointer,
                    into_base,
                })
            })
            .collect(),
        Instruction::Brk => [PreciseInstruction::Brk].to_vec(),
        Instruction::Rdvl => [
            VectorLengthReading::Undefined,
            VectorLengthReading::SmeTrapped(SmeTrapCode::DisabledByCpacr),
            VectorLengthReading::SmeTrapped(SmeTrapCode::NotInStreamingMode),
        ]
        .into_iter()
        .chain(LENGTHS.map(VectorLengthReading::Reads))
        .map(PreciseInstruction::Rdvl)
        .collect(),
        Instruction::MrsCtrEl0 => [0, 1, u64::MAX]
            .map(|cache_type| PreciseInstruction::MrsCtrEl0 { cache_type })
            .to_vec(),
    };
    behaviors.into_iter()
}
