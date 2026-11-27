//! The behaviors of the Apple M5.

#![no_std]

use core::convert::Infallible;

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture_arm64::{
    Arm64, DocumentedInstruction, Instruction, LoadIntoBaseWithWriteback, LoadStoreChoices,
    LoadStoreInstruction, MisalignedStackPointer, PreciseInstruction, SmeTrapCode,
    StoreAcrossFault, User, VectorLengthReading,
};

/// Each choice agrees with every state of its recordings in the recording source, made under Hypervisor.framework on an
/// Apple M5.
pub struct AppleM5;

impl PreciseInstructionSet<Arm64<User>> for AppleM5 {
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
            load_into_base: Some(LoadIntoBaseWithWriteback::Undefined),
            store_of_base: None,
            load_pair_into_one_register: None,
            failed_store_exclusive: None,
            store_exclusive_overlap: None,
            misaligned_exclusive: None,
            misaligned_ordered_or_atomic: None,
            fault_address_tag: None,
            reservation_granule_bytes: None,
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
                // Hypervisor.framework shows the guest no SME, so its EL1 leaves SME trapped; an M5 has SME and no SVE.
                Instruction::Rdvl => Some(PreciseInstruction::Rdvl(
                    VectorLengthReading::SmeTrapped(SmeTrapCode::Disabled),
                )),
                // Read inside the virtual machine: macOS traps CTR_EL0 at EL0.
                Instruction::MrsCtrEl0 => Some(PreciseInstruction::MrsCtrEl0 {
                    cache_type: 0x9_9444_C004,
                }),
                _ => None,
            };
        match precise {
            Some(precise) => precise,
            None => panic!("only the instructions of the recordings have a behavior"),
        }
    }
}
