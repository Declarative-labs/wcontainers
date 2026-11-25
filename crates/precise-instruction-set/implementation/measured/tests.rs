extern crate std;

use alloc::vec::Vec;
use core::convert::Infallible;
use core::iter::Rev;
use std::path::Path;

use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
use wcontainers_behavior_source_recording::{NotRecorded, Recording};
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{
    ExecutionState, MemorySink, MemorySnapshot, ProcessorArchitecture,
};
use wcontainers_processor_architecture_arm64::{
    Arm64, Instruction, LoadIntoBaseWithWriteback, MisalignedStackPointer, PreciseInstruction,
    Processor, SmeTrapCode, StoreAcrossFault, User, VectorLength, VectorLengthReading,
};

use crate::MeasuredInstructionSet;

type Generator = CriticalValueProduct<Arm64InputDomain>;

/// Measures every instruction on the source with the generator and builds the set from what it shows.
fn measure_set(
    mut source: impl BehaviorSource<Architecture = Arm64<User>>,
    generator: &impl DistinguishingStateGenerator<Arm64<User>>,
) -> MeasuredInstructionSet<Arm64<User>> {
    match source.measure_instructions(generator) {
        Ok(measurements) => MeasuredInstructionSet::from_measurements(&measurements),
        Err(_) => panic!("the source shows one known behavior of every instruction"),
    }
}

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
    let measured = measure_set(recording, &Generator::new(Arm64InputDomain));
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
    let mut source = ThreadIdChanged(read_recording("neoverse-v2"));
    let Err(failure) = source.measure_instructions(&Generator::new(Arm64InputDomain)) else {
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

/// The critical value product's states of each instruction, last first, all of which the recording holds.
struct LastFirst(Generator);

impl DistinguishingStateGenerator<Arm64<User>> for LastFirst {
    type States<'generator>
        = Rev<alloc::vec::IntoIter<ExecutionState<'static, Arm64<User>>>>
    where
        Self: 'generator;

    fn generate_states(&self, instruction: Instruction<Infallible>) -> Self::States<'_> {
        let states: Vec<_> = self.0.generate_states(instruction).collect();
        states.into_iter().rev()
    }
}

/// Checks that two generators that each tell the behaviors apart measure one recording as the same set.
#[test]
fn either_generator_measures_the_recording_as_the_same_set() {
    let first = measure_set(
        read_recording("neoverse-v2"),
        &Generator::new(Arm64InputDomain),
    );
    let second = measure_set(
        read_recording("neoverse-v2"),
        &LastFirst(Generator::new(Arm64InputDomain)),
    );
    for instruction in Arm64::<User>::list_instructions() {
        assert_eq!(
            first.to_precise(instruction),
            second.to_precise(instruction)
        );
    }
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
    let measured = measure_set(chip, &Generator::new(Arm64InputDomain));
    let recorded = measure_set(
        read_recording("neoverse-v2"),
        &Generator::new(Arm64InputDomain),
    );
    for instruction in Arm64::<User>::list_instructions() {
        assert_eq!(
            measured.to_precise(instruction),
            recorded.to_precise(instruction)
        );
    }
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
    use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
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
    let recorded = measure_set(
        read_recording("neoverse-v2"),
        &Generator::new(Arm64InputDomain),
    );
    let mut level_0_faults = 0;
    for (instruction, before) in to_states_past_the_last_addressable_byte() {
        let precise = recorded.to_precise(instruction);
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

/// The set gives an instruction the behavior of the first pair that names it, as `from_measurements` promises.
#[test]
fn each_instruction_has_the_behavior_of_its_first_pair() {
    let first = PreciseInstruction::Rdvl(VectorLengthReading::Reads(VectorLength::Bits128));
    let later = PreciseInstruction::Rdvl(VectorLengthReading::Reads(VectorLength::Bits256));
    let measured = MeasuredInstructionSet::<Arm64<User>>::from_measurements(&[
        (Instruction::Brk, PreciseInstruction::Brk),
        (Instruction::Rdvl, first),
        (Instruction::Rdvl, later),
    ]);
    assert_eq!(measured.to_precise(Instruction::Rdvl), first);
    assert_eq!(
        measured.to_precise(Instruction::Brk),
        PreciseInstruction::Brk
    );
}

/// The trusted `unmeasured`, which promises never to return: an instruction the measurements do not hold panics.
#[test]
#[should_panic(expected = "only measured instructions have a behavior")]
fn an_instruction_the_measurements_do_not_hold_panics() {
    let measured = MeasuredInstructionSet::<Arm64<User>>::from_measurements(&[(
        Instruction::Brk,
        PreciseInstruction::Brk,
    )]);
    measured.to_precise(Instruction::Csel64);
}
