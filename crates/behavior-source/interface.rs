#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::convert::Infallible;

use vstd::prelude::*;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, MemorySink, ProcessorArchitecture, Simulator};
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;
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
        let ghost known = <Self::Architecture as ProcessorArchitecture>::known_behaviors(instruction);
        let ghost states = generator.states(instruction);
        let mut candidates: Vec<_> =
            <Self::Architecture as ProcessorArchitecture>::list_known_behaviors(instruction)
                .collect();
        proof {
            assert forall|behavior| #[trigger] known.contains(behavior) implies candidates@.contains(behavior) by {}
        }
        for before in iterator: generator.generate_states(instruction)
            invariant
                iterator.iter.obeys_prophetic_iter_laws(),
                iterator.seq() == states,
                states == generator.states(instruction),
                known == <Self::Architecture as ProcessorArchitecture>::known_behaviors(instruction),
                forall|processor, memory| #[trigger] self.shown(processor, memory) == old(self).shown(processor, memory),
                // Each candidate is a known behavior that agreed with the source on every state so far, and each
                // known behavior that did is a candidate.
                forall|at: int| 0 <= at < candidates@.len() ==> known.contains(#[trigger] candidates@[at])
                    && agrees_on_first(candidates@[at], |processor, memory| old(self).shown(processor, memory), states, iterator.index()),
                forall|behavior| known.contains(behavior) && agrees_on_first(behavior, |processor, memory| old(self).shown(processor, memory), states, iterator.index())
                    ==> #[trigger] candidates@.contains(behavior),
        {
            let ghost done = iterator.index();
            proof {
                assert(0 <= done < states.len());
                assert(before == states[done]);
            }
            // VERUS: no closures that call a method with a specification in an iterator chain, so the behaviors are
            // simulated and filtered by index.
            let mut simulated_results: Vec<(_, ExecutionState<'static, Self::Architecture>)> = Vec::new();
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
                let copy = before.clone();
                let simulated = precise.simulate(copy);
                proof {
                    infallible_result_is_ok(simulated);
                }
                let Ok(after) = simulated;
                proof {
                    assert(simulated_after(precise, copy) == simulated_after(precise, before));
                    assert((after.processor, memory_of(after.memory_mutations@)) == simulated_after(precise, before));
                }
                simulated_results.push((precise, after));
                index += 1;
            }
            let copy = before.clone();
            let ghost shown_now = self.shown(copy.processor, memory_of(copy.memory_mutations@));
            let observed_result = self.execute_single_instruction(copy);
            proof {
                assert(shown_now == old(self).shown(copy.processor, memory_of(copy.memory_mutations@)));
                assert(state_after(copy, shown_now) == state_after(before, shown_now));
            }
            let mut agreeing = Vec::new();
            // Where in the simulated results each agreeing behavior came from.
            let ghost mut sources: Seq<int> = Seq::empty();
            let mut index = 0;
            while index < simulated_results.len()
                invariant
                    index <= simulated_results@.len(),
                    sources.len() == agreeing@.len(),
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
                let agrees = simulated_results[index].1 == observed_result;
                if agrees {
                    agreeing.push(simulated_results[index].0);
                }
                proof {
                    if agrees {
                        sources = sources.push(index as int);
                    }
                    assert forall|at: int| 0 <= at < agreeing@.len() implies 0 <= #[trigger] sources[at] < index + 1
                        && agreeing@[at] == simulated_results@[sources[at]].0
                        && simulated_results@[sources[at]].1.processor == observed_result.processor
                        && memory_of(simulated_results@[sources[at]].1.memory_mutations@) == memory_of(observed_result.memory_mutations@) by {
                        if at < agreeing_before.len() {
                            assert(agreeing@[at] == agreeing_before[at]);
                        }
                    }
                    assert forall|from: int| 0 <= from < index + 1 && #[trigger] simulated_results@[from].1.processor == observed_result.processor
                        && memory_of(simulated_results@[from].1.memory_mutations@) == memory_of(observed_result.memory_mutations@)
                        implies agreeing@.contains(simulated_results@[from].0) by {
                        if from < index {
                            assert(agreeing_before.contains(simulated_results@[from].0));
                            let at = choose|at: int| 0 <= at < agreeing_before.len() && agreeing_before[at] == simulated_results@[from].0;
                            assert(agreeing@[at] == agreeing_before[at]);
                        } else {
                            assert(agreeing@[agreeing@.len() - 1] == simulated_results@[from].0);
                        }
                    }
                }
                index += 1;
            }
            proof {
                assert(index == simulated_results@.len());
                lemma_candidates_step(known, |processor, memory| old(self).shown(processor, memory), states, done, before, candidates@, simulated_results@, agreeing@,
                    observed_result, sources);
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
        // VERUS: no slice pattern with `..`, so the last arm is a wildcard.
        match candidates.as_slice() {
            [] => Err(MeasureInstructionError::NoKnownBehavior { instruction }),
            [precise] => {
                proof {
                    lemma_one_candidate(known, |processor, memory| old(self).shown(processor, memory), states, candidates@);
                }
                Ok(*precise)
            },
            // No generated state tells these candidates apart, so two known behaviors are the same or our generator lacks a state that separates them.
            _ => Err(MeasureInstructionError::IndistinguishableBehaviors {
                instruction,
                behaviors: candidates,
            }),
        }
    }
}

