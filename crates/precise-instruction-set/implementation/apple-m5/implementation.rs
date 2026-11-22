//! The behaviors of the Apple M5.

#![no_std]

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture_arm64::{
    Arm64, Instruction, LoadIntoBaseWithWriteback, PreciseInstruction, StorePairAcrossFault,
    VectorLengthReading,
};

pub struct AppleM5;

/// TODO: temporary placeholder until the M5's recordings: every choice below is expected, none is measured yet.
impl PreciseInstructionSet<Arm64> for AppleM5 {
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
                LoadIntoBaseWithWriteback::WritebackSuppressed,
            ),
            Instruction::Brk => PreciseInstruction::Brk,
            // RDVL x0, #1 raised SIGILL at EL0 under macOS 27.2: the M5 has SME without SVE.
            Instruction::Rdvl => PreciseInstruction::Rdvl(VectorLengthReading::Undefined),
            // macOS traps CTR_EL0 at EL0 (SIGILL), so the value is readable only inside our virtual machine.
            Instruction::MrsCtrEl0 => PreciseInstruction::MrsCtrEl0 { cache_type: 0 },
        }
    }
}
