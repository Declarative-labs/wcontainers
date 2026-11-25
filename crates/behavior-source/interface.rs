#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::convert::Infallible;

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, MemorySink, ProcessorArchitecture, Simulator,
};

/// Something whose behavior can be measured one instruction at a time, such as a live chip or a recording of one.
pub trait BehaviorSource {
    type Architecture: ProcessorArchitecture;

    /// Executes exactly one instruction on the given processor + memory.
    ///
    /// Processor changes are written in place, memory changes are written through the hook.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut <Self::Architecture as ProcessorArchitecture>::ProcessorState,
        memory: &[MemoryDelta<'_, Self::Architecture>],
        mutation_hook: &mut impl MemorySink<Self::Architecture>,
    );

    /// Executes exactly one instruction from the before state.
    fn execute_single_instruction<'memory>(
        &mut self,
        mut before: ExecutionState<'memory, Self::Architecture>,
    ) -> ExecutionState<'memory, Self::Architecture> {
        let mut mutations = Vec::new();
        self.execute_single_instruction_in_place(
            &mut before.processor,
            &before.memory_mutations,
            &mut mutations,
        );
        before.memory_mutations.append(&mut mutations);
        before
    }

    /// Returns the one behavior of the instruction that this source shows.
    ///
    /// The source runs every generated state, so a source that shows no known behavior fails here and not later.
    fn measure_instruction(
        &mut self,
        instruction: <Self::Architecture as ProcessorArchitecture>::Instruction,
        generator: &impl DistinguishingStateGenerator<Self::Architecture>,
    ) -> Result<
        <Self::Architecture as ProcessorArchitecture>::PreciseInstruction,
        MeasureInstructionError<Self::Architecture>,
    >
    where
        Self::Architecture: Clone,
        <Self::Architecture as ProcessorArchitecture>::PreciseInstruction:
            Simulator<Self::Architecture, Error = Infallible>,
    {
        let mut candidates: Vec<_> =
            <Self::Architecture as ProcessorArchitecture>::list_known_behaviors(instruction)
                .collect();
        for before in generator.generate_states(instruction) {
            let simulated_results: Vec<_> = candidates
                .iter()
                .map(|&precise| {
                    let Ok(after) = precise.simulate(before.clone());
                    (precise, after)
                })
                .collect();
            let observed_result = self.execute_single_instruction(before.clone());
            candidates = simulated_results
                .iter()
                .filter(|(_, after)| *after == observed_result)
                .map(|&(precise, _)| precise)
                .collect();
            if candidates.is_empty() {
                return Err(MeasureInstructionError::NoBehaviorAgrees {
                    instruction,
                    before,
                    behaviors: simulated_results,
                    observed: observed_result,
                });
            }
        }
        match candidates.as_slice() {
            [] => Err(MeasureInstructionError::NoKnownBehavior { instruction }),
            [precise] => Ok(*precise),
            // No generated state tells these candidates apart, so two known behaviors are the same or our generator lacks a state that separates them.
            [_, _, ..] => Err(MeasureInstructionError::IndistinguishableBehaviors {
                instruction,
                behaviors: candidates,
            }),
        }
    }
}

pub enum MeasureInstructionError<Architecture: ProcessorArchitecture> {
    NoKnownBehavior {
        instruction: Architecture::Instruction,
    },
    /// On this state the source did what none of the behaviors still in play does.
    NoBehaviorAgrees {
        instruction: Architecture::Instruction,
        before: ExecutionState<'static, Architecture>,
        /// Each behavior with the state after it simulates.
        behaviors: Vec<(
            Architecture::PreciseInstruction,
            ExecutionState<'static, Architecture>,
        )>,
        observed: ExecutionState<'static, Architecture>,
    },
    /// No generated state tells these behaviors apart.
    IndistinguishableBehaviors {
        instruction: Architecture::Instruction,
        behaviors: Vec<Architecture::PreciseInstruction>,
    },
}
