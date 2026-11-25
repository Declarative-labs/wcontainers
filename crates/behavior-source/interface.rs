#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::convert::Infallible;

use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
#[cfg(verus_keep_ghost)]
use wcontainers_processor_architecture::memory_proof::lemma_appended_memory;
#[cfg(verus_keep_ghost)]
use wcontainers_processor_architecture::{
    Assignment, Memory, memory_of, receives, simulated_after, state_after,
};
use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, MemorySink, ProcessorArchitecture, Simulator,
};

// VERUS: no proof apart from the code it proves, so the lemmas the measurement names are a file of their own, public
// since a provided method that names a private function crashes the verifier at a caller in another crate.
pub mod measurement;

#[cfg(verus_keep_ghost)]
use measurement::agrees_on_first;

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
        let mut mutations = Vec::new();
        self.execute_single_instruction_in_place(
            &mut before.processor,
            &before.memory_mutations,
            &mut mutations,
        );
        // VERUS: no fact about the memory of a joined list without its proof, so the proof is named.
        proof { lemma_appended_memory(before.memory_mutations@, mutations); }
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
            ({
                let known = <Self::Architecture as ProcessorArchitecture>::known_behaviors(instruction);
                let states = generator.states(instruction);
                let shown = |processor, memory| old(self).shown(processor, memory);
                match result {
                    Ok(precise) => alone_agrees(precise, known, shown, states),
                    Err(MeasureInstructionError::NoKnownBehavior { .. }) => known.len() == 0 && states.len() == 0,
                    Err(MeasureInstructionError::NoBehaviorAgrees { .. }) => states.len() > 0
                        && forall|behavior| #[trigger] known.contains(behavior) ==> !agrees_on_every_state(behavior, shown, states),
                    Err(MeasureInstructionError::IndistinguishableBehaviors { .. }) => exists|first, second|
                        #![trigger known.contains(first), known.contains(second)]
                        known.contains(first) && known.contains(second) && first != second
                            && agrees_on_every_state(first, shown, states) && agrees_on_every_state(second, shown, states),
                }
            }),
    {
        // VERUS: no function outside the trait can run a source's method, so the loops carry their invariants here; the
        // lemmas they name are in measurement.rs.
        let ghost known = <Self::Architecture as ProcessorArchitecture>::known_behaviors(instruction);
        let ghost states = generator.states(instruction);
        let ghost shown: Shown<Self::Architecture> = |processor, memory| old(self).shown(processor, memory);
        let mut candidates: Vec<_> =
            <Self::Architecture as ProcessorArchitecture>::list_known_behaviors(instruction)
                .collect();
        proof {
            measurement::lemma_listed_candidates(known, candidates@);
        }
        for before in iterator: generator.generate_states(instruction)
            invariant
                iterator.iter.obeys_prophetic_iter_laws(),
                iterator.seq() == states,
                states == generator.states(instruction),
                known == <Self::Architecture as ProcessorArchitecture>::known_behaviors(instruction),
                shown == (|processor, memory| old(self).shown(processor, memory)),
                forall|processor, memory| #[trigger] self.shown(processor, memory) == old(self).shown(processor, memory),
                candidates@.no_duplicates(),
                iterator.index() > 0 ==> candidates@.len() > 0,
                // Each candidate is a known behavior that agreed with the source on every state so far, and each
                // known behavior that did is a candidate.
                forall|at: int| 0 <= at < candidates@.len() ==> known.contains(#[trigger] candidates@[at])
                    && agrees_on_first(candidates@[at], shown, states, iterator.index()),
                forall|behavior| known.contains(behavior) && agrees_on_first(behavior, shown, states, iterator.index())
                    ==> #[trigger] candidates@.contains(behavior),
        {
            let ghost done = iterator.index();
            proof {
                assert(0 <= done < states.len());
                assert(before == states[done]);
            }
            // VERUS: no proof through a `map` and `collect` chain here, so the behaviors are simulated by index into a list
            // sized as `collect` sizes it, whose type is named since the loop's invariant reads it first.
            let mut simulated_results: Vec<(_, ExecutionState<'static, Self::Architecture>)> =
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
                // VERUS: no value of type `Infallible` it can rule out, so the success is taken out by a trusted function.
                let after = measurement::infallible_ok(precise.simulate(before.clone()));
                simulated_results.push((precise, after));
                index += 1;
            }
            let observed_result = self.execute_single_instruction(before.clone());
            proof {
                assert((observed_result.processor, memory_of(observed_result.memory_mutations@))
                    == state_after(before, shown(before.processor, memory_of(before.memory_mutations@))));
            }
            // VERUS: no proof through a `filter` and `collect` chain here, so the agreeing behaviors are kept by index, in a
            // list made to hold them all.
            let mut agreeing = Vec::with_capacity(simulated_results.len());
            // Where in the simulated results each agreeing behavior came from.
            let ghost mut sources: Seq<int> = Seq::empty();
            let mut index = 0;
            while index < simulated_results.len()
                invariant
                    index <= simulated_results@.len(),
                    sources.len() == agreeing@.len(),
                    forall|first: int, second: int| 0 <= first < second < sources.len() ==> #[trigger] sources[first] < #[trigger] sources[second],
                    forall|at: int| 0 <= at < agreeing@.len() ==> 0 <= #[trigger] sources[at] < index
                        && agreeing@[at] == simulated_results@[sources[at]].0
                        && simulated_results@[sources[at]].1.processor == observed_result.processor
                        && memory_of(simulated_results@[sources[at]].1.memory_mutations@) == memory_of(observed_result.memory_mutations@),
                    forall|from: int| 0 <= from < index && #[trigger] simulated_results@[from].1.processor == observed_result.processor
                        && memory_of(simulated_results@[from].1.memory_mutations@) == memory_of(observed_result.memory_mutations@)
                        ==> agreeing@.contains(simulated_results@[from].0),
                decreases simulated_results@.len() - index,
            {
                let ghost agreeing_before = agreeing@;
                let ghost sources_before = sources;
                if simulated_results[index].1 == observed_result {
                    agreeing.push(simulated_results[index].0);
                    proof {
                        sources = sources.push(index as int);
                    }
                }
                proof {
                    measurement::lemma_agreeing_step(simulated_results@, observed_result, index as int, agreeing_before, agreeing@, sources_before,
                        sources);
                }
                index += 1;
            }
            proof {
                measurement::lemma_candidates_step(known, shown, states, done, before, candidates@, simulated_results@, agreeing@, observed_result,
                    sources);
            }
            candidates = agreeing;
            if candidates.is_empty() {
                proof {
                    measurement::lemma_none_agrees(known, shown, states, done + 1, candidates@);
                }
                return Err(MeasureInstructionError::NoBehaviorAgrees {
                    instruction,
                    before,
                    behaviors: simulated_results,
                    observed: observed_result,
                });
            }
        }
        proof {
            measurement::lemma_candidates_at_end(known, shown, states, candidates@);
        }
        // VERUS: no slice pattern with `..`, so the last arm is a wildcard.
        match candidates.as_slice() {
            [] => Err(MeasureInstructionError::NoKnownBehavior { instruction }),
            [precise] => Ok(*precise),
            // No generated state tells these candidates apart, so two known behaviors are the same or our generator lacks a state that separates them.
            _ => Err(MeasureInstructionError::IndistinguishableBehaviors {
                instruction,
                behaviors: candidates,
            }),
        }
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

/// What the source shows, as a function of the processor and memory before.
pub open spec fn shown_by<Source: BehaviorSource + ?Sized>(source: &Source) -> Shown<Source::Architecture> {
    |processor, memory| source.shown(processor, memory)
}

/// Of the instruction's known behaviors, the source shows this one and no other on some states.
pub open spec fn shows_alone<Source>(
    source: &Source,
    instruction: <Source::Architecture as ProcessorArchitecture>::Instruction,
    behavior: <Source::Architecture as ProcessorArchitecture>::PreciseInstruction,
) -> bool
    where
        Source: BehaviorSource + ?Sized,
        <Source::Architecture as ProcessorArchitecture>::PreciseInstruction: Simulator<Source::Architecture, Error = Infallible>,
{
    exists|states: Seq<ExecutionState<'static, Source::Architecture>>| #[trigger] alone_agrees(behavior,
        <Source::Architecture as ProcessorArchitecture>::known_behaviors(instruction), shown_by(source), states)
}

/// The behavior leaves the processor and memory the source shows from every one of the states.
pub open spec fn agrees_on_every_state<Architecture, Behavior>(
    behavior: Behavior,
    shown: Shown<Architecture>,
    states: Seq<ExecutionState<'static, Architecture>>,
) -> bool
    where
        Architecture: ProcessorArchitecture,
        Behavior: Simulator<Architecture, Error = Infallible>,
{
    forall|index: int| 0 <= index < states.len() ==> #[trigger] simulated_after(behavior, states[index])
        == state_after(states[index], shown(states[index].processor, memory_of(states[index].memory_mutations@)))
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
