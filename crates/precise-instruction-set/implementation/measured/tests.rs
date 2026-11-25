extern crate std;

use alloc::vec::Vec;
use core::convert::Infallible;
use std::path::Path;

use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
use wcontainers_behavior_source_recording::Recording;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{MemoryDelta, MemorySink, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{
    Arm64, Instruction, LoadIntoBaseWithWriteback, MisalignedStackPointer, PreciseInstruction,
    Processor, SmeTrapCode, StoreAcrossFault, User, VectorLength, VectorLengthReading,
};

use crate::MeasuredInstructionSet;

type Generator = CriticalValueProduct<Arm64InputDomain>;

/// The folder of a chip's committed recordings.
fn read_recording(chip: &str) -> Recording {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../behavior-source/implementation/recording/recordings")
        .join(chip);
    match Recording::read(&folder) {
        Ok(recording) => recording,
        Err(error) => panic!("reading {}: {error}", folder.display()),
    }
}

/// The behaviors a chip showed where the manual leaves a choice, every other instruction having one behavior.
fn expect_chip(
    into_base: LoadIntoBaseWithWriteback,
    vector_length: VectorLengthReading,
    cache_type: u64,
) -> impl Fn(Instruction<Infallible>) -> PreciseInstruction<Infallible> {
    let misaligned_stack_pointer = MisalignedStackPointer::Faults;
    let across_fault = StoreAcrossFault::WritesNothing;
    move |instruction| match instruction {
        Instruction::AddShiftedRegister64 => PreciseInstruction::AddShiftedRegister64,
        Instruction::AddsShiftedRegister64 => PreciseInstruction::AddsShiftedRegister64,
        Instruction::Csel64 => PreciseInstruction::Csel64,
        Instruction::LdrImmediateUnsignedOffset64 => {
            PreciseInstruction::LdrImmediateUnsignedOffset64 {
                misaligned_stack_pointer,
            }
        }
        Instruction::StrImmediateUnsignedOffset64 => {
            PreciseInstruction::StrImmediateUnsignedOffset64 {
                misaligned_stack_pointer,
                across_fault,
            }
        }
        Instruction::StpSignedOffset64 => PreciseInstruction::StpSignedOffset64 {
            misaligned_stack_pointer,
            across_fault,
        },
        Instruction::LdrImmediatePostIndex64 => PreciseInstruction::LdrImmediatePostIndex64 {
            misaligned_stack_pointer,
            into_base,
        },
        Instruction::Brk => PreciseInstruction::Brk,
        Instruction::Rdvl => PreciseInstruction::Rdvl(vector_length),
        Instruction::MrsCtrEl0 => PreciseInstruction::MrsCtrEl0 { cache_type },
    }
}

fn assert_measures_as(
    recording: Recording,
    expected: impl Fn(Instruction<Infallible>) -> PreciseInstruction<Infallible>,
) {
    let Ok(measured) = MeasuredInstructionSet::<Recording, Generator>::measure(recording) else {
        panic!("a recording of a known chip measures as that chip");
    };
    for instruction in Arm64::<User>::list_instructions() {
        assert_eq!(measured.to_precise(instruction), expected(instruction));
    }
}

#[test]
fn the_neoverse_v2_recording_measures_as_the_neoverse_v2() {
    assert_measures_as(
        read_recording("neoverse-v2"),
        expect_chip(
            LoadIntoBaseWithWriteback::WritebackSuppressed,
            VectorLengthReading::Reads(VectorLength::Bits128),
            0xB444_C004,
        ),
    );
}

#[test]
fn the_apple_m5_recording_measures_as_the_apple_m5() {
    assert_measures_as(
        read_recording("apple-m5"),
        expect_chip(
            LoadIntoBaseWithWriteback::Undefined,
            VectorLengthReading::SmeTrapped(SmeTrapCode::Disabled),
            0x9_9444_C004,
        ),
    );
}

/// A recording whose every state after also changes TPIDR_EL0, which none of the instructions writes.
struct ThreadIdChanged(Recording);

impl BehaviorSource for ThreadIdChanged {
    type Architecture = Arm64<User>;

    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor<()>,
        memory: &[MemoryDelta<'_, Arm64<User>>],
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) {
        self.0
            .execute_single_instruction_in_place(processor, memory, mutation_hook);
        processor.software_thread_id ^= 1;
    }
}

#[test]
fn behavior_no_known_one_agrees_with_is_named_with_its_state() {
    let source = ThreadIdChanged(read_recording("neoverse-v2"));
    let Err(failure) = MeasuredInstructionSet::<ThreadIdChanged, Generator>::measure(source) else {
        panic!("no known behavior changes TPIDR_EL0");
    };
    let MeasureInstructionError::NoBehaviorAgrees {
        instruction,
        before,
        behaviors,
        observed,
    } = failure
    else {
        panic!("the failure is that no behavior agrees");
    };
    assert_eq!(Some(instruction), Arm64::<User>::list_instructions().next());
    assert_ne!(
        observed.processor.software_thread_id,
        before.processor.software_thread_id
    );
    let listed: Vec<_> = Arm64::<User>::list_known_behaviors(instruction).collect();
    let named: Vec<_> = behaviors.iter().map(|(precise, _)| *precise).collect();
    assert_eq!(named, listed);
}

/// The live chip measures as its recording; it runs only where the host is a Neoverse V2 under KVM.
#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
#[test]
#[ignore = "needs KVM on a Neoverse V2"]
fn this_chip_measures_as_the_neoverse_v2_recording() {
    use wcontainers_behavior_source_kvm_single_step::KvmSingleStepMachine;

    let chip = match KvmSingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("this host makes no single step machine: {error:?}"),
    };
    let Ok(measured) = MeasuredInstructionSet::<KvmSingleStepMachine, Generator>::measure(chip) else {
        panic!("the chip shows one known behavior of every instruction");
    };
    let Ok(recorded) =
        MeasuredInstructionSet::<Recording, Generator>::measure(read_recording("neoverse-v2"))
    else {
        panic!("the recording measures");
    };
    for instruction in Arm64::<User>::list_instructions() {
        assert_eq!(
            measured.to_precise(instruction),
            recorded.to_precise(instruction)
        );
    }
}
