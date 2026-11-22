//! The behaviors of the Arm Neoverse V2.

#![no_std]

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture_arm64::{
    Arm64, Instruction, LoadIntoBaseWithWriteback, PreciseInstruction, StorePairAcrossFault,
    VectorLength, VectorLengthReading,
};

pub struct NeoverseV2;

/// TODO: temporary placeholder until the V2's recordings: the store pair and the load into its base are expected, not measured.
impl PreciseInstructionSet<Arm64> for NeoverseV2 {
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
            // RDVL x0, #1 read 16 at EL0 under Linux 7.0 on a Graviton4.
            Instruction::Rdvl => {
                PreciseInstruction::Rdvl(VectorLengthReading::Reads(VectorLength::Bits128))
            }
            // Read at EL0 under Linux 7.0 on a Graviton4, outside a virtual machine.
            Instruction::MrsCtrEl0 => PreciseInstruction::MrsCtrEl0 {
                cache_type: 0xB444_C004,
            },
        }
    }
}
