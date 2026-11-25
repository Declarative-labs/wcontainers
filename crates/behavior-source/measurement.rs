//! The lemmas `measure_instruction` and `measure_instructions` name, and the one trusted function they call; none of it
//! is part of the specification.

use core::convert::Infallible;

use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use wcontainers_processor_architecture::{
    ExecutionState, ProcessorArchitecture, Simulator, memory_of, simulated_after, state_after,
};

#[cfg(verus_keep_ghost)]
use crate::{Shown, agrees_on_every_state, alone_agrees, measured_alone};

verus! {

/// The value of a result whose error is `Infallible`, which no value has.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_body]
pub fn infallible_ok<T>(result: Result<T, Infallible>) -> (value: T)
    ensures
        result == Ok::<T, Infallible>(value),
{
    match result {
        Ok(value) => value,
        Err(never) => match never {},
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

/// The listed behaviors are the known ones, without repeats.
pub proof fn lemma_listed_candidates<Behavior>(known: Seq<Behavior>, candidates: Seq<Behavior>)
    requires
        candidates == known,
        known.no_duplicates(),
    ensures
        candidates.no_duplicates(),
        forall|behavior| #[trigger] known.contains(behavior) ==> candidates.contains(behavior),
{
}

/// One more simulated result looked at keeps the agreeing behaviors those that agreed so far, in order.
pub proof fn lemma_agreeing_step<Architecture, Behavior>(
    simulated: Seq<(Behavior, ExecutionState<'static, Architecture>)>,
    observed: ExecutionState<'static, Architecture>,
    index: int,
    agreeing_before: Seq<Behavior>,
    agreeing: Seq<Behavior>,
    sources_before: Seq<int>,
    sources: Seq<int>,
)
    where
        Architecture: ProcessorArchitecture,
    requires
        0 <= index < simulated.len(),
        sources_before.len() == agreeing_before.len(),
        forall|first: int, second: int| 0 <= first < second < sources_before.len()
            ==> #[trigger] sources_before[first] < #[trigger] sources_before[second],
        forall|at: int| 0 <= at < agreeing_before.len() ==> 0 <= #[trigger] sources_before[at] < index
            && agreeing_before[at] == simulated[sources_before[at]].0
            && simulated[sources_before[at]].1.processor == observed.processor
            && memory_of(simulated[sources_before[at]].1.memory_mutations@) == memory_of(observed.memory_mutations@),
        forall|from: int| 0 <= from < index && #[trigger] simulated[from].1.processor == observed.processor
            && memory_of(simulated[from].1.memory_mutations@) == memory_of(observed.memory_mutations@)
            ==> agreeing_before.contains(simulated[from].0),
        ({
            let agrees = simulated[index].1.processor == observed.processor
                && memory_of(simulated[index].1.memory_mutations@) == memory_of(observed.memory_mutations@);
            &&& agrees ==> agreeing == agreeing_before.push(simulated[index].0) && sources == sources_before.push(index)
            &&& !agrees ==> agreeing == agreeing_before && sources == sources_before
        }),
    ensures
        sources.len() == agreeing.len(),
        forall|first: int, second: int| 0 <= first < second < sources.len() ==> #[trigger] sources[first] < #[trigger] sources[second],
        forall|at: int| 0 <= at < agreeing.len() ==> 0 <= #[trigger] sources[at] < index + 1
            && agreeing[at] == simulated[sources[at]].0
            && simulated[sources[at]].1.processor == observed.processor
            && memory_of(simulated[sources[at]].1.memory_mutations@) == memory_of(observed.memory_mutations@),
        forall|from: int| 0 <= from < index + 1 && #[trigger] simulated[from].1.processor == observed.processor
            && memory_of(simulated[from].1.memory_mutations@) == memory_of(observed.memory_mutations@)
            ==> agreeing.contains(simulated[from].0),
{
    assert forall|first: int, second: int| 0 <= first < second < sources.len()
        implies #[trigger] sources[first] < #[trigger] sources[second] by {
        if second < sources_before.len() {
            assert(sources[first] == sources_before[first] && sources[second] == sources_before[second]);
        } else {
            assert(sources[first] == sources_before[first]);
        }
    }
    assert forall|at: int| 0 <= at < agreeing.len() implies 0 <= #[trigger] sources[at] < index + 1
        && agreeing[at] == simulated[sources[at]].0
        && simulated[sources[at]].1.processor == observed.processor
        && memory_of(simulated[sources[at]].1.memory_mutations@) == memory_of(observed.memory_mutations@) by {
        if at < agreeing_before.len() {
            assert(agreeing[at] == agreeing_before[at]);
            assert(sources[at] == sources_before[at]);
        }
    }
    assert forall|from: int| 0 <= from < index + 1 && #[trigger] simulated[from].1.processor == observed.processor
        && memory_of(simulated[from].1.memory_mutations@) == memory_of(observed.memory_mutations@)
        implies agreeing.contains(simulated[from].0) by {
        if from < index {
            assert(agreeing_before.contains(simulated[from].0));
            let at = choose|at: int| 0 <= at < agreeing_before.len() && agreeing_before[at] == simulated[from].0;
            assert(agreeing[at] == agreeing_before[at]);
        } else {
            assert(agreeing[agreeing.len() - 1] == simulated[from].0);
        }
    }
}

/// The behaviors that agreed on the states so far and on this one are the candidates whose simulation the source
/// matched here, without repeats.
#[verifier::spinoff_prover]
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
        candidates.no_duplicates(),
        simulated.len() == candidates.len(),
        forall|at: int| 0 <= at < candidates.len() ==> (#[trigger] simulated[at]).0 == candidates[at]
            && (simulated[at].1.processor, memory_of(simulated[at].1.memory_mutations@)) == simulated_after(candidates[at], before),
        forall|at: int| 0 <= at < candidates.len() ==> known.contains(#[trigger] candidates[at])
            && agrees_on_first(candidates[at], shown, states, done),
        forall|behavior| known.contains(behavior) && agrees_on_first(behavior, shown, states, done)
            ==> #[trigger] candidates.contains(behavior),
        sources.len() == agreeing.len(),
        forall|first: int, second: int| 0 <= first < second < sources.len() ==> #[trigger] sources[first] < #[trigger] sources[second],
        forall|at: int| 0 <= at < agreeing.len() ==> 0 <= #[trigger] sources[at] < simulated.len()
            && agreeing[at] == simulated[sources[at]].0
            && simulated[sources[at]].1.processor == observed.processor
            && memory_of(simulated[sources[at]].1.memory_mutations@) == memory_of(observed.memory_mutations@),
        forall|from: int| 0 <= from < simulated.len() && #[trigger] simulated[from].1.processor == observed.processor
            && memory_of(simulated[from].1.memory_mutations@) == memory_of(observed.memory_mutations@)
            ==> agreeing.contains(simulated[from].0),
    ensures
        agreeing.no_duplicates(),
        forall|at: int| 0 <= at < agreeing.len() ==> known.contains(#[trigger] agreeing[at])
            && agrees_on_first(agreeing[at], shown, states, done + 1),
        forall|behavior| known.contains(behavior) && agrees_on_first(behavior, shown, states, done + 1)
            ==> #[trigger] agreeing.contains(behavior),
{
    let shown_after = state_after(before, shown(before.processor, memory_of(before.memory_mutations@)));
    assert forall|first: int, second: int| 0 <= first < second < agreeing.len() implies agreeing[first] != agreeing[second] by {
        assert(sources[first] < sources[second]);
        assert(agreeing[first] == candidates[sources[first]]);
        assert(agreeing[second] == candidates[sources[second]]);
    }
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

/// Agreeing on the first states, all of them, is agreeing on every state.
pub proof fn lemma_agrees_on_all<Architecture, Behavior>(
    shown: Shown<Architecture>,
    behavior: Behavior,
    states: Seq<ExecutionState<'static, Architecture>>,
)
    where
        Architecture: ProcessorArchitecture,
        Behavior: Simulator<Architecture, Error = Infallible>,
    ensures
        agrees_on_first(behavior, shown, states, states.len() as int) <==> agrees_on_every_state(behavior, shown, states),
{
}

/// When no known behavior agreed on the first states, none agrees on every state.
pub proof fn lemma_none_agrees<Architecture, Behavior>(
    known: Seq<Behavior>,
    shown: Shown<Architecture>,
    states: Seq<ExecutionState<'static, Architecture>>,
    count: int,
    candidates: Seq<Behavior>,
)
    where
        Architecture: ProcessorArchitecture,
        Behavior: Simulator<Architecture, Error = Infallible>,
    requires
        0 < count <= states.len(),
        candidates.len() == 0,
        forall|behavior| known.contains(behavior) && agrees_on_first(behavior, shown, states, count)
            ==> #[trigger] candidates.contains(behavior),
    ensures
        states.len() > 0,
        forall|behavior| #[trigger] known.contains(behavior) ==> !agrees_on_every_state(behavior, shown, states),
{
    assert forall|behavior| #[trigger] known.contains(behavior) implies !agrees_on_every_state(behavior, shown, states) by {
        lemma_agrees_on_all(shown, behavior, states);
        if agrees_on_every_state(behavior, shown, states) {
            assert forall|index: int| 0 <= index < count implies #[trigger] simulated_after(behavior, states[index])
                == state_after(states[index], shown(states[index].processor, memory_of(states[index].memory_mutations@))) by {
                assert(simulated_after(behavior, states[index])
                    == state_after(states[index], shown(states[index].processor, memory_of(states[index].memory_mutations@))));
            }
            assert(candidates.contains(behavior));
        }
    }
}

/// What the candidates left after every state say: none means no behavior is known and no state was generated, one
/// is the behavior that alone agrees, and more are two different known behaviors that both agree.
#[verifier::spinoff_prover]
pub proof fn lemma_candidates_at_end<Architecture, Behavior>(
    known: Seq<Behavior>,
    shown: Shown<Architecture>,
    states: Seq<ExecutionState<'static, Architecture>>,
    candidates: Seq<Behavior>,
)
    where
        Architecture: ProcessorArchitecture,
        Behavior: Simulator<Architecture, Error = Infallible>,
    requires
        candidates.no_duplicates(),
        states.len() > 0 ==> candidates.len() > 0,
        forall|at: int| 0 <= at < candidates.len() ==> known.contains(#[trigger] candidates[at])
            && agrees_on_first(candidates[at], shown, states, states.len() as int),
        forall|behavior| known.contains(behavior) && agrees_on_first(behavior, shown, states, states.len() as int)
            ==> #[trigger] candidates.contains(behavior),
    ensures
        candidates.len() == 0 ==> known.len() == 0 && states.len() == 0,
        candidates.len() == 1 ==> alone_agrees(candidates[0], known, shown, states),
        candidates.len() > 1 ==> exists|first, second| #![trigger known.contains(first), known.contains(second)]
            known.contains(first) && known.contains(second) && first != second
                && agrees_on_every_state(first, shown, states) && agrees_on_every_state(second, shown, states),
{
    if candidates.len() == 0 && known.len() > 0 {
        assert(agrees_on_first(known[0], shown, states, states.len() as int));
        assert(known.contains(known[0]));
        assert(candidates.contains(known[0]));
    }
    if candidates.len() == 1 {
        assert(known.contains(candidates[0]));
        assert forall|behavior| #[trigger] known.contains(behavior)
            implies (agrees_on_every_state(behavior, shown, states) <==> behavior == candidates[0]) by {
            lemma_agrees_on_all(shown, behavior, states);
            if agrees_on_first(behavior, shown, states, states.len() as int) {
                assert(candidates.contains(behavior));
            }
        }
    }
    if candidates.len() > 1 {
        let (first, second) = (candidates[0], candidates[1]);
        assert(known.contains(first) && known.contains(second));
        lemma_agrees_on_all(shown, first, states);
        lemma_agrees_on_all(shown, second, states);
    }
}

/// One more instruction measured, the next one listed, keeps every measured instruction with the one behavior that
/// alone agrees with the source.
pub proof fn lemma_measured_one_more<Architecture>(
    measured_before: Seq<(Architecture::Instruction, Architecture::PreciseInstruction)>,
    measured: Seq<(Architecture::Instruction, Architecture::PreciseInstruction)>,
    shown: Shown<Architecture>,
    shown_at_call: Shown<Architecture>,
    states: spec_fn(Architecture::Instruction) -> Seq<ExecutionState<'static, Architecture>>,
)
    where
        Architecture: ProcessorArchitecture,
        Architecture::PreciseInstruction: Simulator<Architecture, Error = Infallible>,
    requires
        measured_alone(measured_before, measured_before.len() as int, shown, states),
        measured.len() == measured_before.len() + 1,
        forall|at: int| 0 <= at < measured_before.len() ==> #[trigger] measured[at] == measured_before[at],
        measured.last().0 == Architecture::instructions()[measured_before.len() as int],
        alone_agrees(measured.last().1, Architecture::known_behaviors(measured.last().0), shown_at_call, states(measured.last().0)),
        forall|processor, memory| #[trigger] shown_at_call(processor, memory) == shown(processor, memory),
    ensures
        measured_alone(measured, measured.len() as int, shown, states),
{
    assert(shown_at_call =~= shown);
    assert forall|at: int| 0 <= at < measured.len() implies (#[trigger] measured[at]).0 == Architecture::instructions()[at]
        && alone_agrees(measured[at].1, Architecture::known_behaviors(measured[at].0), shown, states(measured[at].0)) by {
        if at < measured_before.len() {
            assert(measured[at] == measured_before[at]);
        } else {
            assert(measured[at] == measured.last());
        }
    }
}

} // verus!

#[cfg(test)]
mod tests {
    extern crate std;

    use core::convert::Infallible;

    use super::infallible_ok;

    /// The one trusted line of this crate, checked on every kind of value it can be given: an `Ok`, since no `Err` of
    /// `Infallible` can be made.
    #[test]
    fn the_value_of_an_infallible_result_is_its_success() {
        for value in [u64::MIN, 1, u64::MAX] {
            assert_eq!(infallible_ok(Ok::<u64, Infallible>(value)), value);
        }
        let text = alloc::string::String::from("kept");
        assert_eq!(infallible_ok(Ok::<_, Infallible>(text.clone())), text);
    }
}