/// The behavior left the processor and memory the source shows from each of the first states.
pub open spec fn agrees_on_first<Architecture, Behavior>(
    behavior: Behavior,
    shown: Shown<Architecture>,
    states: Seq<ExecutionState<'static, Architecture>>,
    count: int,
) -> bool
    where
        Architecture: ProcessorArchitecture,
        Behavior: Simulator<Architecture, Error = Infallible>,
{
    forall|index: int| 0 <= index < count ==> #[trigger] simulated_after(behavior, states[index])
        == state_after(states[index], shown(states[index].processor, memory_of(states[index].memory_mutations@)))
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

pub proof fn lemma_candidates_step<Architecture, Behavior>(
    known: Seq<Behavior>,
    shown: Shown<Architecture>,
    states: Seq<ExecutionState<'static, Architecture>>,
    done: int,
    before: ExecutionState<'static, Architecture>,
    candidates: Seq<Behavior>,
    simulated: Seq<(Behavior, ExecutionState<'static, Architecture>)>,
    agreeing: Seq<Behavior>,
    observed: ExecutionState<'static, Architecture>,
    sources: Seq<int>,
)
    where
        Architecture: ProcessorArchitecture,
        Behavior: Simulator<Architecture, Error = Infallible>,
    requires
        0 <= done < states.len(),
        before == states[done],
        (observed.processor, memory_of(observed.memory_mutations@))
            == state_after(before, shown(before.processor, memory_of(before.memory_mutations@))),
        simulated.len() == candidates.len(),
        forall|at: int| 0 <= at < candidates.len() ==> (#[trigger] simulated[at]).0 == candidates[at]
            && (simulated[at].1.processor, memory_of(simulated[at].1.memory_mutations@)) == simulated_after(candidates[at], before),
        forall|at: int| 0 <= at < candidates.len() ==> known.contains(#[trigger] candidates[at])
            && agrees_on_first(candidates[at], shown, states, done),
        forall|behavior| known.contains(behavior) && agrees_on_first(behavior, shown, states, done)
            ==> #[trigger] candidates.contains(behavior),
        sources.len() == agreeing.len(),
        forall|at: int| 0 <= at < agreeing.len() ==> 0 <= #[trigger] sources[at] < simulated.len()
            && agreeing[at] == simulated[sources[at]].0
            && simulated[sources[at]].1.processor == observed.processor
            && memory_of(simulated[sources[at]].1.memory_mutations@) == memory_of(observed.memory_mutations@),
        forall|from: int| 0 <= from < simulated.len() && #[trigger] simulated[from].1.processor == observed.processor
            && memory_of(simulated[from].1.memory_mutations@) == memory_of(observed.memory_mutations@)
            ==> agreeing.contains(simulated[from].0),
    ensures
        forall|at: int| 0 <= at < agreeing.len() ==> known.contains(#[trigger] agreeing[at])
            && agrees_on_first(agreeing[at], shown, states, done + 1),
        forall|behavior| known.contains(behavior) && agrees_on_first(behavior, shown, states, done + 1)
            ==> #[trigger] agreeing.contains(behavior),
{
    let shown_after = state_after(before, shown(before.processor, memory_of(before.memory_mutations@)));
    assert forall|at: int| 0 <= at < agreeing.len() implies known.contains(#[trigger] agreeing[at])
        && agrees_on_first(agreeing[at], shown, states, done + 1) by {
        let from = sources[at];
        assert(simulated[from].0 == candidates[from]);
        assert(known.contains(candidates[from]) && agrees_on_first(candidates[from], shown, states, done));
        assert(simulated_after(candidates[from], before) == shown_after);
        assert forall|index: int| 0 <= index < done + 1 implies #[trigger] simulated_after(agreeing[at], states[index])
            == state_after(states[index], shown(states[index].processor, memory_of(states[index].memory_mutations@))) by {
            if index < done {
                assert(simulated_after(candidates[from], states[index])
                    == state_after(states[index], shown(states[index].processor, memory_of(states[index].memory_mutations@))));
            }
        }
    }
    assert forall|behavior| known.contains(behavior) && agrees_on_first(behavior, shown, states, done + 1)
        implies #[trigger] agreeing.contains(behavior) by {
        assert forall|index: int| 0 <= index < done implies #[trigger] simulated_after(behavior, states[index])
            == state_after(states[index], shown(states[index].processor, memory_of(states[index].memory_mutations@))) by {
            assert(simulated_after(behavior, states[index])
                == state_after(states[index], shown(states[index].processor, memory_of(states[index].memory_mutations@))));
        }
        assert(candidates.contains(behavior));
        let at = choose|at: int| 0 <= at < candidates.len() && candidates[at] == behavior;
        assert(simulated[at].0 == candidates[at]);
        assert(simulated_after(behavior, states[done]) == shown_after);
        assert(simulated[at].1.processor == observed.processor);
    }
}

/// A result whose error is `Infallible` is a success: no value has that type.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_body]
pub proof fn infallible_result_is_ok<T>(result: Result<T, Infallible>)
    ensures
        result is Ok,
{
}

pub proof fn lemma_one_candidate<Architecture, Behavior>(
    known: Seq<Behavior>,
    shown: Shown<Architecture>,
    states: Seq<ExecutionState<'static, Architecture>>,
    candidates: Seq<Behavior>,
)
    where
        Architecture: ProcessorArchitecture,
        Behavior: Simulator<Architecture, Error = Infallible>,
    requires
        candidates.len() == 1,
        forall|at: int| 0 <= at < candidates.len() ==> known.contains(#[trigger] candidates[at])
            && agrees_on_first(candidates[at], shown, states, states.len() as int),
        forall|behavior| known.contains(behavior) && agrees_on_first(behavior, shown, states, states.len() as int)
            ==> #[trigger] candidates.contains(behavior),
    ensures
        known.contains(candidates[0]),
        forall|behavior| #[trigger] known.contains(behavior) ==> (behavior == candidates[0] <==>
            forall|index: int| 0 <= index < states.len() ==> #[trigger] simulated_after(behavior, states[index])
                == state_after(states[index], shown(states[index].processor, memory_of(states[index].memory_mutations@)))),
{
    assert(known.contains(candidates[0]));
    assert forall|behavior| #[trigger] known.contains(behavior) implies (behavior == candidates[0] <==>
        forall|index: int| 0 <= index < states.len() ==> #[trigger] simulated_after(behavior, states[index])
            == state_after(states[index], shown(states[index].processor, memory_of(states[index].memory_mutations@)))) by {
        if agrees_on_first(behavior, shown, states, states.len() as int) {
            assert(candidates.contains(behavior));
        }
        if behavior == candidates[0] {
            assert(agrees_on_first(candidates[0], shown, states, states.len() as int));
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
