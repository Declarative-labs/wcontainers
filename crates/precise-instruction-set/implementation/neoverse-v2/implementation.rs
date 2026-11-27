//! The behaviors of the Arm Neoverse V2.

#![no_std]

use core::convert::Infallible;

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture_arm64::{
    Arm64, DocumentedInstruction, Instruction, LoadIntoBaseWithWriteback, LoadStoreChoices,
    LoadStoreInstruction, MisalignedStackPointer, PreciseInstruction, StoreAcrossFault, User,
    VectorLength, VectorLengthReading,
};

/// Each choice agrees with every state of its recordings in the recording source, made under KVM on an AWS Graviton4.
pub struct NeoverseV2;

impl PreciseInstructionSet<Arm64<User>> for NeoverseV2 {
    // The choices are written out below; no measurement changes them.
    fn from_measurements(_: &[(Instruction<Infallible>, PreciseInstruction<Infallible>)]) -> Self {
        Self
    }

    /// Panics for an instruction none of its recordings holds.
    fn to_precise(&self, instruction: Instruction<Infallible>) -> PreciseInstruction<Infallible> {
        // Each load or store keeps the choices of the cases it has.
        let load_store = LoadStoreChoices {
            misaligned_stack_pointer: Some(MisalignedStackPointer::Faults),
            across_fault: Some(StoreAcrossFault::WritesNothing),
            load_into_base: Some(LoadIntoBaseWithWriteback::WritebackSuppressed),
            store_of_base: None,
            load_pair_into_one_register: None,
            failed_store_exclusive: None,
            store_exclusive_overlap: None,
            misaligned_exclusive: None,
            misaligned_ordered_or_atomic: None,
            fault_address_tag: None,
        };
        let precise =
            match instruction {
                Instruction::AddShiftedRegister64
                | Instruction::AddsShiftedRegister64
                | Instruction::Csel64 => {
                    DocumentedInstruction::new(instruction).map(PreciseInstruction::Documented)
                }
                Instruction::LdrImmediateUnsignedOffset64
                | Instruction::StrImmediateUnsignedOffset64
                | Instruction::StpSignedOffset64
                | Instruction::LdrImmediatePostIndex64 => LoadStoreInstruction::new(instruction)
                    .map(|instruction| PreciseInstruction::LoadStore {
                        instruction,
                        choices: load_store.restrict_to(instruction),
                    }),
                Instruction::Brk => Some(PreciseInstruction::Brk),
                Instruction::Rdvl => Some(PreciseInstruction::Rdvl(VectorLengthReading::Reads(
                    VectorLength::Bits128,
                ))),
                Instruction::MrsCtrEl0 => Some(PreciseInstruction::MrsCtrEl0 {
                    cache_type: 0xB444_C004,
                }),
                _ => None,
            };
        match precise {
            Some(precise) => precise,
            None => panic!("only the instructions of the recordings have a behavior"),
        }
    }
}
