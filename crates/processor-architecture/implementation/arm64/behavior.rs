//! The behaviors an instruction can have where the manual leaves a choice or chips differ.

use alloc::vec::Vec;
use core::convert::Infallible;

use crate::{Instruction, PreciseInstruction, SmeTrapCode};

/// What a load or store does when its base is the stack pointer and that is not a multiple of 16.
///
/// A chip raises the [alignment fault](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#impl-aarch64.CheckSPAlignment.0) Linux turns on for every level; QEMU leaves the check out.
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

/// An SVE vector length, as [ZCR_EL1.LEN](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ZCR-EL1--SVE-Control-Register-EL1-#:~:text=LEN%2C%20bits%20%5B3%3A0%5D) permits it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    pub const fn to_instruction(self) -> Instruction<Infallible> {
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

/// Returns every behavior a chip is known to show for the instruction, each cache type one that a chip was seen to read.
pub(crate) fn list_known_behaviors<LevelInstruction, LevelPrecise: Copy>(
    instruction: Instruction<LevelInstruction>,
) -> Vec<PreciseInstruction<LevelPrecise>> {
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
    const CACHE_TYPES: [u64; 3] = [
        NEOVERSE_V2_CACHE_TYPE,
        APPLE_M5_CACHE_TYPE,
        PRESENTED_CACHE_TYPE,
    ];
    match instruction {
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
            VectorLengthReading::SmeTrapped(SmeTrapCode::Disabled),
            VectorLengthReading::SmeTrapped(SmeTrapCode::NotInStreamingMode),
        ]
        .into_iter()
        .chain(LENGTHS.map(VectorLengthReading::Reads))
        .map(PreciseInstruction::Rdvl)
        .collect(),
        Instruction::MrsCtrEl0 => CACHE_TYPES
            .map(|cache_type| PreciseInstruction::MrsCtrEl0 { cache_type })
            .to_vec(),
        // TODO: temporary placeholder until the privilege level interface lists the behaviors of a level's instructions.
        Instruction::LevelSpecific(_) => Vec::new(),
    }
}

/// [CTR_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CTR-EL0--Cache-Type-Register) as an Arm Neoverse V2 under KVM reads it, measured on an AWS Graviton4.
const NEOVERSE_V2_CACHE_TYPE: u64 = 0xB444_C004;

/// [CTR_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CTR-EL0--Cache-Type-Register) as an Apple M5 reads it inside a virtual machine, since macOS traps a program's read.
const APPLE_M5_CACHE_TYPE: u64 = 0x9_9444_C004;

/// [CTR_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CTR-EL0--Cache-Type-Register) as the translator presents it and unicorn's `max` model reads it.
const PRESENTED_CACHE_TYPE: u64 = 0x8444_C004;
