use std::convert::Infallible;
use std::iter::Rev;
use std::path::Path;
use std::vec::IntoIter;

use wcontainers_behavior_source_recording::Recording;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_arm64::{Arm64, Instruction, User};

/// What a Neoverse V2 under KVM did, written by the record tool.
const RECORDINGS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../behavior-source/implementation/recording/recordings/neoverse-v2"
);

type Measured = MeasuredInstructionSet<Recording>;

fn recording() -> Recording {
    match Recording::read(Path::new(RECORDINGS)) {
        Ok(recording) => recording,
        Err(error) => panic!("reading {RECORDINGS}: {error}"),
    }
}

/// An instruction, a state before and the state the chip gave after.
type RecordedCase = (
    Instruction<Infallible>,
    ExecutionState<'static, Arm64<User>>,
    ExecutionState<'static, Arm64<User>>,
);

fn recorded_cases() -> Vec<RecordedCase> {
    recording()
        .cases()
        .map(|(instruction, before, after)| (instruction, before.clone(), after.clone()))
        .collect()
}

/// The critical value product's states of each instruction, last first, all of which the recording holds.
struct LastFirst(CriticalValueProduct<Arm64InputDomain>);

impl DistinguishingStateGenerator<Arm64<User>> for LastFirst {
    type States<'generator>
        = Rev<IntoIter<ExecutionState<'static, Arm64<User>>>>
    where
        Self: 'generator;

    fn generate_states(&self, instruction: Instruction<Infallible>) -> Self::States<'_> {
        let states: Vec<_> = self.0.generate_states(instruction).collect();
        states.into_iter().rev()
    }
}

wcontainers_precise_instruction_set_conformance::tests!(
    Measured,
    recording(),
    CriticalValueProduct::new(Arm64InputDomain),
    LastFirst(CriticalValueProduct::new(Arm64InputDomain)),
    recorded_cases()
);
