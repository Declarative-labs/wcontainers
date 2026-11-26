//! The behaviors of the Apple M5.

#![no_std]

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture_arm64::{
    Arm64, Instruction, LoadIntoBaseWithWriteback, MisalignedStackPointer, PreciseInstruction,
    SmeTrapCode, StoreAcrossFault, VectorLengthReading,
};

/// Each choice agrees with every state of the recordings beside this file, made under Hypervisor.framework on an
/// Apple M5.
pub struct AppleM5;

impl PreciseInstructionSet<Arm64> for AppleM5 {
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
                into_base: LoadIntoBaseWithWriteback::Undefined,
            },
            Instruction::Brk => PreciseInstruction::Brk,
            // Hypervisor.framework shows the guest no SME, so its EL1 leaves SME trapped; an M5 has SME and no SVE.
            Instruction::Rdvl => PreciseInstruction::Rdvl(VectorLengthReading::SmeTrapped(
                SmeTrapCode::DisabledByCpacr,
            )),
            // Read inside the virtual machine: macOS traps CTR_EL0 at EL0.
            Instruction::MrsCtrEl0 => PreciseInstruction::MrsCtrEl0 {
                cache_type: 0x9_9444_C004,
            },
        }
    }
}
