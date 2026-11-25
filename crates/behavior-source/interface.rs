#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::convert::Infallible;

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::{
    ExecutionState, ExecutionStateExpectation, MemorySink, MemorySnapshot, ProcessorArchitecture,
    Simulator,
};

/// Something whose behavior can be measured one instruction at a time, such as a live chip or a recording of one.
pub trait BehaviorSource {
    type Architecture: ProcessorArchitecture;
    #[allow(
        reviewable::explicit_shared_error,
        reason = "execute_single_instruction only runs execute_single_instruction_in_place and passes its error on unchanged"
    )]
    type Error;

    /// Executes exactly one instruction on the given processor + memory.
    ///
    /// Processor changes are written in place, memory changes are written through the hook.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut <Self::Architecture as ProcessorArchitecture>::ProcessorState,
        memory: MemorySnapshot<'_, '_, Self::Architecture>,
        mutation_hook: &mut impl MemorySink<Self::Architecture>,
    ) -> Result<(), Self::Error>;

    /// Executes exactly one instruction from the before state.
    fn execute_single_instruction<'memory>(
        &mut self,
        mut before: ExecutionState<'memory, Self::Architecture>,
    ) -> Result<ExecutionState<'memory, Self::Architecture>, Self::Error> {
        let mut mutations = Vec::new();
        self.execute_single_instruction_in_place(
            &mut before.processor,
            MemorySnapshot {
                mutations: &before.memory_mutations,
            },
            &mut mutations,
        )?;
        before.memory_mutations.append(&mut mutations);
        Ok(before)
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
        MeasureInstructionError<Self::Architecture, Self::Error>,
    >
    where
        // VERUS: no trait that refers back to itself, so the `Simulator` bound is stated here.
        <Self::Architecture as ProcessorArchitecture>::PreciseInstruction:
            Simulator<Self::Architecture, Error = Infallible>,
    {
        let mut candidates: Vec<_> =
            <Self::Architecture as ProcessorArchitecture>::list_known_behaviors(instruction)
                .collect();
        if candidates.is_empty() {
            return Err(MeasureInstructionError::NoKnownBehavior { instruction });
        }
        for before in generator.generate_states(instruction) {
            let simulated_results: Vec<_> = candidates
                .iter()
                .map(|&precise| {
                    let Ok(after) = precise.simulate_state(before.clone());
                    (precise, after)
                })
                .collect();
            let observed_result = match self.execute_single_instruction(before.clone()) {
                Ok(observed_result) => observed_result,
                Err(error) => {
                    return Err(MeasureInstructionError::ExecutionFailed {
                        instruction,
                        before,
                        error,
                    });
                }
            };
            candidates = simulated_results
                .iter()
                .filter(|(_, after)| observed_result.meets_expectations(after))
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
            [precise] => Ok(*precise),
            // No generated state tells these candidates apart, so two known behaviors are the same or our generator lacks a state that separates them.
            _ => Err(MeasureInstructionError::IndistinguishableBehaviors {
                instruction,
                behaviors: candidates,
            }),
        }
    }

    /// Returns each instruction of the architecture with the one behavior this source shows for it.
    fn measure_instructions(
        &mut self,
        generator: &impl DistinguishingStateGenerator<Self::Architecture>,
    ) -> Result<
        Vec<(
            <Self::Architecture as ProcessorArchitecture>::Instruction,
            <Self::Architecture as ProcessorArchitecture>::PreciseInstruction,
        )>,
        MeasureInstructionError<Self::Architecture, Self::Error>,
    >
    where
        // VERUS: no trait that refers back to itself, so the `Simulator` bound is stated here.
        <Self::Architecture as ProcessorArchitecture>::PreciseInstruction:
            Simulator<Self::Architecture, Error = Infallible>,
    {
        let mut measurements = Vec::new();
        for instruction in <Self::Architecture as ProcessorArchitecture>::list_instructions() {
            measurements.push((
                instruction,
                self.measure_instruction(instruction, generator)?,
            ));
        }
        Ok(measurements)
    }
}

pub enum MeasureInstructionError<Architecture: ProcessorArchitecture, ExecutionError> {
    NoKnownBehavior {
        instruction: Architecture::Instruction,
    },
    ExecutionFailed {
        instruction: Architecture::Instruction,
        before: ExecutionState<'static, Architecture>,
        error: ExecutionError,
    },
    /// On this state the source did what none of the behaviors still in play does.
    NoBehaviorAgrees {
        instruction: Architecture::Instruction,
        before: ExecutionState<'static, Architecture>,
        /// Each behavior with what it expects after.
        behaviors: Vec<(
            Architecture::PreciseInstruction,
            ExecutionStateExpectation<'static, Architecture>,
        )>,
        observed: ExecutionState<'static, Architecture>,
    },
    /// No generated state tells these behaviors apart.
    IndistinguishableBehaviors {
        instruction: Architecture::Instruction,
        behaviors: Vec<Architecture::PreciseInstruction>,
    },
}
