//! The precise instruction set of a chip, found by running every generated state on a source of its behavior and
//! keeping the one known behavior that agrees with it.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::convert::Infallible;
use core::marker::PhantomData;

use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;
use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
#[cfg(verus_keep_ghost)]
use wcontainers_behavior_source::{alone_agrees, shown_by, shows_alone};
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
#[cfg(verus_keep_ghost)]
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture::{ProcessorArchitecture, Simulator};

#[cfg(test)]
mod tests;

type InstructionOf<Source> =
    <<Source as BehaviorSource>::Architecture as ProcessorArchitecture>::Instruction;
type PreciseInstructionOf<Source> =
    <<Source as BehaviorSource>::Architecture as ProcessorArchitecture>::PreciseInstruction;

verus! {

/// Each listed instruction with the behavior a source showed on every state `Generator` makes for it.
pub struct MeasuredInstructionSet<Source: BehaviorSource, Generator> {
    behaviors: Vec<(InstructionOf<Source>, PreciseInstructionOf<Source>)>,
    measured_with: PhantomData<(Source, Generator)>,
}

/// The behavior of the first entry for the instruction.
pub open spec fn first_behavior<Instruction, Precise>(behaviors: Seq<(Instruction, Precise)>, instruction: Instruction) -> Precise
    decreases behaviors.len(),
{
    if behaviors.len() == 0 {
        arbitrary()
    } else if behaviors[0].0 == instruction {
        behaviors[0].1
    } else {
        first_behavior(behaviors.drop_first(), instruction)
    }
}

impl<Source, Generator> PreciseInstructionSet<Source::Architecture>
    for MeasuredInstructionSet<Source, Generator>
where
    Source: BehaviorSource,
    Source::Architecture: Clone,
    PreciseInstructionOf<Source>: Simulator<Source::Architecture, Error = Infallible>,
    Generator: DistinguishingStateGenerator<Source::Architecture> + Default,
{
    type Source = Source;
    type MeasureError = MeasureInstructionError<Source::Architecture>;

    closed spec fn behavior(&self, instruction: InstructionOf<Source>) -> PreciseInstructionOf<Source> {
        first_behavior(self.behaviors@, instruction)
    }

    fn measure(mut source: Source) -> (set: Result<Self, Self::MeasureError>) {
        let ghost instructions = <Source::Architecture as ProcessorArchitecture>::instructions();
        let ghost shown = shown_by(&source);
        let ghost given = source;
        let generator = Generator::default();
        // VERUS: no closure that captures a mutable reference, so the instructions are measured in a loop, into a list
        // whose type is named, since the loop's invariant reads it first.
        let mut behaviors: Vec<(InstructionOf<Source>, PreciseInstructionOf<Source>)> = Vec::new();
        for instruction in iterator: Source::Architecture::list_instructions()
            invariant
                iterator.iter.obeys_prophetic_iter_laws(),
                iterator.seq() == instructions,
                instructions == <Source::Architecture as ProcessorArchitecture>::instructions(),
                instructions.no_duplicates(),
                forall|processor, memory| #[trigger] source.shown(processor, memory) == shown(processor, memory),
                behaviors@.len() == iterator.index(),
                forall|at: int| 0 <= at < behaviors@.len() ==> #[trigger] behaviors@[at].0 == instructions[at]
                    && alone_agrees(behaviors@[at].1, <Source::Architecture as ProcessorArchitecture>::known_behaviors(instructions[at]),
                        shown, generator.states(instructions[at])),
        {
            let ghost before_call = source;
            let ghost at = iterator.index();
            proof {
                assert(instruction == instructions[at]);
            }
            let precise = source.measure_instruction(instruction, &generator)?;
            proof {
                assert((|processor, memory| before_call.shown(processor, memory)) =~= shown);
                assert(alone_agrees(precise, <Source::Architecture as ProcessorArchitecture>::known_behaviors(instruction), shown,
                    generator.states(instruction)));
            }
            let ghost measured_before = behaviors@;
            behaviors.push((instruction, precise));
            proof {
                assert forall|index: int| 0 <= index < behaviors@.len() implies #[trigger] behaviors@[index].0 == instructions[index]
                    && alone_agrees(behaviors@[index].1, <Source::Architecture as ProcessorArchitecture>::known_behaviors(instructions[index]),
                        shown, generator.states(instructions[index])) by {
                    if index < measured_before.len() {
                        assert(behaviors@[index] == measured_before[index]);
                    } else {
                        assert(behaviors@[index] == (instruction, precise));
                    }
                }
            }
        }
        proof {
            lemma_every_listed_instruction_measured::<Source::Architecture>(behaviors@, instructions, shown,
                |instruction| generator.states(instruction));
        }
        proof {
            let measured = Self { behaviors, measured_with: PhantomData };
            assert forall|instruction| #[trigger] instructions.contains(instruction)
                implies shows_alone(&given, instruction, measured.behavior(instruction)) by {
                assert(measured.behavior(instruction) == first_behavior(behaviors@, instruction));
                assert(shown_by(&given) == shown);
            }
        }
        Ok(Self {
            behaviors,
            measured_with: PhantomData,
        })
    }

    /// Panics for an instruction that `list_instructions` does not list.
    // VERUS: no `find` with a closure and no panic it can rule out without a precondition the trait does not have, so
    // the lookup is trusted, its types written out.
    #[cfg_attr(verus_keep_ghost, verus::trusted)]
    #[verifier::external_body]
    fn to_precise(
        &self,
        instruction: <Source::Architecture as ProcessorArchitecture>::Instruction,
    ) -> <Source::Architecture as ProcessorArchitecture>::PreciseInstruction {
        let measured = self
            .behaviors
            .iter()
            .find(|(measured, _)| *measured == instruction);
        match measured {
            Some((_, precise)) => *precise,
            None => panic!("only listed instructions are measured"),
        }
    }
}

/// The first entry for an instruction is the one at its place in a list of instructions without repeats.
proof fn lemma_first_behavior_at<Instruction, Precise>(behaviors: Seq<(Instruction, Precise)>, instructions: Seq<Instruction>, at: int)
    requires
        behaviors.len() == instructions.len(),
        instructions.no_duplicates(),
        forall|index: int| 0 <= index < behaviors.len() ==> #[trigger] behaviors[index].0 == instructions[index],
        0 <= at < behaviors.len(),
    ensures
        first_behavior(behaviors, instructions[at]) == behaviors[at].1,
    decreases at,
{
    if at > 0 {
        assert(behaviors[0].0 != instructions[at]);
        assert forall|index: int| 0 <= index < behaviors.drop_first().len()
            implies #[trigger] behaviors.drop_first()[index].0 == instructions.drop_first()[index] by {
            assert(behaviors[index + 1].0 == instructions[index + 1]);
        }
        assert(instructions.drop_first().no_duplicates()) by {
            assert forall|first: int, second: int| 0 <= first < second < instructions.drop_first().len()
                implies instructions.drop_first()[first] != instructions.drop_first()[second] by {
                assert(instructions[first + 1] != instructions[second + 1]);
            }
        }
        lemma_first_behavior_at(behaviors.drop_first(), instructions.drop_first(), at - 1);
        assert(instructions.drop_first()[at - 1] == instructions[at]);
    }
}

