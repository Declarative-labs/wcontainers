//! A recording small enough to commit: of each instruction, only the states that tell its known behaviors apart on the
//! chip, each written with its state before so that no generator is needed to play it.

use std::convert::Infallible;

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture, Simulator};
use wcontainers_processor_architecture_arm64::{Arm64, Instruction, User};

use crate::{Cases, RecordedCase, Recording, run_case};

impl Recording {
    /// Runs each of the instructions on the source from every state the generator gives for it, up to the first error,
    /// and keeps the states that rule out a known behavior no earlier state ruled out, or the first state where none
    /// does.
    ///
    /// Measuring on the kept states, in their order, leaves the behaviors measuring on every state leaves, since each
    /// behavior is ruled out at a kept state.
    pub fn record_deciding<Source: BehaviorSource<Architecture = Arm64<User>>>(
        source: &mut Source,
        generator: &impl DistinguishingStateGenerator<Arm64<User>>,
        instructions: impl IntoIterator<Item = Instruction<Infallible>>,
    ) -> Result<Recording, Source::Error> {
        let instructions = instructions
            .into_iter()
            .map(|instruction| {
                let cases = record_deciding_cases(source, generator, instruction)?;
                Ok((instruction, Cases::Written(cases)))
            })
            .collect::<Result<_, _>>()?;
        Ok(Recording::new(instructions))
    }
}

fn record_deciding_cases<Source: BehaviorSource<Architecture = Arm64<User>>>(
    source: &mut Source,
    generator: &impl DistinguishingStateGenerator<Arm64<User>>,
    instruction: Instruction<Infallible>,
) -> Result<Vec<RecordedCase>, Source::Error> {
    let mut candidates: Vec<_> = Arm64::<User>::list_known_behaviors(instruction).collect();
    let mut kept = Vec::new();
    let mut first = None;
    for before in generator.generate_states(instruction) {
        let (before, after) = run_case(source, before)?;
        let remaining: Vec<_> = candidates
            .iter()
            .copied()
            .filter(|precise| {
                let Ok(expected) = precise.simulate_state(before.clone());
                after.meets_expectations(&expected)
            })
            .collect();
        if remaining.len() < candidates.len() {
            candidates = remaining;
            kept.push((before, after));
        } else if kept.is_empty() && first.is_none() {
            first = Some((before, after));
        }
    }
    if kept.is_empty() {
        kept.extend(first);
    }
    Ok(kept)
}

/// The states before a recording writes for each of its instructions, given as a generator gives states, so that a
/// recording that keeps its states before is measured on exactly them.
pub struct WrittenStates(
    pub(crate)  Vec<(
        Instruction<Infallible>,
        Vec<ExecutionState<'static, Arm64<User>>>,
    )>,
);

impl WrittenStates {
    /// Takes the written states of every instruction of the recording; one recorded from a generator gets none.
    pub fn new(recording: &Recording) -> Self {
        let states = recording
            .instructions
            .iter()
            .map(|(instruction, cases)| {
                let written = match cases {
                    Cases::Written(cases) => {
                        cases.iter().map(|(before, _)| before.clone()).collect()
                    }
                    Cases::Generated(_) => Vec::new(),
                };
                (*instruction, written)
            })
            .collect();
        Self(states)
    }
}

impl DistinguishingStateGenerator<Arm64<User>> for WrittenStates {
    type States<'generator>
        = std::iter::Cloned<std::slice::Iter<'generator, ExecutionState<'static, Arm64<User>>>>
    where
        Self: 'generator;

    fn generate_states(&self, instruction: Instruction<Infallible>) -> Self::States<'_> {
        let states = self
            .0
            .iter()
            .find(|(listed, _)| *listed == instruction)
            .map_or(&[][..], |(_, states)| states.as_slice());
        states.iter().cloned()
    }
}
