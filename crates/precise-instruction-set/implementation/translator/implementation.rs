//! The behaviors the WebAssembly translator gives ARM64 instructions.

#![no_std]

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture_arm64::{
    Arm64, Instruction, LoadIntoBaseWithWriteback, MisalignedStackPointer, PreciseInstruction,
    StoreAcrossFault, VectorLengthReading,
};

/// Each choice is what the WebAssembly executor's suite run shows the translator does.
pub struct Translator;

impl PreciseInstructionSet<Arm64> for Translator {
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
                into_base: LoadIntoBaseWithWriteback::WritebackOverwritesLoad,
            },
            Instruction::Brk => PreciseInstruction::Brk,
            // The translator has no SVE.
            Instruction::Rdvl => PreciseInstruction::Rdvl(VectorLengthReading::Undefined),
            Instruction::MrsCtrEl0 => PreciseInstruction::MrsCtrEl0 {
                cache_type: wcontainers_arm64::CACHE_TYPE,
            },
        }
    }
}
