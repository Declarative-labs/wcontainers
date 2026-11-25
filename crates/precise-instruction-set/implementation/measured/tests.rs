extern crate std;

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::convert::Infallible;
use core::iter::Rev;
use std::path::Path;

use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
use wcontainers_behavior_source_recording::{NotRecorded, Recording};
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, MemorySink, MemorySnapshot, ProcessorArchitecture,
};
use wcontainers_processor_architecture_arm64::{
    Arm64, FailedStoreExclusive, FaultAddressTag, Instruction, LoadIntoBaseWithWriteback,
    LoadPairIntoOneRegister, LoadStoreChoices, LoadStoreInstruction, MisalignedExclusive,
    MisalignedStackPointer, PreciseInstruction, Processor, SmeTrapCode, StoreAcrossFault,
    StoreExclusiveOverlap, StoreOfBaseWithWriteback, User, VectorLength, VectorLengthReading,
};

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

/// An instruction with every state it was recorded on, in order.
type InstructionStates = (
    Instruction<Infallible>,
    Vec<ExecutionState<'static, Arm64<User>>>,
);

/// The states a recording was made from, for each instruction it holds and none for any other.
struct RecordedStates(Vec<InstructionStates>);

impl RecordedStates {
    fn new(recording: &Recording) -> Self {
        let mut states: Vec<(Instruction<Infallible>, Vec<_>)> = recording
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

/// The behaviors a chip showed where the manual leaves a choice, every other instruction having one behavior.
fn expect_chip(
    into_base: LoadIntoBaseWithWriteback,
    vector_length: VectorLengthReading,
    cache_type: u64,
) -> impl Fn(Instruction<Infallible>) -> PreciseInstruction<Infallible> {
    let choices = LoadStoreChoices {
        misaligned_stack_pointer: Some(MisalignedStackPointer::Faults),
        across_fault: Some(StoreAcrossFault::WritesNothing),
        load_into_base: Some(into_base),
        store_of_base: Some(StoreOfBaseWithWriteback::StoresOriginalValue),
        load_pair_into_one_register: Some(LoadPairIntoOneRegister::Undefined),
        failed_store_exclusive: Some(FailedStoreExclusive::ChecksAccess),
        store_exclusive_overlap: Some(StoreExclusiveOverlap::Runs),
        misaligned_exclusive: Some(MisalignedExclusive::WithinSixteenBytes),
        fault_address_tag: Some(FaultAddressTag::KeepsTopByte),
    };
    move |instruction| {
        let behaviors: Vec<_> = Arm64::<User>::list_known_behaviors(instruction).collect();
        if let Some(load_store) = LoadStoreInstruction::new(instruction) {
            return PreciseInstruction::LoadStore {
                instruction: load_store,
                choices: choices.restrict_to(load_store),
            };
        }
        match instruction {
            Instruction::Rdvl => PreciseInstruction::Rdvl(vector_length),
            Instruction::MrsCtrEl0 => PreciseInstruction::MrsCtrEl0 { cache_type },
            _ => match behaviors.as_slice() {
                [only] => *only,
                _ => panic!("{instruction:?} has one behavior"),
            },
        }
    }
}

/// Each instruction the recording holds measures as the chip's behavior, from the states it was recorded on; where
/// those states cannot tell the chip's behavior from another, it is among those left.
fn assert_measures_as(
    recording: Recording,
    expected: impl Fn(Instruction<Infallible>) -> PreciseInstruction<Infallible>,
) {
    let states = RecordedStates::new(&recording);
    let instructions: Vec<_> = recording.instructions().collect();
    let mut source = recording;
    for instruction in instructions {
        let expected = expected(instruction);
        match source.measure_instruction(instruction, &states) {
            Ok(measured) => assert_eq!(measured, expected),
            Err(MeasureInstructionError::IndistinguishableBehaviors { behaviors, .. }) => {
                assert!(
                    behaviors.contains(&expected),
                    "{instruction:?}: {behaviors:?}"
                );
            }
            Err(_) => panic!("{instruction:?}: no known behavior agrees with the recording"),
        }
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
    type Error = NotRecorded;

    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor<()>,
        memory: MemorySnapshot<'_, '_, Arm64<User>>,
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) -> Result<(), NotRecorded> {
        self.0
            .execute_single_instruction_in_place(processor, memory, mutation_hook)?;
        processor.software_thread_id ^= 1;
        Ok(())
    }
}

#[test]
fn behavior_no_known_one_agrees_with_is_named_with_its_state() {
    let recording = read_recording("neoverse-v2");
    let states = RecordedStates::new(&recording);
    let Some(instruction) = recording.instructions().next() else {
        panic!("the recording holds an instruction");
    };
    let mut source = ThreadIdChanged(recording);
    let Err(failure) = source.measure_instruction(instruction, &states) else {
        panic!("no known behavior changes TPIDR_EL0");
    };
    let MeasureInstructionError::NoBehaviorAgrees {
        before,
        behaviors,
        observed,
        ..
    } = failure
    else {
        panic!("the failure is that no behavior agrees");
    };
    assert_ne!(
        observed.processor.software_thread_id,
        before.processor.software_thread_id
    );
    let listed: Vec<_> = Arm64::<User>::list_known_behaviors(instruction).collect();
    let named: Vec<_> = behaviors.iter().map(|(precise, _)| *precise).collect();
    assert_eq!(named, listed);
}

/// The states of each instruction another generator gives, last first.
struct LastFirst<Generator>(Generator);

impl<Generator: DistinguishingStateGenerator<Arm64<User>>> DistinguishingStateGenerator<Arm64<User>>
    for LastFirst<Generator>
{
    type States<'generator>
        = Rev<alloc::vec::IntoIter<ExecutionState<'static, Arm64<User>>>>
    where
        Self: 'generator;

    fn generate_states(&self, instruction: Instruction<Infallible>) -> Self::States<'_> {
        let states: Vec<_> = self.0.generate_states(instruction).collect();
        states.into_iter().rev()
    }
}

/// Returns the behaviors a measurement leaves: the one measured, or those its states cannot tell apart.
fn to_survivors<Error>(
    instruction: Instruction<Infallible>,
    measured: Result<PreciseInstruction<Infallible>, MeasureInstructionError<Arm64<User>, Error>>,
) -> Vec<PreciseInstruction<Infallible>> {
    match measured {
        Ok(precise) => [precise].to_vec(),
        Err(MeasureInstructionError::IndistinguishableBehaviors { behaviors, .. }) => behaviors,
        Err(_) => panic!("{instruction:?}: no known behavior agrees"),
    }
}

/// Checks that the recorded states in either order measure each instruction of one recording the same.
#[test]
fn either_generator_measures_the_recording_the_same() {
    let mut recording = read_recording("neoverse-v2");
    let states = RecordedStates::new(&recording);
    let last_first = LastFirst(RecordedStates::new(&recording));
    let instructions: Vec<_> = recording.instructions().collect();
    for instruction in instructions {
        let first = recording.measure_instruction(instruction, &states);
        let second = recording.measure_instruction(instruction, &last_first);
        assert_eq!(
            to_survivors(instruction, first),
            to_survivors(instruction, second),
            "{instruction:?}"
        );
    }
}

/// The live chip measures as its recording on the recorded states; it runs only where the host is a Neoverse V2 under
/// KVM.
#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
#[test]
#[ignore = "needs KVM on a Neoverse V2"]
fn this_chip_measures_as_the_neoverse_v2_recording() {
    use wcontainers_behavior_source_kvm_single_step::KvmSingleStepMachine;

    let mut chip = match KvmSingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("this host makes no single step machine: {error:?}"),
    };
    let mut recording = read_recording("neoverse-v2");
    let states = RecordedStates::new(&recording);
    let instructions: Vec<_> = recording.instructions().collect();
    for instruction in instructions {
        let measured = chip.measure_instruction(instruction, &states);
        let recorded = recording.measure_instruction(instruction, &states);
        assert_eq!(
            to_survivors(instruction, measured),
            to_survivors(instruction, recorded),
            "{instruction:?}"
        );
    }
}

