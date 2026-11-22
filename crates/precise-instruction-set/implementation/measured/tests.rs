use alloc::vec::Vec;
use core::iter;

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_executor::Executor;
use wcontainers_executor_simulation::Simulation;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_precise_instruction_set_apple_m5::AppleM5;
use wcontainers_precise_instruction_set_neoverse_v2::NeoverseV2;
use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator,
};
use wcontainers_processor_architecture_arm64::{
    Arm64, Instruction, PreciseInstruction, Processor, VectorLength, VectorLengthReading,
};

use crate::{MeasuredInstructionSet, MeasurementFailure};

/// Every combination of the critical values of each instruction's operands.
const GENERATOR: CriticalValueProduct<Arm64InputDomain> =
    CriticalValueProduct::new(Arm64InputDomain);

const KNOWN: [&dyn PreciseInstructionSet<Arm64>; 2] = [&AppleM5, &NeoverseV2];

/// A chip like the M5 whose cache type no known set gives.
struct UnknownCacheType;

const UNKNOWN_CACHE_TYPE: u64 = 0x8444_C004;

impl PreciseInstructionSet<Arm64> for UnknownCacheType {
    fn to_precise(&self, instruction: Instruction) -> PreciseInstruction {
        match AppleM5.to_precise(instruction) {
            PreciseInstruction::MrsCtrEl0 { .. } => PreciseInstruction::MrsCtrEl0 {
                cache_type: UNKNOWN_CACHE_TYPE,
            },
            precise => precise,
        }
    }
}

/// A generator that gives no states, so it tells no two behaviors apart.
struct NoStates;

impl DistinguishingStateGenerator<Arm64> for NoStates {
    fn generate_states(
        &self,
        _: Instruction,
    ) -> impl Iterator<Item = ExecutionState<'static, Arm64>> {
        iter::empty()
    }
}

/// A chip that counts the instructions it runs.
struct Counted<Chip> {
    chip: Chip,
    runs: usize,
}

impl<Chip: Executor<Architecture = Arm64>> Executor for Counted<Chip> {
    type Architecture = Arm64;
    type TranslatorConfig = ();

    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor,
        memory: &[MemoryDelta<'_, Arm64>],
        on_memory_mutated: impl FnMut(MemoryDelta<'_, Arm64>),
    ) {
        self.runs += 1;
        self.chip
            .execute_single_instruction_in_place(processor, memory, on_memory_mutated);
    }
}

/// Measures a stand-in that plays the chip of `played`, which `set` is a second value of.
fn assert_measures_as<Set: PreciseInstructionSet<Arm64>>(set: Set, played: Set) {
    let Ok(measured) =
        MeasuredInstructionSet::measure(&mut Simulation::new(played), &KNOWN, &GENERATOR)
    else {
        panic!("a stand-in playing a known set measures as that set");
    };
    for instruction in Arm64::list_instructions() {
        assert_eq!(
            measured.to_precise(instruction),
            set.to_precise(instruction)
        );
    }
}

#[test]
fn stand_in_playing_the_m5_measures_as_the_m5() {
    assert_measures_as(AppleM5, AppleM5);
}

#[test]
fn stand_in_playing_the_v2_measures_as_the_v2() {
    assert_measures_as(NeoverseV2, NeoverseV2);
}

#[test]
fn instruction_the_known_sets_agree_on_is_not_run() {
    let mut chip = Counted {
        chip: Simulation::new(AppleM5),
        runs: 0,
    };
    assert!(MeasuredInstructionSet::measure(&mut chip, &[&AppleM5, &AppleM5], &GENERATOR).is_ok());
    assert_eq!(chip.runs, 0);
    assert!(MeasuredInstructionSet::measure(&mut chip, &KNOWN, &GENERATOR).is_ok());
    let differing = [Instruction::Rdvl, Instruction::MrsCtrEl0].map(|instruction| {
        DistinguishingStateGenerator::<Arm64>::generate_states(&GENERATOR, instruction).count()
    });
    assert_eq!(chip.runs, differing.iter().sum::<usize>());
}

#[test]
fn behavior_no_set_knows_is_named_with_its_state() {
    let Err(failure) =
        MeasuredInstructionSet::measure(&mut Simulation::new(UnknownCacheType), &KNOWN, &GENERATOR)
    else {
        panic!("no known set gives the chip's cache type");
    };
    let MeasurementFailure::NoBehaviorAgrees {
        instruction,
        before,
        behaviors,
        chip,
    } = failure
    else {
        panic!("the failure is that no behavior agrees");
    };
    assert_eq!(instruction, Instruction::MrsCtrEl0);
    let said: Vec<PreciseInstruction> = behaviors.iter().map(|(precise, _)| *precise).collect();
    assert_eq!(
        said,
        KNOWN.map(|set| set.to_precise(Instruction::MrsCtrEl0))
    );
    let Ok(expected) = PreciseInstruction::MrsCtrEl0 {
        cache_type: UNKNOWN_CACHE_TYPE,
    }
    .simulate(before);
    assert!(chip == expected);
}

#[test]
fn behaviors_no_state_tells_apart_are_named() {
    let measured =
        MeasuredInstructionSet::measure(&mut Simulation::new(AppleM5), &KNOWN, &NoStates);
    let Err(MeasurementFailure::IndistinguishableBehaviors {
        instruction,
        behaviors,
    }) = measured
    else {
        panic!("with no states, the first instruction the sets differ on cannot be told");
    };
    assert_eq!(instruction, Instruction::Rdvl);
    assert_eq!(
        behaviors,
        [
            PreciseInstruction::Rdvl(VectorLengthReading::Undefined),
            PreciseInstruction::Rdvl(VectorLengthReading::Reads(VectorLength::Bits128)),
        ]
    );
}

#[test]
fn no_known_set_is_named() {
    let measured = MeasuredInstructionSet::measure(&mut Simulation::new(AppleM5), &[], &GENERATOR);
    assert!(matches!(
        measured,
        Err(MeasurementFailure::NoKnownBehavior {
            instruction: Instruction::AddShiftedRegister64
        })
    ));
}
