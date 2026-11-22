//! The behaviors unicorn-engine's `max` processor model gives ARM64 instructions, as the stand-in executor runs them.

#![no_std]

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture_arm64::{
    Arm64, Instruction, LoadIntoBaseWithWriteback, PreciseInstruction, StorePairAcrossFault,
    VectorLengthReading,
};

pub struct UnicornMax;

/// Each choice is what the stand-in executor's suite run shows it does; the store pair writes nothing because the stand-in
/// undoes the bytes unicorn writes below a fault.
impl PreciseInstructionSet<Arm64> for UnicornMax {
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
            Instruction::Rdvl => PreciseInstruction::Rdvl(VectorLengthReading::Undefined),
            Instruction::MrsCtrEl0 => PreciseInstruction::MrsCtrEl0 {
                cache_type: 0x8444_C004,
            },
        }
    }
}