/// The Neoverse V2's behavior of a load or store or of BRK, as its recording measures it.
#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn to_neoverse_v2_behavior(instruction: Instruction<Infallible>) -> PreciseInstruction<Infallible> {
    expect_chip(
        LoadIntoBaseWithWriteback::WritebackSuppressed,
        VectorLengthReading::Reads(VectorLength::Bits128),
        0xB444_C004,
    )(instruction)
}

/// States whose access or fetch reaches past the last address a program can use, each with a page above it that the
/// state maps and that would lie over the code page if the guest's tables dropped the address's high bits.
#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn to_states_past_the_last_addressable_byte() -> Vec<(
    Instruction<Infallible>,
    wcontainers_processor_architecture::ExecutionState<'static, Arm64<User>>,
)> {
    use alloc::borrow::Cow;
    use alloc::vec;
    use wcontainers_processor_architecture::ExecutionState;
    use wcontainers_processor_architecture_arm64::encoding::{Field, IMM7, IMM9, RN, RT, RT2};
    use wcontainers_processor_architecture_arm64::{
        DataAccess, INSTRUCTION_SIZE, InstructionFetch, PagePermission, ZERO_PROCESSOR,
    };

    const PAGE: u64 = 1 << 12;
    const CODE: u64 = PAGE;
    let end = Arm64::<User>::ADDRESSABLE.end;
    let read_write = PagePermission::Mapped {
        data_access: DataAccess::ReadWrite,
        instruction_fetch: InstructionFetch::Forbidden,
    };
    let executable = PagePermission::Mapped {
        data_access: DataAccess::ReadOnly,
        instruction_fetch: InstructionFetch::Allowed,
    };
    let encode = |instruction: Instruction<Infallible>, fields: &[(Field, u32)]| {
        fields
            .iter()
            .fold(instruction.to_fixed_bits().value, |word, (field, value)| {
                word | field.to_bits(*value)
            })
    };
    let to_state =
        |instruction, word: u32, base: u64, program_counter: u64, above: PagePermission| {
            let mut processor = ZERO_PROCESSOR;
            processor.program_counter = program_counter;
            processor.general_purpose_registers[1] = base;
            processor.general_purpose_registers[2] = u64::MAX;
            processor.general_purpose_registers[3] = 0x0123_4567_89AB_CDEF;
            let code = |at: u64| {
                [
                    MemoryDelta::PermissionAssignment {
                        addresses: at..at + PAGE,
                        permission: executable,
                    },
                    MemoryDelta::ValueAssignment {
                        start: program_counter,
                        bytes: Cow::Owned(word.to_le_bytes().to_vec()),
                    },
                ]
            };
            let mut memory = vec![
                MemoryDelta::PermissionAssignment {
                    addresses: end - PAGE..end,
                    permission: read_write,
                },
                MemoryDelta::ValueAssignment {
                    start: end - PAGE,
                    bytes: Cow::Owned((0..PAGE).map(|offset| offset as u8).collect()),
                },
                MemoryDelta::PermissionAssignment {
                    addresses: end + CODE..end + CODE + PAGE,
                    permission: above,
                },
            ];
            memory.extend(code(program_counter - program_counter % PAGE));
            (
                instruction,
                ExecutionState {
                    processor,
                    memory_mutations: memory,
                },
            )
        };
    let store = encode(
        Instruction::StrImmediateUnsignedOffset64,
        &[(RT, 2), (RN, 1)],
    );
    let pair = encode(
        Instruction::StpSignedOffset64,
        &[(RT, 2), (RT2, 3), (RN, 1), (IMM7, 0)],
    );
    let load = encode(
        Instruction::LdrImmediateUnsignedOffset64,
        &[(RT, 0), (RN, 1)],
    );
    let post_index = encode(
        Instruction::LdrImmediatePostIndex64,
        &[(RT, 0), (RN, 1), (IMM9, 8)],
    );
    let breakpoint = encode(Instruction::Brk, &[]);
    let mut states = Vec::new();
    for above in [read_write, PagePermission::Unmapped] {
        states.extend([
            to_state(
                Instruction::StrImmediateUnsignedOffset64,
                store,
                end - 4,
                CODE,
                above,
            ),
            to_state(
                Instruction::StrImmediateUnsignedOffset64,
                store,
                end,
                CODE,
                above,
            ),
            to_state(Instruction::StpSignedOffset64, pair, end - 8, CODE, above),
            to_state(Instruction::StpSignedOffset64, pair, end - 12, CODE, above),
            to_state(
                Instruction::LdrImmediateUnsignedOffset64,
                load,
                end - 4,
                CODE,
                above,
            ),
            to_state(
                Instruction::LdrImmediatePostIndex64,
                post_index,
                end - 1,
                CODE,
                above,
            ),
            to_state(Instruction::Brk, breakpoint, end - 4, end, above),
            // Runs only if the page above is kept out of the guest's tables.
            to_state(Instruction::Brk, breakpoint, end - 4, CODE, above),
            to_state(
                Instruction::Brk,
                breakpoint,
                end - 4,
                end - INSTRUCTION_SIZE,
                above,
            ),
        ]);
    }
    states
}

