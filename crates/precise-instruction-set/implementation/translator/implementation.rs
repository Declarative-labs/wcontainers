//! The behaviors the WebAssembly translator gives ARM64 instructions.

#![no_std]

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture_arm64::{
    Arm64, Instruction, LoadIntoBaseWithWriteback, PreciseInstruction, StorePairAcrossFault,
    VectorLengthReading,
};

pub struct Translator;

/// Each choice is what the WebAssembly executor's suite run shows the translator does.
impl PreciseInstructionSet<Arm64> for Translator {
    fn to_precise(&self, instruction: Instruction) -> PreciseInstruction {
        match instruction {
            Instruction::AddShiftedRegister64 => PreciseInstruction::AddShiftedRegister64,
            Instruction::AddsShiftedRegister64 => PreciseInstruction::AddsShiftedRegister64,
            Instruction::Csel64 => PreciseInstruction::Csel64,
            Instruction::LdrImmediateUnsignedOffset64 => {
                PreciseInstruction::LdrImmediateUnsignedOffset64
            }
            Instruction::StrImmediateUnsignedOffset64 => {
                PreciseInstruction::StrImmediateUnsignedOffset64
            }
            Instruction::StpSignedOffset64 => {
                PreciseInstruction::StpSignedOffset64(StorePairAcrossFault::WritesNothing)
            }
            Instruction::LdrImmediatePostIndex64 => PreciseInstruction::LdrImmediatePostIndex64(
                LoadIntoBaseWithWriteback::WritebackOverwritesLoad,
            ),
            Instruction::Brk => PreciseInstruction::Brk,
            // The translator has no SVE.
            Instruction::Rdvl => PreciseInstruction::Rdvl(VectorLengthReading::Undefined),
            Instruction::MrsCtrEl0 => PreciseInstruction::MrsCtrEl0 {
                cache_type: wcontainers_arm64::CACHE_TYPE,
            },
        }
    }
}
