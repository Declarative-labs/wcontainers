use alloc::vec::Vec;

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{Arm64, Instruction};

use crate::CriticalValueProduct;

fn generate_states(
    instruction: Instruction,
) -> impl Iterator<Item = ExecutionState<'static, Arm64>> {
    DistinguishingStateGenerator::<Arm64>::generate_states(&CriticalValueProduct, instruction)
}

#[test]
fn states_per_instruction() {
    let counts: Vec<(Instruction, usize)> = Arm64::list_instructions()
        .map(|instruction| (instruction, generate_states(instruction).count()))
        .collect();
    assert_eq!(
        counts,
        [
            (Instruction::AddShiftedRegister64, 5184),
            (Instruction::AddsShiftedRegister64, 5184),
            (Instruction::Csel64, 6912),
            (Instruction::LdrImmediateUnsignedOffset64, 1728),
            (Instruction::StrImmediateUnsignedOffset64, 1728),
            (Instruction::StpSignedOffset64, 8640),
            (Instruction::LdrImmediatePostIndex64, 2880),
            (Instruction::Brk, 3),
            (Instruction::Rdvl, 15),
            (Instruction::MrsCtrEl0, 3),
        ]
    );
}

#[test]
fn every_state_is_about_to_run_its_instruction() {
    for instruction in Arm64::list_instructions() {
        let fixed = instruction.to_fixed_bits();
        for state in generate_states(instruction) {
            let word = state.memory_mutations.iter().find_map(|delta| match delta {
                MemoryDelta::ValueAssignment { start, bytes }
                    if *start == state.processor.program_counter =>
                {
                    <[u8; 4]>::try_from(&bytes[..]).ok().map(u32::from_le_bytes)
                }
                MemoryDelta::ValueAssignment { .. } | MemoryDelta::PermissionAssignment { .. } => {
                    None
                }
            });
            assert_eq!(
                word.map(|word| word & fixed.mask),
                Some(fixed.value),
                "{instruction:?}"
            );
        }
    }
}