/// Accesses and a fetch past the last address a program can use fault at level 0 on the live chip, as the behaviors
/// its recording measures as simulate them; it runs only where the host is a Neoverse V2 under KVM.
#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
#[test]
#[ignore = "needs KVM on a Neoverse V2"]
fn this_chip_faults_past_the_last_addressable_byte_as_its_recording_simulates() {
    use wcontainers_behavior_source_kvm_single_step::KvmSingleStepMachine;
    use wcontainers_processor_architecture::Simulator;
    use wcontainers_processor_architecture_arm64::{Exception, FaultStatus};

    let mut chip = match KvmSingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("this host makes no single step machine: {error:?}"),
    };
    let mut level_0_faults = 0;
    for (instruction, before) in to_states_past_the_last_addressable_byte() {
        let precise = to_neoverse_v2_behavior(instruction);
        let Ok(simulated) = precise.simulate_state(before.clone());
        let Ok(observed) = chip.execute_single_instruction(before) else {
            panic!("{instruction:?}: the chip refused a state it runs");
        };
        assert!(
            observed.meets_expectations(&simulated),
            "{instruction:?}: the chip gives {:?}, {precise:?} simulates {:?}",
            observed.processor,
            simulated.processor
        );
        if let Some(
            Exception::DataAbort {
                status: FaultStatus::TranslationFaultLevel0,
                ..
            }
            | Exception::InstructionAbort {
                status: FaultStatus::TranslationFaultLevel0,
                ..
            },
        ) = observed.processor.exception
        {
            level_0_faults += 1;
        }
    }
    // Each state faults at level 0 but the two whose instruction runs, at the last addressable word and at the code page.
    assert_eq!(level_0_faults, 14);
}
