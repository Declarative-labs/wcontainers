extern crate std;

use core::convert::Infallible;
use std::boxed::Box;
use std::path::Path;
use std::println;
use std::vec::Vec;

use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
use wcontainers_behavior_source_recording::Recording;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{Arm64, Instruction, PreciseInstruction, User};

use crate::{SLICE, keeps_translator_choices};

/// The instructions of the slice where both recorded chips answer otherwise than the translator.
const CHIPS_DIFFER: &[Instruction<Infallible>] = &[
    Instruction::LdrImmediatePostIndex64,
    Instruction::Rdvl,
    Instruction::MrsCtrEl0,
];

/// Reads a chip's committed recording of the slice.
fn read_recording(chip: &str) -> Recording {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../behavior-source/implementation/recording/recordings")
        .join(chip);
    match Recording::read(&folder) {
        Ok(recording) => recording,
        Err(error) => panic!("reading {}: {error}", folder.display()),
    }
}

/// An instruction with every state it was recorded on, in order.
type InstructionStates = (
    Instruction<Infallible>,
    Vec<ExecutionState<'static, Arm64<User>>>,
);

/// The states a recording was made from, since a recording answers no other state.
struct RecordedStates(Vec<InstructionStates>);

impl RecordedStates {
    fn new(recording: &Recording) -> Self {
        let mut states: Vec<InstructionStates> = recording
            .instructions()
            .map(|instruction| (instruction, Vec::new()))
            .collect();
        for (instruction, before, _) in recording.cases() {
            if let Some((_, of_instruction)) = states
                .iter_mut()
                .find(|(recorded, _)| *recorded == instruction)
            {
                of_instruction.push(before.clone());
            }
        }
        Self(states)
    }
}

impl DistinguishingStateGenerator<Arm64<User>> for RecordedStates {
    type States<'generator>
        = Box<dyn Iterator<Item = ExecutionState<'static, Arm64<User>>> + 'generator>
    where
        Self: 'generator;

    fn generate_states(&self, instruction: Instruction<Infallible>) -> Self::States<'_> {
        Box::new(
            self.0
                .iter()
                .filter(move |(recorded, _)| *recorded == instruction)
                .flat_map(|(_, states)| states.iter().cloned()),
        )
    }
}

/// Measures each instruction of the slice on the source, and returns those whose measured behavior breaks a choice.
///
/// Where the states leave several behaviors, they must all keep the choices or all break them. Panics on an
/// instruction no known behavior agrees with, naming it.
fn list_broken_choices<Source>(
    source: &mut Source,
    generator: &impl DistinguishingStateGenerator<Arm64<User>>,
) -> Vec<Instruction<Infallible>>
where
    Source: BehaviorSource<Architecture = Arm64<User>>,
{
    let mut broken = Vec::new();
    for &instruction in SLICE {
        let left = match source.measure_instruction(instruction, generator) {
            Ok(behavior) => [behavior].to_vec(),
            Err(MeasureInstructionError::IndistinguishableBehaviors { behaviors, .. }) => behaviors,
            Err(MeasureInstructionError::NoBehaviorAgrees { .. }) => {
                panic!("no known behavior of {instruction:?} agrees with the source")
            }
            Err(MeasureInstructionError::NoKnownBehavior { .. }) => {
                panic!("{instruction:?} has no known behavior")
            }
            Err(MeasureInstructionError::ExecutionFailed { .. }) => {
                panic!("the source did not execute a state of {instruction:?}")
            }
        };
        let kept = left
            .iter()
            .filter(|behavior| keeps_translator_choices(behavior))
            .count();
        match kept {
            0 => broken.push(instruction),
            _ if kept == left.len() => {}
            _ => panic!(
                "the states leave {instruction:?} behaviors that keep the choices and behaviors that break them"
            ),
        }
    }
    broken
}

#[test]
fn every_slice_instruction_has_a_known_behavior_that_keeps_the_choices() {
    for &instruction in SLICE {
        let behaviors: Vec<PreciseInstruction<Infallible>> =
            Arm64::<User>::list_known_behaviors(instruction).collect();
        assert!(
            behaviors.iter().any(keeps_translator_choices),
            "no known behavior of {instruction:?} keeps the translator's choices"
        );
    }
}

#[test]
fn the_generator_makes_states_for_every_slice_instruction() {
    let generator = CriticalValueCut::default();
    let mut total = 0;
    for &instruction in SLICE {
        let states = generator.generate_states(instruction).count();
        println!("{instruction:?}: {states} states");
        assert!(states > 0, "no state for {instruction:?}");
        total += states;
    }
    println!("slice: {total} states");
}

#[test]
fn a_neoverse_v2_breaks_exactly_the_choices_it_makes_otherwise() {
    let recording = read_recording("neoverse-v2");
    let states = RecordedStates::new(&recording);
    assert_eq!(
        list_broken_choices(&mut { recording }, &states),
        CHIPS_DIFFER
    );
}

#[test]
fn an_apple_m5_breaks_exactly_the_choices_it_makes_otherwise() {
    let recording = read_recording("apple-m5");
    let states = RecordedStates::new(&recording);
    assert_eq!(
        list_broken_choices(&mut { recording }, &states),
        CHIPS_DIFFER
    );
}
