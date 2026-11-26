//! The behaviors of the Arm Neoverse V2.

#![no_std]

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture_arm64::{
    Arm64, Instruction, LoadIntoBaseWithWriteback, MisalignedStackPointer, PreciseInstruction,
    StoreAcrossFault, VectorLength, VectorLengthReading,
};

/// Each choice agrees with every state of the recordings beside this file, made under KVM on an AWS Graviton4.
pub struct NeoverseV2;

impl PreciseInstructionSet<Arm64> for NeoverseV2 {
    fn to_precise(&self, instruction: Instruction) -> PreciseInstruction {
        let misaligned_stack_pointer = MisalignedStackPointer::Faults;
        match instruction {
            Instruction::AddShiftedRegister64 => PreciseInstruction::AddShiftedRegister64,
            Instruction::AddsShiftedRegister64 => PreciseInstruction::AddsShiftedRegister64,
            Instruction::Csel64 => PreciseInstruction::Csel64,
            Instruction::LdrImmediateUnsignedOffset64 => {
                PreciseInstruction::LdrImmediateUnsignedOffset64 {
                    misaligned_stack_pointer,
                }
            }
            Instruction::StrImmediateUnsignedOffset64 => {
                PreciseInstruction::StrImmediateUnsignedOffset64 {
                    misaligned_stack_pointer,
                    across_fault: StoreAcrossFault::WritesNothing,
                }
            }
            Instruction::StpSignedOffset64 => PreciseInstruction::StpSignedOffset64 {
                misaligned_stack_pointer,
                across_fault: StoreAcrossFault::WritesNothing,
            },
            Instruction::LdrImmediatePostIndex64 => PreciseInstruction::LdrImmediatePostIndex64 {
                misaligned_stack_pointer,
                into_base: LoadIntoBaseWithWriteback::WritebackSuppressed,
            },
            Instruction::Brk => PreciseInstruction::Brk,
            Instruction::Rdvl => {
                PreciseInstruction::Rdvl(VectorLengthReading::Reads(VectorLength::Bits128))
            }
            Instruction::MrsCtrEl0 => PreciseInstruction::MrsCtrEl0 {
                cache_type: 0xB444_C004,
            },
        }
    }
}
