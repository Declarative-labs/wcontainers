extern crate std;

use core::convert::Infallible;
use std::boxed::Box;
use std::path::Path;
use std::vec::Vec;

use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
use wcontainers_behavior_source_recording::Recording;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{
    Arm64, Instruction, LoadIntoBaseWithWriteback, LoadStoreChoices, MisalignedStackPointer,
    PreciseInstruction, StoreAcrossFault, User,
};

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

/// Asserts that changing one choice of each kept load or store of the slice breaks the choices.
///
/// The change returns whether the choices have that case; at least one kept behavior must have it.
fn assert_change_breaks_choices(case: &str, change: impl Fn(&mut LoadStoreChoices) -> bool) {
    let mut changed = 0_usize;
    for &instruction in SLICE {
        for behavior in Arm64::<User>::list_known_behaviors(instruction) {
            if !keeps_translator_choices(&behavior) {
                continue;
            }
            if let PreciseInstruction::LoadStore {
                instruction: load_store,
                mut choices,
            } = behavior
                && change(&mut choices)
            {
                let broken = PreciseInstruction::LoadStore {
                    instruction: load_store,
                    choices,
                };
                assert!(
                    !keeps_translator_choices(&broken),
                    "{instruction:?} keeps the choices with {case}: {broken:?}"
                );
                changed += 1;
            }
        }
    }
    assert!(
        changed > 0,
        "no kept behavior of the slice has the case {case}"
    );
}

#[test]
fn a_misaligned_stack_pointer_that_accesses_breaks_the_choices() {
    assert_change_breaks_choices("a misaligned stack pointer that accesses", |choices| {
        choices
            .misaligned_stack_pointer
            .replace(MisalignedStackPointer::Accesses)
            .is_some()
    });
}

#[test]
fn a_store_that_writes_the_bytes_below_a_fault_breaks_the_choices() {
    assert_change_breaks_choices("a store that writes the bytes below a fault", |choices| {
        choices
            .across_fault
            .replace(StoreAcrossFault::WritesBytesBelowFault)
            .is_some()
    });
}

#[test]
fn a_load_into_its_base_that_suppresses_the_writeback_breaks_the_choices() {
    assert_change_breaks_choices(
        "a load into its base that suppresses the writeback",
        |choices| {
            choices
                .load_into_base
                .replace(LoadIntoBaseWithWriteback::WritebackSuppressed)
                .is_some()
        },
    );
}

/// The number of states `CriticalValueCut` makes for each instruction of the slice, counted on 2026-11-26.
const SLICE_STATES: &[(Instruction<Infallible>, usize)] = &[
    (Instruction::AddShiftedRegister64, 3_680),
    (Instruction::AddsShiftedRegister64, 4_880),
    (Instruction::Csel64, 4_360),
    (Instruction::LdrImmediateUnsignedOffset64, 10_528),
    (Instruction::StrImmediateUnsignedOffset64, 15_036),
    (Instruction::StpSignedOffset64, 31_280),
    (Instruction::LdrImmediatePostIndex64, 9_024),
    (Instruction::Brk, 18),
    (Instruction::Rdvl, 27),
    (Instruction::MrsCtrEl0, 3),
];

/// The number of states `CriticalValueCut` makes for the whole slice, the sum of `SLICE_STATES`.
const SLICE_TOTAL_STATES: usize = 78_836;

#[test]
fn the_generator_makes_states_for_every_slice_instruction() {
    let generator = CriticalValueCut::default();
    let counted: Vec<(Instruction<Infallible>, usize)> = SLICE
        .iter()
        .map(|&instruction| (instruction, generator.generate_states(instruction).count()))
        .collect();
    assert_eq!(counted, SLICE_STATES);
    assert_eq!(
        counted.iter().map(|&(_, states)| states).sum::<usize>(),
        SLICE_TOTAL_STATES
    );
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
