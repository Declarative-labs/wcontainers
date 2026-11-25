#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::convert::Infallible;

use vstd::prelude::*;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, MemorySink, ProcessorArchitecture, Simulator};
#[cfg(verus_keep_ghost)]
use wcontainers_processor_architecture::{
    Assignment, Memory, apply_all, memory_of, receives, simulated_after, state_after,
};

verus! {

/// Something whose behavior can be measured one instruction at a time, such as a live chip or a recording of one.
pub trait BehaviorSource {
    type Architecture: ProcessorArchitecture;

    /// The processor this source leaves and the changes it makes to memory, running one instruction from the processor and memory before.
    spec fn shown(
        &self,
        processor: <Self::Architecture as ProcessorArchitecture>::ProcessorState,
        memory: Memory<<Self::Architecture as ProcessorArchitecture>::PagePermission>,
    ) -> (
        <Self::Architecture as ProcessorArchitecture>::ProcessorState,
        Seq<Assignment<<Self::Architecture as ProcessorArchitecture>::PagePermission>>,
    );

    /// Executes exactly one instruction on the given processor + memory.
    ///
    /// Processor changes are written in place, memory changes are written through the hook.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut <Self::Architecture as ProcessorArchitecture>::ProcessorState,
        memory: &[MemoryDelta<'_, Self::Architecture>],
        mutation_hook: &mut impl MemorySink<Self::Architecture>,
    )
        ensures
            forall|processor, memory| #[trigger] final(self).shown(processor, memory) == old(self).shown(processor, memory),
            ({
                let (after, changes) = old(self).shown(*old(processor), memory_of(memory@));
                &&& *final(processor) == after
                &&& receives(*old(mutation_hook), *final(mutation_hook), changes)
            }),
    ;

    /// Executes exactly one instruction from the before state.
    fn execute_single_instruction<'memory>(
        &mut self,
        mut before: ExecutionState<'memory, Self::Architecture>,
    ) -> (after: ExecutionState<'memory, Self::Architecture>)
        ensures
            forall|processor, memory| #[trigger] final(self).shown(processor, memory) == old(self).shown(processor, memory),
            (after.processor, memory_of(after.memory_mutations@))
                == state_after(before, old(self).shown(before.processor, memory_of(before.memory_mutations@))),
    {
        let ghost memory = before.memory_mutations@;
        let ghost processor = before.processor;
        let mut mutations: Vec<MemoryDelta<'memory, Self::Architecture>> = Vec::new();
        let ghost empty = mutations;
        self.execute_single_instruction_in_place(
            &mut before.processor,
            before.memory_mutations.as_slice(),
            &mut mutations,
        );
        let ghost filled = mutations;
        before.memory_mutations.append(&mut mutations);
        proof {
            let (_, changes) = old(self).shown(processor, memory_of(memory));
            assert(filled.applied_to(memory_of(memory)) == apply_all(empty.applied_to(memory_of(memory)), changes));
            wcontainers_processor_architecture::memory_proof::lemma_no_assignments(empty@, memory_of(memory));
            wcontainers_processor_architecture::memory_proof::lemma_appended_memory(memory, filled@);
        }
        before
    }

    /// Returns the one behavior of the instruction that this source shows.
    ///
    /// The source runs every generated state, so a source that shows no known behavior fails here and not later.
    #[cfg_attr(verus_keep_ghost, verus::trusted)]
    #[verifier::external_body]
    fn measure_instruction(
        &mut self,
        instruction: <Self::Architecture as ProcessorArchitecture>::Instruction,
        generator: &impl DistinguishingStateGenerator<Self::Architecture>,
    ) -> (result: Result<
        <Self::Architecture as ProcessorArchitecture>::PreciseInstruction,
        MeasureInstructionError<Self::Architecture>,
    >)
    where
        Self::Architecture: Clone,
        <Self::Architecture as ProcessorArchitecture>::PreciseInstruction:
            Simulator<Self::Architecture, Error = Infallible>,
        ensures
            forall|processor, memory| #[trigger] final(self).shown(processor, memory) == old(self).shown(processor, memory),
            result matches Ok(precise) ==> {
                let known = <Self::Architecture as ProcessorArchitecture>::known_behaviors(instruction);
                let states = generator.states(instruction);
                &&& known.contains(precise)
                &&& forall|behavior| #[trigger] known.contains(behavior) ==> (behavior == precise <==>
                    forall|index: int| 0 <= index < states.len() ==> #[trigger] simulated_after(behavior, states[index])
                        == state_after(states[index], old(self).shown(states[index].processor, memory_of(states[index].memory_mutations@))))
            },
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

} // verus!
