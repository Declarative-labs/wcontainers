//! The choices the WebAssembly translator was written to make for ARM64, which its measured behaviors must keep.
//!
//! The translator's set itself is a `MeasuredInstructionSet` measured on the WebAssembly executor.

#![no_std]

use core::convert::Infallible;

use wcontainers_processor_architecture_arm64::{
    Instruction, LoadIntoBaseWithWriteback, LoadStoreChoices, MisalignedStackPointer,
    PreciseInstruction, StoreAcrossFault, VectorLengthReading,
};

#[cfg(test)]
mod tests;

/// The instructions of the first slice, whose behaviors the translator's set was first written by hand for.
pub const SLICE: &[Instruction<Infallible>] = &[
    Instruction::AddShiftedRegister64,
    Instruction::AddsShiftedRegister64,
    Instruction::Csel64,
    Instruction::LdrImmediateUnsignedOffset64,
    Instruction::StrImmediateUnsignedOffset64,
    Instruction::StpSignedOffset64,
    Instruction::LdrImmediatePostIndex64,
    Instruction::Brk,
    Instruction::Rdvl,
    Instruction::MrsCtrEl0,
];

/// Returns whether the behavior gives the translator's answer in every case its hand-written set answered.
///
/// A case that set left open, such as what a fault address keeps of a tag, is left to the measurement.
pub fn keeps_translator_choices(behavior: &PreciseInstruction<Infallible>) -> bool {
    match behavior {
        PreciseInstruction::LoadStore { choices, .. } => keeps_load_store_choices(choices),
        // The translator has no SVE.
        PreciseInstruction::Rdvl(reading) => *reading == VectorLengthReading::Undefined,
        PreciseInstruction::MrsCtrEl0 { cache_type } => {
            *cache_type == wcontainers_arm64::CACHE_TYPE
        }
        _ => true,
    }
}

/// Returns whether a load or store keeps the translator's three answers in the cases it has.
///
/// They are: a misaligned stack pointer faults, a store that partly faults writes nothing, and a post-indexed load
/// into its own base ends with the written-back address, not the loaded value.
fn keeps_load_store_choices(choices: &LoadStoreChoices) -> bool {
    choices
        .misaligned_stack_pointer
        .is_none_or(|choice| choice == MisalignedStackPointer::Faults)
        && choices
            .across_fault
            .is_none_or(|choice| choice == StoreAcrossFault::WritesNothing)
        && choices
            .load_into_base
            .is_none_or(|choice| choice == LoadIntoBaseWithWriteback::WritebackOverwritesLoad)
}
