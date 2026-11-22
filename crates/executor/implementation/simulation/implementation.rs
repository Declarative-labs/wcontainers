//! A stand-in for an ARM64 chip, for tests that need a chip whose behaviors are known.

#![no_std]

use wcontainers_executor::Executor;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{MemoryDelta, ProcessorArchitecture, Simulator};
use wcontainers_processor_architecture_arm64::memory::read_bytes;
use wcontainers_processor_architecture_arm64::{Arm64, Processor};

/// Runs each instruction as the set's precise instruction simulates it.
pub struct Simulation<Set> {
    set: Set,
}

impl<Set: PreciseInstructionSet<Arm64>> Simulation<Set> {
    /// TODO: temporary placeholder until the executor interface says how an executor is built.
    pub const fn new(set: Set) -> Self {
        Self { set }
    }
}

impl<Set: PreciseInstructionSet<Arm64>> Executor for Simulation<Set> {
    type Architecture = Arm64;
    type TranslatorConfig = ();

    /// Panics when the encoding at the program counter is none of the listed instructions.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor,
        memory: &[MemoryDelta<'_, Arm64>],
        on_memory_mutated: impl FnMut(MemoryDelta<'_, Arm64>),
    ) {
        let word = u32::from_le_bytes(read_bytes(memory, processor.program_counter));
        let instruction = Arm64::list_instructions()
            .find(|instruction| {
                let fixed = instruction.to_fixed_bits();
                word & fixed.mask == fixed.value
            })
            .unwrap_or_else(|| panic!("{word:#010x} is none of the listed instructions"));
        let Ok(()) = self.set.to_precise(instruction).simulate_in_place(
            processor,
            memory,
            on_memory_mutated,
        );
    }
}
