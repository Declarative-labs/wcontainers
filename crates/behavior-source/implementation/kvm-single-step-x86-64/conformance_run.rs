//! The behavior source suite on this host's chip, which needs x86-64 Linux with KVM.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::borrow::Cow;

use wcontainers_behavior_source_kvm_single_step_x86_64::SingleStepMachine;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_x86_64::CriticalValueCut;
use wcontainers_instruction_input_domain_x86_64::X86_64InputDomain;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_x86_64::{
    DataAccess, Instruction, InstructionFetch, PagePermission, Processor, X86_64, ZERO_PROCESSOR,
};

fn chip() -> SingleStepMachine {
    match SingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("this host makes no single step machine: {error:?}"),
    }
}

/// The address the code of each state below starts at, on its own executable page.
const CODE: u64 = 0x1000;

/// A state whose code page holds the bytes, with the changes after it.
fn make(code: &[u8], after: Vec<MemoryDelta<'static, X86_64>>) -> ExecutionState<'static, X86_64> {
    let mut memory_mutations = vec![
        MemoryDelta::PermissionAssignment {
            addresses: CODE..2 * CODE,
            permission: PagePermission::Mapped {
                data_access: DataAccess::ReadOnly,
                instruction_fetch: InstructionFetch::Allowed,
            },
        },
        MemoryDelta::ValueAssignment {
            start: CODE,
            bytes: Cow::Owned(code.to_vec()),
        },
    ];
    memory_mutations.extend(after);
    ExecutionState {
        processor: Processor {
            instruction_pointer: CODE,
            ..ZERO_PROCESSOR
        },
        memory_mutations,
    }
}

/// States the chip refuses: a permission that starts inside a page, bytes past the program's addresses, INT 0x80, which
/// Linux takes as a 32-bit system call, and TILERELEASE, which under Linux traps its first use of the tiles.
fn to_unexecutable() -> Vec<ExecutionState<'static, X86_64>> {
    const ADD: [u8; 3] = [0x48, 0x01, 0xD8];
    vec![
        make(
            &ADD,
            vec![MemoryDelta::PermissionAssignment {
                addresses: 0x2001..0x3000,
                permission: PagePermission::Unmapped,
            }],
        ),
        make(
            &ADD,
            vec![MemoryDelta::ValueAssignment {
                start: 0x8000_0000_0000,
                bytes: Cow::Owned(vec![0xCC; 8]),
            }],
        ),
        make(&[0xCD, 0x80], Vec::new()),
        make(&[0xC4, 0xE2, 0x78, 0x49, 0xC0], Vec::new()),
    ]
}

/// The generator, without the states of RDTSC and RDTSCP, whose results the time decides, and of RDRAND and RDSEED,
/// whose results are random, so that no two runs of one state agree; their behaviors do not compare those registers.
struct UndecidedExcluded(CriticalValueCut);

impl DistinguishingStateGenerator<X86_64> for UndecidedExcluded {
    type States<'generator> = std::vec::IntoIter<ExecutionState<'static, X86_64>>;

    fn generate_states(&self, instruction: Instruction) -> Self::States<'_> {
        if matches!(
            instruction,
            Instruction::Rdtsc | Instruction::Rdtscp | Instruction::RdrandGprv | Instruction::RdseedGprv
        ) {
            return Vec::new().into_iter();
        }
        self.0.generate_states(instruction)
    }
}

const GENERATOR: UndecidedExcluded = UndecidedExcluded(CriticalValueCut::new(X86_64InputDomain));

#[test]
fn execute_agrees_with_execute_in_place() {
    wcontainers_behavior_source_conformance::execute_agrees_with_execute_in_place(chip(), GENERATOR);
}

#[test]
fn execution_is_deterministic() {
    wcontainers_behavior_source_conformance::execution_is_deterministic(chip(), GENERATOR);
}

// The suite's measuring needs every listed form to show one known behavior, which waits for every family.
#[test]
#[ignore = "the whole list measures once every form has a known behavior the chip shows; the probe measures it now"]
fn measured_behavior_agrees_with_the_source() {
    wcontainers_behavior_source_conformance::measured_behavior_agrees_with_the_source(
        chip(),
        CriticalValueCut::new(X86_64InputDomain),
    );
}

#[test]
fn next_state_after_an_error_executes_as_on_a_fresh_source() {
    wcontainers_behavior_source_conformance::next_state_after_an_error_executes_as_on_a_fresh_source(
        chip(),
        chip(),
        GENERATOR,
        to_unexecutable(),
    );
}
