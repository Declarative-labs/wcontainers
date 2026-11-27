#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::convert::Infallible;

use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
#[cfg(verus_keep_ghost)]
use wcontainers_processor_architecture::{
    Assignment, Memory, apply_all, assignments, lemma_apply_all_append, memory_of, receives,
    simulated_after, state_after, zeroed,
};
use wcontainers_processor_architecture::{
    ExecutionState, ExecutionStateExpectation, MemorySink, MemorySnapshot, ProcessorArchitecture,
    Simulator,
};

// VERUS: no value of type `Infallible` it can rule out, so a trusted function takes out the success; public, since a
// provided method that names a private function crashes the verifier at a caller in another crate.
pub mod trusted;

verus! {

/// Something whose behavior can be measured one instruction at a time, such as a live chip or a recording of one.
#[allow(reviewable::trait_not_verified, reason = "the contract is not written yet")]
pub trait BehaviorSource {
    type Architecture: ProcessorArchitecture;
    #[allow(
        reviewable::explicit_shared_error,
        reason = "execute_single_instruction only runs execute_single_instruction_in_place and passes its error on unchanged"
    )]
    type Error;

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
        memory: MemorySnapshot<'_, '_, Self::Architecture>,
        mutation_hook: &mut impl MemorySink<Self::Architecture>,
    ) -> (result: Result<(), Self::Error>)
        ensures
            result is Ok ==> forall|processor, memory| #[trigger] final(self).shown(processor, memory) == old(self).shown(processor, memory),
            result is Ok ==> ({
                let (after, changes) = old(self).shown(*old(processor), memory_of(memory.mutations@));
                &&& *final(processor) == after
                &&& receives(*old(mutation_hook), *final(mutation_hook), changes)
            }),
    ;

    /// Executes exactly one instruction from the before state.
    fn execute_single_instruction<'memory>(
        &mut self,
        mut before: ExecutionState<'memory, Self::Architecture>,
    ) -> (after: Result<ExecutionState<'memory, Self::Architecture>, Self::Error>)
        ensures
            after is Ok ==> forall|processor, memory| #[trigger] final(self).shown(processor, memory) == old(self).shown(processor, memory),
            after matches Ok(after) ==> (after.processor, memory_of(after.memory_mutations@))
                == state_after(before, old(self).shown(before.processor, memory_of(before.memory_mutations@))),
    {
        let mut mutations = Vec::new();
        self.execute_single_instruction_in_place(
            &mut before.processor,
            MemorySnapshot {
                mutations: &before.memory_mutations,
            },
            &mut mutations,
        )?;
        proof {
            assert(assignments(before.memory_mutations@ + mutations@) =~= assignments(before.memory_mutations@) + assignments(mutations@));
            lemma_apply_all_append(zeroed::<Self::Architecture>(), assignments(before.memory_mutations@), assignments(mutations@));
            assert(mutations.applied_to(memory_of(before.memory_mutations@))
                == apply_all(memory_of(before.memory_mutations@), assignments(mutations@)));
        }
        before.memory_mutations.append(&mut mutations);
        Ok(before)
    }

    /// Returns the one behavior of the instruction that this source shows.
    ///
    /// The source runs every generated state, so a source that shows no known behavior fails here and not later.
    #[verifier::loop_isolation(false)]
    fn measure_instruction(
        &mut self,
        instruction: <Self::Architecture as ProcessorArchitecture>::Instruction,
        generator: &impl DistinguishingStateGenerator<Self::Architecture>,
    ) -> (result: Result<
        <Self::Architecture as ProcessorArchitecture>::PreciseInstruction,
        MeasureInstructionError<Self::Architecture, Self::Error>,
    >)
    where
        // VERUS: no trait that refers back to itself, so the `Simulator` bound is stated here.
        <Self::Architecture as ProcessorArchitecture>::PreciseInstruction:
            Simulator<Self::Architecture, Error = Infallible>,
        ensures
            result is Ok ==> forall|processor, memory| #[trigger] final(self).shown(processor, memory) == old(self).shown(processor, memory),
            result matches Ok(precise) ==> alone_agrees(
                precise,
                <Self::Architecture as ProcessorArchitecture>::known_behaviors(instruction),
                |processor, memory| old(self).shown(processor, memory),
                generator.states(instruction),
            ),
    {
        let ghost known = <Self::Architecture as ProcessorArchitecture>::known_behaviors(instruction);
        let ghost states = generator.states(instruction);
        let ghost shown: Shown<Self::Architecture> = |processor, memory| old(self).shown(processor, memory);
        let mut candidates: Vec<_> =
            <Self::Architecture as ProcessorArchitecture>::list_known_behaviors(instruction)
                .collect();
        if candidates.is_empty() {
            return Err(MeasureInstructionError::NoKnownBehavior { instruction });
        }
        for before in iterator: generator.generate_states(instruction)
            invariant
                iterator.iter.obeys_prophetic_iter_laws(),
                iterator.seq() == states,
                forall|processor, memory| #[trigger] self.shown(processor, memory) == old(self).shown(processor, memory),
                // The candidates are the known behaviors that agreed with the source on every state so far.
                forall|at: int| 0 <= at < candidates@.len() ==> known.contains(#[trigger] candidates@[at])
                    && agrees_on_every_state(candidates@[at], shown, states.take(iterator.index() as int)),
                forall|behavior| known.contains(behavior) && agrees_on_every_state(behavior, shown, states.take(iterator.index() as int))
                    ==> #[trigger] candidates@.contains(behavior),
        {
            let ghost done = iterator.index() as int;
            // VERUS: no proof through a `map` and `collect` chain here, so the behaviors are simulated by index into a list
            // sized as `collect` sizes it, whose type is named since the loop's invariant reads it first.
            let mut simulated_results: Vec<(_, ExecutionStateExpectation<'static, Self::Architecture>)> =
                Vec::with_capacity(candidates.len());
            let mut index = 0;
            while index < candidates.len()
                invariant
                    index <= candidates@.len(),
                    simulated_results@.len() == index,
                    forall|at: int| 0 <= at < index ==> (#[trigger] simulated_results@[at]).0 == candidates@[at]
                        && (simulated_results@[at].1.processor, memory_of(simulated_results@[at].1.memory_mutations@))
                            == simulated_after(candidates@[at], before),
                decreases candidates@.len() - index,
            {
                let precise = candidates[index];
                let after = trusted::infallible_ok(precise.simulate_state(before.clone()));
                simulated_results.push((precise, after));
                index += 1;
            }
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
            // VERUS: no proof through a `filter` and `collect` chain here, so the agreeing behaviors are kept by index, in a
            // list made to hold them all.
            let mut agreeing = Vec::with_capacity(simulated_results.len());
            let mut index = 0;
            while index < simulated_results.len()
                invariant
                    index <= simulated_results@.len(),
                    // The agreeing behaviors are the candidates so far that agree here too.
                    forall|at: int| 0 <= at < agreeing@.len() ==> known.contains(#[trigger] agreeing@[at])
                        && agrees_on_every_state(agreeing@[at], shown, states.take(done + 1)),
                    forall|from: int| 0 <= from < index && #[trigger] agrees_on(candidates@[from], shown, before)
                        ==> agreeing@.contains(candidates@[from]),
                decreases simulated_results@.len() - index,
            {
                let ghost agreeing_before = agreeing@;
                let agrees = observed_result.meets_expectations(&simulated_results[index].1);
                if agrees {
                    agreeing.push(simulated_results[index].0);
                }
                proof {
                    let candidate = candidates@[index as int];
                    if agrees {
                        assert(agreeing@[agreeing@.len() - 1] == candidate);
                    }
                    assert forall|from: int| 0 <= from < index && #[trigger] agrees_on(candidates@[from], shown, before)
                        implies agreeing@.contains(candidates@[from]) by {
                        let at = choose|at: int| 0 <= at < agreeing_before.len() && agreeing_before[at] == candidates@[from];
                        assert(agreeing@[at] == agreeing_before[at]);
                    }
                }
                index += 1;
            }
            proof {
                assert forall|behavior| known.contains(behavior) && agrees_on_every_state(behavior, shown, states.take(done + 1))
                    implies #[trigger] agreeing@.contains(behavior) by {
                    assert(candidates@.contains(behavior));
                    let at = choose|at: int| 0 <= at < candidates@.len() && candidates@[at] == behavior;
                    assert(agrees_on(candidates@[at], shown, states.take(done + 1)[done]));
                }
            }
            candidates = agreeing;
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
            [precise] => {
                proof {
                    assert(states.take(states.len() as int) =~= states);
                    assert forall|behavior| #[trigger] known.contains(behavior)
                        implies (agrees_on_every_state(behavior, shown, states) <==> behavior == candidates@[0]) by {
                        if agrees_on_every_state(behavior, shown, states) {
                            assert(candidates@.contains(behavior));
                        }
                    }
                }
                Ok(*precise)
            }
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
    ) -> (result: Result<
        Vec<(
            <Self::Architecture as ProcessorArchitecture>::Instruction,
            <Self::Architecture as ProcessorArchitecture>::PreciseInstruction,
        )>,
        MeasureInstructionError<Self::Architecture, Self::Error>,
    >)
    where
        // VERUS: no trait that refers back to itself, so the `Simulator` bound is stated here.
        <Self::Architecture as ProcessorArchitecture>::PreciseInstruction:
            Simulator<Self::Architecture, Error = Infallible>,
        ensures
            result is Ok ==> forall|processor, memory| #[trigger] final(self).shown(processor, memory) == old(self).shown(processor, memory),
            result matches Ok(measurements) ==> measured_alone(
                measurements@,
                <Self::Architecture as ProcessorArchitecture>::instructions().len() as int,
                |processor, memory| old(self).shown(processor, memory),
                |instruction| generator.states(instruction),
            ),
    {
        let ghost instructions = <Self::Architecture as ProcessorArchitecture>::instructions();
        let ghost shown: Shown<Self::Architecture> = |processor, memory| old(self).shown(processor, memory);
        let ghost states = |instruction| generator.states(instruction);
        let mut measurements = Vec::new();
        for instruction in iterator: <Self::Architecture as ProcessorArchitecture>::list_instructions()
            invariant
                iterator.iter.obeys_prophetic_iter_laws(),
                iterator.seq() == instructions,
                instructions == <Self::Architecture as ProcessorArchitecture>::instructions(),
                shown == (|processor, memory| old(self).shown(processor, memory)),
                states == (|instruction| generator.states(instruction)),
                forall|processor, memory| #[trigger] self.shown(processor, memory) == old(self).shown(processor, memory),
                measurements@.len() == iterator.index(),
                measured_alone(measurements@, iterator.index() as int, shown, states),
        {
            let ghost measured_before = measurements@;
            let ghost shown_at_call: Shown<Self::Architecture> = |processor, memory| self.shown(processor, memory);
            measurements.push((
                instruction,
                self.measure_instruction(instruction, generator)?,
            ));
            proof {
                assert(shown_at_call =~= shown);
            }
        }
        Ok(measurements)
    }
}

/// What a source shows, as a function of the processor and memory before.
#[cfg(verus_keep_ghost)]
pub type Shown<Architecture> = spec_fn(
    <Architecture as ProcessorArchitecture>::ProcessorState,
    Memory<<Architecture as ProcessorArchitecture>::PagePermission>,
) -> (
    <Architecture as ProcessorArchitecture>::ProcessorState,
    Seq<Assignment<<Architecture as ProcessorArchitecture>::PagePermission>>,
);

/// The behavior expects what the source shows from the state: the same memory, and a processor with every value the
/// behavior expects.
pub open spec fn agrees_on<Architecture, Behavior>(
    behavior: Behavior,
    shown: Shown<Architecture>,
    state: ExecutionState<'static, Architecture>,
) -> bool
    where
        Architecture: ProcessorArchitecture,
        Behavior: Simulator<Architecture, Error = Infallible>,
{
    let (expected, memory) = simulated_after(behavior, state);
    let (observed, observed_memory) = state_after(state, shown(state.processor, memory_of(state.memory_mutations@)));
    observed_memory == memory && Architecture::meets(observed, expected)
}

/// The behavior expects what the source shows from every one of the states.
pub open spec fn agrees_on_every_state<Architecture, Behavior>(
    behavior: Behavior,
    shown: Shown<Architecture>,
    states: Seq<ExecutionState<'static, Architecture>>,
) -> bool
    where
        Architecture: ProcessorArchitecture,
        Behavior: Simulator<Architecture, Error = Infallible>,
{
    forall|index: int| 0 <= index < states.len() ==> #[trigger] agrees_on(behavior, shown, states[index])
}

/// Of the known behaviors, this one and no other leaves what the source shows on every one of the states.
pub open spec fn alone_agrees<Architecture, Behavior>(
    behavior: Behavior,
    known: Seq<Behavior>,
    shown: Shown<Architecture>,
    states: Seq<ExecutionState<'static, Architecture>>,
) -> bool
    where
        Architecture: ProcessorArchitecture,
        Behavior: Simulator<Architecture, Error = Infallible>,
{
    &&& known.contains(behavior)
    &&& forall|other| #[trigger] known.contains(other) ==> (agrees_on_every_state(other, shown, states) <==> other == behavior)
}

/// Each of the first `count` instructions of the architecture, in order, with the one known behavior that alone agrees
/// with what the source shows on every state generated for it.
pub open spec fn measured_alone<Architecture>(
    measurements: Seq<(Architecture::Instruction, Architecture::PreciseInstruction)>,
    count: int,
    shown: Shown<Architecture>,
    states: spec_fn(Architecture::Instruction) -> Seq<ExecutionState<'static, Architecture>>,
) -> bool
    where
        Architecture: ProcessorArchitecture,
        Architecture::PreciseInstruction: Simulator<Architecture, Error = Infallible>,
{
    &&& measurements.len() == count
    &&& forall|at: int| 0 <= at < count ==> (#[trigger] measurements[at]).0 == Architecture::instructions()[at]
        && alone_agrees(measurements[at].1, Architecture::known_behaviors(measurements[at].0), shown, states(measurements[at].0))
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

} // verus!