/// Once every listed instruction is measured, the set gives each one its measured behavior.
proof fn lemma_every_listed_instruction_measured<Architecture>(
    behaviors: Seq<(Architecture::Instruction, Architecture::PreciseInstruction)>,
    instructions: Seq<Architecture::Instruction>,
    shown: wcontainers_behavior_source::Shown<Architecture>,
    states: spec_fn(Architecture::Instruction) -> Seq<ExecutionState<'static, Architecture>>,
)
    where
        Architecture: ProcessorArchitecture,
        Architecture::PreciseInstruction: Simulator<Architecture, Error = Infallible>,
    requires
        behaviors.len() == instructions.len(),
        instructions.no_duplicates(),
        forall|at: int| 0 <= at < behaviors.len() ==> #[trigger] behaviors[at].0 == instructions[at]
            && alone_agrees(behaviors[at].1, Architecture::known_behaviors(instructions[at]), shown, states(instructions[at])),
    ensures
        forall|instruction| #[trigger] instructions.contains(instruction) ==> exists|measured: Seq<ExecutionState<'static, Architecture>>|
            #[trigger] alone_agrees(first_behavior(behaviors, instruction), Architecture::known_behaviors(instruction), shown, measured),
{
    assert forall|instruction| #[trigger] instructions.contains(instruction) implies exists|measured: Seq<ExecutionState<'static, Architecture>>|
        #[trigger] alone_agrees(first_behavior(behaviors, instruction), Architecture::known_behaviors(instruction), shown, measured) by {
        let at = choose|at: int| 0 <= at < instructions.len() && instructions[at] == instruction;
        assert(behaviors[at].0 == instructions[at]);
        lemma_first_behavior_at(behaviors, instructions, at);
        assert(alone_agrees(first_behavior(behaviors, instruction), Architecture::known_behaviors(instruction), shown, states(instruction)));
    }
}

} // verus!
